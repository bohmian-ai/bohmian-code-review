//! Public two-pass symbolic-context facade for code review.
//!
//! [`SymbolicContext`] composes the real stage owners over caller-supplied
//! paths and bytes: pass one upserts one blob at a time into an in-memory
//! symbol index; [`parse_unified_diff`] turns PR diff text into each changed
//! file's head-side lines; pass two ranks other indexed files against a
//! changed head-side blob, and [`build_context`] packs changed hunks and
//! ranked spans under caller limits. Nothing here reads disk, fetches source, or
//! retains blobs; the index covers only files the caller supplied.

use bohmian_code_review_extract::{ExtractionConfig, Extractor};
use bohmian_code_review_index::SymbolIndex;
use std::collections::BTreeMap;
use thiserror::Error;

pub use bohmian_code_review_core::{
    ExtractionDiagnostic, InvalidRelativePath, Node, RawReference, RelativePath,
};
pub use bohmian_code_review_extract::ExtractionError;
pub use bohmian_code_review_index::SymbolHit;
pub use bohmian_code_review_select::{
    ChangedLines, Context, ContextLimits, ContextSpan, DiffError, FileDiff, LineRange, MatchReason,
    Omission, OmissionReason, RankedFile, Ranking, SelectError, SpanKind, parse_hunks,
    parse_unified_diff,
};

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
    /// The extractor rejected or deferred the file.
    #[error("extraction failed for `{path}`: {source}")]
    Extraction {
        /// The file's path.
        path: RelativePath,
        /// The extractor's typed failure.
        source: ExtractionError,
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
    /// [`Error::Extraction`] for unsupported, disabled, deferred, or failed
    /// extraction.
    pub fn upsert(&mut self, path: &str, source: &[u8]) -> Result<FileFacts<'_>, Error> {
        let path = relative_path(path)?;
        let text = decode(&path, source)?;
        let result = self
            .extractor
            .extract_from_source(path.as_str(), text)
            .map_err(|source| Error::Extraction {
                path: path.clone(),
                source,
            })?;
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
        let path = relative_path(path)?;
        let text = decode(&path, head)?;
        let changed = ChangedLines::new(changed, text)?;
        let result = self
            .extractor
            .extract_from_source(path.as_str(), text)
            .map_err(|source| Error::Extraction {
                path: path.clone(),
                source,
            })?;
        Ok(bohmian_code_review_select::rank(
            &self.index,
            &path,
            &result.nodes,
            &result.references,
            &changed,
        ))
    }
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
