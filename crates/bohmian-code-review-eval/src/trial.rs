//! Leave-one-out ranking trials over the public rankers.
//!
//! For one sampled PR, [`evaluate`] reads the base snapshot's index,
//! persisted declarations and reference names included, prepares each
//! changed file from its patch and head blob, and runs up to [`MAX_TRIALS`]
//! trials chosen by [`TRIAL_RULE`]: the withheld file leaves the change set
//! and stays a candidate at its base version, and every other changed path
//! is excluded from the candidates. Every arm is a caller-named public
//! [`Ranker`] over the same [`bohmian_code_review::RankInput`]. Source is
//! read only while a PR is evaluated; trials carry the withheld path and
//! each arm's rank and hit@10 only.

use bohmian_code_review::{
    ChangedBlob, ChangedFile, Error as SymbolicError, HeadFacts, RankedFile, Ranker, RelativePath,
    SymbolicContext, parse_hunks,
};
use bohmian_code_review_pipeline::{FileStatus, PullRequestFile};
use serde::{Serialize, Serializer};
use std::collections::BTreeMap;
use thiserror::Error;

/// Rarity rule of the C arm as recorded in the report.
pub const RARITY_RULE: &str = "ignore a name when df(name) > 100; a downstream file referencing it earns 1/df(name); an upstream file declaring it earns 1/(df(name) * declaring_file_count(name)); a file matching both directions keeps the larger credit; score sums credits over distinct names";
/// Most trials one PR contributes.
pub const MAX_TRIALS: usize = 5;
/// The trial selection rule as recorded in the report.
pub const TRIAL_RULE: &str = "when more than 5 files are withholdable, keep the 5 with the smallest FNV-1a 64-bit hash of \"<pr>/<path>\", ties by path; otherwise keep all; trials run in listing order";
/// Worst rank that is a hit.
pub const HIT_RANK: usize = 10;
/// Caller-named rankers, each arm's report id paired with its ranker.
pub type Rankers<'a> = [(&'a str, &'a dyn Ranker)];

/// Why a PR's trials cannot be run; the run stops.
#[derive(Debug, Error)]
pub enum TrialError {
    /// A head blob a trial ranks was not supplied.
    #[error("PR #{pr}: required source for `{path}` is missing")]
    MissingSource {
        /// PR number.
        pr: u64,
        /// File whose source is missing.
        path: RelativePath,
    },
    /// The symbolic facade rejected a head or its changed lines.
    #[error("PR #{pr}: {error}")]
    Symbolic {
        /// PR number.
        pr: u64,
        /// The facade's failure.
        error: SymbolicError,
    },
}

/// One arm's result for the withheld file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Outcome {
    /// One-based rank, `None` (serialized `"unranked"`) when unranked.
    #[serde(serialize_with = "rank_or_unranked")]
    pub rank: Option<usize>,
    /// Whether it ranks [`HIT_RANK`] or better.
    pub hit_at_10: bool,
}

impl Outcome {
    /// `target`'s outcome in `ranked`, best first.
    fn of(ranked: &[RankedFile], target: &RelativePath) -> Outcome {
        let rank = ranked
            .iter()
            .position(|file| &file.path == target)
            .map(|index| index + 1);
        Outcome {
            rank,
            hit_at_10: rank.is_some_and(|rank| rank <= HIT_RANK),
        }
    }
}

/// One withheld file's paired results under every arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trial {
    /// PR number.
    pub pr: u64,
    /// The withheld file, ranked at its base version.
    pub withheld: RelativePath,
    /// Each arm's outcome by its caller-given id.
    #[serde(flatten)]
    pub arms: BTreeMap<String, Outcome>,
}

/// Serializes an absent rank as `"unranked"`.
///
/// # Errors
///
/// Returns the serializer's error.
fn rank_or_unranked<S: Serializer>(rank: &Option<usize>, serializer: S) -> Result<S::Ok, S::Error> {
    match rank {
        Some(rank) => serializer.serialize_u64(*rank as u64),
        None => serializer.serialize_str("unranked"),
    }
}

/// The base-side lines a hunk-only `patch` removes; `patch` has already
/// parsed with [`parse_hunks`].
fn removed_lines(patch: &str) -> Vec<u32> {
    let mut removed = Vec::new();
    let mut line = 0;
    for text in patch.lines() {
        if let Some(header) = text.strip_prefix("@@ -") {
            let start = header.split([',', ' ']).next().unwrap_or_default();
            line = start.parse().unwrap_or(0);
        } else if text.starts_with('-') {
            removed.push(line);
            line += 1;
        } else if text.starts_with(' ') || text.is_empty() {
            line += 1;
        }
    }
    removed
}

/// One changed file prepared for every trial.
struct Change<'a> {
    /// Its listing entry.
    file: &'a PullRequestFile,
    /// Whether the base index holds its head path, so it anchors history.
    in_base: bool,
    /// Head facts when its patch parsed to head-side changed lines.
    head: Option<HeadFacts>,
    /// Simple and qualified names of base declarations its removed lines
    /// touch.
    removed: Vec<String>,
}

impl<'a> Change<'a> {
    /// Prepare `file` of PR `pr` from its patch, its base facts in `index`,
    /// and its head blob in `heads`.
    ///
    /// A missing or malformed patch supplies no names.
    ///
    /// # Errors
    ///
    /// [`TrialError::MissingSource`] when a head blob with changed lines
    /// was not supplied, and [`TrialError::Symbolic`] when the facade
    /// rejects that head or its parsed changed lines.
    fn new(
        pr: u64,
        index: &SymbolicContext,
        file: &'a PullRequestFile,
        heads: &BTreeMap<RelativePath, &[u8]>,
    ) -> Result<Change<'a>, TrialError> {
        let mut change = Change {
            file,
            in_base: index.byte_len(&file.path).is_some(),
            head: None,
            removed: Vec::new(),
        };
        let Some((patch, Ok(ranges))) = file
            .patch
            .as_deref()
            .map(|patch| (patch, parse_hunks(patch)))
        else {
            return Ok(change);
        };
        let base_path = file.previous_path.as_ref().unwrap_or(&file.path);
        if file.status != FileStatus::Added
            && let Some(facts) = index.export_file(base_path)
        {
            let removed = removed_lines(patch);
            for fact in facts.declarations {
                if removed
                    .iter()
                    .any(|&line| fact.start_line <= line && line <= fact.end_line)
                {
                    change.removed.push(fact.name);
                    change.removed.push(fact.qualified_name);
                }
            }
        }
        if file.status == FileStatus::Removed || ranges.is_empty() {
            return Ok(change);
        }
        let head = *heads
            .get(&file.path)
            .ok_or_else(|| TrialError::MissingSource {
                pr,
                path: file.path.clone(),
            })?;
        let blob = ChangedBlob {
            path: file.path.as_str(),
            head,
            changed: ranges,
        };
        change.head = Some(
            index
                .head_facts(&blob)
                .map_err(|error| TrialError::Symbolic { pr, error })?,
        );
        Ok(change)
    }
}

/// The ranker input of every change but `changes[withheld]`. A previous
/// path equal to the withheld path is dropped so the withheld file stays a
/// candidate.
fn remaining<'a>(changes: &'a [Change<'_>], withheld: usize) -> Vec<ChangedFile<'a>> {
    let target = &changes[withheld].file.path;
    changes
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != withheld)
        .map(|(_, change)| ChangedFile {
            path: &change.file.path,
            previous_path: change
                .file
                .previous_path
                .as_ref()
                .filter(|previous| *previous != target),
            in_base: change.in_base,
            head: change.head.as_ref().map(HeadFacts::head),
            removed: change.removed.iter().map(String::as_str).collect(),
        })
        .collect()
}

/// PR `pr`'s stable pseudo-random draw for `path`: FNV-1a 64 of
/// `"<pr>/<path>"`, identical across builds and platforms.
fn draw(pr: u64, path: &RelativePath) -> u64 {
    format!("{pr}/{}", path.as_str())
        .bytes()
        .fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
        })
}

/// The withholdable `paths` of PR `pr` that get trials by [`TRIAL_RULE`],
/// in their given order.
fn chosen<'p>(pr: u64, paths: &[&'p RelativePath]) -> Vec<&'p RelativePath> {
    let mut drawn = paths.to_vec();
    drawn.sort_by_key(|path| (draw(pr, path), *path));
    drawn.truncate(MAX_TRIALS);
    paths
        .iter()
        .copied()
        .filter(|path| drawn.contains(path))
        .collect()
}

/// Run the trial withholding `changes[withheld]` against `history` under
/// every arm of `rankers`.
fn trial(
    pr: u64,
    index: &SymbolicContext,
    changes: &[Change<'_>],
    withheld: usize,
    history: &[&[RelativePath]],
    rankers: &Rankers<'_>,
) -> Trial {
    let target = &changes[withheld].file.path;
    let changed = remaining(changes, withheld);
    Trial {
        pr,
        withheld: target.clone(),
        arms: rankers
            .iter()
            .map(|(id, ranker)| {
                let ranked = index.rank_with(*ranker, &changed, history);
                ((*id).to_owned(), Outcome::of(&ranked, target))
            })
            .collect(),
    }
}

/// Run PR `pr`'s trials over its changed files the listing marks modified
/// and the base `index` declares, chosen by [`TRIAL_RULE`], in listing
/// order, under every arm of `rankers`.
///
/// `index` must be the PR's complete base snapshot read back, so persisted
/// reference names cover every indexed file; `heads` holds the head blob of
/// every changed file with head-side changed lines; `history` the changed
/// paths of each history PR merged before the PR's base commit.
///
/// # Errors
///
/// [`TrialError::MissingSource`] for a missing head blob, and
/// [`TrialError::Symbolic`] for a head and its parsed changed lines the
/// facade rejects.
pub fn evaluate(
    pr: u64,
    index: &SymbolicContext,
    listed: &[PullRequestFile],
    heads: &BTreeMap<RelativePath, &[u8]>,
    history: &[&[RelativePath]],
    rankers: &Rankers<'_>,
) -> Result<Vec<Trial>, TrialError> {
    let changes = listed
        .iter()
        .map(|file| Change::new(pr, index, file, heads))
        .collect::<Result<Vec<_>, _>>()?;
    let withholdable: Vec<&RelativePath> = changes
        .iter()
        .filter(|change| change.file.status == FileStatus::Modified && change.in_base)
        .map(|change| &change.file.path)
        .collect();
    let chosen = chosen(pr, &withholdable);
    Ok(changes
        .iter()
        .enumerate()
        .filter(|(_, change)| chosen.contains(&&change.file.path))
        .map(|(withheld, _)| trial(pr, index, &changes, withheld, history, rankers))
        .collect())
}

/// Trial contract checks over the real symbolic facade on in-memory
/// source: bounded withholding, C's rarity ranking, co-change and fused
/// ranks over time-eligible history, and the ranking-only record.
#[cfg(test)]
mod tests {
    use super::*;
    use bohmian_code_review::{CoChange, DEFAULT_RANKER, RaritySymbol, SymbolOverlap};
    use bohmian_code_review_pipeline::{History, HistoryError, MergedPr, ObjectSha};
    use serde_json::json;
    use std::collections::BTreeSet;

    /// The live experiment's arms.
    const ARMS: [(&str, &dyn Ranker); 4] = [
        ("S", &SymbolOverlap),
        ("C", &RaritySymbol),
        ("E", &CoChange),
        ("F", &DEFAULT_RANKER),
    ];

    /// Canonical fixture path.
    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("fixture path is canonical")
    }

    /// A listed PR file with `status` and `patch`.
    fn listed(raw: &str, status: FileStatus, patch: Option<&str>) -> PullRequestFile {
        PullRequestFile {
            path: path(raw),
            status,
            sha: ObjectSha::new(&"c".repeat(40)).expect("fixture sha is valid"),
            patch: patch.map(str::to_owned),
            previous_path: None,
        }
    }

    /// The base index exactly as a snapshot read-back builds it: facts
    /// exported after extraction and loaded into a fresh context.
    fn base_index(files: &[(&str, String)]) -> SymbolicContext {
        let mut extracted = SymbolicContext::new();
        for (raw, text) in files {
            extracted
                .upsert(raw, text.as_bytes())
                .expect("fixture extracts");
        }
        let mut index = SymbolicContext::new();
        for facts in extracted.export() {
            index.load(facts).expect("exported facts load");
        }
        index
    }

    /// The trial withholding `raw`.
    fn withheld<'t>(trials: &'t [Trial], raw: &str) -> &'t Trial {
        trials
            .iter()
            .find(|trial| trial.withheld.as_str() == raw)
            .expect("trial for the withheld file")
    }

    /// Rank of arm `id` in `trial`.
    fn rank(trial: &Trial, id: &str) -> Option<usize> {
        trial.arms[id].rank
    }

    /// `ranker`'s ranking with `changes[withheld]` withheld.
    fn ranked(
        index: &SymbolicContext,
        changes: &[Change<'_>],
        withheld: usize,
        history: &[&[RelativePath]],
        ranker: &dyn Ranker,
    ) -> Vec<(String, f64)> {
        index
            .rank_with(ranker, &remaining(changes, withheld), history)
            .into_iter()
            .map(|file| (file.path.to_string(), file.score))
            .collect()
    }

    /// Paths of `ranked`, best first.
    fn paths(ranked: &[(String, f64)]) -> Vec<&str> {
        ranked.iter().map(|(path, _)| path.as_str()).collect()
    }

    /// Prepared changes of `files`.
    fn prepare<'f>(
        pr: u64,
        index: &SymbolicContext,
        files: &'f [PullRequestFile],
        heads: &BTreeMap<RelativePath, &[u8]>,
    ) -> Vec<Change<'f>> {
        files
            .iter()
            .map(|file| Change::new(pr, index, file, heads))
            .collect::<Result<Vec<_>, _>>()
            .expect("changes")
    }

    /// Scenario 1: a trial withholds one modified, declared file, keeps it
    /// rankable at its base version, and excludes every remaining changed
    /// path, even one C would otherwise rank.
    #[test]
    fn withholding_keeps_base_file_rankable_and_excludes_remaining_paths() {
        let index = base_index(&[
            (
                "pkg/models.py",
                "class Model:\n    def run(self):\n        return 1\n".to_owned(),
            ),
            ("pkg/api.py", "def serve():\n    pass\n".to_owned()),
            ("pkg/views.py", "def render():\n    pass\n".to_owned()),
            ("pkg/extra.py", "class Model:\n    pass\n".to_owned()),
        ]);
        let files = vec![
            listed(
                "pkg/models.py",
                FileStatus::Modified,
                Some("@@ -3 +3 @@\n-        return 1\n+        return 2"),
            ),
            listed(
                "pkg/api.py",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    pass\n+    return Model().run() + render()"),
            ),
            listed(
                "pkg/views.py",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    pass\n+    return Model"),
            ),
            listed("pkg/new.py", FileStatus::Added, None),
        ];
        let heads = BTreeMap::from([
            (
                path("pkg/models.py"),
                b"class Model:\n    def run(self):\n        return 2\n".as_slice(),
            ),
            (
                path("pkg/api.py"),
                b"def serve():\n    return Model().run() + render()\n".as_slice(),
            ),
            (
                path("pkg/views.py"),
                b"def render():\n    return Model\n".as_slice(),
            ),
        ]);
        let trials = evaluate(9, &index, &files, &heads, &[], &ARMS).expect("trials");
        assert_eq!(
            trials
                .iter()
                .map(|t| t.withheld.as_str())
                .collect::<Vec<_>>(),
            ["pkg/models.py", "pkg/api.py", "pkg/views.py"]
        );
        assert_eq!(rank(withheld(&trials, "pkg/models.py"), "C"), Some(1));

        let changes = prepare(9, &index, &files, &heads);
        let changed = remaining(&changes, 0);
        assert_eq!(
            changed.iter().map(|c| c.path.as_str()).collect::<Vec<_>>(),
            ["pkg/api.py", "pkg/views.py", "pkg/new.py"]
        );
        // views declares render, which api uses, but is a remaining change.
        let c = ranked(&index, &changes, 0, &[], &RaritySymbol);
        assert_eq!(paths(&c), ["pkg/models.py", "pkg/extra.py"]);
        let unexcluded = index.rank_with(&RaritySymbol, &changed[..1], &[]);
        assert!(
            unexcluded
                .iter()
                .any(|file| file.path.as_str() == "pkg/views.py")
        );
    }

    /// Scenario 1: at most five withholdable files all get trials in
    /// listing order; more get exactly five, reproducibly chosen by PR
    /// number and path, still in listing order.
    #[test]
    fn trial_selection_is_bounded_and_deterministic() {
        let names: Vec<RelativePath> = (0..7).map(|i| path(&format!("pkg/m{i}.py"))).collect();
        let all: Vec<&RelativePath> = names.iter().collect();
        assert_eq!(chosen(3, &all[..5]), &all[..5]);
        let picked = chosen(3, &all);
        assert_eq!(picked.len(), MAX_TRIALS);
        assert_eq!(picked, chosen(3, &all));
        assert!(picked.windows(2).all(|pair| pair[0] < pair[1]));
        let mut drawn = all.clone();
        drawn.sort_by_key(|path| draw(3, path));
        assert_eq!(
            picked.iter().copied().collect::<BTreeSet<_>>(),
            drawn[..5].iter().copied().collect::<BTreeSet<_>>()
        );
        assert_eq!(draw(3, &names[0]), draw(3, &path("pkg/m0.py")));
        assert_ne!(draw(3, &names[0]), draw(4, &names[0]));

        let index = base_index(
            &(0..7)
                .map(|i| (names[i].as_str(), format!("def f{i}():\n    return 0\n")))
                .collect::<Vec<_>>(),
        );
        let heads_owned: BTreeMap<RelativePath, Vec<u8>> = (0..7)
            .map(|i| {
                (
                    names[i].clone(),
                    format!("def f{i}():\n    return 1\n").into_bytes(),
                )
            })
            .collect();
        let heads: BTreeMap<RelativePath, &[u8]> = heads_owned
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.as_slice()))
            .collect();
        let files: Vec<PullRequestFile> = names
            .iter()
            .map(|name| {
                listed(
                    name.as_str(),
                    FileStatus::Modified,
                    Some("@@ -2 +2 @@\n-    return 0\n+    return 1"),
                )
            })
            .collect();
        let run = || evaluate(3, &index, &files, &heads, &[], &ARMS).expect("trials");
        let trials = run();
        assert_eq!(
            trials.iter().map(|t| &t.withheld).collect::<Vec<_>>(),
            picked
        );
        assert_eq!(trials, run());
        let five = evaluate(3, &index, &files[..5], &heads, &[], &ARMS).expect("trials");
        assert_eq!(five.len(), 5);
    }

    /// C regression: rarity weighting and ambiguous-declarer splitting in
    /// Python, removed declarations contributing base names, and Rust
    /// upstream ambiguity, all from persisted reference names.
    #[test]
    fn c_ranks_by_direction_and_rarity() {
        let index = base_index(&[
            ("pkg/util.py", "def helper(x):\n    return x\n\ndef gone():\n    return 0\n".to_owned()),
            ("pkg/legacy.py", "def helper(x):\n    return -x\n".to_owned()),
            ("pkg/models.py", "from pkg.util import helper\n\nclass Model:\n    def run(self):\n        return helper(1)\n".to_owned()),
            ("pkg/far.py", "from pkg.legacy import helper\n\ndef far():\n    return helper(2)\n".to_owned()),
            ("pkg/loose.py", "from .util import helper\n\ndef loose():\n    return gone()\n".to_owned()),
            ("pkg/caller.py", "from pkg.util import helper\n\ndef call():\n    return 0\n".to_owned()),
        ]);
        let files = vec![
            listed(
                "pkg/models.py",
                FileStatus::Modified,
                Some("@@ -5 +5 @@\n-        return helper(1)\n+        return helper(3)"),
            ),
            listed(
                "pkg/util.py",
                FileStatus::Modified,
                Some(
                    "@@ -1,5 +1,2 @@\n def helper(x):\n-    return x\n-\n-def gone():\n-    return 0\n+    return x + 1",
                ),
            ),
            listed(
                "pkg/caller.py",
                FileStatus::Modified,
                Some("@@ -4 +4 @@\n-    return 0\n+    return helper(0)"),
            ),
        ];
        let heads = BTreeMap::from([
            (path("pkg/models.py"), b"from pkg.util import helper\n\nclass Model:\n    def run(self):\n        return helper(3)\n".as_slice()),
            (path("pkg/util.py"), b"def helper(x):\n    return x + 1\n".as_slice()),
            (path("pkg/caller.py"), b"from pkg.util import helper\n\ndef call():\n    return helper(0)\n".as_slice()),
        ]);
        let changes = prepare(1, &index, &files, &heads);
        let c = ranked(&index, &changes, 0, &[], &RaritySymbol);
        // Remaining util changes helper and removes gone; caller uses
        // helper. helper: declared by util and legacy, referenced by
        // models, far, loose, caller: df 6. gone: declared by util,
        // referenced by loose: df 2. loose 1/6 + 1/2; far and models 1/6;
        // legacy 1/(6*2).
        let expected = [
            ("pkg/loose.py", 1.0 / 6.0 + 0.5),
            ("pkg/far.py", 1.0 / 6.0),
            ("pkg/models.py", 1.0 / 6.0),
            ("pkg/legacy.py", 1.0 / 12.0),
        ];
        assert_eq!(c.len(), expected.len());
        for ((path, score), (want, want_score)) in c.iter().zip(expected) {
            assert_eq!(path, want);
            assert!((score - want_score).abs() < 1e-12);
        }
        let trials = evaluate(1, &index, &files, &heads, &[], &ARMS).expect("trials");
        assert_eq!(rank(withheld(&trials, "pkg/models.py"), "C"), Some(3));

        let rust = base_index(&[
            (
                "tokio/src/lib.rs",
                "pub mod io;\npub mod alt;\npub mod net;\n".to_owned(),
            ),
            (
                "tokio/src/io.rs",
                "pub fn read_all() -> u8 {\n    0\n}\n".to_owned(),
            ),
            (
                "tokio/src/alt.rs",
                "pub fn read_all() -> u8 {\n    1\n}\n".to_owned(),
            ),
            (
                "tokio/src/net.rs",
                "use crate::io::read_all;\n\npub fn fetch() -> u8 {\n    read_all()\n}\n"
                    .to_owned(),
            ),
            (
                "tokio/src/user.rs",
                "use crate::io::read_all;\n\npub fn user() -> u8 {\n    0\n}\n".to_owned(),
            ),
        ]);
        let files = vec![
            listed(
                "tokio/src/io.rs",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    0\n+    2"),
            ),
            listed(
                "tokio/src/user.rs",
                FileStatus::Modified,
                Some("@@ -4 +4 @@\n-    0\n+    read_all()"),
            ),
        ];
        let heads = BTreeMap::from([
            (
                path("tokio/src/io.rs"),
                b"pub fn read_all() -> u8 {\n    2\n}\n".as_slice(),
            ),
            (
                path("tokio/src/user.rs"),
                b"use crate::io::read_all;\n\npub fn user() -> u8 {\n    read_all()\n}\n"
                    .as_slice(),
            ),
        ]);
        let trials = evaluate(2, &rust, &files, &heads, &[], &ARMS).expect("trials");
        // user's read_all resolves upstream to io and alt equally; alt
        // wins the tie by path.
        assert_eq!(rank(withheld(&trials, "tokio/src/io.rs"), "C"), Some(2));

        let mut missing = heads.clone();
        missing.remove(&path("tokio/src/user.rs"));
        assert!(matches!(
            evaluate(2, &rust, &files, &missing, &[], &ARMS),
            Err(TrialError::MissingSource { pr: 2, .. })
        ));
    }

    /// A parsed patch whose changed lines lie beyond its verified head is
    /// provider inconsistency: the run fails instead of omitting that
    /// head's names from every trial.
    #[test]
    fn parsed_patch_beyond_verified_head_fails_the_run() {
        let index = base_index(&[
            ("pkg/a.py", "def wanted():\n    return 1\n".to_owned()),
            ("pkg/caller.py", "def call():\n    return 0\n".to_owned()),
        ]);
        let files = vec![
            listed(
                "pkg/a.py",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    return 1\n+    return 2"),
            ),
            listed(
                "pkg/caller.py",
                FileStatus::Modified,
                Some("@@ -1 +4 @@\n-old\n+wanted()"),
            ),
        ];
        let heads = BTreeMap::from([
            (
                path("pkg/a.py"),
                b"def wanted():\n    return 2\n".as_slice(),
            ),
            (
                path("pkg/caller.py"),
                b"def call():\n    return wanted()\n".as_slice(),
            ),
        ]);
        assert!(matches!(
            evaluate(5, &index, &files, &heads, &[], &ARMS),
            Err(TrialError::Symbolic { pr: 5, .. })
        ));
    }

    /// History PR `number` merged on day `day` of January 2026.
    fn merged(number: u64, day: u32) -> MergedPr {
        let sha = |c: &str| ObjectSha::new(&c.repeat(40)).expect("fixture sha is valid");
        MergedPr {
            number,
            merged_at: format!("2026-01-{day:02}T12:00:00Z"),
            base: sha("a"),
            head: sha("b"),
        }
    }

    /// Scenario 2: E sums `n(d, f) / n(d)` over remaining changed files
    /// with history, skipping paths absent from the base and excluded
    /// paths; F fuses C and E ranks with constant 60; every arm breaks ties
    /// by path and leaves zero scores unranked; a history PR merged after
    /// the trial's base changes nothing.
    #[tokio::test]
    async fn cochange_and_fused_ranks_use_only_eligible_history() {
        let def = |name: &str| format!("def {name}():\n    return 0\n");
        let index = base_index(&[
            ("pkg/a.py", def("a")),
            ("pkg/b.py", def("b")),
            ("pkg/c.py", def("c")),
            ("pkg/v.py", def("helper")),
            ("pkg/w.py", def("w")),
            ("pkg/x.py", def("x")),
            ("pkg/z.py", def("z")),
        ]);
        let one = "@@ -2 +2 @@\n-    return 0\n+    return 1";
        let files = vec![
            listed("pkg/a.py", FileStatus::Modified, Some(one)),
            listed(
                "pkg/b.py",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    return 0\n+    return helper()"),
            ),
            listed("pkg/c.py", FileStatus::Modified, Some(one)),
            listed("pkg/u.py", FileStatus::Added, None),
        ];
        let heads = BTreeMap::from([
            (path("pkg/a.py"), b"def a():\n    return 1\n".as_slice()),
            (
                path("pkg/b.py"),
                b"def b():\n    return helper()\n".as_slice(),
            ),
            (path("pkg/c.py"), b"def c():\n    return 1\n".as_slice()),
        ]);
        let changed_paths = |number: u64| -> Vec<&str> {
            match number {
                8 => vec!["pkg/b.py", "pkg/x.py", "pkg/w.py"],
                9 => vec!["pkg/b.py", "pkg/x.py", "pkg/w.py", "pkg/y.py"],
                10 => vec!["pkg/b.py", "pkg/a.py", "pkg/c.py"],
                11 => vec!["pkg/c.py", "pkg/a.py"],
                12 => vec!["pkg/z.py"],
                13 => vec!["pkg/b.py", "pkg/x.py", "pkg/c.py"],
                _ => vec!["pkg/other.py"],
            }
        };
        let mut prs: Vec<MergedPr> = (1..=13).map(|n| merged(n, n as u32)).collect();
        prs.reverse();
        let history = History::build(
            &prs,
            "2026-01-11T00:00:00Z",
            "2026-01-20T00:00:00Z",
            async |number| {
                let changed = Some(changed_paths(number).into_iter().map(path).collect());
                Ok::<_, HistoryError>((changed, 1))
            },
        )
        .await
        .expect("history");
        let eligible: Vec<&[RelativePath]> = history
            .for_trial(99, "2026-01-12T13:00:00Z")
            .map(|pr| pr.paths.as_slice())
            .collect();
        assert_eq!(eligible.len(), 12);

        let changes = prepare(99, &index, &files, &heads);
        // n(b) = 3 (PRs 8, 9, 10): w and x 2/3, a 1/3, y absent, c excluded.
        // n(c) = 2 (PRs 10, 11): a 2/2. u has no history. PR 13 is future.
        let e = ranked(&index, &changes, 0, &eligible, &CoChange);
        assert_eq!(paths(&e), ["pkg/a.py", "pkg/w.py", "pkg/x.py"]);
        assert!((e[0].1 - (1.0 / 3.0 + 1.0)).abs() < 1e-12);
        assert!((e[1].1 - 2.0 / 3.0).abs() < 1e-12);
        let c = ranked(&index, &changes, 0, &eligible, &RaritySymbol);
        assert_eq!(paths(&c), ["pkg/v.py"]);
        // F: a and v tie at 1/61 and sort by path; then w 1/62, x 1/63.
        let f = ranked(&index, &changes, 0, &eligible, &DEFAULT_RANKER);
        assert_eq!(paths(&f), ["pkg/a.py", "pkg/v.py", "pkg/w.py", "pkg/x.py"]);
        assert_eq!(f[0].1, 1.0 / 61.0);
        assert_eq!(f[3].1, 1.0 / 63.0);

        let trials = evaluate(99, &index, &files, &heads, &eligible, &ARMS).expect("trials");
        let a = withheld(&trials, "pkg/a.py");
        assert_eq!(
            (rank(a, "C"), rank(a, "E"), rank(a, "F")),
            (None, Some(1), Some(1))
        );
        let leaked: Vec<&[RelativePath]> = history
            .for_trial(99, "2026-01-13T13:00:00Z")
            .map(|pr| pr.paths.as_slice())
            .collect();
        let with_future = evaluate(99, &index, &files, &heads, &leaked, &ARMS).expect("trials");
        assert_ne!(with_future, trials);
    }

    /// Scenario 2: a remaining changed path absent from the base anchors
    /// no co-change history, so it lends E and F nothing, while an indexed
    /// remaining path still credits `n(d, f) / n(d)`.
    #[test]
    fn absent_base_changed_path_anchors_no_cochange() {
        let def = |name: &str| format!("def {name}():\n    return 0\n");
        let index = base_index(&[
            ("pkg/a.py", def("a")),
            ("pkg/c.py", def("c")),
            ("pkg/w.py", def("w")),
        ]);
        let files = vec![
            listed("pkg/a.py", FileStatus::Modified, None),
            listed("pkg/c.py", FileStatus::Modified, None),
            listed("pkg/u.py", FileStatus::Added, None),
        ];
        let owned: Vec<Vec<RelativePath>> = [
            ["pkg/u.py", "pkg/a.py"],
            ["pkg/u.py", "pkg/a.py"],
            ["pkg/c.py", "pkg/w.py"],
            ["pkg/c.py", "pkg/w.py"],
            ["pkg/c.py", "pkg/a.py"],
        ]
        .iter()
        .map(|pr| pr.iter().copied().map(path).collect())
        .collect();
        let history: Vec<&[RelativePath]> = owned.iter().map(Vec::as_slice).collect();

        let heads = BTreeMap::new();
        let changes = prepare(7, &index, &files, &heads);
        // u is absent from the base; only c anchors: n(c) = 3, w 2/3, a 1/3.
        let e = ranked(&index, &changes, 0, &history, &CoChange);
        assert_eq!(paths(&e), ["pkg/w.py", "pkg/a.py"]);
        assert!((e[0].1 - 2.0 / 3.0).abs() < 1e-12);
        assert!((e[1].1 - 1.0 / 3.0).abs() < 1e-12);

        let trials = evaluate(7, &index, &files, &heads, &history, &ARMS).expect("trials");
        let a = withheld(&trials, "pkg/a.py");
        assert_eq!(
            (rank(a, "C"), rank(a, "E"), rank(a, "F")),
            (None, Some(2), Some(2))
        );
    }

    /// Scenario 3: a trial records only the withheld path and each arm's
    /// rank or `unranked` with hit@10: rank 10 hits, rank 11 and unranked
    /// miss.
    #[test]
    fn trial_record_is_ranking_only() {
        let ranked: Vec<RankedFile> = (1..=11)
            .map(|i| RankedFile {
                path: path(&format!("pkg/f{i:02}.py")),
                score: 1.0 / f64::from(i),
                reasons: Vec::new(),
            })
            .collect();
        let tenth = Outcome::of(&ranked, &path("pkg/f10.py"));
        let eleventh = Outcome::of(&ranked, &path("pkg/f11.py"));
        let unranked = Outcome::of(&ranked, &path("pkg/none.py"));
        assert_eq!((tenth.rank, tenth.hit_at_10), (Some(10), true));
        assert_eq!((eleventh.rank, eleventh.hit_at_10), (Some(11), false));
        assert_eq!((unranked.rank, unranked.hit_at_10), (None, false));
        let trial = Trial {
            pr: 7,
            withheld: path("pkg/f10.py"),
            arms: BTreeMap::from([
                ("S".to_owned(), unranked),
                ("C".to_owned(), unranked),
                ("E".to_owned(), tenth),
                ("F".to_owned(), eleventh),
            ]),
        };
        assert_eq!(
            serde_json::to_value(&trial).expect("serializes"),
            json!({
                "pr": 7,
                "withheld": "pkg/f10.py",
                "S": {"rank": "unranked", "hit_at_10": false},
                "C": {"rank": "unranked", "hit_at_10": false},
                "E": {"rank": 10, "hit_at_10": true},
                "F": {"rank": 11, "hit_at_10": false},
            })
        );
    }
}
