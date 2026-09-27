//! Public two-pass symbolic-context facade for code review.
//!
//! [`SymbolicContext`] composes the real stage owners over caller-supplied
//! paths and bytes: pass one upserts one blob at a time into an in-memory
//! symbol index, or loads previously exported source-free facts stamped
//! with [`EXTRACTION_VERSION`]; [`parse_unified_diff`] turns PR diff text
//! into each changed file's head-side lines; pass two ranks other indexed
//! files against one or many changed head-side blobs, and
//! [`build_context`] or [`build_candidate_context`] packs spans under
//! caller limits. Nothing here reads disk, fetches source, or retains
//! blobs; the index covers only files the caller supplied.

use bohmian_code_review_extract::{ExtractionConfig, ExtractionResult, Extractor};
use bohmian_code_review_index::SymbolIndex;
use bohmian_code_review_select::ChangedFile;
use std::collections::BTreeMap;
use thiserror::Error;

pub use bohmian_code_review_core::{
    DeclarationFact, ExtractionDiagnostic, IndexedFileFacts, InvalidFact, InvalidRelativePath,
    Language, Node, RawReference, RelativePath,
};
pub use bohmian_code_review_extract::{EXTRACTION_VERSION, ExtractionError};
pub use bohmian_code_review_index::SymbolHit;
pub use bohmian_code_review_select::{
    ChangedLines, Context, ContextLimits, ContextSpan, DiffError, FileDiff, LineRange, MatchReason,
    Omission, OmissionReason, RankedFile, Ranking, SelectError, SpanKind, parse_hunks,
    parse_unified_diff,
};

/// One changed head-side blob of a pull request to rank against the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedBlob<'a> {
    /// Repository-relative path of the changed file.
    pub path: &'a str,
    /// The full head-side blob; not indexed.
    pub head: &'a [u8],
    /// Head-side changed line ranges, validated against `head`.
    pub changed: Vec<LineRange>,
}

/// Facts extracted from one upserted file.
#[derive(Debug, Clone, PartialEq)]
pub struct FileFacts<'a> {
    /// Byte length of the supplied source, which is not retained.
    pub byte_len: usize,
    /// Extracted nodes, as retained by the index.
    pub nodes: &'a [Node],
    /// Extracted references, returned but not retained.
    pub references: Vec<RawReference>,
    /// File-local extraction diagnostics, returned but not retained.
    pub diagnostics: Vec<ExtractionDiagnostic>,
}

/// Typed facade failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Error {
    /// A supplied path is not a canonical repository-relative path.
    #[error("invalid repository-relative path `{path}`: {source}")]
    InvalidPath {
        /// The path as supplied.
        path: String,
        /// Why it was rejected.
        source: InvalidRelativePath,
    },
    /// Supplied source bytes are not UTF-8.
    #[error("source for `{path}` is not valid UTF-8")]
    NotUtf8 {
        /// The file's path.
        path: RelativePath,
    },
    /// The extractor rejected the file.
    #[error("extraction failed for `{path}`: {source}")]
    Extraction {
        /// The file's path.
        path: RelativePath,
        /// The extractor's typed failure.
        source: ExtractionError,
    },
    /// Persisted facts were produced by a different extractor.
    #[error("facts for `{path}` were extracted by `{found}`, this library is `{expected}`")]
    ExtractionVersionMismatch {
        /// The file's path.
        path: RelativePath,
        /// The version stamped on the facts.
        found: String,
        /// This library's [`EXTRACTION_VERSION`].
        expected: &'static str,
    },
    /// Persisted facts violate a declaration invariant.
    #[error("invalid facts for `{path}`: {source}")]
    InvalidFacts {
        /// The file's path.
        path: RelativePath,
        /// The first violated invariant.
        source: InvalidFact,
    },
    /// Diff text could not be analyzed into changed lines.
    #[error(transparent)]
    Diff(#[from] DiffError),
    /// Changed-line validation or context packing failed.
    #[error(transparent)]
    Select(#[from] SelectError),
}

/// Stateful in-memory symbolic context over caller-supplied files.
#[derive(Debug)]
pub struct SymbolicContext {
    /// Extractor for every native and composite route.
    extractor: Extractor,
    /// Facts and postings for successfully upserted files.
    index: SymbolIndex,
}

impl Default for SymbolicContext {
    fn default() -> Self {
        SymbolicContext::new()
    }
}

impl SymbolicContext {
    /// Builds an empty context that extracts every native language.
    ///
    /// # Panics
    ///
    /// Never in practice: `ExtractionConfig::all_native` enables at least one
    /// language, which is the extractor's only construction requirement.
    #[must_use]
    pub fn new() -> Self {
        SymbolicContext {
            extractor: Extractor::new(ExtractionConfig::all_native())
                .expect("all_native enables at least one language"),
            index: SymbolIndex::new(),
        }
    }

    /// Extracts one supplied blob and replaces `path`'s indexed facts.
    ///
    /// Extraction completes before the index is touched, so any failure
    /// leaves the previous facts for `path` and every other file intact.
    /// Only nodes and the byte length are retained.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`], [`Error::NotUtf8`], or
    /// [`Error::Extraction`] for unsupported, disabled, or failed
    /// extraction.
    pub fn upsert(&mut self, path: &str, source: &[u8]) -> Result<FileFacts<'_>, Error> {
        let path = relative_path(path)?;
        let result = self.extract(&path, source)?;
        self.index.upsert(path.clone(), source.len(), result.nodes);
        Ok(FileFacts {
            byte_len: source.len(),
            nodes: self.index.nodes(&path).unwrap_or_default(),
            references: result.references,
            diagnostics: result.diagnostics,
        })
    }

    /// Removes `path`'s facts; returns `false` when it was not indexed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`] for a non-canonical path.
    pub fn remove(&mut self, path: &str) -> Result<bool, Error> {
        Ok(self.index.remove(&relative_path(path)?))
    }

    /// Exports every indexed file's source-free declaration facts in
    /// canonical path order, stamped with [`EXTRACTION_VERSION`].
    ///
    /// The result contains no docstrings, signatures, or other source
    /// fragments; loading it into a fresh context with
    /// [`SymbolicContext::load`] reproduces lookup, ranking, and candidate
    /// spans without reparsing.
    #[must_use]
    pub fn export(&self) -> Vec<IndexedFileFacts> {
        self.index.export(EXTRACTION_VERSION)
    }

    /// Loads previously exported facts, replacing `facts.path`'s indexed
    /// facts exactly as an upsert would.
    ///
    /// Version and invariant checks complete before the index is touched,
    /// so a rejected record leaves every file unchanged.
    ///
    /// # Errors
    ///
    /// Returns [`Error::ExtractionVersionMismatch`] when the facts were
    /// stamped by another extractor and [`Error::InvalidFacts`] for a
    /// malformed declaration.
    pub fn load(&mut self, facts: IndexedFileFacts) -> Result<(), Error> {
        if facts.extraction_version != EXTRACTION_VERSION {
            return Err(Error::ExtractionVersionMismatch {
                path: facts.path,
                found: facts.extraction_version,
                expected: EXTRACTION_VERSION,
            });
        }
        let path = facts.path.clone();
        self.index
            .load(facts)
            .map_err(|source| Error::InvalidFacts { path, source })
    }

    /// The language this context would extract `path` as, from the path
    /// alone, or `None` when the path would be rejected as unsupported.
    ///
    /// Answers eligibility before source is downloaded. Ambiguous routes
    /// such as `.h` may still be refined from content at parse time.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`] for a non-canonical path.
    pub fn path_language(&self, path: &str) -> Result<Option<Language>, Error> {
        let path = relative_path(path)?;
        Ok(self.extractor.path_language(path.as_str()))
    }

    /// Every indexed declaration named exactly `name`, in canonical
    /// path/location/ID order; more than one hit is ambiguous.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Vec<SymbolHit<'_>> {
        self.index.lookup(name)
    }

    /// Byte length retained for an indexed `path`.
    #[must_use]
    pub fn byte_len(&self, path: &RelativePath) -> Option<usize> {
        self.index.byte_len(path)
    }

    /// Number of caller-supplied files currently indexed; not a repository
    /// coverage claim.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.index.file_count()
    }

    /// Parses the full changed head blob and ranks other indexed files by
    /// distinct names on `changed` lines. The head blob is not indexed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`], [`Error::NotUtf8`],
    /// [`Error::Select`] for invalid or deletion-only ranges, and
    /// [`Error::Extraction`] when the head blob cannot be extracted.
    pub fn rank(&self, path: &str, head: &[u8], changed: Vec<LineRange>) -> Result<Ranking, Error> {
        let (path, changed, result) = self.prepare_changed(path, head, changed)?;
        Ok(bohmian_code_review_select::rank(
            &self.index,
            &path,
            &result.nodes,
            &result.references,
            &changed,
        ))
    }

    /// Parses every changed head blob of a pull request and ranks other
    /// indexed files by the union of their distinct changed-line names.
    ///
    /// Every changed path is excluded from the candidates; files sort by
    /// descending score then canonical path. No head blob is indexed. Pack
    /// the result with [`build_candidate_context`].
    ///
    /// # Errors
    ///
    /// Returns the first per-file error as [`SymbolicContext::rank`] does.
    pub fn rank_files(&self, changed: &[ChangedBlob<'_>]) -> Result<Vec<RankedFile>, Error> {
        let prepared = changed
            .iter()
            .map(|blob| self.prepare_changed(blob.path, blob.head, blob.changed.clone()))
            .collect::<Result<Vec<_>, Error>>()?;
        let files: Vec<ChangedFile<'_>> = prepared
            .iter()
            .map(|(path, changed_lines, result)| ChangedFile {
                path,
                nodes: &result.nodes,
                references: &result.references,
                changed_lines,
            })
            .collect();
        Ok(bohmian_code_review_select::rank_files(&self.index, &files))
    }

    /// Validates one changed blob's path and lines and extracts its facts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidPath`], [`Error::NotUtf8`], [`Error::Select`]
    /// for invalid or deletion-only ranges, and [`Error::Extraction`].
    fn prepare_changed(
        &self,
        path: &str,
        head: &[u8],
        changed: Vec<LineRange>,
    ) -> Result<(RelativePath, ChangedLines, ExtractionResult), Error> {
        let path = relative_path(path)?;
        let changed = ChangedLines::new(changed, decode(&path, head)?)?;
        let result = self.extract(&path, head)?;
        Ok((path, changed, result))
    }

    /// Decodes and extracts one blob without touching the index.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotUtf8`] or [`Error::Extraction`].
    fn extract(&self, path: &RelativePath, source: &[u8]) -> Result<ExtractionResult, Error> {
        let text = decode(path, source)?;
        self.extractor
            .extract_from_source(path.as_str(), text)
            .map_err(|source| Error::Extraction {
                path: path.clone(),
                source,
            })
    }
}

/// Packs only ranked candidate spans from caller-supplied `blobs` under
/// `limits` counted by `count_tokens`; the diff is delivered separately.
///
/// `max_files` and the returned file count cover selected candidate files
/// alone. Every omission is reported.
///
/// # Errors
///
/// Returns [`Error::Select`] for a missing or unusable candidate blob or a
/// blob that does not match its indexed spans.
pub fn build_candidate_context(
    files: &[RankedFile],
    blobs: &BTreeMap<RelativePath, &[u8]>,
    limits: ContextLimits,
    count_tokens: impl Fn(&str) -> usize,
) -> Result<Context, Error> {
    Ok(bohmian_code_review_select::pack_candidates(
        files,
        blobs,
        limits,
        count_tokens,
    )?)
}

/// Packs `ranking`'s changed hunks from `head`, then ranked candidate spans
/// from caller-supplied `blobs`, under `limits` counted by `count_tokens`.
///
/// # Errors
///
/// Returns [`Error::NotUtf8`] when `head` is not UTF-8 and [`Error::Select`]
/// for an unfittable changed hunk, a missing or unusable candidate blob, or
/// a blob that does not match its indexed spans.
pub fn build_context(
    ranking: &Ranking,
    head: &[u8],
    blobs: &BTreeMap<RelativePath, &[u8]>,
    limits: ContextLimits,
    count_tokens: impl Fn(&str) -> usize,
) -> Result<Context, Error> {
    let head = decode(&ranking.changed_path, head)?;
    Ok(bohmian_code_review_select::pack(
        ranking,
        head,
        blobs,
        limits,
        count_tokens,
    )?)
}

/// Validates a caller-supplied path.
///
/// # Errors
///
/// Returns [`Error::InvalidPath`] when `raw` is not canonical.
fn relative_path(raw: &str) -> Result<RelativePath, Error> {
    RelativePath::new(raw).map_err(|source| Error::InvalidPath {
        path: raw.to_owned(),
        source,
    })
}

/// Decodes `path`'s supplied bytes as UTF-8.
///
/// # Errors
///
/// Returns [`Error::NotUtf8`] when the bytes are not UTF-8.
fn decode<'s>(path: &RelativePath, source: &'s [u8]) -> Result<&'s str, Error> {
    std::str::from_utf8(source).map_err(|_| Error::NotUtf8 { path: path.clone() })
}
