//! Shared regex and coordinate helpers for the composite extractors.
//!
//! The upstream composite extractors scan source with JavaScript regular
//! expressions and compute columns from `match.index` against the containing
//! line. JavaScript `\w`/`\b`/`\s` are ASCII classes, so every pattern here
//! is compiled with `unicode(false)`; JavaScript string offsets count UTF-16
//! code units, so byte offsets from the Rust `regex` crate are converted
//! with [`utf16_len`] before they become emitted columns.

use regex::Regex;

/// Compile a literal upstream pattern for `&str` matching.
///
/// Upstream scans operate on JavaScript strings (UTF-16 code units), so
/// patterns must match at whole-character boundaries. The `regex` crate
/// requires Unicode-aware classes for `&str` matching (`\S`/`[^...]` under
/// `unicode(false)` can match invalid UTF-8 bytes and are rejected); for
/// ASCII source — the corpus fixtures — `\w`/`\b` behave identically to
/// the frozen JavaScript ASCII classes, and the `\s` class actually covers
/// the same non-ASCII whitespace members as JavaScript's.
///
/// # Panics
///
/// Panics if the pattern fails to compile — every pattern is a literal
/// port of a frozen upstream regex and must compile.
pub fn js_re(pattern: &str) -> Regex {
    Regex::new(pattern).expect("upstream composite regex must compile")
}

/// The number of `\n` characters in `s` (upstream `(s.match(/\n/g)||[]).length`).
pub fn count_newlines(s: &str) -> u32 {
    s.bytes().filter(|b| *b == b'\n').count() as u32
}

/// The 1-based line number of the byte offset `index` within `source`
/// (upstream `getLineNumber`).
pub fn line_of(source: &str, index: usize) -> u32 {
    count_newlines(&source[..index]) + 1
}

/// The number of lines in `source` (`source.split('\n').length`).
pub fn line_count(source: &str) -> u32 {
    if source.is_empty() {
        1
    } else {
        count_newlines(source) + 1
    }
}

/// The UTF-16 length of the final line of `source` (the upstream
/// `lines[lines.length - 1]?.length || 0` column).
pub fn last_line_utf16_len(source: &str) -> u32 {
    match source.rfind('\n') {
        Some(index) => utf16_len(&source[index + 1..]) as u32,
        None => utf16_len(source) as u32,
    }
}

/// The UTF-16 code-unit length of `s` (upstream `s.length`).
pub fn utf16_len(s: &str) -> usize {
    s.chars().map(|c| c.len_utf16()).sum()
}

/// The UTF-16 code-unit column of the byte offset `byte_offset` within the
/// single line `line` (upstream `match.index` semantics against a JS string).
pub fn utf16_col_in_line(line: &str, byte_offset: usize) -> u32 {
    utf16_len(&line[..byte_offset]) as u32
}

/// The 0-indexed line of the byte offset `index` within `source`.
pub fn zero_indexed_line(source: &str, index: usize) -> u32 {
    count_newlines(&source[..index])
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Converts byte columns to UTF-16 units the way the upstream JS
    /// `match.index` does.
    fn byte_offsets_become_utf16_columns() {
        let line = "abé😀x";
        // bytes: a=0 b=1 é=2..4 😀=4..8 x=8; units: a=0 b=1 é=2 😀=3 x=5
        assert_eq!(utf16_col_in_line(line, 0), 0);
        assert_eq!(utf16_col_in_line(line, 2), 2);
        assert_eq!(utf16_col_in_line(line, 4), 3);
        assert_eq!(utf16_col_in_line(line, 8), 5);
        assert_eq!(utf16_len(line), 6);
    }

    #[test]
    /// Counts lines with the upstream split semantics.
    fn line_accounting_matches_js() {
        assert_eq!(line_count("a\nb"), 2);
        assert_eq!(line_count("a\nb\n"), 3);
        assert_eq!(line_count(""), 1);
        assert_eq!(line_of("a\nb\nc", 3), 2);
        assert_eq!(zero_indexed_line("a\nb\nc", 3), 1);
    }
}
