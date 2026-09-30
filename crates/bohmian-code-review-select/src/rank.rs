//! Pluggable pull-request file ranking.
//!
//! A [`Ranker`] scores indexed candidate files for one pull request from a
//! [`RankInput`]: the index, every changed file, and optional caller-supplied
//! co-change history. Every ranker excludes each changed path and previous
//! path, drops zero scores, and orders files by descending score, then
//! ascending canonical path. [`SymbolOverlap`] counts distinct matched
//! names; [`RaritySymbol`] credits directional name matches by rarity;
//! [`CoChange`] credits files earlier changes touched together with a
//! changed file; [`Fused`] combines two rankings by reciprocal rank.
//! [`DEFAULT_RANKER`] fuses rarity and co-change, and with empty history it
//! orders exactly as [`RaritySymbol`].

use crate::{ChangedLines, LineRange, MatchReason, RankedFile};
use bohmian_code_review_core::{Node, RawReference, RelativePath};
use bohmian_code_review_index::{SymbolHit, SymbolIndex, is_declaration};
use bohmian_code_review_resolve::{declaration_names, reference_names, resolve};
use std::collections::{BTreeMap, BTreeSet};

/// [`RaritySymbol`] ignores a name declared or referenced by more indexed
/// files than this.
pub const DF_CEILING: usize = 100;
/// Reciprocal-rank constant `k` of [`Fused`]: rank `r` earns `1 / (k + r)`.
pub const FUSION_CONSTANT: usize = 60;
/// The ranker the pipeline uses: rarity-weighted symbols fused with
/// co-change. Empty history leaves exactly the [`RaritySymbol`] order.
pub const DEFAULT_RANKER: Fused<RaritySymbol, CoChange> = Fused(RaritySymbol, CoChange);

/// Scores indexed candidate files for one pull request.
///
/// Implementations are IO-free and deterministic: the same input yields the
/// same files, excluding every changed and previous path, with positive
/// scores, by descending score then ascending path.
pub trait Ranker {
    /// Ranks the candidates of `input`.
    fn rank(&self, input: &RankInput<'_>) -> Vec<RankedFile>;
}

/// Everything a [`Ranker`] may read.
#[derive(Debug, Clone, Copy)]
pub struct RankInput<'a> {
    /// The candidate universe; only indexed files are ranked.
    pub index: &'a SymbolIndex,
    /// Every changed file of the pull request, in listing order.
    pub changed: &'a [ChangedFile<'a>],
    /// Changed paths of each earlier merged change, empty when the caller
    /// has no co-change history.
    pub history: &'a [&'a [RelativePath]],
}

/// A changed file's full head facts and validated head-side changed lines.
#[derive(Debug, Clone, Copy)]
pub struct ChangedHead<'a> {
    /// The full head blob's extracted nodes.
    pub nodes: &'a [Node],
    /// The full head blob's extracted references.
    pub references: &'a [RawReference],
    /// Validated changed lines of the head blob.
    pub changed_lines: &'a ChangedLines,
}

impl<'a> ChangedHead<'a> {
    /// Simple and qualified names of declarations overlapping the changed
    /// lines.
    fn declared(self) -> impl Iterator<Item = &'a str> {
        self.nodes
            .iter()
            .filter(move |node| {
                is_declaration(node.kind)
                    && self.changed_lines.overlaps(node.start_line, node.end_line)
            })
            .flat_map(declaration_names)
    }

    /// Reference and candidate names of references on the changed lines.
    fn used(self) -> impl Iterator<Item = &'a str> {
        self.references
            .iter()
            .filter(move |reference| self.changed_lines.contains(reference.line))
            .flat_map(reference_names)
    }
}

/// One changed file of a pull request as rankers see it.
#[derive(Debug, Clone)]
pub struct ChangedFile<'a> {
    /// Head-side path; never a candidate.
    pub path: &'a RelativePath,
    /// Path before a rename or copy; never a candidate.
    pub previous_path: Option<&'a RelativePath>,
    /// Whether `path` existed before the change; only such paths anchor
    /// co-change history.
    pub in_base: bool,
    /// Head facts, absent for a removed, deletion-only, or unanalyzed file.
    pub head: Option<ChangedHead<'a>>,
    /// Simple and qualified names of base declarations the change removes
    /// lines from; empty when the caller has no base facts.
    pub removed: Vec<&'a str>,
}

/// Counts distinct query names — changed-line declarations and references —
/// matching a declaration in each candidate, however often they repeat.
#[derive(Debug, Clone, Copy, Default)]
pub struct SymbolOverlap;

impl Ranker for SymbolOverlap {
    /// Scores each candidate by its distinct matched names, with every
    /// matched name/declaration pair as a reason.
    fn rank(&self, input: &RankInput<'_>) -> Vec<RankedFile> {
        let excluded = excluded(input.changed);
        let names = heads(input.changed).flat_map(|head| head.declared().chain(head.used()));
        let mut matched: BTreeMap<&RelativePath, (BTreeSet<String>, Vec<MatchReason>)> =
            BTreeMap::new();
        for found in resolve(input.index, names) {
            for hit in found.hits.iter().filter(|hit| !excluded.contains(hit.path)) {
                let (names, reasons) = matched.entry(hit.path).or_default();
                names.insert(found.name.clone());
                reasons.push(reason(&found.name, hit));
            }
        }
        ordered(
            matched
                .into_iter()
                .map(|(path, (names, reasons))| RankedFile {
                    path: path.clone(),
                    score: names.len() as f64,
                    reasons,
                })
                .collect(),
        )
    }
}

/// Directional, rarity-weighted symbol ranking.
///
/// A name's document frequency `df` counts the distinct indexed files
/// declaring or referencing it; names with `df` above [`DF_CEILING`] are
/// ignored. A file declaring a name the change uses (upstream) earns
/// `1 / (df * declaring files)`; a file referencing a name the change
/// declares or removes (downstream) earns `1 / df`. A file matching a name
/// both ways keeps the larger credit, and its score sums credits over
/// distinct names. Only upstream matches carry reasons.
#[derive(Debug, Clone, Copy, Default)]
pub struct RaritySymbol;

impl Ranker for RaritySymbol {
    /// Scores candidates by rarity-weighted upstream and downstream credit.
    fn rank(&self, input: &RankInput<'_>) -> Vec<RankedFile> {
        let index = input.index;
        let excluded = excluded(input.changed);
        let mut credits = Credits::default();
        let used: BTreeSet<&str> = heads(input.changed).flat_map(ChangedHead::used).collect();
        for name in used {
            let hits = index.lookup(name);
            let declaring: BTreeSet<&RelativePath> = hits.iter().map(|hit| hit.path).collect();
            let df = document_frequency(index, name, &declaring);
            if df > DF_CEILING {
                continue;
            }
            let credit = 1.0 / (df * declaring.len()) as f64;
            for hit in hits.iter().filter(|hit| !excluded.contains(hit.path)) {
                credits.credit(hit.path, name, credit);
                credits
                    .reasons
                    .entry(hit.path)
                    .or_default()
                    .push(reason(name, hit));
            }
        }
        let changed: BTreeSet<&str> = input
            .changed
            .iter()
            .flat_map(|file| {
                let declared = file.head.into_iter().flat_map(ChangedHead::declared);
                declared.chain(file.removed.iter().copied())
            })
            .collect();
        for name in changed {
            let referencing: BTreeSet<&RelativePath> = index.referencing(name).collect();
            if referencing.is_empty() {
                continue;
            }
            let declaring = index.lookup(name).iter().map(|hit| hit.path).collect();
            let df = document_frequency(index, name, &declaring);
            if df > DF_CEILING {
                continue;
            }
            for path in referencing
                .into_iter()
                .filter(|path| !excluded.contains(path))
            {
                credits.credit(path, name, 1.0 / df as f64);
            }
        }
        credits.into_ranked()
    }
}

/// Per-file rarity credits by name and the upstream reasons behind them.
#[derive(Default)]
struct Credits<'a> {
    /// Largest credit each file earned per distinct name.
    by_name: BTreeMap<&'a RelativePath, BTreeMap<&'a str, f64>>,
    /// Upstream name/declaration matches per file, by name then location.
    reasons: BTreeMap<&'a RelativePath, Vec<MatchReason>>,
}

impl<'a> Credits<'a> {
    /// Keeps the larger of `credit` and `path`'s credit for `name`.
    fn credit(&mut self, path: &'a RelativePath, name: &'a str, credit: f64) {
        let held = self
            .by_name
            .entry(path)
            .or_default()
            .entry(name)
            .or_default();
        *held = held.max(credit);
    }

    /// Files scored by their credits summed in name order.
    fn into_ranked(mut self) -> Vec<RankedFile> {
        ordered(
            self.by_name
                .into_iter()
                .map(|(path, names)| RankedFile {
                    path: path.clone(),
                    score: names.values().sum(),
                    reasons: self.reasons.remove(path).unwrap_or_default(),
                })
                .collect(),
        )
    }
}

/// Co-change ranking over caller-supplied history.
///
/// For each changed file `d` that existed before the change, is indexed,
/// and appears in `n(d) > 0` history entries, every indexed candidate `f`
/// earns `n(d, f) / n(d)`, where `n(d, f)` counts the entries touching
/// both; scores sum over `d` in listing order. No reasons are given, so a
/// packer selects such a candidate's declarations in location order.
#[derive(Debug, Clone, Copy, Default)]
pub struct CoChange;

impl Ranker for CoChange {
    /// Scores candidates by conditional co-change frequency.
    fn rank(&self, input: &RankInput<'_>) -> Vec<RankedFile> {
        let index = input.index;
        let excluded = excluded(input.changed);
        let mut scores: BTreeMap<&RelativePath, f64> = BTreeMap::new();
        for file in input
            .changed
            .iter()
            .filter(|file| file.in_base && index.contains(file.path))
        {
            let touching: Vec<&[RelativePath]> = input
                .history
                .iter()
                .copied()
                .filter(|paths| paths.contains(file.path))
                .collect();
            let mut together: BTreeMap<&RelativePath, usize> = BTreeMap::new();
            for path in touching.iter().copied().flatten() {
                if index.contains(path) && !excluded.contains(path) {
                    *together.entry(path).or_default() += 1;
                }
            }
            for (path, count) in together {
                *scores.entry(path).or_default() += count as f64 / touching.len() as f64;
            }
        }
        ordered(
            scores
                .into_iter()
                .map(|(path, score)| RankedFile {
                    path: path.clone(),
                    score,
                    reasons: Vec::new(),
                })
                .collect(),
        )
    }
}

/// Reciprocal-rank fusion of two rankers.
///
/// A file at one-based rank `r` in either ranking earns
/// `1 / (FUSION_CONSTANT + r)`, summed over both, first ranker first. Its
/// reasons are the first ranker's followed by the second's not already
/// given.
#[derive(Debug, Clone, Copy, Default)]
pub struct Fused<A, B>(pub A, pub B);

impl<A: Ranker, B: Ranker> Ranker for Fused<A, B> {
    /// Fuses both rankings of `input`.
    fn rank(&self, input: &RankInput<'_>) -> Vec<RankedFile> {
        let mut fused: BTreeMap<RelativePath, (f64, Vec<MatchReason>)> = BTreeMap::new();
        for ranking in [self.0.rank(input), self.1.rank(input)] {
            for (position, file) in ranking.into_iter().enumerate() {
                let (score, reasons) = fused.entry(file.path).or_default();
                *score += 1.0 / (FUSION_CONSTANT + position + 1) as f64;
                for reason in file.reasons {
                    if !reasons.contains(&reason) {
                        reasons.push(reason);
                    }
                }
            }
        }
        ordered(
            fused
                .into_iter()
                .map(|(path, (score, reasons))| RankedFile {
                    path,
                    score,
                    reasons,
                })
                .collect(),
        )
    }
}

/// Every changed and previous path; none may be a candidate.
fn excluded<'a>(changed: &'a [ChangedFile<'a>]) -> BTreeSet<&'a RelativePath> {
    changed
        .iter()
        .flat_map(|file| std::iter::once(file.path).chain(file.previous_path))
        .collect()
}

/// Head facts of every changed file that has them.
fn heads<'a>(changed: &'a [ChangedFile<'a>]) -> impl Iterator<Item = ChangedHead<'a>> {
    changed.iter().filter_map(|file| file.head)
}

/// Distinct indexed files declaring (`declaring`) or referencing `name`.
fn document_frequency(
    index: &SymbolIndex,
    name: &str,
    declaring: &BTreeSet<&RelativePath>,
) -> usize {
    declaring
        .iter()
        .copied()
        .chain(index.referencing(name))
        .collect::<BTreeSet<_>>()
        .len()
}

/// The reason `name` matched `hit`'s declaration.
fn reason(name: &str, hit: &SymbolHit<'_>) -> MatchReason {
    MatchReason {
        name: name.to_owned(),
        node_id: hit.node.id.clone(),
        lines: LineRange {
            start: hit.node.start_line,
            end: hit.node.end_line,
        },
    }
}

/// `files` without nonpositive scores, by descending score then path.
fn ordered(mut files: Vec<RankedFile>) -> Vec<RankedFile> {
    files.retain(|file| file.score > 0.0);
    files.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    files
}

#[cfg(test)]
/// Owner tests for each ranker over the real index and resolver with
/// hand-built facts: rarity credit, co-change anchoring, fusion, and
/// exclusion.
mod tests {
    use super::*;
    use bohmian_code_review_core::{EdgeKind, Language, NodeKind, ReferenceKind};

    /// Parses a path known to be canonical.
    ///
    /// # Panics
    ///
    /// Panics if `raw` is not a valid repository-relative path.
    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid path")
    }

    /// A Python function declaration of `name` on lines `start..=end`.
    fn function(file: &str, name: &str, start: u32, end: u32) -> Node {
        Node {
            id: format!("function:{file}:{name}"),
            kind: NodeKind::Function,
            name: name.to_owned(),
            qualified_name: name.to_owned(),
            file_path: file.to_owned(),
            language: Language::Python,
            start_line: start,
            end_line: end,
            start_column: 0,
            end_column: 1,
            docstring: None,
            signature: None,
            visibility: None,
            is_exported: None,
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: None,
            extra: None,
        }
    }

    /// A call to `name` on `line`.
    fn call(name: &str, line: u32) -> RawReference {
        RawReference {
            from_node_id: "function:changed".to_owned(),
            reference_name: name.to_owned(),
            reference_kind: ReferenceKind::Edge(EdgeKind::Calls),
            line,
            column: 0,
            file_path: None,
            language: None,
            candidates: None,
        }
    }

    /// Indexes `file` declaring `declared` (one line each) and referencing
    /// `referenced`.
    fn add(index: &mut SymbolIndex, file: &str, declared: &[&str], referenced: &[&str]) {
        let nodes = (1..)
            .zip(declared)
            .map(|(line, name)| function(file, name, line, line))
            .collect();
        let names = referenced.iter().map(|&name| name.to_owned()).collect();
        index.upsert(path(file), 1, nodes, names);
    }

    /// `(path, score)` of `ranked`, best first.
    fn scores(ranked: &[RankedFile]) -> Vec<(&str, f64)> {
        ranked
            .iter()
            .map(|file| (file.path.as_str(), file.score))
            .collect()
    }

    /// Asserts `ranked` holds exactly `expected` in order, scores within
    /// `1e-12`.
    ///
    /// # Panics
    ///
    /// Panics when a path, the order, or a score differs.
    fn assert_scores(ranked: &[RankedFile], expected: &[(&str, f64)]) {
        let got = scores(ranked);
        assert_eq!(got.len(), expected.len(), "{got:?}");
        for ((path, score), (want, want_score)) in got.iter().zip(expected) {
            assert_eq!(path, want);
            assert!((score - want_score).abs() < 1e-12, "{path}: {score}");
        }
    }

    #[test]
    /// Upstream credit splits across ambiguous declarers, downstream credit
    /// reaches referencing files through changed and removed names, a file
    /// earning both keeps the larger per name and sums names, changed paths
    /// never rank, names past the ceiling are ignored, and only upstream
    /// matches carry reasons.
    fn rarity_credits_by_direction_and_document_frequency() {
        let mut index = SymbolIndex::new();
        add(&mut index, "pkg/util.py", &["helper", "gone"], &[]);
        add(&mut index, "pkg/legacy.py", &["helper"], &[]);
        add(&mut index, "pkg/models.py", &["Model"], &["helper"]);
        add(&mut index, "pkg/far.py", &["far"], &["helper"]);
        add(&mut index, "pkg/loose.py", &["loose"], &["helper", "gone"]);
        add(&mut index, "pkg/caller.py", &["call"], &["helper"]);
        add(&mut index, "pkg/common.py", &["common"], &[]);
        for n in 0..DF_CEILING {
            add(&mut index, &format!("use/{n}.py"), &[], &["common"]);
        }
        let util_nodes = [function("pkg/util.py", "helper", 1, 2)];
        let util_lines =
            ChangedLines::new(vec![LineRange { start: 2, end: 2 }], "a\nb\n").expect("valid lines");
        let caller_references = [call("helper", 1), call("common", 1)];
        let caller_lines =
            ChangedLines::new(vec![LineRange { start: 1, end: 1 }], "a\n").expect("valid lines");
        let (util, caller) = (path("pkg/util.py"), path("pkg/caller.py"));
        let changed = [
            ChangedFile {
                path: &util,
                previous_path: None,
                in_base: true,
                head: Some(ChangedHead {
                    nodes: &util_nodes,
                    references: &[],
                    changed_lines: &util_lines,
                }),
                removed: vec!["gone"],
            },
            ChangedFile {
                path: &caller,
                previous_path: None,
                in_base: true,
                head: Some(ChangedHead {
                    nodes: &[],
                    references: &caller_references,
                    changed_lines: &caller_lines,
                }),
                removed: Vec::new(),
            },
        ];
        let ranked = RaritySymbol.rank(&RankInput {
            index: &index,
            changed: &changed,
            history: &[],
        });
        // helper: df 6 (util, legacy declare; models, far, loose, caller
        // reference). gone: df 2. common: df 101, ignored.
        assert_scores(
            &ranked,
            &[
                ("pkg/loose.py", 1.0 / 6.0 + 0.5),
                ("pkg/far.py", 1.0 / 6.0),
                ("pkg/models.py", 1.0 / 6.0),
                ("pkg/legacy.py", 1.0 / 12.0),
            ],
        );
        let reasons: Vec<(&str, &str)> = ranked
            .iter()
            .flat_map(|file| {
                file.reasons
                    .iter()
                    .map(|reason| (file.path.as_str(), reason.name.as_str()))
            })
            .collect();
        assert_eq!(reasons, [("pkg/legacy.py", "helper")]);
    }

    #[test]
    /// Co-change credits `n(d, f) / n(d)` summed over changed files that
    /// existed before the change and are indexed; unindexed history paths,
    /// changed paths, and anchors that did not exist before never count.
    fn cochange_anchors_only_indexed_base_paths() {
        let mut index = SymbolIndex::new();
        for file in ["a", "b", "c", "w", "x", "z"] {
            add(&mut index, &format!("pkg/{file}.py"), &[file], &[]);
        }
        let paths: Vec<RelativePath> = ["b", "c", "u", "z"]
            .iter()
            .map(|file| path(&format!("pkg/{file}.py")))
            .collect();
        let changed: Vec<ChangedFile<'_>> = paths
            .iter()
            .map(|path| ChangedFile {
                path,
                previous_path: None,
                in_base: !path.as_str().ends_with("z.py"),
                head: None,
                removed: Vec::new(),
            })
            .collect();
        let owned: Vec<Vec<RelativePath>> = [
            &["b", "x", "w"][..],
            &["b", "x", "w", "y"],
            &["b", "a", "c"],
            &["c", "a"],
            &["u", "w"],
            &["z", "w"],
        ]
        .iter()
        .map(|entry| {
            entry
                .iter()
                .map(|file| path(&format!("pkg/{file}.py")))
                .collect()
        })
        .collect();
        let history: Vec<&[RelativePath]> = owned.iter().map(Vec::as_slice).collect();
        let ranked = CoChange.rank(&RankInput {
            index: &index,
            changed: &changed,
            history: &history,
        });
        assert_scores(
            &ranked,
            &[
                ("pkg/a.py", 1.0 / 3.0 + 1.0),
                ("pkg/w.py", 2.0 / 3.0),
                ("pkg/x.py", 2.0 / 3.0),
            ],
        );
        assert!(ranked.iter().all(|file| file.reasons.is_empty()));
        let empty = CoChange.rank(&RankInput {
            index: &index,
            changed: &changed,
            history: &[],
        });
        assert!(empty.is_empty());
    }

    /// A ranker returning a fixed ranking.
    struct Fixed(Vec<RankedFile>);

    impl Ranker for Fixed {
        /// The fixed ranking, whatever the input.
        fn rank(&self, _: &RankInput<'_>) -> Vec<RankedFile> {
            self.0.clone()
        }
    }

    /// A ranked file with `reasons` named as given.
    fn ranked(raw: &str, score: f64, reasons: &[&str]) -> RankedFile {
        RankedFile {
            path: path(raw),
            score,
            reasons: reasons
                .iter()
                .map(|&name| MatchReason {
                    name: name.to_owned(),
                    node_id: format!("function:{raw}:{name}"),
                    lines: LineRange { start: 1, end: 1 },
                })
                .collect(),
        }
    }

    #[test]
    /// Fusion sums `1 / (60 + rank)` over both rankings, breaks ties by
    /// path, and merges reasons without repeats; the default ranker with
    /// empty history orders exactly as rarity alone.
    fn fusion_sums_reciprocal_ranks_and_default_matches_rarity_without_history() {
        let first = Fixed(vec![ranked("pkg/v.py", 3.0, &["helper"])]);
        let second = Fixed(vec![
            ranked("pkg/a.py", 2.0, &[]),
            ranked("pkg/w.py", 1.0, &[]),
            ranked("pkg/v.py", 0.5, &["helper", "other"]),
        ]);
        let index = SymbolIndex::new();
        let input = RankInput {
            index: &index,
            changed: &[],
            history: &[],
        };
        let fused = Fused(first, second).rank(&input);
        assert_eq!(
            scores(&fused),
            [
                ("pkg/v.py", 1.0 / 61.0 + 1.0 / 63.0),
                ("pkg/a.py", 1.0 / 61.0),
                ("pkg/w.py", 1.0 / 62.0),
            ]
        );
        let names: Vec<&str> = fused[0]
            .reasons
            .iter()
            .map(|reason| reason.name.as_str())
            .collect();
        assert_eq!(names, ["helper", "other"]);

        let mut index = SymbolIndex::new();
        add(&mut index, "pkg/b.py", &["load"], &[]);
        add(&mut index, "pkg/a.py", &["load", "save"], &[]);
        add(&mut index, "pkg/c.py", &["save"], &["load"]);
        let references = [call("load", 1), call("save", 1)];
        let lines =
            ChangedLines::new(vec![LineRange { start: 1, end: 1 }], "a\n").expect("valid lines");
        let handler = path("pkg/handler.py");
        let changed = [ChangedFile {
            path: &handler,
            previous_path: None,
            in_base: true,
            head: Some(ChangedHead {
                nodes: &[],
                references: &references,
                changed_lines: &lines,
            }),
            removed: Vec::new(),
        }];
        let input = RankInput {
            index: &index,
            changed: &changed,
            history: &[],
        };
        let rarity = RaritySymbol.rank(&input);
        let default = DEFAULT_RANKER.rank(&input);
        assert_eq!(rarity.len(), 3);
        let order = |files: &[RankedFile]| -> Vec<RelativePath> {
            files.iter().map(|file| file.path.clone()).collect()
        };
        assert_eq!(order(&default), order(&rarity));
        assert_eq!(default[0].reasons, rarity[0].reasons);
    }

    #[test]
    /// Symbol overlap never ranks a changed path or a renamed file's
    /// previous path, even when it declares a queried name.
    fn previous_paths_are_never_candidates() {
        let mut index = SymbolIndex::new();
        add(&mut index, "pkg/old.py", &["load"], &[]);
        add(&mut index, "pkg/other.py", &["load"], &[]);
        let references = [call("load", 1)];
        let lines =
            ChangedLines::new(vec![LineRange { start: 1, end: 1 }], "a\n").expect("valid lines");
        let (new, old) = (path("pkg/new.py"), path("pkg/old.py"));
        let changed = [ChangedFile {
            path: &new,
            previous_path: Some(&old),
            in_base: false,
            head: Some(ChangedHead {
                nodes: &[],
                references: &references,
                changed_lines: &lines,
            }),
            removed: Vec::new(),
        }];
        let ranked = SymbolOverlap.rank(&RankInput {
            index: &index,
            changed: &changed,
            history: &[],
        });
        assert_eq!(scores(&ranked), [("pkg/other.py", 1.0)]);
    }
}
