//! Leave-one-out relevance trials for the context-relevance evaluation.
//!
//! For one sampled PR, [`evaluate`] builds the base corpus from the base
//! snapshot's index and the hash-verified base blob of every indexed file,
//! prepares each changed Python or Rust file from its patch and head blob,
//! and runs one trial per withholdable file: the withheld file leaves the
//! change set and stays a candidate at its base version, and every other
//! changed path is excluded from the candidates. Variant A is the public
//! facade's production ranking; B–D rank the same candidate universe by
//! directional references, rarity weighting, and explicit-import scoping.
//! All four are packed under one common experimental policy, and A is also
//! packed by the unmodified production packer. Source is read only while a
//! PR is evaluated; trials carry paths, names, ranks, scores, and counts.

use bohmian_code_review::{
    ChangedBlob, ContextLimits, DeclarationFact, Error as SymbolicError, LineRange, Node,
    RankedFile, RawReference, RelativePath, SymbolicContext, build_candidate_context, parse_hunks,
};
use bohmian_code_review_pipeline::{FileStatus, PrContextLimits, PullRequestFile};
use serde::{Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Candidate-context tokens every variant is packed into.
pub const CANDIDATE_ALLOWANCE: usize = 100_000;
/// Tokens reserved for the downstream prompt, reported beside the diff.
pub const PROMPT_RESERVE: usize = 4_096;
/// Names referenced or declared by more base files than this are ignored
/// by variants C and D.
pub const DF_CEILING: usize = 100;
/// Variant C's rarity rule as recorded in the report.
pub const RARITY_RULE: &str = "ignore a name when df(name) > 100; a downstream file referencing it earns 1/df(name); an upstream file declaring it earns 1/(df(name) * declaring_file_count(name)); a file matching both directions keeps the larger credit; score sums credits over distinct names";
/// The pipeline's token estimate as recorded in the report.
pub const TOKEN_ESTIMATE: &str = "ceil(utf8_bytes / 4)";

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

/// Which lookup found a ranked file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// It declares a name the remaining changes use or change; variant A's
    /// only lookup.
    Upstream,
    /// It references a name the remaining changes add, modify, or remove.
    Downstream,
    /// Both lookups.
    Both,
}

/// What the common experimental packing did with the withheld file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Sent {
    /// It shares no name with the remaining changes.
    Unranked,
    /// Sent as the whole file.
    Whole,
    /// Too large whole; sent as its matched spans.
    SpansOnly,
    /// Neither the whole file nor its spans fit the remaining allowance.
    Omitted,
}

/// One variant's result for the withheld file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Outcome {
    /// One-based rank, `None` (serialized `"unranked"`) when unranked.
    #[serde(serialize_with = "rank_or_unranked")]
    pub rank: Option<usize>,
    /// Its score; zero when unranked.
    pub score: f64,
    /// Distinct names that matched it.
    pub names: Vec<String>,
    /// Which lookup found it.
    pub direction: Option<Direction>,
    /// Estimated tokens of the whole base file.
    pub whole_tokens: usize,
    /// Estimated tokens of its merged matched spans; zero when unranked.
    pub snippet_tokens: usize,
    /// Its common experimental packing result.
    pub sent: Sent,
}

/// A changed file that supplies no changed lines, recorded not dropped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileOmission {
    /// Head-side path.
    pub path: RelativePath,
    /// Missing patch, malformed patch, or analysis failure.
    pub reason: String,
}

/// One withheld file's results under every variant.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Trial {
    /// PR number.
    pub pr: u64,
    /// The withheld file, ranked at its base version.
    pub withheld: RelativePath,
    /// Whether it sits on a conventional test path.
    pub test_file: bool,
    /// Estimated tokens of every listed patch except the withheld file's.
    pub diff_tokens: usize,
    /// Remaining changed files that supplied no changed lines.
    pub omitted: Vec<FileOmission>,
    /// Remaining changed head and previous paths, never candidates.
    pub excluded: Vec<RelativePath>,
    /// Production ranking.
    #[serde(rename = "A")]
    pub a: Outcome,
    /// Directional references.
    #[serde(rename = "B")]
    pub b: Outcome,
    /// B with rarity weighting.
    #[serde(rename = "C")]
    pub c: Outcome,
    /// C with explicit-import scoping.
    #[serde(rename = "D")]
    pub d: Outcome,
    /// Whether the unmodified production packer selects any span of the
    /// withheld file from A's ranking under [`CANDIDATE_ALLOWANCE`].
    pub production_selected: bool,
    /// Variant D lookups, one per name and referencing file, scoped by an
    /// explicit import.
    pub names_scoped: usize,
    /// Variant D lookups that kept C's unscoped lookup.
    pub names_unscoped: usize,
}

impl Trial {
    /// Each variant's id and outcome, A to D.
    pub fn variants(&self) -> [(&'static str, &Outcome); 4] {
        [
            ("A", &self.a),
            ("B", &self.b),
            ("C", &self.c),
            ("D", &self.d),
        ]
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

/// The pipeline's token estimate: one token per four UTF-8 bytes, rounded
/// up.
pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Whether `path` is a conventional Python or Rust test path: under a
/// `tests` or `test` directory, or a `test_*.py`, `*_test.py`,
/// `conftest.py`, `tests.rs`, or `*_test(s).rs` file.
pub fn is_test_path(path: &RelativePath) -> bool {
    let mut parts = path.as_str().rsplit('/');
    let file = parts.next().unwrap_or_default();
    parts.any(|dir| matches!(dir, "tests" | "test"))
        || file == "conftest.py"
        || file == "tests.rs"
        || (file.starts_with("test_") && file.ends_with(".py"))
        || [".py", ".rs"].iter().any(|ext| {
            file.strip_suffix(ext).is_some_and(|stem| {
                stem.ends_with("_test") || (*ext == ".rs" && stem.ends_with("_tests"))
            })
        })
}

/// Bound import name → the one indexed base module it names, or `None`
/// when that module is relative, external, ambiguous, or bound twice.
type Bindings = BTreeMap<String, Option<RelativePath>>;

/// One indexed base file's facts beyond the index.
struct BaseFile {
    /// Persisted declarations, for enclosing-declaration snippets.
    declarations: Vec<DeclarationFact>,
    /// Explicit import bindings of its base version.
    bindings: Bindings,
}

/// The complete base candidate universe of one PR.
///
/// Invariant: `blobs` and `files` hold exactly the indexed paths of
/// `index`, and `referenced` covers every reference in those blobs.
struct Corpus<'a> {
    /// The base snapshot's index: A's ranking and every declaration lookup.
    index: SymbolicContext,
    /// Verified base source of every indexed file.
    blobs: BTreeMap<RelativePath, &'a [u8]>,
    /// Declarations and import bindings per indexed file.
    files: BTreeMap<RelativePath, BaseFile>,
    /// Name → referencing file → reference lines.
    referenced: BTreeMap<String, BTreeMap<RelativePath, Vec<u32>>>,
}

impl<'a> Corpus<'a> {
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
        sources: &BTreeMap<RelativePath, &'a [u8]>,
        scratch: &mut SymbolicContext,
    ) -> Result<Corpus<'a>, TrialError> {
        let facts = index.export();
        let paths: BTreeSet<RelativePath> = facts.iter().map(|fact| fact.path.clone()).collect();
        let mut corpus = Corpus {
            index,
            blobs: BTreeMap::new(),
            files: BTreeMap::new(),
            referenced: BTreeMap::new(),
        };
        for fact in facts {
            let source = *sources
                .get(&fact.path)
                .ok_or_else(|| TrialError::MissingSource {
                    pr,
                    path: fact.path.clone(),
                })?;
            let (nodes, references) = extract(scratch, &fact.path, source)
                .map_err(|error| TrialError::Symbolic { pr, error })?;
            for reference in &references {
                for name in reference_names(reference) {
                    corpus
                        .referenced
                        .entry(name.to_owned())
                        .or_default()
                        .entry(fact.path.clone())
                        .or_default()
                        .push(reference.line);
                }
            }
            let bindings = bindings(&paths, &fact.path, &nodes, &references);
            corpus.blobs.insert(fact.path.clone(), source);
            corpus.files.insert(
                fact.path,
                BaseFile {
                    declarations: fact.declarations,
                    bindings,
                },
            );
        }
        Ok(corpus)
    }

    /// Files declaring exactly `name`, with each matching declaration's
    /// lines.
    fn declaring(&self, name: &str) -> BTreeMap<RelativePath, Vec<LineRange>> {
        let mut declaring: BTreeMap<RelativePath, Vec<LineRange>> = BTreeMap::new();
        for hit in self.index.lookup(name) {
            declaring
                .entry(hit.path.clone())
                .or_default()
                .push(LineRange {
                    start: hit.node.start_line,
                    end: hit.node.end_line,
                });
        }
        declaring
    }

    /// Distinct base files declaring or referencing `name`.
    fn document_frequency(
        &self,
        name: &str,
        declaring: &BTreeMap<RelativePath, Vec<LineRange>>,
    ) -> usize {
        let referencing = self
            .referenced
            .get(name)
            .into_iter()
            .flat_map(BTreeMap::keys);
        declaring
            .keys()
            .chain(referencing)
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// The innermost declaration of `path` enclosing `line`, or the line
    /// alone at module level.
    fn enclosing(&self, path: &RelativePath, line: u32) -> LineRange {
        self.files[path]
            .declarations
            .iter()
            .filter(|fact| fact.start_line <= line && line <= fact.end_line)
            .min_by_key(|fact| fact.end_line - fact.start_line)
            .map_or(
                LineRange {
                    start: line,
                    end: line,
                },
                |fact| LineRange {
                    start: fact.start_line,
                    end: fact.end_line,
                },
            )
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

/// The explicit import bindings of `path` from its extracted `nodes` and
/// import `references`, each resolved against the indexed `paths`.
///
/// Python binds each name imported by `from <module> import` to the module
/// file `<module>.py` or `<module>/__init__.py` matched by path suffix. The
/// import statement is the one import node whose line and column span
/// contains the reference, so several statements on one line bind
/// independently; a reference inside several import nodes (`import a, b`)
/// or naming its own module never binds.
/// Rust binds the last segment of a `crate::`, `self::`, or `super::` use
/// path to the module file below the nearest `src/` directory. Relative
/// Python, external Rust, wildcard, and aliased-away imports never scope.
fn bindings(
    paths: &BTreeSet<RelativePath>,
    path: &RelativePath,
    nodes: &[Node],
    references: &[RawReference],
) -> Bindings {
    let python = path.as_str().ends_with(".py");
    let mut bound = Bindings::new();
    for reference in references
        .iter()
        .filter(|reference| reference.reference_kind.wire_name() == "imports")
    {
        let binding = if python {
            let position = (reference.line, reference.column);
            let statements: Vec<&Node> = nodes
                .iter()
                .filter(|node| {
                    node.kind.wire_name() == "import"
                        && (node.start_line, node.start_column) <= position
                        && position <= (node.end_line, node.end_column)
                })
                .collect();
            match statements.as_slice() {
                [node] if node.name != reference.reference_name => Some((
                    reference.reference_name.as_str(),
                    python_module(paths, &node.name),
                )),
                _ => None,
            }
        } else {
            reference
                .reference_name
                .rsplit_once("::")
                .map(|(module, name)| (name, rust_module(paths, path, module)))
        };
        if let Some((name, module)) = binding {
            let entry = bound
                .entry(name.to_owned())
                .or_insert_with(|| module.clone());
            if *entry != module {
                *entry = None;
            }
        }
    }
    bound
}

/// The one indexed path in `candidates`, or `None` for zero or several.
fn unique<'p>(mut candidates: impl Iterator<Item = &'p RelativePath>) -> Option<RelativePath> {
    let first = candidates.next()?;
    candidates.next().is_none().then(|| first.clone())
}

/// The one indexed file of absolute Python `module`.
fn python_module(paths: &BTreeSet<RelativePath>, module: &str) -> Option<RelativePath> {
    if module.starts_with('.') {
        return None;
    }
    let stem = module.replace('.', "/");
    let wanted = [format!("{stem}.py"), format!("{stem}/__init__.py")];
    unique(paths.iter().filter(|path| {
        wanted
            .iter()
            .any(|want| path.as_str() == want || path.as_str().ends_with(&format!("/{want}")))
    }))
}

/// The one indexed file of Rust `module` as used from `file`.
fn rust_module(
    paths: &BTreeSet<RelativePath>,
    file: &RelativePath,
    module: &str,
) -> Option<RelativePath> {
    let parts: Vec<&str> = file.as_str().split('/').collect();
    let src = parts.iter().rposition(|part| *part == "src")?;
    let root = parts[..=src].join("/");
    let mut here = parts[src + 1..].to_vec();
    let last = here.pop()?.strip_suffix(".rs")?;
    if last != "mod" && !(here.is_empty() && matches!(last, "lib" | "main")) {
        here.push(last);
    }
    let mut segments = module.split("::");
    let mut target = match segments.next()? {
        "crate" => Vec::new(),
        "self" => here,
        "super" => {
            here.pop()?;
            here
        }
        _ => return None,
    };
    for segment in segments {
        if segment == "super" {
            target.pop()?;
        } else {
            target.push(segment);
        }
    }
    let wanted = if target.is_empty() {
        [format!("{root}/lib.rs"), format!("{root}/main.rs")]
    } else {
        let stem = target.join("/");
        [format!("{root}/{stem}.rs"), format!("{root}/{stem}/mod.rs")]
    };
    unique(
        paths
            .iter()
            .filter(|path| wanted.iter().any(|want| path.as_str() == want)),
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

/// One changed Python or Rust file prepared for every trial.
struct Change<'a> {
    /// Its listing entry.
    file: &'a PullRequestFile,
    /// Its head blob when it has analyzable head-side changed lines.
    head: Option<&'a [u8]>,
    /// Head-side changed lines.
    ranges: Vec<LineRange>,
    /// Why it supplies no changed lines, when it does not.
    omission: Option<String>,
    /// Names referenced on its head changed lines.
    used: BTreeSet<String>,
    /// Names of head and base declarations its changes add, modify, or
    /// remove.
    changed: BTreeSet<String>,
    /// Explicit import bindings of its head version.
    bindings: Bindings,
    /// Estimated tokens of its patch.
    patch_tokens: usize,
}

impl<'a> Change<'a> {
    /// Prepare `file` of PR `pr` from its patch, its base file in
    /// `corpus`, and its head blob in `heads`.
    ///
    /// A missing or malformed patch, or a head the facade cannot rank, is
    /// an omission that supplies no names.
    ///
    /// # Errors
    ///
    /// [`TrialError::MissingSource`] when a head blob with changed lines
    /// was not supplied.
    fn new(
        pr: u64,
        corpus: &Corpus<'_>,
        scratch: &mut SymbolicContext,
        file: &'a PullRequestFile,
        heads: &BTreeMap<RelativePath, &'a [u8]>,
    ) -> Result<Change<'a>, TrialError> {
        let mut change = Change {
            file,
            head: None,
            ranges: Vec::new(),
            omission: None,
            used: BTreeSet::new(),
            changed: BTreeSet::new(),
            bindings: Bindings::new(),
            patch_tokens: file.patch.as_deref().map_or(0, estimate_tokens),
        };
        let (patch, ranges) = match file
            .patch
            .as_deref()
            .map(|patch| (patch, parse_hunks(patch)))
        {
            Some((patch, Ok(ranges))) => (patch, ranges),
            Some((_, Err(error))) => return Ok(change.omit(format!("malformed patch: {error}"))),
            None => return Ok(change.omit("missing patch".to_owned())),
        };
        let base_path = file.previous_path.as_ref().unwrap_or(&file.path);
        if file.status != FileStatus::Added
            && let Some(base) = corpus.files.get(base_path)
        {
            let removed = removed_lines(patch);
            for fact in &base.declarations {
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
        let analyzed = corpus
            .index
            .rank(file.path.as_str(), head, ranges.clone())
            .and_then(|_| extract(scratch, &file.path, head));
        let (nodes, references) = match analyzed {
            Ok(extracted) => extracted,
            Err(error) => return Ok(change.omit(format!("analysis failed: {error}"))),
        };
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
        change.bindings = bindings(
            &corpus.files.keys().cloned().collect(),
            &file.path,
            &nodes,
            &references,
        );
        change.head = Some(head);
        change.ranges = ranges;
        Ok(change)
    }

    /// This change with `reason` recorded and no names.
    fn omit(mut self, reason: String) -> Change<'a> {
        self.omission = Some(reason);
        self.changed.clear();
        self
    }
}

/// A ranked candidate of any variant.
struct Candidate {
    /// Candidate path.
    path: RelativePath,
    /// Variant score.
    score: f64,
    /// Distinct matched names.
    names: BTreeSet<String>,
    /// Found by an upstream lookup.
    upstream: bool,
    /// Found by a downstream lookup.
    downstream: bool,
    /// Matched declarations (upstream) and enclosing declarations
    /// (downstream), possibly overlapping.
    spans: Vec<LineRange>,
}

/// One candidate's credit for one name.
#[derive(Default)]
struct Hit {
    /// The larger direction's credit.
    credit: f64,
    /// Found upstream.
    upstream: bool,
    /// Found downstream.
    downstream: bool,
    /// Its spans for this name.
    spans: Vec<LineRange>,
}

/// How B–D look names up and credit them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rule {
    /// B: one point per distinct name per file.
    Directional,
    /// C: rarity weights, ambiguity split, and the df ceiling.
    Rare,
    /// D: C scoped by explicit imports.
    Scoped,
}

/// The remaining changes' names, with who used or changed each.
struct Query<'c> {
    /// Used name → head bindings of each change using it.
    used: BTreeMap<&'c str, Vec<&'c Bindings>>,
    /// Changed name → head and base paths of each change changing it.
    changed: BTreeMap<&'c str, BTreeSet<&'c RelativePath>>,
}

impl<'c> Query<'c> {
    /// Collect the names of `remaining`.
    fn new(remaining: &[&'c Change<'_>]) -> Query<'c> {
        let mut query = Query {
            used: BTreeMap::new(),
            changed: BTreeMap::new(),
        };
        for change in remaining {
            for name in &change.used {
                query.used.entry(name).or_default().push(&change.bindings);
            }
            for name in &change.changed {
                let sources = query.changed.entry(name).or_default();
                sources.insert(&change.file.path);
                sources.extend(&change.file.previous_path);
            }
        }
        query
    }
}

/// The bound module of `name`'s first path segment in `bindings`.
fn bound<'b>(bindings: &'b Bindings, name: &str) -> Option<&'b RelativePath> {
    let key = name.split(['.', ':']).next().unwrap_or(name);
    bindings.get(key).and_then(Option::as_ref)
}

/// Rank the corpus for `query` under `rule`, skipping `excluded`; returns
/// the candidates by descending score then path, and D's scoped and
/// unscoped lookup counts.
fn score(
    corpus: &Corpus<'_>,
    query: &Query<'_>,
    excluded: &BTreeSet<RelativePath>,
    rule: Rule,
) -> (Vec<Candidate>, usize, usize) {
    let mut hits: BTreeMap<RelativePath, BTreeMap<&str, Hit>> = BTreeMap::new();
    let (mut scoped, mut unscoped) = (0, 0);
    let mut credit =
        |path: &RelativePath, name, credit: f64, upstream: bool, spans: Vec<LineRange>| {
            let hit: &mut Hit = hits
                .entry(path.clone())
                .or_default()
                .entry(name)
                .or_default();
            hit.credit = hit.credit.max(credit);
            hit.upstream |= upstream;
            hit.downstream |= !upstream;
            hit.spans.extend(spans);
        };
    for (&name, users) in &query.used {
        let declaring = corpus.declaring(name);
        let df = corpus.document_frequency(name, &declaring);
        if rule != Rule::Directional && df > DF_CEILING {
            continue;
        }
        for bindings in users {
            let module = (rule == Rule::Scoped)
                .then(|| bound(bindings, name).filter(|module| declaring.contains_key(*module)))
                .flatten();
            if rule == Rule::Scoped {
                *if module.is_some() {
                    &mut scoped
                } else {
                    &mut unscoped
                } += 1;
            }
            for (path, spans) in &declaring {
                if excluded.contains(path) || module.is_some_and(|module| module != path) {
                    continue;
                }
                let weight = match rule {
                    Rule::Directional => 1.0,
                    _ => 1.0 / (df * declaring.len()) as f64,
                };
                credit(path, name, weight, true, spans.clone());
            }
        }
    }
    for (&name, sources) in &query.changed {
        let Some(referencing) = corpus.referenced.get(name) else {
            continue;
        };
        let df = corpus.document_frequency(name, &corpus.declaring(name));
        if rule != Rule::Directional && df > DF_CEILING {
            continue;
        }
        for (path, lines) in referencing {
            if excluded.contains(path) {
                continue;
            }
            if rule == Rule::Scoped {
                match bound(&corpus.files[path].bindings, name) {
                    Some(module) => {
                        scoped += 1;
                        if !sources.contains(module) {
                            continue;
                        }
                    }
                    None => unscoped += 1,
                }
            }
            let weight = match rule {
                Rule::Directional => 1.0,
                _ => 1.0 / df as f64,
            };
            let spans = lines
                .iter()
                .map(|&line| corpus.enclosing(path, line))
                .collect();
            credit(path, name, weight, false, spans);
        }
    }
    let mut candidates: Vec<Candidate> = hits
        .into_iter()
        .map(|(path, names)| Candidate {
            path,
            score: names.values().map(|hit| hit.credit).sum(),
            upstream: names.values().any(|hit| hit.upstream),
            downstream: names.values().any(|hit| hit.downstream),
            spans: names
                .values()
                .flat_map(|hit| hit.spans.iter().copied())
                .collect(),
            names: names.into_keys().map(str::to_owned).collect(),
        })
        .collect();
    candidates.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.path.cmp(&right.path))
    });
    (candidates, scoped, unscoped)
}

/// Variant A: the facade's production ranking of the remaining analyzed
/// heads, without any remaining changed path.
///
/// # Errors
///
/// Returns the facade's ranking error.
fn rank_a(
    corpus: &Corpus<'_>,
    remaining: &[&Change<'_>],
    excluded: &BTreeSet<RelativePath>,
) -> Result<Vec<RankedFile>, SymbolicError> {
    let heads: Vec<ChangedBlob<'_>> = remaining
        .iter()
        .filter_map(|change| {
            change.head.map(|head| ChangedBlob {
                path: change.file.path.as_str(),
                head,
                changed: change.ranges.clone(),
            })
        })
        .collect();
    let mut ranked = corpus.index.rank_files(&heads)?;
    ranked.retain(|file| !excluded.contains(&file.path));
    Ok(ranked)
}

/// Variant A's ranking as common candidates.
fn a_candidates(ranked: &[RankedFile]) -> Vec<Candidate> {
    ranked
        .iter()
        .map(|file| Candidate {
            path: file.path.clone(),
            score: file.score as f64,
            names: file
                .reasons
                .iter()
                .map(|reason| reason.name.clone())
                .collect(),
            upstream: true,
            downstream: false,
            spans: file.reasons.iter().map(|reason| reason.lines).collect(),
        })
        .collect()
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

/// Estimated tokens of `spans` of `source` after merging overlapping and
/// adjacent spans.
fn snippet_tokens(source: &[u8], spans: &[LineRange]) -> usize {
    let mut spans = spans.to_vec();
    spans.sort_by_key(|span| (span.start, span.end));
    let mut merged: Vec<LineRange> = Vec::new();
    for span in spans {
        match merged.last_mut() {
            Some(last) if span.start <= last.end + 1 => last.end = last.end.max(span.end),
            _ => merged.push(span),
        }
    }
    let lines: Vec<&[u8]> = source.split_inclusive(|&byte| byte == b'\n').collect();
    merged
        .iter()
        .map(|span| {
            let (start, end) = (
                span.start as usize - 1,
                (span.end as usize).min(lines.len()),
            );
            lines.get(start..end).map_or(0, |lines| {
                lines
                    .iter()
                    .map(|line| line.len())
                    .sum::<usize>()
                    .div_ceil(4)
            })
        })
        .sum()
}

/// Pack `candidates` under the common experimental policy and report the
/// withheld `target`: whole files in rank order within
/// [`CANDIDATE_ALLOWANCE`], no file cap, matched spans when the whole file
/// does not fit.
fn outcome(corpus: &Corpus<'_>, candidates: &[Candidate], target: &RelativePath) -> Outcome {
    let whole = |path: &RelativePath| corpus.blobs[path].len().div_ceil(4);
    let mut remaining = CANDIDATE_ALLOWANCE;
    for (index, candidate) in candidates.iter().enumerate() {
        let whole_tokens = whole(&candidate.path);
        let is_target = candidate.path == *target;
        let snippet =
            |candidate: &Candidate| snippet_tokens(corpus.blobs[&candidate.path], &candidate.spans);
        let (sent, snippet_tokens) = if whole_tokens <= remaining {
            remaining -= whole_tokens;
            (Sent::Whole, if is_target { snippet(candidate) } else { 0 })
        } else {
            let tokens = snippet(candidate);
            if tokens <= remaining {
                remaining -= tokens;
                (Sent::SpansOnly, tokens)
            } else {
                (Sent::Omitted, tokens)
            }
        };
        if is_target {
            return Outcome {
                rank: Some(index + 1),
                score: candidate.score,
                names: candidate.names.iter().cloned().collect(),
                direction: Some(match (candidate.upstream, candidate.downstream) {
                    (true, true) => Direction::Both,
                    (false, true) => Direction::Downstream,
                    _ => Direction::Upstream,
                }),
                whole_tokens,
                snippet_tokens,
                sent,
            };
        }
    }
    Outcome {
        rank: None,
        score: 0.0,
        names: Vec::new(),
        direction: None,
        whole_tokens: whole(target),
        snippet_tokens: 0,
        sent: Sent::Unranked,
    }
}

/// Run the trial withholding `changes[withheld]`.
///
/// # Errors
///
/// Returns the facade's ranking or production packing error.
fn trial(
    pr: u64,
    corpus: &Corpus<'_>,
    changes: &[Change<'_>],
    withheld: usize,
    listing_patch_tokens: usize,
) -> Result<Trial, TrialError> {
    let symbolic = |error: SymbolicError| TrialError::Symbolic { pr, error };
    let target = &changes[withheld].file.path;
    let remaining: Vec<&Change<'_>> = changes
        .iter()
        .enumerate()
        .filter_map(|(index, change)| (index != withheld).then_some(change))
        .collect();
    let excluded = excluded(&remaining, target);
    let ranked = rank_a(corpus, &remaining, &excluded).map_err(symbolic)?;
    let limits = ContextLimits {
        max_files: PrContextLimits::default().max_candidate_files,
        max_tokens: CANDIDATE_ALLOWANCE,
    };
    let production = build_candidate_context(&ranked, &corpus.blobs, limits, estimate_tokens)
        .map_err(symbolic)?;
    let query = Query::new(&remaining);
    let (b, _, _) = score(corpus, &query, &excluded, Rule::Directional);
    let (c, _, _) = score(corpus, &query, &excluded, Rule::Rare);
    let (d, names_scoped, names_unscoped) = score(corpus, &query, &excluded, Rule::Scoped);
    Ok(Trial {
        pr,
        withheld: target.clone(),
        test_file: is_test_path(target),
        diff_tokens: listing_patch_tokens.saturating_sub(changes[withheld].patch_tokens),
        omitted: remaining
            .iter()
            .filter_map(|change| {
                change.omission.as_ref().map(|reason| FileOmission {
                    path: change.file.path.clone(),
                    reason: reason.clone(),
                })
            })
            .collect(),
        excluded: excluded.into_iter().collect(),
        a: outcome(corpus, &a_candidates(&ranked), target),
        b: outcome(corpus, &b, target),
        c: outcome(corpus, &c, target),
        d: outcome(corpus, &d, target),
        production_selected: production.spans.iter().any(|span| span.path == *target),
        names_scoped,
        names_unscoped,
    })
}

/// Run every trial of PR `pr`: one per changed file the listing marks
/// modified and the base `index` declares, in listing order.
///
/// `sources` must hold the verified base blob of every file `index`
/// declares; `heads` the head blob of every changed file with head-side
/// changed lines; `listing_patch_tokens` is the estimated token total of
/// every patch in the PR's listing.
///
/// # Errors
///
/// [`TrialError::MissingSource`] for a missing base or head blob, and
/// [`TrialError::Symbolic`] for base source the facade rejects.
pub fn evaluate(
    pr: u64,
    index: SymbolicContext,
    sources: &BTreeMap<RelativePath, &[u8]>,
    listed: &[PullRequestFile],
    heads: &BTreeMap<RelativePath, &[u8]>,
    listing_patch_tokens: usize,
) -> Result<Vec<Trial>, TrialError> {
    let mut scratch = SymbolicContext::new();
    let corpus = Corpus::new(pr, index, sources, &mut scratch)?;
    let changes = listed
        .iter()
        .map(|file| Change::new(pr, &corpus, &mut scratch, file, heads))
        .collect::<Result<Vec<_>, _>>()?;
    changes
        .iter()
        .enumerate()
        .filter(|(_, change)| {
            change.file.status == FileStatus::Modified
                && corpus.files.contains_key(&change.file.path)
        })
        .map(|(withheld, _)| trial(pr, &corpus, &changes, withheld, listing_patch_tokens))
        .collect()
}

/// Trial contract checks over the real symbolic facade on in-memory
/// source: withholding, variant scoring, packing, and omissions.
#[cfg(test)]
mod tests {
    use super::*;
    use bohmian_code_review_pipeline::ObjectSha;

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

    /// Scenario 1: a trial withholds exactly one modified, declared file,
    /// keeps it rankable at its base version, excludes every remaining
    /// changed path, and variant A equals the public facade's ranking.
    #[test]
    fn withholding_keeps_base_file_rankable_and_a_is_production() {
        let base = sources(&[
            (
                "pkg/models.py",
                "class Model:\n    def run(self):\n        return 1\n".to_owned(),
            ),
            ("pkg/api.py", "def serve():\n    pass\n".to_owned()),
            ("pkg/views.py", "def render():\n    pass\n".to_owned()),
            ("pkg/extra.py", "class Model:\n    pass\n".to_owned()),
        ]);
        let api_head = b"def serve():\n    return Model().run()\n".as_slice();
        let views_head = b"def render():\n    return Model\n".as_slice();
        let files = vec![
            listed(
                "pkg/models.py",
                FileStatus::Modified,
                Some("@@ -3 +3 @@\n-        return 1\n+        return 2"),
            ),
            listed(
                "pkg/api.py",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    pass\n+    return Model().run()"),
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
            (path("pkg/api.py"), api_head),
            (path("pkg/views.py"), views_head),
        ]);
        let view = borrowed(&base);
        let trials = evaluate(9, base_index(&view), &view, &files, &heads, 100).expect("trials");
        assert_eq!(
            trials
                .iter()
                .map(|t| t.withheld.as_str())
                .collect::<Vec<_>>(),
            ["pkg/models.py", "pkg/api.py", "pkg/views.py"]
        );
        let trial = withheld(&trials, "pkg/models.py");
        assert_eq!(trial.pr, 9);
        assert_eq!(
            trial
                .excluded
                .iter()
                .map(RelativePath::as_str)
                .collect::<Vec<_>>(),
            ["pkg/api.py", "pkg/new.py", "pkg/views.py"]
        );
        assert_eq!(
            trial.diff_tokens,
            100 - estimate_tokens("@@ -3 +3 @@\n-        return 1\n+        return 2")
        );
        assert_eq!(trial.a.rank, Some(1));
        assert_eq!(trial.a.names, ["Model", "run"]);
        assert_eq!(trial.a.direction, Some(Direction::Upstream));
        assert!(trial.production_selected);
        assert_eq!(trial.omitted.len(), 1);
        assert_eq!(trial.omitted[0].reason, "missing patch");

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
        let evaluated = rank_a(&corpus, &remaining, &excluded).expect("rank A");
        let production = base_index(&view)
            .rank_files(&[
                ChangedBlob {
                    path: "pkg/api.py",
                    head: api_head,
                    changed: vec![LineRange { start: 2, end: 2 }],
                },
                ChangedBlob {
                    path: "pkg/views.py",
                    head: views_head,
                    changed: vec![LineRange { start: 2, end: 2 }],
                },
            ])
            .expect("production ranking");
        assert_eq!(evaluated, production);
        assert_eq!(
            evaluated
                .iter()
                .map(|f| f.path.as_str())
                .collect::<Vec<_>>(),
            ["pkg/models.py", "pkg/extra.py"]
        );
    }

    /// Scenario 2: B counts directional names once, C weights by rarity and
    /// splits ambiguous upstream credit, D scopes by explicit imports and
    /// otherwise falls back to C, in Python and Rust; removed declarations
    /// contribute base names; a missing base blob fails the run.
    #[test]
    fn variants_score_direction_rarity_and_imports() {
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
        let trials = evaluate(1, base_index(&view), &view, &files, &heads, 0).expect("trials");
        let trial = withheld(&trials, "pkg/models.py");
        // Remaining util changes helper and removes gone; caller uses
        // helper. helper: declared by util and legacy, referenced by
        // models, far, loose, caller: df 6. gone: declared by util,
        // referenced by loose: df 2. B: loose 2; far, legacy, models 1.
        assert_eq!(trial.b.rank, Some(4));
        assert_eq!(trial.b.score, 1.0);
        assert_eq!(trial.b.direction, Some(Direction::Downstream));
        assert_eq!(trial.b.names, ["helper"]);
        assert!((trial.c.score - 1.0 / 6.0).abs() < 1e-12);
        // C: loose 1/6 + 1/2; far and models 1/6; legacy 1/(6*2).
        assert_eq!(trial.c.rank, Some(3));
        // D: far imports helper from legacy (scoped away); models imports it
        // from util (scoped in); loose's relative import stays unscoped;
        // caller's upstream helper is scoped to util, so legacy drops.
        assert_eq!(trial.d.rank, Some(2));
        // Scoped: caller's helper, models' and far's helper imports.
        // Unscoped: loose's helper and gone.
        assert_eq!((trial.names_scoped, trial.names_unscoped), (3, 2));

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
                "tokio/src/other.rs",
                "use crate::alt::read_all;\n\npub fn other() -> u8 {\n    read_all()\n}\n"
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
        let trials = evaluate(2, base_index(&view), &view, &files, &heads, 0).expect("trials");
        let trial = withheld(&trials, "tokio/src/io.rs");
        // user's read_all resolves upstream to io and alt in A-C; D scopes
        // it to io through `use crate::io::read_all`.
        assert_eq!(trial.a.rank, Some(2));
        assert_eq!(trial.c.rank, Some(2));
        assert_eq!(trial.d.rank, Some(1));
        assert_eq!(trial.d.direction, Some(Direction::Upstream));

        let mut missing = view.clone();
        missing.remove(&path("tokio/src/alt.rs"));
        assert!(matches!(
            evaluate(2, base_index(&view), &missing, &files, &heads, 0),
            Err(TrialError::MissingSource { pr: 2, .. })
        ));
    }

    /// D keeps only the uniquely imported declarer of an ambiguous name but
    /// credits it C's `1 / (df × declaring_file_count)`, so a rarer
    /// single-declarer name outranks it.
    #[test]
    fn scoped_upstream_keeps_declaring_count_in_credit() {
        let base = sources(&[
            ("pkg/util.py", "def helper(x):\n    return x\n".to_owned()),
            ("pkg/legacy.py", "def helper(x):\n    return -x\n".to_owned()),
            ("pkg/rare.py", "def rare():\n    return 0\n".to_owned()),
            ("pkg/r1.py", "from pkg.rare import rare\n".to_owned()),
            ("pkg/r2.py", "from pkg.rare import rare\n".to_owned()),
            (
                "pkg/caller.py",
                "from pkg.util import helper\nfrom pkg.rare import rare\n\ndef call():\n    return 0\n"
                    .to_owned(),
            ),
        ]);
        let files = vec![
            listed(
                "pkg/util.py",
                FileStatus::Modified,
                Some("@@ -2 +2 @@\n-    return x\n+    return x + 1"),
            ),
            listed(
                "pkg/caller.py",
                FileStatus::Modified,
                Some("@@ -5 +5 @@\n-    return 0\n+    return helper(0) + rare()"),
            ),
        ];
        let heads = BTreeMap::from([
            (path("pkg/util.py"), b"def helper(x):\n    return x + 1\n".as_slice()),
            (
                path("pkg/caller.py"),
                b"from pkg.util import helper\nfrom pkg.rare import rare\n\ndef call():\n    return helper(0) + rare()\n"
                    .as_slice(),
            ),
        ]);
        let view = borrowed(&base);
        let trials = evaluate(4, base_index(&view), &view, &files, &heads, 0).expect("trials");
        let trial = withheld(&trials, "pkg/util.py");
        // helper: declared by util and legacy, referenced by caller: df 3,
        // credit 1/(3*2). rare: declared by rare, referenced by caller, r1,
        // r2: df 4, credit 1/4. C: rare, then the legacy/util tie by path.
        // D scopes helper to util only, which stays behind rare.
        assert!((trial.c.score - 1.0 / 6.0).abs() < 1e-12);
        assert!((trial.d.score - 1.0 / 6.0).abs() < 1e-12);
        assert_eq!(trial.c.rank, Some(3));
        assert_eq!(trial.d.rank, Some(2));
        assert_eq!(trial.d.sent, Sent::Whole);
        assert_eq!(trial.b.rank, Some(3));
        assert_eq!((trial.names_scoped, trial.names_unscoped), (2, 0));
    }

    /// Two Python `from` imports on one line bind each name to its own
    /// statement's module, upstream from a remaining head and downstream
    /// from an indexed base candidate; a name bound by both falls back to C.
    #[test]
    fn same_line_python_imports_bind_their_own_module() {
        let base = sources(&[
            ("pkg/a.py", "def x():\n    return 0\n\ndef y():\n    return 0\n".to_owned()),
            ("pkg/b.py", "def y():\n    return 1\n".to_owned()),
            (
                "pkg/caller.py",
                "from pkg.a import x; from pkg.b import y\n\ndef call():\n    return 0\n".to_owned(),
            ),
            (
                "pkg/user.py",
                "from pkg.a import x; from pkg.b import y\n\ndef use():\n    return y()\n\ndef other():\n    return 0\n"
                    .to_owned(),
            ),
            (
                "pkg/both.py",
                "from pkg.a import y; from pkg.b import y\n\ndef both():\n    return 0\n".to_owned(),
            ),
        ]);
        let run = |caller: &str| {
            let files = vec![
                listed(
                    "pkg/b.py",
                    FileStatus::Modified,
                    Some("@@ -2 +2 @@\n-    return 1\n+    return 2"),
                ),
                listed(
                    caller,
                    FileStatus::Modified,
                    Some("@@ -4 +4 @@\n-    return 0\n+    return y()"),
                ),
                listed(
                    "pkg/user.py",
                    FileStatus::Modified,
                    Some("@@ -7 +7 @@\n-    return 0\n+    return 1"),
                ),
            ];
            let head = String::from_utf8_lossy(base[&path(caller)].as_slice())
                .replacen("    return 0\n", "    return y()\n", 1);
            let heads: BTreeMap<RelativePath, Vec<u8>> = BTreeMap::from([
                (path("pkg/b.py"), b"def y():\n    return 2\n".to_vec()),
                (path(caller), head.into_bytes()),
                (
                    path("pkg/user.py"),
                    base[&path("pkg/user.py")]
                        .strip_suffix(b"    return 0\n")
                        .expect("fixture tail")
                        .iter()
                        .chain(b"    return 1\n")
                        .copied()
                        .collect(),
                ),
            ]);
            let view = borrowed(&base);
            evaluate(5, base_index(&view), &view, &files, &borrowed(&heads), 0).expect("trials")
        };

        let trials = run("pkg/caller.py");
        // Upstream: caller's y is bound to b, not a.
        let b = withheld(&trials, "pkg/b.py");
        assert_eq!(b.d.rank, Some(1));
        assert_eq!(b.d.direction, Some(Direction::Upstream));
        // Downstream: user's base y is bound to b, which the change edits.
        let user = withheld(&trials, "pkg/user.py");
        assert_eq!(user.d.rank, user.c.rank);
        assert!(user.d.rank.is_some());
        assert_eq!(user.d.direction, Some(Direction::Downstream));

        let trials = run("pkg/both.py");
        let b = withheld(&trials, "pkg/b.py");
        assert_eq!((b.d.rank, b.d.score), (b.c.rank, b.c.score));
        assert!(b.d.rank.is_some());
        assert_eq!(b.names_scoped, 0);
    }

    /// Scenario 3: under the common allowance the withheld file is sent
    /// whole, as spans only, omitted, or is unranked; a malformed patch is
    /// an excluded omission; the report has no source text.
    #[test]
    fn packing_records_whole_spans_omitted_and_unranked() {
        let pad = |lines: usize, indent: &str| {
            format!("{indent}# {}\n", "x".repeat(96 - indent.len())).repeat(lines)
        };
        let base = sources(&[
            ("app/change.py", "def change():\n    pass\n".to_owned()),
            ("app/broken.py", "def broken():\n    pass\n".to_owned()),
            (
                "lib/alpha.py",
                format!("def alpha():\n    pass\n{}", pad(3_600, "")),
            ),
            (
                "lib/beta.py",
                format!("def beta():\n    pass\n{}", pad(800, "")),
            ),
            (
                "lib/gamma.py",
                format!("def gamma():\n{}    pass\n", pad(480, "    ")),
            ),
            ("lib/lonely.py", "def lonely():\n    pass\n".to_owned()),
        ]);
        let change_patch =
            "@@ -1,2 +1,4 @@\n def change():\n-    pass\n+    alpha()\n+    beta()\n+    gamma()";
        let heads = BTreeMap::from([(
            path("app/change.py"),
            b"def change():\n    alpha()\n    beta()\n    gamma()\n".as_slice(),
        )]);
        let view = borrowed(&base);
        let sent = |raw: &str, broken: bool| {
            let mut files = vec![
                listed("app/change.py", FileStatus::Modified, Some(change_patch)),
                listed(
                    raw,
                    FileStatus::Modified,
                    Some("@@ -2 +2 @@\n-    pass\n+    return"),
                ),
            ];
            if broken {
                files.push(listed(
                    "app/broken.py",
                    FileStatus::Modified,
                    Some("not a patch"),
                ));
            }
            let head =
                String::from_utf8_lossy(view[&path(raw)]).replacen("    pass\n", "    return\n", 1);
            let mut heads = heads.clone();
            heads.insert(path(raw), head.as_bytes());
            let trials = evaluate(3, base_index(&view), &view, &files, &heads, 0).expect("trials");
            withheld(&trials, raw).clone()
        };
        let alpha = sent("lib/alpha.py", false);
        assert_eq!((alpha.a.rank, alpha.a.sent), (Some(1), Sent::Whole));
        let beta = sent("lib/beta.py", false);
        assert_eq!((beta.b.rank, beta.b.sent), (Some(2), Sent::SpansOnly));
        assert_eq!(
            beta.b.snippet_tokens,
            estimate_tokens("def beta():\n    pass\n")
        );
        assert!(beta.b.whole_tokens > beta.b.snippet_tokens);
        let gamma = sent("lib/gamma.py", true);
        assert_eq!((gamma.c.rank, gamma.c.sent), (Some(3), Sent::Omitted));
        assert!(gamma.excluded.iter().any(|p| p.as_str() == "app/broken.py"));
        assert!(gamma.omitted[0].reason.starts_with("malformed patch"));
        let lonely = sent("lib/lonely.py", false);
        for outcome in [&lonely.a, &lonely.b, &lonely.c, &lonely.d] {
            assert_eq!((outcome.rank, outcome.sent), (None, Sent::Unranked));
            assert_eq!(
                outcome.whole_tokens,
                estimate_tokens("def lonely():\n    pass\n")
            );
        }
        assert!(!lonely.production_selected);

        let json = serde_json::to_string(&[&alpha, &beta, &gamma, &lonely]).expect("serializes");
        assert!(json.contains("\"rank\":\"unranked\""));
        assert!(!json.contains("pass") && !json.contains("xxxx"));
    }
}
