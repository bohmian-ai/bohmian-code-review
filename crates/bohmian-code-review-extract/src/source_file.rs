//! [`SourceFile`] — one input file to the native extractor.

use bohmian_code_review_core::{Language, RelativePath};

/// One source file handed to [`crate::Extractor::extract`].
///
/// `path` is the repository-relative path whose bytes form node IDs and the
/// `file:` node; `bytes` is the raw file content (validated as UTF-8 by the
/// extractor, mirroring the kernel seam that consumed strings); `language`
/// selects the walker.
#[derive(Debug, Clone, Copy)]
pub struct SourceFile<'a> {
    /// Repository-relative path (validated [`RelativePath`]).
    pub path: &'a RelativePath,
    /// Raw source bytes.
    pub bytes: &'a [u8],
    /// Source language.
    pub language: Language,
}

impl<'a> SourceFile<'a> {
    /// Construct a source file.
    pub fn new(path: &'a RelativePath, bytes: &'a [u8], language: Language) -> Self {
        SourceFile {
            path,
            bytes,
            language,
        }
    }
}
