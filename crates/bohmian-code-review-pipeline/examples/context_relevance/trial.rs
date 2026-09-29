//! Leave-one-out ranking trials for the co-change experiment.
//!
//! For one sampled PR, [`evaluate`] builds the base candidate universe from
//! the base snapshot's index and the hash-verified base blob of every
//! indexed file, prepares each changed file from its patch and head blob,
//! and runs up to [`MAX_TRIALS`] trials chosen by [`TRIAL_RULE`]: the
//! withheld file leaves the change set and stays a candidate at its base
//! version, and every other changed path is excluded from the candidates.
//! C ranks by directional, rarity-weighted references; E by co-change in
//! the trial's time-eligible history; F fuses the C and E ranks. Every arm
//! ranks the same candidates. Source is read only while a PR is evaluated;
//! trials carry the withheld path and each arm's rank and hit@10 only.

use bohmian_code_review::{
    DeclarationFact, Error as SymbolicError, Node, RawReference, RelativePath, SymbolicContext,
    parse_hunks,
};
use bohmian_code_review_pipeline::{FileStatus, PullRequestFile};
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Names referenced or declared by more base files than this are ignored
/// by C.
pub const DF_CEILING: usize = 100;
/// C's rarity rule as recorded in the report.
pub const RARITY_RULE: &str = "ignore a name when df(name) > 100; a downstream file referencing it earns 1/df(name); an upstream file declaring it earns 1/(df(name) * declaring_file_count(name)); a file matching both directions keeps the larger credit; score sums credits over distinct names";
/// Most trials one PR contributes.
pub const MAX_TRIALS: usize = 5;
/// The trial selection rule as recorded in the report.
pub const TRIAL_RULE: &str = "when more than 5 files are withholdable, keep the 5 with the smallest FNV-1a 64-bit hash of \"<pr>/<path>\", ties by path; otherwise keep all; trials run in listing order";
/// Reciprocal-rank fusion constant of F.
pub const FUSION_CONSTANT: usize = 60;
/// Worst rank that is a hit.
pub const HIT_RANK: usize = 10;

/// Why a PR's trials cannot be run; the run stops.
#[derive(Debug, Error)]
pub enum TrialError {
    /// A base blob of an indexed file, or a head blob a trial ranks, was
    /// not supplied.
    #[error("PR #{pr}: required source for `{path}` is missing")]
    MissingSource {
        /// PR number.
        pr: u64,
        /// File whose source is missing.
        path: RelativePath,
    },
    /// The symbolic facade rejected base source or ranking input.
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
    fn of(ranked: &[(RelativePath, f64)], target: &RelativePath) -> Outcome {
        let rank = ranked
            .iter()
            .position(|(path, _)| path == target)
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
    /// Directional rarity-weighted references; the control.
    #[serde(rename = "C")]
    pub c: Outcome,
    /// Co-change alone.
    #[serde(rename = "E")]
    pub e: Outcome,
    /// C and E fused by reciprocal rank.
    #[serde(rename = "F")]
    pub f: Outcome,
}

impl Trial {
    /// Each arm's id and outcome: C, E, F.
    pub fn arms(&self) -> [(&'static str, &Outcome); 3] {
        [("C", &self.c), ("E", &self.e), ("F", &self.f)]
    }
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

/// The complete base candidate universe of one PR.
///
/// Invariant: `declarations` holds exactly the indexed paths of `index`,
/// and `referenced` covers every reference in their base blobs.
struct Corpus {
    /// The base snapshot's index, for every declaration lookup.
    index: SymbolicContext,
    /// Persisted declarations per indexed file.
    declarations: BTreeMap<RelativePath, Vec<DeclarationFact>>,
    /// Name → base files referencing it.
    referenced: BTreeMap<String, BTreeSet<RelativePath>>,
}

impl Corpus {
    /// Build the corpus for PR `pr` from its base `index` and `sources`,
    /// extracting each base blob's references with `scratch`.
    ///
    /// # Errors
    ///
    /// [`TrialError::MissingSource`] when an indexed file has no source,
    /// and [`TrialError::Symbolic`] when its source does not extract.
    fn new(
        pr: u64,
        index: SymbolicContext,
        sources: &BTreeMap<RelativePath, &[u8]>,
        scratch: &mut SymbolicContext,
    ) -> Result<Corpus, TrialError> {
        let facts = index.export();
        let mut corpus = Corpus {
            index,
            declarations: BTreeMap::new(),
            referenced: BTreeMap::new(),
        };
        for fact in facts {
            let source = *sources
                .get(&fact.path)
                .ok_or_else(|| TrialError::MissingSource {
                    pr,
                    path: fact.path.clone(),
                })?;
            let (_, references) = extract(scratch, &fact.path, source)
                .map_err(|error| TrialError::Symbolic { pr, error })?;
            for reference in &references {
                for name in reference_names(reference) {
                    corpus
                        .referenced
                        .entry(name.to_owned())
                        .or_default()
                        .insert(fact.path.clone());
                }
            }
            corpus.declarations.insert(fact.path, fact.declarations);
        }
        Ok(corpus)
    }

    /// Files declaring exactly `name`.
    fn declaring(&self, name: &str) -> BTreeSet<RelativePath> {
        self.index
            .lookup(name)
            .into_iter()
            .map(|hit| hit.path.clone())
            .collect()
    }

    /// Distinct base files declaring or referencing `name`.
    fn document_frequency(&self, name: &str, declaring: &BTreeSet<RelativePath>) -> usize {
        let referencing = self.referenced.get(name).into_iter().flatten();
        declaring
            .iter()
            .chain(referencing)
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// Extract `source` through the public facade without keeping it indexed.
///
/// # Errors
///
/// Returns the facade's path, UTF-8, or extraction error.
fn extract(
    scratch: &mut SymbolicContext,
    path: &RelativePath,
    source: &[u8],
) -> Result<(Vec<Node>, Vec<RawReference>), SymbolicError> {
    let facts = scratch.upsert(path.as_str(), source)?;
    let extracted = (facts.nodes.to_vec(), facts.references);
    scratch.remove(path.as_str())?;
    Ok(extracted)
}

/// A reference's exact name and its candidate names, as production
/// ranking reads them.
fn reference_names(reference: &RawReference) -> impl Iterator<Item = &str> {
    std::iter::once(reference.reference_name.as_str())
        .chain(reference.candidates.iter().flatten().map(String::as_str))
}

/// Whether `node` is a declaration the index posts.
fn is_declaration(node: &Node) -> bool {
    !matches!(
        node.kind.wire_name(),
        "file" | "import" | "export" | "parameter"
    )
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
    /// Names referenced on its head changed lines.
    used: BTreeSet<String>,
    /// Names of head and base declarations its changes add, modify, or
    /// remove.
    changed: BTreeSet<String>,
}

impl<'a> Change<'a> {
    /// Prepare `file` of PR `pr` from its patch, its base file in
    /// `corpus`, and its head blob in `heads`.
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
        corpus: &Corpus,
        scratch: &mut SymbolicContext,
        file: &'a PullRequestFile,
        heads: &BTreeMap<RelativePath, &[u8]>,
    ) -> Result<Change<'a>, TrialError> {
        let mut change = Change {
            file,
            used: BTreeSet::new(),
            changed: BTreeSet::new(),
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
            && let Some(declarations) = corpus.declarations.get(base_path)
        {
            let removed = removed_lines(patch);
            for fact in declarations {
                if removed
                    .iter()
                    .any(|&line| fact.start_line <= line && line <= fact.end_line)
                {
                    change.changed.insert(fact.name.clone());
                    change.changed.insert(fact.qualified_name.clone());
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
        let (nodes, references) = corpus
            .index
            .rank(file.path.as_str(), head, ranges.clone())
            .and_then(|_| extract(scratch, &file.path, head))
            .map_err(|error| TrialError::Symbolic { pr, error })?;
        let on_changed = |start: u32, end: u32| {
            ranges
                .iter()
                .any(|range| range.start <= end && start <= range.end)
        };
        for node in nodes
            .iter()
            .filter(|node| is_declaration(node) && on_changed(node.start_line, node.end_line))
        {
            change.changed.insert(node.name.clone());
            change.changed.insert(node.qualified_name.clone());
        }
        for reference in references
            .iter()
            .filter(|reference| on_changed(reference.line, reference.line))
        {
            change
                .used
                .extend(reference_names(reference).map(str::to_owned));
        }
        Ok(change)
    }
}

/// `scores` without zero scores, by descending score then path.
fn ranked(scores: BTreeMap<RelativePath, f64>) -> Vec<(RelativePath, f64)> {
    let mut ranked: Vec<(RelativePath, f64)> = scores
        .into_iter()
        .filter(|(_, score)| *score > 0.0)
        .collect();
    ranked.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    ranked
}

/// C: rank the corpus for the names `remaining` uses and changes, skipping
/// `excluded`, by [`RARITY_RULE`].
fn rank_c(
    corpus: &Corpus,
    remaining: &[&Change<'_>],
    excluded: &BTreeSet<RelativePath>,
) -> Vec<(RelativePath, f64)> {
    let mut credits: BTreeMap<RelativePath, BTreeMap<&str, f64>> = BTreeMap::new();
    let mut credit = |path: &RelativePath, name, credit: f64| {
        let held = credits
            .entry(path.clone())
            .or_default()
            .entry(name)
            .or_default();
        *held = held.max(credit);
    };
    let used: BTreeSet<&str> = remaining
        .iter()
        .flat_map(|change| change.used.iter().map(String::as_str))
        .collect();
    for name in used {
        let declaring = corpus.declaring(name);
        let df = corpus.document_frequency(name, &declaring);
        if df > DF_CEILING {
            continue;
        }
        for path in declaring.iter().filter(|path| !excluded.contains(*path)) {
            credit(path, name, 1.0 / (df * declaring.len()) as f64);
        }
    }
    let changed: BTreeSet<&str> = remaining
        .iter()
        .flat_map(|change| change.changed.iter().map(String::as_str))
        .collect();
    for name in changed {
        let Some(referencing) = corpus.referenced.get(name) else {
            continue;
        };
        let df = corpus.document_frequency(name, &corpus.declaring(name));
        if df > DF_CEILING {
            continue;
        }
        for path in referencing.iter().filter(|path| !excluded.contains(*path)) {
            credit(path, name, 1.0 / df as f64);
        }
    }
    ranked(
        credits
            .into_iter()
            .map(|(path, names)| (path, names.values().sum()))
            .collect(),
    )
}

/// E: for each remaining changed file `d` touched by `n(d) > 0` of the
/// `history` PRs' paths, credit each indexed, non-`excluded` candidate `f`
/// with `n(d, f) / n(d)`, summed over `d`.
fn rank_e(
    corpus: &Corpus,
    remaining: &[&Change<'_>],
    excluded: &BTreeSet<RelativePath>,
    history: &[&[RelativePath]],
) -> Vec<(RelativePath, f64)> {
    let mut scores: BTreeMap<RelativePath, f64> = BTreeMap::new();
    for change in remaining {
        let touching: Vec<&[RelativePath]> = history
            .iter()
            .copied()
            .filter(|paths| paths.contains(&change.file.path))
            .collect();
        let mut together: BTreeMap<&RelativePath, usize> = BTreeMap::new();
        for path in touching.iter().copied().flatten() {
            if corpus.declarations.contains_key(path) && !excluded.contains(path) {
                *together.entry(path).or_default() += 1;
            }
        }
        for (path, count) in together {
            *scores.entry(path.clone()).or_default() += count as f64 / touching.len() as f64;
        }
    }
    ranked(scores)
}

/// F: `1 / (FUSION_CONSTANT + rank)` summed over the arms ranking a file.
fn rank_f(c: &[(RelativePath, f64)], e: &[(RelativePath, f64)]) -> Vec<(RelativePath, f64)> {
    let mut scores: BTreeMap<RelativePath, f64> = BTreeMap::new();
    for arm in [c, e] {
        for (index, (path, _)) in arm.iter().enumerate() {
            *scores.entry(path.clone()).or_default() += 1.0 / (FUSION_CONSTANT + index + 1) as f64;
        }
    }
    ranked(scores)
}

/// Every remaining changed head and previous path except `withheld`.
fn excluded(remaining: &[&Change<'_>], withheld: &RelativePath) -> BTreeSet<RelativePath> {
    remaining
        .iter()
        .flat_map(|change| std::iter::once(&change.file.path).chain(&change.file.previous_path))
        .filter(|path| *path != withheld)
        .cloned()
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

/// Run the trial withholding `changes[withheld]` against `history`.
fn trial(
    pr: u64,
    corpus: &Corpus,
    changes: &[Change<'_>],
    withheld: usize,
    history: &[&[RelativePath]],
) -> Trial {
    let target = &changes[withheld].file.path;
    let remaining: Vec<&Change<'_>> = changes
        .iter()
        .enumerate()
        .filter_map(|(index, change)| (index != withheld).then_some(change))
        .collect();
    let excluded = excluded(&remaining, target);
    let c = rank_c(corpus, &remaining, &excluded);
    let e = rank_e(corpus, &remaining, &excluded, history);
    let f = rank_f(&c, &e);
    Trial {
        pr,
        withheld: target.clone(),
        c: Outcome::of(&c, target),
        e: Outcome::of(&e, target),
        f: Outcome::of(&f, target),
    }
}

/// Run PR `pr`'s trials over its changed files the listing marks modified
/// and the base `index` declares, chosen by [`TRIAL_RULE`], in listing
/// order.
///
/// `sources` must hold the verified base blob of every file `index`
/// declares; `heads` the head blob of every changed file with head-side
/// changed lines; `history` the changed paths of each history PR merged
/// before the PR's base commit.
///
/// # Errors
///
/// [`TrialError::MissingSource`] for a missing base or head blob, and
/// [`TrialError::Symbolic`] for base source, or a head and its parsed
/// changed lines, the facade rejects.
pub fn evaluate(
    pr: u64,
    index: SymbolicContext,
    sources: &BTreeMap<RelativePath, &[u8]>,
    listed: &[PullRequestFile],
    heads: &BTreeMap<RelativePath, &[u8]>,
    history: &[&[RelativePath]],
) -> Result<Vec<Trial>, TrialError> {
    let mut scratch = SymbolicContext::new();
    let corpus = Corpus::new(pr, index, sources, &mut scratch)?;
    let changes = listed
        .iter()
        .map(|file| Change::new(pr, &corpus, &mut scratch, file, heads))
        .collect::<Result<Vec<_>, _>>()?;
    let withholdable: Vec<&RelativePath> = changes
        .iter()
        .filter(|change| {
            change.file.status == FileStatus::Modified
                && corpus.declarations.contains_key(&change.file.path)
        })
        .map(|change| &change.file.path)
        .collect();
    let chosen = chosen(pr, &withholdable);
    Ok(changes
        .iter()
        .enumerate()
        .filter(|(_, change)| chosen.contains(&&change.file.path))
        .map(|(withheld, _)| trial(pr, &corpus, &changes, withheld, history))
        .collect())
}

/// Trial contract checks over the real symbolic facade on in-memory
/// source: bounded withholding, C's rarity ranking, co-change and fused
/// ranks over time-eligible history, and the ranking-only record.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::{Changed, History};
    use crate::sample::MergedPr;
    use bohmian_code_review_pipeline::ObjectSha;
    use serde_json::json;

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
    fn base_index(sources: &BTreeMap<RelativePath, &[u8]>) -> SymbolicContext {
        let mut extracted = SymbolicContext::new();
        for (path, source) in sources {
            extracted
                .upsert(path.as_str(), source)
                .expect("fixture extracts");
        }
        let mut index = SymbolicContext::new();
        for facts in extracted.export() {
            index.load(facts).expect("exported facts load");
        }
        index
    }

    /// Owned fixture sources keyed by path.
    fn sources(files: &[(&str, String)]) -> BTreeMap<RelativePath, Vec<u8>> {
        files
            .iter()
            .map(|(raw, text)| (path(raw), text.clone().into_bytes()))
            .collect()
    }

    /// Borrowed view of `sources`.
    fn borrowed(sources: &BTreeMap<RelativePath, Vec<u8>>) -> BTreeMap<RelativePath, &[u8]> {
        sources
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.as_slice()))
            .collect()
    }

    /// The trial withholding `raw`.
    fn withheld<'t>(trials: &'t [Trial], raw: &str) -> &'t Trial {
        trials
            .iter()
            .find(|trial| trial.withheld.as_str() == raw)
            .expect("trial for the withheld file")
    }

    /// Paths of `ranked`, best first.
    fn paths(ranked: &[(RelativePath, f64)]) -> Vec<&str> {
        ranked.iter().map(|(path, _)| path.as_str()).collect()
    }

    /// Scenario 1: a trial withholds one modified, declared file, keeps it
    /// rankable at its base version, and excludes every remaining changed
    /// path, even one C would otherwise rank.
    #[test]
    fn withholding_keeps_base_file_rankable_and_excludes_remaining_paths() {
        let base = sources(&[
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
        let view = borrowed(&base);
        let trials = evaluate(9, base_index(&view), &view, &files, &heads, &[]).expect("trials");
        assert_eq!(
            trials
                .iter()
                .map(|t| t.withheld.as_str())
                .collect::<Vec<_>>(),
            ["pkg/models.py", "pkg/api.py", "pkg/views.py"]
        );
        assert_eq!(withheld(&trials, "pkg/models.py").c.rank, Some(1));

        let mut scratch = SymbolicContext::new();
        let corpus = Corpus::new(9, base_index(&view), &view, &mut scratch).expect("corpus");
        let changes = files
            .iter()
            .map(|file| Change::new(9, &corpus, &mut scratch, file, &heads))
            .collect::<Result<Vec<_>, _>>()
            .expect("changes");
        let remaining: Vec<&Change<'_>> = changes
            .iter()
            .filter(|c| c.file.path.as_str() != "pkg/models.py")
            .collect();
        let excluded = excluded(&remaining, &path("pkg/models.py"));
        assert_eq!(
            excluded
                .iter()
                .map(RelativePath::as_str)
                .collect::<Vec<_>>(),
            ["pkg/api.py", "pkg/new.py", "pkg/views.py"]
        );
        // views declares render, which api uses, but is a remaining change.
        let c = rank_c(&corpus, &remaining, &excluded);
        assert_eq!(paths(&c), ["pkg/models.py", "pkg/extra.py"]);
        assert!(paths(&rank_c(&corpus, &remaining, &BTreeSet::new())).contains(&"pkg/views.py"));
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

        let base = sources(
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
        let view = borrowed(&base);
        let heads = borrowed(&heads_owned);
        let run = || evaluate(3, base_index(&view), &view, &files, &heads, &[]).expect("trials");
        let trials = run();
        assert_eq!(
            trials.iter().map(|t| &t.withheld).collect::<Vec<_>>(),
            picked
        );
        assert_eq!(trials, run());
        let five = evaluate(3, base_index(&view), &view, &files[..5], &heads, &[]).expect("trials");
        assert_eq!(five.len(), 5);
    }

    /// C regression: rarity weighting and ambiguous-declarer splitting in
    /// Python, removed declarations contributing base names, Rust upstream
    /// ambiguity, and a missing base blob failing the run.
    #[test]
    fn c_ranks_by_direction_and_rarity() {
        let base = sources(&[
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
        let view = borrowed(&base);
        let mut scratch = SymbolicContext::new();
        let corpus = Corpus::new(1, base_index(&view), &view, &mut scratch).expect("corpus");
        let changes = files
            .iter()
            .map(|file| Change::new(1, &corpus, &mut scratch, file, &heads))
            .collect::<Result<Vec<_>, _>>()
            .expect("changes");
        let remaining: Vec<&Change<'_>> = changes[1..].iter().collect();
        let c = rank_c(
            &corpus,
            &remaining,
            &excluded(&remaining, &path("pkg/models.py")),
        );
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
            assert_eq!(path.as_str(), want);
            assert!((score - want_score).abs() < 1e-12);
        }
        let trials = evaluate(1, base_index(&view), &view, &files, &heads, &[]).expect("trials");
        assert_eq!(withheld(&trials, "pkg/models.py").c.rank, Some(3));

        let rust = sources(&[
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
        let view = borrowed(&rust);
        let trials = evaluate(2, base_index(&view), &view, &files, &heads, &[]).expect("trials");
        // user's read_all resolves upstream to io and alt equally; alt
        // wins the tie by path.
        assert_eq!(withheld(&trials, "tokio/src/io.rs").c.rank, Some(2));

        let mut missing = view.clone();
        missing.remove(&path("tokio/src/alt.rs"));
        assert!(matches!(
            evaluate(2, base_index(&view), &missing, &files, &heads, &[]),
            Err(TrialError::MissingSource { pr: 2, .. })
        ));
    }

    /// A parsed patch whose changed lines lie beyond its verified head is
    /// provider inconsistency: the run fails instead of omitting that
    /// head's names from every trial.
    #[test]
    fn parsed_patch_beyond_verified_head_fails_the_run() {
        let base = sources(&[
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
        let view = borrowed(&base);
        assert!(matches!(
            evaluate(5, base_index(&view), &view, &files, &heads, &[]),
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
        let base = sources(&[
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
        let heads_owned: BTreeMap<RelativePath, Vec<u8>> = BTreeMap::from([
            (path("pkg/a.py"), b"def a():\n    return 1\n".to_vec()),
            (
                path("pkg/b.py"),
                b"def b():\n    return helper()\n".to_vec(),
            ),
            (path("pkg/c.py"), b"def c():\n    return 1\n".to_vec()),
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
                let changed: Changed = Some(changed_paths(number).into_iter().map(path).collect());
                Ok::<_, crate::sample::SampleError>((changed, 1))
            },
        )
        .await
        .expect("history");
        let eligible: Vec<&[RelativePath]> = history
            .for_trial(99, "2026-01-12T13:00:00Z")
            .map(|pr| pr.paths.as_slice())
            .collect();
        assert_eq!(eligible.len(), 12);

        let view = borrowed(&base);
        let heads = borrowed(&heads_owned);
        let mut scratch = SymbolicContext::new();
        let corpus = Corpus::new(99, base_index(&view), &view, &mut scratch).expect("corpus");
        let changes = files
            .iter()
            .map(|file| Change::new(99, &corpus, &mut scratch, file, &heads))
            .collect::<Result<Vec<_>, _>>()
            .expect("changes");
        let remaining: Vec<&Change<'_>> = changes[1..].iter().collect();
        let excluded = excluded(&remaining, &path("pkg/a.py"));
        // n(b) = 3 (PRs 8, 9, 10): w and x 2/3, a 1/3, y absent, c excluded.
        // n(c) = 2 (PRs 10, 11): a 2/2. u has no history. PR 13 is future.
        let e = rank_e(&corpus, &remaining, &excluded, &eligible);
        assert_eq!(paths(&e), ["pkg/a.py", "pkg/w.py", "pkg/x.py"]);
        assert!((e[0].1 - (1.0 / 3.0 + 1.0)).abs() < 1e-12);
        assert!((e[1].1 - 2.0 / 3.0).abs() < 1e-12);
        let c = rank_c(&corpus, &remaining, &excluded);
        assert_eq!(paths(&c), ["pkg/v.py"]);
        // F: a and v tie at 1/61 and sort by path; then w 1/62, x 1/63.
        let f = rank_f(&c, &e);
        assert_eq!(paths(&f), ["pkg/a.py", "pkg/v.py", "pkg/w.py", "pkg/x.py"]);
        assert_eq!(f[0].1, 1.0 / 61.0);
        assert_eq!(f[3].1, 1.0 / 63.0);

        let trials =
            evaluate(99, base_index(&view), &view, &files, &heads, &eligible).expect("trials");
        let a = withheld(&trials, "pkg/a.py");
        assert_eq!((a.c.rank, a.e.rank, a.f.rank), (None, Some(1), Some(1)));
        let leaked: Vec<&[RelativePath]> = history
            .for_trial(99, "2026-01-13T13:00:00Z")
            .map(|pr| pr.paths.as_slice())
            .collect();
        let with_future =
            evaluate(99, base_index(&view), &view, &files, &heads, &leaked).expect("trials");
        assert_ne!(with_future, trials);
    }

    /// Scenario 3: a trial records only the withheld path and each arm's
    /// rank or `unranked` with hit@10: rank 10 hits, rank 11 and unranked
    /// miss.
    #[test]
    fn trial_record_is_ranking_only() {
        let ranked: Vec<(RelativePath, f64)> = (1..=11)
            .map(|i| (path(&format!("pkg/f{i:02}.py")), 1.0 / f64::from(i)))
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
            c: unranked,
            e: tenth,
            f: eleventh,
        };
        assert_eq!(
            serde_json::to_value(&trial).expect("serializes"),
            json!({
                "pr": 7,
                "withheld": "pkg/f10.py",
                "C": {"rank": "unranked", "hit_at_10": false},
                "E": {"rank": 10, "hit_at_10": true},
                "F": {"rank": 11, "hit_at_10": false},
            })
        );
        assert_eq!(trial.arms().map(|(id, _)| id), ["C", "E", "F"]);
    }
}
