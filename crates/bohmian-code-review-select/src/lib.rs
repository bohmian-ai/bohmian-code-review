//! Changed-line file ranking and token-bounded context packing.
//!
//! [`rank`] turns a changed head-side file into query names — declarations
//! overlapping [`ChangedLines`] and references located on them — resolves
//! them through the real index, and scores every other file by its count of
//! distinct matched names. Ranks describe only indexed files and name-based
//! candidates, never proven definitions.

use bohmian_code_review_core::{Node, RawReference, RelativePath};
use bohmian_code_review_index::{SymbolIndex, is_declaration};
use bohmian_code_review_resolve::{declaration_names, reference_names, resolve};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// A one-based, inclusive source line range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LineRange {
    /// First line, one-based.
    pub start: u32,
    /// Last line, inclusive.
    pub end: u32,
}

impl LineRange {
    /// Whether `start..=end` shares at least one line with this range.
    fn overlaps(self, start: u32, end: u32) -> bool {
        self.start <= end && start <= self.end
    }

    /// Whether `line` falls inside this range.
    fn contains(self, line: u32) -> bool {
        self.start <= line && line <= self.end
    }
}

/// Head-side changed lines validated against the head source.
///
/// Invariant: nonempty, one-based, ascending, nonoverlapping, and within the
/// head source's line count at construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedLines(Vec<LineRange>);

impl ChangedLines {
    /// Validates `ranges` against `head`.
    ///
    /// # Errors
    ///
    /// Returns [`SelectError::DeletionOnly`] for no ranges,
    /// [`SelectError::InvalidRange`] for a zero or reversed range,
    /// [`SelectError::UnorderedRanges`] when a range does not start after
    /// its predecessor ends, and [`SelectError::RangeBeyondSource`] when a
    /// range ends past the last line of `head`.
    pub fn new(ranges: Vec<LineRange>, head: &str) -> Result<Self, SelectError> {
        if ranges.is_empty() {
            return Err(SelectError::DeletionOnly);
        }
        let source_lines = line_count(head);
        let mut previous: Option<LineRange> = None;
        for &lines in &ranges {
            if lines.start == 0 || lines.start > lines.end {
                return Err(SelectError::InvalidRange {
                    start: lines.start,
                    end: lines.end,
                });
            }
            if let Some(previous) = previous.filter(|previous| lines.start <= previous.end) {
                return Err(SelectError::UnorderedRanges {
                    previous,
                    next: lines,
                });
            }
            if lines.end as usize > source_lines {
                return Err(SelectError::RangeBeyondSource {
                    lines,
                    source_lines,
                });
            }
            previous = Some(lines);
        }
        Ok(ChangedLines(ranges))
    }

    /// The validated ranges in ascending order.
    #[must_use]
    pub fn ranges(&self) -> &[LineRange] {
        &self.0
    }

    /// Whether any changed range shares a line with `start..=end`.
    fn overlaps(&self, start: u32, end: u32) -> bool {
        self.0.iter().any(|lines| lines.overlaps(start, end))
    }

    /// Whether `line` is changed.
    fn contains(&self, line: u32) -> bool {
        self.0.iter().any(|lines| lines.contains(line))
    }
}

/// One matched name linking a ranked file to a declaration in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchReason {
    /// The exact query name that matched.
    pub name: String,
    /// ID of the matched declaration.
    pub node_id: String,
    /// The matched declaration's line span.
    pub lines: LineRange,
}

/// A secondary candidate file and why it ranked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedFile {
    /// Canonical path of the candidate.
    pub path: RelativePath,
    /// Count of distinct query names matching a declaration in this file.
    pub score: usize,
    /// Every matched name/declaration pair, ordered by name then location.
    pub reasons: Vec<MatchReason>,
}

/// The ranked candidates for one changed head-side file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranking {
    /// The changed file, never a secondary candidate.
    pub changed_path: RelativePath,
    /// The validated changed lines the ranking was computed from.
    pub changed_lines: ChangedLines,
    /// Positive-score files by descending score, then ascending path.
    pub files: Vec<RankedFile>,
}

/// Ranks indexed files against the changed lines of one head-side file.
///
/// `nodes` and `references` are the full head blob's extracted facts.
/// Declarations overlapping `changed_lines` contribute their names and
/// references on changed lines contribute their reference and candidate
/// names. Each distinct name adds one point to every other file declaring
/// it, however often it repeats; ambiguous names score every matching file.
#[must_use]
pub fn rank(
    index: &SymbolIndex,
    changed_path: &RelativePath,
    nodes: &[Node],
    references: &[RawReference],
    changed_lines: &ChangedLines,
) -> Ranking {
    let declared = nodes
        .iter()
        .filter(|node| {
            is_declaration(node.kind) && changed_lines.overlaps(node.start_line, node.end_line)
        })
        .flat_map(declaration_names);
    let referenced = references
        .iter()
        .filter(|reference| changed_lines.contains(reference.line))
        .flat_map(reference_names);

    let mut matched: BTreeMap<&RelativePath, (BTreeSet<String>, Vec<MatchReason>)> =
        BTreeMap::new();
    for found in resolve(index, declared.chain(referenced)) {
        for hit in found.hits.iter().filter(|hit| hit.path != changed_path) {
            let (names, reasons) = matched.entry(hit.path).or_default();
            names.insert(found.name.clone());
            reasons.push(MatchReason {
                name: found.name.clone(),
                node_id: hit.node.id.clone(),
                lines: LineRange {
                    start: hit.node.start_line,
                    end: hit.node.end_line,
                },
            });
        }
    }

    let mut files: Vec<RankedFile> = matched
        .into_iter()
        .map(|(path, (names, reasons))| RankedFile {
            path: path.clone(),
            score: names.len(),
            reasons,
        })
        .collect();
    files.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ranking {
        changed_path: changed_path.clone(),
        changed_lines: changed_lines.clone(),
        files,
    }
}

/// Typed ranking and packing failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SelectError {
    /// No changed head-side lines were supplied.
    #[error("no changed lines: deletion-only changes are unsupported")]
    DeletionOnly,
    /// A range starts at zero or ends before it starts.
    #[error("invalid changed-line range {start}..={end}: lines are one-based and inclusive")]
    InvalidRange {
        /// Supplied start line.
        start: u32,
        /// Supplied end line.
        end: u32,
    },
    /// A range does not start after its predecessor ends.
    #[error("changed-line ranges must ascend without overlap: {previous:?} then {next:?}")]
    UnorderedRanges {
        /// The earlier range.
        previous: LineRange,
        /// The range that overlaps or precedes it.
        next: LineRange,
    },
    /// A changed range ends past the head source.
    #[error("changed-line range {lines:?} exceeds the head source's {source_lines} lines")]
    RangeBeyondSource {
        /// The offending range.
        lines: LineRange,
        /// Line count of the head source.
        source_lines: usize,
    },
}

/// Number of lines in `source`, counting a final unterminated line.
fn line_count(source: &str) -> usize {
    source.split_inclusive('\n').count()
}

#[cfg(test)]
/// Owner tests for ranking and packing over the real index and resolver.
mod tests {
    use super::*;
    use bohmian_code_review_core::{EdgeKind, Language, NodeKind, ReferenceKind};

    /// Builds a TypeScript fact of `kind` spanning `start..=end`.
    fn node(kind: NodeKind, file: &str, name: &str, start: u32, end: u32) -> Node {
        Node {
            id: format!("{}:{file}:{name}", kind.wire_name()),
            kind,
            name: name.to_owned(),
            qualified_name: name.to_owned(),
            file_path: file.to_owned(),
            language: Language::Typescript,
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

    /// Builds a function declaration spanning `start..=end`.
    fn function(file: &str, name: &str, start: u32, end: u32) -> Node {
        node(NodeKind::Function, file, name, start, end)
    }

    /// Builds a call reference to `name` on `line`.
    fn call(name: &str, line: u32) -> RawReference {
        RawReference {
            from_node_id: "function:src/changed.ts:changed".to_owned(),
            reference_name: name.to_owned(),
            reference_kind: ReferenceKind::Edge(EdgeKind::Calls),
            line,
            column: 2,
            file_path: None,
            language: None,
            candidates: None,
        }
    }

    /// Parses a path known to be canonical.
    ///
    /// # Panics
    ///
    /// Panics if `raw` is not a valid repository-relative path.
    fn path(raw: &str) -> RelativePath {
        RelativePath::new(raw).expect("valid path")
    }

    /// Builds `lines` numbered source lines, each `lineN\n`.
    fn source(lines: u32) -> String {
        (1..=lines).map(|line| format!("line{line}\n")).collect()
    }

    /// Validates `(start, end)` ranges against `head`.
    ///
    /// # Errors
    ///
    /// Returns the selector's range validation error.
    fn changed(ranges: &[(u32, u32)], head: &str) -> Result<ChangedLines, SelectError> {
        let ranges = ranges
            .iter()
            .map(|&(start, end)| LineRange { start, end })
            .collect();
        ChangedLines::new(ranges, head)
    }

    /// Index shared by ranking tests: `b.ts` and `a.ts` each declare two
    /// queried names, `c.ts` one, `d.ts` only an off-range name, and the
    /// changed file declares a name that must not rank itself.
    fn fixture_index() -> SymbolIndex {
        let mut index = SymbolIndex::new();
        let files = [
            (
                "src/b.ts",
                vec![
                    function("src/b.ts", "load", 1, 3),
                    function("src/b.ts", "save", 5, 7),
                ],
            ),
            (
                "src/a.ts",
                vec![
                    function("src/a.ts", "load", 2, 4),
                    function("src/a.ts", "Changed", 6, 6),
                ],
            ),
            ("src/c.ts", vec![function("src/c.ts", "save", 1, 1)]),
            ("src/d.ts", vec![function("src/d.ts", "offRange", 1, 1)]),
            (
                "src/changed.ts",
                vec![function("src/changed.ts", "load", 20, 20)],
            ),
        ];
        for (file, nodes) in files {
            index.upsert(path(file), 1, nodes);
        }
        index
    }

    #[test]
    /// Distinct names score once per file; off-range references, the changed
    /// file, and zero scores are excluded; ties sort by canonical path.
    fn ranking_counts_distinct_changed_names_and_breaks_ties_by_path() {
        let index = fixture_index();
        let head = source(12);
        let nodes = vec![
            node(NodeKind::File, "src/changed.ts", "changed.ts", 1, 12),
            node(NodeKind::Import, "src/changed.ts", "load", 1, 1),
            function("src/changed.ts", "Changed", 4, 9),
            function("src/changed.ts", "untouched", 11, 12),
        ];
        let references = vec![
            call("load", 5),
            call("load", 6),
            call("save", 7),
            call("save", 8),
            call("offRange", 10),
            call("missing", 5),
        ];
        let changed = changed(&[(5, 5), (7, 8)], &head).expect("valid ranges");
        let ranking = rank(
            &index,
            &path("src/changed.ts"),
            &nodes,
            &references,
            &changed,
        );

        let scores: Vec<(&str, usize)> = ranking
            .files
            .iter()
            .map(|file| (file.path.as_str(), file.score))
            .collect();
        assert_eq!(scores, [("src/a.ts", 2), ("src/b.ts", 2), ("src/c.ts", 1)]);

        let reasons: Vec<(&str, u32, u32)> = ranking.files[1]
            .reasons
            .iter()
            .map(|reason| (reason.name.as_str(), reason.lines.start, reason.lines.end))
            .collect();
        assert_eq!(reasons, [("load", 1, 3), ("save", 5, 7)]);
        assert_eq!(ranking.files[0].reasons[0].name, "Changed");
        assert_eq!(
            ranking,
            rank(
                &index,
                &path("src/changed.ts"),
                &nodes,
                &references,
                &changed
            )
        );
    }

    #[test]
    /// Changed lines must be nonempty, one-based, ordered, disjoint, and
    /// inside the head source.
    fn changed_lines_reject_invalid_and_deletion_only_input() {
        let head = source(5);
        assert_eq!(changed(&[], &head), Err(SelectError::DeletionOnly));
        assert_eq!(
            changed(&[(0, 1)], &head),
            Err(SelectError::InvalidRange { start: 0, end: 1 })
        );
        assert_eq!(
            changed(&[(3, 2)], &head),
            Err(SelectError::InvalidRange { start: 3, end: 2 })
        );
        assert_eq!(
            changed(&[(2, 3), (3, 4)], &head),
            Err(SelectError::UnorderedRanges {
                previous: LineRange { start: 2, end: 3 },
                next: LineRange { start: 3, end: 4 },
            })
        );
        assert_eq!(
            changed(&[(4, 6)], &head),
            Err(SelectError::RangeBeyondSource {
                lines: LineRange { start: 4, end: 6 },
                source_lines: 5,
            })
        );
        assert!(changed(&[(1, 1), (3, 5)], &head).is_ok());
    }
}
