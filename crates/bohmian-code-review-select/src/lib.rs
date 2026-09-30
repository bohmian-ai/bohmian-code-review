//! Changed-line file ranking and token-bounded context packing.
//!
//! [`rank`] turns a changed head-side file into query names — declarations
//! overlapping [`ChangedLines`] and references located on them — resolves
//! them through the real index, and scores every other file by its count of
//! distinct matched names. [`rank_files`] does the same for every changed
//! file of a pull request at once, and [`pack_candidates`] packs the result
//! without a changed head blob. Ranks describe only indexed files and
//! name-based candidates, never proven definitions.

use bohmian_code_review_core::{Node, RawReference, RelativePath};
use bohmian_code_review_index::{SymbolIndex, is_declaration};
use bohmian_code_review_resolve::{declaration_names, reference_names, resolve};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

mod diff;

pub use diff::{DiffError, FileDiff, parse_hunks, parse_unified_diff};

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

/// One changed head-side file's extracted facts and validated changed lines.
#[derive(Debug, Clone, Copy)]
pub struct ChangedFile<'a> {
    /// Canonical path of the changed file; never a secondary candidate.
    pub path: &'a RelativePath,
    /// The full head blob's extracted nodes.
    pub nodes: &'a [Node],
    /// The full head blob's extracted references.
    pub references: &'a [RawReference],
    /// Validated changed lines of the head blob.
    pub changed_lines: &'a ChangedLines,
}

impl ChangedFile<'_> {
    /// The query names this file contributes: names of declarations
    /// overlapping its changed lines and reference/candidate names of
    /// references on them.
    fn query_names(&self) -> impl Iterator<Item = &str> {
        let declared = self
            .nodes
            .iter()
            .filter(|node| {
                is_declaration(node.kind)
                    && self.changed_lines.overlaps(node.start_line, node.end_line)
            })
            .flat_map(declaration_names);
        let referenced = self
            .references
            .iter()
            .filter(|reference| self.changed_lines.contains(reference.line))
            .flat_map(reference_names);
        declared.chain(referenced)
    }
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
    let changed = ChangedFile {
        path: changed_path,
        nodes,
        references,
        changed_lines,
    };
    Ranking {
        changed_path: changed_path.clone(),
        changed_lines: changed_lines.clone(),
        files: rank_files(index, &[changed]),
    }
}

/// Ranks indexed files against every changed file of one pull request.
///
/// The query names of all `changed` files are unioned, so a name changed
/// in two files still scores one point per candidate file. Every changed
/// path is excluded from the candidates. Files sort by descending score,
/// then ascending canonical path.
#[must_use]
pub fn rank_files(index: &SymbolIndex, changed: &[ChangedFile<'_>]) -> Vec<RankedFile> {
    let excluded: BTreeSet<&RelativePath> = changed.iter().map(|file| file.path).collect();
    let names = changed.iter().flat_map(ChangedFile::query_names);

    let mut matched: BTreeMap<&RelativePath, (BTreeSet<String>, Vec<MatchReason>)> =
        BTreeMap::new();
    for found in resolve(index, names) {
        for hit in found.hits.iter().filter(|hit| !excluded.contains(hit.path)) {
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
    files
}

/// Caller-supplied context ceilings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextLimits {
    /// Maximum distinct files selected; [`pack`] counts the changed file,
    /// [`pack_candidates`] counts only selected candidate files.
    pub max_files: usize,
    /// Maximum caller-counted tokens across all selected text.
    pub max_tokens: usize,
}

/// Why a span was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanKind {
    /// A changed head-side hunk.
    ChangedHunk,
    /// A ranked candidate declaration.
    Candidate {
        /// The candidate file's score.
        score: usize,
        /// Query names that matched this declaration.
        names: Vec<String>,
    },
}

/// One whole span of selected source text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextSpan {
    /// Canonical path of the span's file.
    pub path: RelativePath,
    /// One-based inclusive lines of `text`.
    pub lines: LineRange,
    /// Why the span was selected.
    pub kind: SpanKind,
    /// The exact source lines, including line terminators.
    pub text: String,
    /// Tokens counted by the caller's tokenizer for `text`.
    pub tokens: usize,
}

/// Which ceiling excluded a candidate span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OmissionReason {
    /// Selecting the span's file would exceed `max_files`.
    FileLimit,
    /// The span's `tokens` would exceed the remaining token budget.
    TokenLimit {
        /// Caller-counted tokens of the omitted span.
        tokens: usize,
    },
}

/// A ranked candidate span left out of the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Omission {
    /// Canonical path of the omitted span's file.
    pub path: RelativePath,
    /// The omitted declaration's lines.
    pub lines: LineRange,
    /// Query names that matched the omitted declaration.
    pub names: Vec<String>,
    /// The ceiling that excluded it.
    pub reason: OmissionReason,
}

/// Bounded review context: changed hunks, then ranked candidate spans.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Context {
    /// Selected spans in packing order.
    pub spans: Vec<ContextSpan>,
    /// Candidate spans excluded by a ceiling, in packing order.
    pub omissions: Vec<Omission>,
    /// Distinct files selected, counted as `max_files` counts them.
    pub files: usize,
    /// Sum of selected span tokens.
    pub tokens: usize,
}

/// Packs changed hunks and then ranked candidate spans under `limits`.
///
/// Every changed hunk must fit. Candidates are visited in rank order and,
/// within a file, by declaration location; each declaration is one whole
/// span. A span that does not fit is omitted with its reason and packing
/// continues, so a later smaller span may still fit. A file-limited
/// candidate's blob is never read; every other candidate must be in
/// `blobs`. Text is only counted by `count_tokens`, never cut.
///
/// # Errors
///
/// Returns [`SelectError::NoFileBudget`] when `max_files` is zero,
/// [`SelectError::RangeBeyondSource`] when `head` is shorter than the
/// changed lines, [`SelectError::ChangedHunkOverBudget`] when a hunk does
/// not fit, [`SelectError::MissingBlob`] or [`SelectError::BlobNotUtf8`]
/// for an unusable candidate blob, and [`SelectError::SpanBeyondSource`]
/// when a candidate blob is shorter than its indexed declaration.
pub fn pack(
    ranking: &Ranking,
    head: &str,
    blobs: &BTreeMap<RelativePath, &[u8]>,
    limits: ContextLimits,
    count_tokens: impl Fn(&str) -> usize,
) -> Result<Context, SelectError> {
    if limits.max_files == 0 {
        return Err(SelectError::NoFileBudget);
    }
    let mut context = Context {
        files: 1,
        ..Context::default()
    };
    for &lines in ranking.changed_lines.ranges() {
        let text = slice_lines(head, lines).ok_or(SelectError::RangeBeyondSource {
            lines,
            source_lines: line_count(head),
        })?;
        let tokens = count_tokens(text);
        let remaining_tokens = limits.max_tokens - context.tokens;
        if tokens > remaining_tokens {
            return Err(SelectError::ChangedHunkOverBudget {
                lines,
                tokens,
                remaining_tokens,
            });
        }
        context.tokens += tokens;
        context.spans.push(ContextSpan {
            path: ranking.changed_path.clone(),
            lines,
            kind: SpanKind::ChangedHunk,
            text: text.to_owned(),
            tokens,
        });
    }

    pack_candidates_into(&mut context, &ranking.files, blobs, limits, &count_tokens)?;
    Ok(context)
}

/// Packs only ranked candidate spans under `limits`; the changed hunks are
/// delivered separately, so none is counted here.
///
/// `files` is a ranking from [`rank_files`] or [`Ranking::files`]. Packing
/// follows [`pack`]'s candidate rules, except that `max_files` and the
/// returned file count cover selected candidate files alone. A zero
/// `max_files` omits every candidate with a file-limit reason rather than
/// failing.
///
/// # Errors
///
/// Returns [`SelectError::MissingBlob`] or [`SelectError::BlobNotUtf8`]
/// for an unusable candidate blob, and [`SelectError::SpanBeyondSource`]
/// when a candidate blob is shorter than its indexed declaration.
pub fn pack_candidates(
    files: &[RankedFile],
    blobs: &BTreeMap<RelativePath, &[u8]>,
    limits: ContextLimits,
    count_tokens: impl Fn(&str) -> usize,
) -> Result<Context, SelectError> {
    let mut context = Context::default();
    pack_candidates_into(&mut context, files, blobs, limits, &count_tokens)?;
    Ok(context)
}

/// Appends ranked candidate spans to `context` under `limits`.
///
/// Candidates are visited in rank order and, within a file, by declaration
/// location; each declaration is one whole span. A span that does not fit
/// is omitted with its reason and packing continues. A file-limited
/// candidate's blob is never read; every other candidate must be in
/// `blobs`.
///
/// # Errors
///
/// Returns the candidate blob and span errors documented on [`pack`].
fn pack_candidates_into(
    context: &mut Context,
    files: &[RankedFile],
    blobs: &BTreeMap<RelativePath, &[u8]>,
    limits: ContextLimits,
    count_tokens: &impl Fn(&str) -> usize,
) -> Result<(), SelectError> {
    for file in files {
        let spans = candidate_spans(file);
        if context.files >= limits.max_files {
            for (lines, names) in spans {
                context.omissions.push(Omission {
                    path: file.path.clone(),
                    lines,
                    names,
                    reason: OmissionReason::FileLimit,
                });
            }
            continue;
        }
        let source = blob_text(blobs, &file.path)?;
        let mut selected = false;
        for (lines, names) in spans {
            let text = slice_lines(source, lines).ok_or_else(|| SelectError::SpanBeyondSource {
                path: file.path.clone(),
                lines,
                source_lines: line_count(source),
            })?;
            let tokens = count_tokens(text);
            if tokens > limits.max_tokens - context.tokens {
                context.omissions.push(Omission {
                    path: file.path.clone(),
                    lines,
                    names,
                    reason: OmissionReason::TokenLimit { tokens },
                });
                continue;
            }
            selected = true;
            context.tokens += tokens;
            context.spans.push(ContextSpan {
                path: file.path.clone(),
                lines,
                kind: SpanKind::Candidate {
                    score: file.score,
                    names,
                },
                text: text.to_owned(),
                tokens,
            });
        }
        context.files += usize::from(selected);
    }
    Ok(())
}

/// One span per matched declaration in location order, with every query
/// name that matched it.
// ponytail: nested matches (a method inside a matched class) pack as
// separate overlapping spans; merge containment if duplicate text matters.
fn candidate_spans(file: &RankedFile) -> Vec<(LineRange, Vec<String>)> {
    let mut spans: BTreeMap<(LineRange, &str), Vec<String>> = BTreeMap::new();
    for reason in &file.reasons {
        spans
            .entry((reason.lines, reason.node_id.as_str()))
            .or_default()
            .push(reason.name.clone());
    }
    spans
        .into_iter()
        .map(|((lines, _), names)| (lines, names))
        .collect()
}

/// The UTF-8 text of `path`'s caller-supplied blob.
///
/// # Errors
///
/// Returns [`SelectError::MissingBlob`] when `blobs` lacks `path` and
/// [`SelectError::BlobNotUtf8`] when its bytes are not UTF-8.
fn blob_text<'b>(
    blobs: &BTreeMap<RelativePath, &'b [u8]>,
    path: &RelativePath,
) -> Result<&'b str, SelectError> {
    let bytes = blobs
        .get(path)
        .ok_or_else(|| SelectError::MissingBlob { path: path.clone() })?;
    std::str::from_utf8(bytes).map_err(|_| SelectError::BlobNotUtf8 { path: path.clone() })
}

/// The exact text of `lines` in `source`, or `None` past its last line.
fn slice_lines(source: &str, lines: LineRange) -> Option<&str> {
    let mut start = None;
    let mut offset = 0;
    for (number, line) in (1u32..).zip(source.split_inclusive('\n')) {
        if number == lines.start {
            start = Some(offset);
        }
        offset += line.len();
        if number == lines.end {
            return start.map(|start| &source[start..offset]);
        }
    }
    None
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
    /// `max_files` is zero, so not even the changed file fits.
    #[error("max_files is 0: the changed file cannot be selected")]
    NoFileBudget,
    /// A changed hunk does not fit the remaining token budget.
    #[error("changed hunk {lines:?} needs {tokens} tokens but only {remaining_tokens} remain")]
    ChangedHunkOverBudget {
        /// The unfittable hunk.
        lines: LineRange,
        /// Caller-counted tokens of the hunk.
        tokens: usize,
        /// Tokens left when the hunk was reached.
        remaining_tokens: usize,
    },
    /// A candidate that must be read has no caller-supplied blob.
    #[error("no candidate blob supplied for `{path}`")]
    MissingBlob {
        /// The candidate's path.
        path: RelativePath,
    },
    /// A candidate blob is not UTF-8.
    #[error("candidate blob for `{path}` is not valid UTF-8")]
    BlobNotUtf8 {
        /// The candidate's path.
        path: RelativePath,
    },
    /// A candidate blob ends before an indexed declaration does, so it does
    /// not match the indexed facts.
    #[error("candidate `{path}` span {lines:?} exceeds the blob's {source_lines} lines")]
    SpanBeyondSource {
        /// The candidate's path.
        path: RelativePath,
        /// The declaration's lines.
        lines: LineRange,
        /// Line count of the supplied blob.
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
    /// Two changed files union their names, a name changed in both scores
    /// once per candidate, every changed path is excluded, and the single
    /// file ranking equals the one-element multi-file ranking.
    fn multi_file_ranking_unions_names_and_excludes_every_changed_path() {
        let index = fixture_index();
        let head = source(12);
        let handler_nodes = vec![function("src/changed.ts", "Changed", 4, 9)];
        let handler_references = vec![call("load", 5), call("save", 7)];
        let handler_lines = changed(&[(5, 5), (7, 8)], &head).expect("valid ranges");
        let handler = ChangedFile {
            path: &path("src/changed.ts"),
            nodes: &handler_nodes,
            references: &handler_references,
            changed_lines: &handler_lines,
        };
        assert_eq!(
            rank_files(&index, &[handler]),
            rank(
                &index,
                &path("src/changed.ts"),
                &handler_nodes,
                &handler_references,
                &handler_lines
            )
            .files
        );

        let other_references = vec![call("load", 1), call("offRange", 1)];
        let other_lines = changed(&[(1, 1)], &head).expect("valid ranges");
        let other = ChangedFile {
            path: &path("src/d.ts"),
            nodes: &[],
            references: &other_references,
            changed_lines: &other_lines,
        };
        let ranked = rank_files(&index, &[handler, other]);
        let scores: Vec<(&str, usize)> = ranked
            .iter()
            .map(|file| (file.path.as_str(), file.score))
            .collect();
        assert_eq!(scores, [("src/a.ts", 2), ("src/b.ts", 2), ("src/c.ts", 1)]);
        assert_eq!(rank_files(&index, &[other, handler]), ranked);
        assert!(rank_files(&index, &[]).is_empty());
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

    /// Counts whitespace-separated words, standing in for a model tokenizer.
    fn words(text: &str) -> usize {
        text.split_whitespace().count()
    }

    /// Ranks the fixture with changed lines 5 and 7..=8 of a 12-line head.
    fn fixture_ranking(head: &str) -> Ranking {
        let references = vec![call("load", 5), call("save", 7)];
        let nodes = vec![function("src/changed.ts", "Changed", 4, 9)];
        let changed = changed(&[(5, 5), (7, 8)], head).expect("valid ranges");
        rank(
            &fixture_index(),
            &path("src/changed.ts"),
            &nodes,
            &references,
            &changed,
        )
    }

    /// Packs the fixture ranking with `blobs` under `max_files`/`max_tokens`.
    ///
    /// # Errors
    ///
    /// Returns the selector's packing error.
    fn pack_fixture(
        blobs: &[(&str, &[u8])],
        max_files: usize,
        max_tokens: usize,
    ) -> Result<Context, SelectError> {
        let head = source(12);
        let blobs = blobs
            .iter()
            .map(|&(file, bytes)| (path(file), bytes))
            .collect();
        let limits = ContextLimits {
            max_files,
            max_tokens,
        };
        pack(&fixture_ranking(&head), &head, &blobs, limits, words)
    }

    #[test]
    /// Changed hunks take budget first; one secondary span fits, the rest are
    /// reported as token or file omissions, and file-limited blobs are never
    /// required.
    fn packing_selects_changed_hunks_then_ranked_spans_within_limits() {
        let a = source(6);
        let context = pack_fixture(&[("src/a.ts", a.as_bytes())], 2, 6).expect("fits");

        let spans: Vec<(&str, u32, u32, &str, usize, &SpanKind)> = context
            .spans
            .iter()
            .map(|span| {
                (
                    span.path.as_str(),
                    span.lines.start,
                    span.lines.end,
                    span.text.as_str(),
                    span.tokens,
                    &span.kind,
                )
            })
            .collect();
        let load = SpanKind::Candidate {
            score: 2,
            names: vec!["load".to_owned()],
        };
        assert_eq!(
            spans,
            [
                ("src/changed.ts", 5, 5, "line5\n", 1, &SpanKind::ChangedHunk),
                (
                    "src/changed.ts",
                    7,
                    8,
                    "line7\nline8\n",
                    2,
                    &SpanKind::ChangedHunk
                ),
                ("src/a.ts", 2, 4, "line2\nline3\nline4\n", 3, &load),
            ]
        );
        assert_eq!((context.files, context.tokens), (2, 6));

        let omissions: Vec<(&str, u32, &str, OmissionReason)> = context
            .omissions
            .iter()
            .map(|omission| {
                (
                    omission.path.as_str(),
                    omission.lines.start,
                    omission.names[0].as_str(),
                    omission.reason,
                )
            })
            .collect();
        assert_eq!(
            omissions,
            [
                (
                    "src/a.ts",
                    6,
                    "Changed",
                    OmissionReason::TokenLimit { tokens: 1 }
                ),
                ("src/b.ts", 1, "load", OmissionReason::FileLimit),
                ("src/b.ts", 5, "save", OmissionReason::FileLimit),
                ("src/c.ts", 1, "save", OmissionReason::FileLimit),
            ]
        );
        assert_eq!(
            context,
            pack_fixture(&[("src/a.ts", a.as_bytes())], 2, 6).expect("fits")
        );
    }

    #[test]
    /// Candidate-only packing emits no changed hunk, counts only selected
    /// candidate files against `max_files`, reports omissions, and treats a
    /// zero file limit as all-omitted rather than an error.
    fn candidate_packing_counts_only_selected_candidates() {
        let head = source(12);
        let ranking = fixture_ranking(&head);
        let a = source(6);
        let b = source(7);
        let blobs = BTreeMap::from([
            (path("src/a.ts"), a.as_bytes()),
            (path("src/b.ts"), b.as_bytes()),
        ]);
        let limits = ContextLimits {
            max_files: 2,
            max_tokens: 7,
        };
        let context = pack_candidates(&ranking.files, &blobs, limits, words).expect("packs");
        let spans: Vec<(&str, u32, u32, usize)> = context
            .spans
            .iter()
            .map(|span| {
                (
                    span.path.as_str(),
                    span.lines.start,
                    span.lines.end,
                    span.tokens,
                )
            })
            .collect();
        assert_eq!(
            spans,
            [
                ("src/a.ts", 2, 4, 3),
                ("src/a.ts", 6, 6, 1),
                ("src/b.ts", 1, 3, 3)
            ]
        );
        assert!(
            context
                .spans
                .iter()
                .all(|span| matches!(span.kind, SpanKind::Candidate { .. }))
        );
        assert_eq!((context.files, context.tokens), (2, 7));
        let omissions: Vec<(&str, u32, OmissionReason)> = context
            .omissions
            .iter()
            .map(|omission| {
                (
                    omission.path.as_str(),
                    omission.lines.start,
                    omission.reason,
                )
            })
            .collect();
        assert_eq!(
            omissions,
            [
                ("src/b.ts", 5, OmissionReason::TokenLimit { tokens: 3 }),
                ("src/c.ts", 1, OmissionReason::FileLimit),
            ]
        );

        let none = ContextLimits {
            max_files: 0,
            max_tokens: 100,
        };
        let starved = pack_candidates(&ranking.files, &BTreeMap::new(), none, words)
            .expect("nothing is read");
        assert!(starved.spans.is_empty());
        assert_eq!((starved.files, starved.tokens), (0, 0));
        assert_eq!(starved.omissions.len(), 5);
        assert_eq!(
            pack_candidates(&ranking.files, &BTreeMap::new(), limits, words),
            Err(SelectError::MissingBlob {
                path: path("src/a.ts")
            })
        );
    }

    #[test]
    /// Unfittable changed hunks, missing or undecodable selected blobs, and
    /// spans past a blob's end fail explicitly instead of shrinking context.
    fn packing_rejects_unfittable_hunks_and_bad_candidate_blobs() {
        let a = source(6);
        assert_eq!(
            pack_fixture(&[], 2, 2),
            Err(SelectError::ChangedHunkOverBudget {
                lines: LineRange { start: 7, end: 8 },
                tokens: 2,
                remaining_tokens: 1,
            })
        );
        assert_eq!(pack_fixture(&[], 0, 100), Err(SelectError::NoFileBudget));
        assert_eq!(
            pack_fixture(&[("src/a.ts", a.as_bytes())], 3, 100),
            Err(SelectError::MissingBlob {
                path: path("src/b.ts")
            })
        );
        assert_eq!(
            pack_fixture(&[("src/a.ts", b"\xff")], 2, 100),
            Err(SelectError::BlobNotUtf8 {
                path: path("src/a.ts")
            })
        );
        assert_eq!(
            pack_fixture(&[("src/a.ts", source(3).as_bytes())], 2, 100),
            Err(SelectError::SpanBeyondSource {
                path: path("src/a.ts"),
                lines: LineRange { start: 2, end: 4 },
                source_lines: 3,
            })
        );
    }
}
