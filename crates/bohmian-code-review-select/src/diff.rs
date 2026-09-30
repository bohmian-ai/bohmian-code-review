//! Unified-diff analysis into head-side changed lines.
//!
//! [`parse_unified_diff`] reads `git diff`/`git show`/`diff -u` text and
//! [`parse_hunks`] reads hunk-only patches such as a Git provider's per-file
//! `patch` field. Both report the one-based head-side lines each file adds or
//! modifies, which is what ranking consumes. Hunk bodies are checked against
//! their header counts, so a truncated or corrupted patch fails instead of
//! silently dropping changed lines. Nothing here fetches or reads source.

use crate::LineRange;
use bohmian_code_review_core::{InvalidRelativePath, RelativePath};
use thiserror::Error;

/// One file's change in a unified diff.
///
/// A file with a `new_path` and nonempty `added` ranges is a ranking input;
/// deletions, pure renames, mode changes, binary files, and deletion-only
/// hunks add no head-side lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Base-side path; `None` when the file is added.
    pub old_path: Option<RelativePath>,
    /// Head-side path; `None` when the file is deleted.
    pub new_path: Option<RelativePath>,
    /// Head-side lines added or modified, ascending, nonoverlapping, with
    /// adjacent lines coalesced.
    pub added: Vec<LineRange>,
}

/// Typed diff-parsing failures, each naming the one-based diff line.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DiffError {
    /// A hunk appeared before any file header.
    #[error("diff line {line}: hunk before any file header")]
    HunkWithoutFile {
        /// Offending diff line.
        line: usize,
    },
    /// A hunk header is not `@@ -a[,b] +c[,d] @@`.
    #[error("diff line {line}: malformed hunk header")]
    InvalidHunkHeader {
        /// Offending diff line.
        line: usize,
    },
    /// A hunk does not start after the previous hunk's head-side lines.
    #[error("diff line {line}: hunk does not follow the previous hunk")]
    UnorderedHunks {
        /// Offending hunk header line.
        line: usize,
    },
    /// Input ended before a hunk supplied the lines its header declared.
    #[error("diff line {line}: hunk body is shorter than its header declares")]
    HunkLengthMismatch {
        /// The truncated hunk's header line.
        line: usize,
    },
    /// A line is neither a valid hunk body line nor allowed outside a hunk.
    #[error("diff line {line}: unexpected line for the hunk structure")]
    UnexpectedLine {
        /// Offending diff line.
        line: usize,
    },
    /// A C-style quoted path has a bad escape or is not UTF-8.
    #[error("diff line {line}: malformed quoted path")]
    InvalidQuotedPath {
        /// Offending diff line.
        line: usize,
    },
    /// A file's headers never name one of its paths.
    #[error("diff line {line}: file header names no path")]
    MissingPath {
        /// The file's first header line.
        line: usize,
    },
    /// A named path is not a canonical repository-relative path.
    #[error("diff line {line}: invalid path `{path}`: {source}")]
    InvalidPath {
        /// The file's first header line.
        line: usize,
        /// The path as written after prefix removal.
        path: String,
        /// Why it was rejected.
        source: InvalidRelativePath,
    },
}

/// Parses multi-file unified diff text into per-file head-side changes.
///
/// Accepts `git diff`, `git show`, and `git format-patch` output, including
/// renames, copies, mode and binary changes, and quoted paths, as well as
/// plain `diff -u` output split by `---`/`+++` headers. Text before the
/// first file header is ignored.
///
/// # Errors
///
/// Returns a [`DiffError`] naming the first malformed line: a hunk without a
/// file, a malformed, unordered, or truncated hunk, a stray body line, or a
/// missing, badly quoted, or non-canonical path.
pub fn parse_unified_diff(diff: &str) -> Result<Vec<FileDiff>, DiffError> {
    let mut files = Vec::new();
    let mut file: Option<FileHeaders> = None;
    for (index, text) in diff.lines().enumerate() {
        let line = index + 1;
        if let Some(open) = file.as_mut().filter(|file| file.hunks.is_open()) {
            open.hunks.feed(text, line)?;
        } else if let Some(names) = text.strip_prefix("diff --git ") {
            files.extend(file.take().map(FileHeaders::finish).transpose()?);
            file = Some(FileHeaders::new(line, symmetric_path(names)));
        } else if let Some(rest) = text.strip_prefix("--- ") {
            if file.as_ref().is_none_or(|file| file.minus_seen) {
                files.extend(file.take().map(FileHeaders::finish).transpose()?);
                file = Some(FileHeaders::new(line, None));
            }
            let headers = file.as_mut().expect("a file was just ensured");
            headers.minus_seen = true;
            match side_path(rest, "a/", line)? {
                Some(path) => headers.old = Some(path),
                None => headers.created = true,
            }
        } else if let Some(headers) = file.as_mut() {
            headers.outside_hunk(text, line)?;
        } else if text.starts_with("@@") {
            return Err(DiffError::HunkWithoutFile { line });
        }
    }
    files.extend(file.map(FileHeaders::finish).transpose()?);
    Ok(files)
}

/// Parses a hunk-only patch, such as a provider's per-file `patch` field,
/// into head-side added ranges.
///
/// # Errors
///
/// Returns a [`DiffError`] for a malformed, unordered, or truncated hunk, or
/// any non-empty line outside a hunk other than a `\` marker.
pub fn parse_hunks(patch: &str) -> Result<Vec<LineRange>, DiffError> {
    let mut hunks = Hunks::default();
    for (index, text) in patch.lines().enumerate() {
        let line = index + 1;
        if hunks.is_open() {
            hunks.feed(text, line)?;
        } else if text.starts_with("@@") {
            hunks.start(text, line)?;
        } else if !text.is_empty() && !text.starts_with('\\') {
            return Err(DiffError::UnexpectedLine { line });
        }
    }
    hunks.finish()
}

/// Head-side added-line tracking across one file's hunks.
#[derive(Debug, Default)]
struct Hunks {
    /// Coalesced added ranges so far.
    added: Vec<LineRange>,
    /// Base and head lines the open hunk still expects.
    remaining: (u32, u32),
    /// Head line number of the next body line.
    next: u32,
    /// Last head line covered by the previous hunk, once one started.
    last: Option<u32>,
    /// Diff line of the open hunk's header.
    header: usize,
}

impl Hunks {
    /// Whether the current hunk still expects body lines.
    fn is_open(&self) -> bool {
        self.remaining != (0, 0)
    }

    /// Whether any hunk has started.
    fn started(&self) -> bool {
        self.last.is_some()
    }

    /// Opens the hunk whose header is `text`.
    ///
    /// # Errors
    ///
    /// Returns [`DiffError::InvalidHunkHeader`] for a malformed header or
    /// impossible head range, and [`DiffError::UnorderedHunks`] when it does
    /// not start after the previous hunk.
    fn start(&mut self, text: &str, line: usize) -> Result<(), DiffError> {
        let (old, start, new) = hunk_header(text)
            .filter(|&(_, start, new)| start > 0 || new == 0)
            .ok_or(DiffError::InvalidHunkHeader { line })?;
        if self.last.is_some_and(|last| start <= last) {
            return Err(DiffError::UnorderedHunks { line });
        }
        let last = start
            .checked_add(new.max(1) - 1)
            .ok_or(DiffError::InvalidHunkHeader { line })?;
        self.last = Some(last);
        self.remaining = (old, new);
        self.next = start;
        self.header = line;
        Ok(())
    }

    /// Consumes one body line of the open hunk.
    ///
    /// An empty line counts as blank context, as `patch` accepts.
    ///
    /// # Errors
    ///
    /// Returns [`DiffError::UnexpectedLine`] for an unknown prefix or a line
    /// the header's counts do not allow.
    fn feed(&mut self, text: &str, line: usize) -> Result<(), DiffError> {
        let (old, new) = &mut self.remaining;
        let unexpected = DiffError::UnexpectedLine { line };
        match text.as_bytes().first() {
            Some(b'+') => {
                *new = new.checked_sub(1).ok_or(unexpected)?;
                let line = self.next;
                self.next += 1;
                match self.added.last_mut() {
                    Some(range) if range.end + 1 == line => range.end = line,
                    _ => self.added.push(LineRange {
                        start: line,
                        end: line,
                    }),
                }
            }
            Some(b'-') => *old = old.checked_sub(1).ok_or(unexpected)?,
            Some(b' ') | None => {
                *old = old.checked_sub(1).ok_or(unexpected.clone())?;
                *new = new.checked_sub(1).ok_or(unexpected)?;
                self.next += 1;
            }
            Some(b'\\') => {}
            Some(_) => return Err(unexpected),
        }
        Ok(())
    }

    /// Returns the added ranges.
    ///
    /// # Errors
    ///
    /// Returns [`DiffError::HunkLengthMismatch`] when a hunk is still open.
    fn finish(self) -> Result<Vec<LineRange>, DiffError> {
        if self.is_open() {
            return Err(DiffError::HunkLengthMismatch { line: self.header });
        }
        Ok(self.added)
    }
}

/// Headers and hunks collected for one file of a multi-file diff.
#[derive(Debug)]
struct FileHeaders {
    /// First header line, reported by path errors.
    line: usize,
    /// Path from an unambiguous `diff --git a/P b/P` line.
    fallback: Option<String>,
    /// Base path from `---`, `rename from`, or `copy from`.
    old: Option<String>,
    /// Head path from `+++`, `rename to`, or `copy to`.
    new: Option<String>,
    /// The base side is `/dev/null` or a `new file mode` header appeared.
    created: bool,
    /// The head side is `/dev/null` or a `deleted file mode` header appeared.
    deleted: bool,
    /// A `---` header was seen, so another one starts a new plain-diff file.
    minus_seen: bool,
    /// The file's hunks.
    hunks: Hunks,
}

impl FileHeaders {
    /// Starts a file at diff line `line`.
    fn new(line: usize, fallback: Option<String>) -> Self {
        FileHeaders {
            line,
            fallback,
            old: None,
            new: None,
            created: false,
            deleted: false,
            minus_seen: false,
            hunks: Hunks::default(),
        }
    }

    /// Applies one line seen outside a hunk.
    ///
    /// Unknown extended headers (`index`, `similarity index`, `old mode`,
    /// `Binary files`) and trailing `format-patch` text are ignored, but a
    /// body-like line after this file's hunks means a hunk ran long.
    ///
    /// # Errors
    ///
    /// Returns [`DiffError::UnexpectedLine`] for such a body-like line and
    /// hunk-header or quoted-path errors for the relevant headers.
    fn outside_hunk(&mut self, text: &str, line: usize) -> Result<(), DiffError> {
        if let Some(rest) = text.strip_prefix("+++ ") {
            match side_path(rest, "b/", line)? {
                Some(path) => self.new = Some(path),
                None => self.deleted = true,
            }
        } else if text.starts_with("@@") {
            self.hunks.start(text, line)?;
        } else if let Some(rest) = text
            .strip_prefix("rename from ")
            .or_else(|| text.strip_prefix("copy from "))
        {
            self.old = Some(unquote(rest, line)?);
        } else if let Some(rest) = text
            .strip_prefix("rename to ")
            .or_else(|| text.strip_prefix("copy to "))
        {
            self.new = Some(unquote(rest, line)?);
        } else if text.starts_with("new file mode ") {
            self.created = true;
        } else if text.starts_with("deleted file mode ") {
            self.deleted = true;
        } else if self.hunks.started()
            && text != "-- "
            && matches!(text.as_bytes().first(), Some(b'+' | b'-' | b' '))
        {
            return Err(DiffError::UnexpectedLine { line });
        }
        Ok(())
    }

    /// Validates the collected paths and returns the file's change.
    ///
    /// # Errors
    ///
    /// Returns [`DiffError::HunkLengthMismatch`] for a truncated last hunk,
    /// [`DiffError::MissingPath`] when a present side has no path, and
    /// [`DiffError::InvalidPath`] for a non-canonical path.
    fn finish(self) -> Result<FileDiff, DiffError> {
        let line = self.line;
        let side = |absent: bool, named: Option<String>| {
            (!absent)
                .then(|| relative(named.or_else(|| self.fallback.clone()), line))
                .transpose()
        };
        Ok(FileDiff {
            old_path: side(self.created, self.old.clone())?,
            new_path: side(self.deleted, self.new.clone())?,
            added: self.hunks.finish()?,
        })
    }
}

/// Parses `@@ -a[,b] +c[,d] @@` into (base count, head start, head count).
fn hunk_header(text: &str) -> Option<(u32, u32, u32)> {
    let (ranges, _) = text.strip_prefix("@@ -")?.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let count = |range: &str| -> Option<(u32, u32)> {
        match range.split_once(',') {
            Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
            None => Some((range.parse().ok()?, 1)),
        }
    };
    let (_, old) = count(old)?;
    let (start, new) = count(new)?;
    Some((old, start, new))
}

/// The path shared by both sides of an unquoted `a/P b/P` header, if any.
///
/// Renames and quoted names are ambiguous here; their `---`/`+++` or
/// `rename` headers supply the paths instead.
fn symmetric_path(names: &str) -> Option<String> {
    let body = names.strip_prefix("a/")?;
    let half = body.len().checked_sub(3)? / 2;
    let name = body.get(..half)?;
    (body.get(half..)?.strip_prefix(" b/")? == name).then(|| name.to_owned())
}

/// Reads a `---`/`+++` path, dropping any tab-separated timestamp and the
/// side's `a/` or `b/` prefix; `None` means `/dev/null`.
///
/// # Errors
///
/// Returns [`DiffError::InvalidQuotedPath`] for a malformed quoted path.
fn side_path(rest: &str, prefix: &str, line: usize) -> Result<Option<String>, DiffError> {
    let token = rest.split_once('\t').map_or(rest, |(token, _)| token);
    if token == "/dev/null" {
        return Ok(None);
    }
    let path = unquote(token, line)?;
    // ponytail: assumes default a/ b/ prefixes; parse --src-prefix/--no-prefix output if a caller needs it.
    Ok(Some(
        path.strip_prefix(prefix)
            .map_or(path.clone(), str::to_owned),
    ))
}

/// Decodes Git's C-style quoted path, or returns an unquoted path as is.
///
/// # Errors
///
/// Returns [`DiffError::InvalidQuotedPath`] for an unknown or truncated
/// escape or non-UTF-8 bytes.
fn unquote(raw: &str, line: usize) -> Result<String, DiffError> {
    let invalid = DiffError::InvalidQuotedPath { line };
    let Some(inner) = raw.strip_prefix('"').and_then(|raw| raw.strip_suffix('"')) else {
        return Ok(raw.to_owned());
    };
    let mut bytes = Vec::with_capacity(inner.len());
    let mut input = inner.bytes();
    while let Some(byte) = input.next() {
        if byte != b'\\' {
            bytes.push(byte);
            continue;
        }
        let escaped = input.next().ok_or(invalid.clone())?;
        bytes.push(match escaped {
            b'a' => 0x07,
            b'b' => 0x08,
            b't' => b'\t',
            b'n' => b'\n',
            b'v' => 0x0b,
            b'f' => 0x0c,
            b'r' => b'\r',
            b'"' | b'\\' => escaped,
            b'0'..=b'3' => {
                let mut value = escaped - b'0';
                for _ in 0..2 {
                    let digit = input
                        .next()
                        .filter(u8::is_ascii_digit)
                        .ok_or(invalid.clone())?;
                    if digit > b'7' {
                        return Err(invalid);
                    }
                    value = value * 8 + (digit - b'0');
                }
                value
            }
            _ => return Err(invalid),
        });
    }
    String::from_utf8(bytes).map_err(|_| invalid)
}

/// Validates a collected path.
///
/// # Errors
///
/// Returns [`DiffError::MissingPath`] for `None` and
/// [`DiffError::InvalidPath`] for a non-canonical path.
fn relative(path: Option<String>, line: usize) -> Result<RelativePath, DiffError> {
    let path = path.ok_or(DiffError::MissingPath { line })?;
    RelativePath::new(&path).map_err(|source| DiffError::InvalidPath { line, path, source })
}

#[cfg(test)]
/// Owner tests for diff parsing over realistic Git output and corrupt input.
mod tests {
    use super::*;

    /// Parses a path known to be canonical.
    ///
    /// # Panics
    ///
    /// Panics if `raw` is not a valid repository-relative path.
    fn path(raw: &str) -> Option<RelativePath> {
        Some(RelativePath::new(raw).expect("valid path"))
    }

    /// Shorthand for a one-based inclusive range.
    fn lines(start: u32, end: u32) -> LineRange {
        LineRange { start, end }
    }

    #[test]
    /// `git show` output yields per-file head-side added ranges for modified,
    /// added, deleted, renamed, binary, and quoted non-ASCII paths; commit
    /// text, blank context, and no-newline markers are tolerated.
    fn git_diff_reports_added_ranges_per_file() {
        let diff = concat!(
            "commit 0123456789abcdef\n",
            "Author: A <a@example.com>\n",
            "\n",
            "    change handler\n",
            "\n",
            "diff --git a/src/handler.ts b/src/handler.ts\n",
            "index 1111111..2222222 100644\n",
            "--- a/src/handler.ts\n",
            "+++ b/src/handler.ts\n",
            "@@ -2,5 +2,8 @@ import { lookupUser } from \"./user\";\n",
            " import { formatUser } from \"./format\";\n",
            " \n",
            " export function handleRequest(id: string): string {\n",
            "-  return id;\n",
            "+  const user = lookupUser(id);\n",
            "+  saveUser(user);\n",
            "+  saveUser(user);\n",
            "+  return formatUser(user);\n",
            " }\n",
            "@@ -20,2 +23,2 @@\n",
            " keep\n",
            "--- removed line that looks like a header\n",
            "+added\n",
            "\\ No newline at end of file\n",
            "diff --git a/src/new.ts b/src/new.ts\n",
            "new file mode 100644\n",
            "--- /dev/null\n",
            "+++ b/src/new.ts\n",
            "@@ -0,0 +1,2 @@\n",
            "+one\n",
            "+two\n",
            "diff --git a/src/gone.ts b/src/gone.ts\n",
            "deleted file mode 100644\n",
            "--- a/src/gone.ts\n",
            "+++ /dev/null\n",
            "@@ -1 +0,0 @@\n",
            "-bye\n",
            "diff --git a/src/old name.ts b/src/new name.ts\n",
            "similarity index 100%\n",
            "rename from src/old name.ts\n",
            "rename to src/new name.ts\n",
            "diff --git a/img/logo.png b/img/logo.png\n",
            "index 3333333..4444444 100644\n",
            "Binary files a/img/logo.png and b/img/logo.png differ\n",
            "diff --git \"a/src/f\\303\\266o.ts\" \"b/src/f\\303\\266o.ts\"\n",
            "--- \"a/src/f\\303\\266o.ts\"\n",
            "+++ \"b/src/f\\303\\266o.ts\"\n",
            "@@ -1 +1 @@\n",
            "-a\n",
            "+b\n",
            "-- \n",
            "2.47.0\n",
        );

        assert_eq!(
            parse_unified_diff(diff),
            Ok(vec![
                FileDiff {
                    old_path: path("src/handler.ts"),
                    new_path: path("src/handler.ts"),
                    added: vec![lines(5, 8), lines(24, 24)],
                },
                FileDiff {
                    old_path: None,
                    new_path: path("src/new.ts"),
                    added: vec![lines(1, 2)],
                },
                FileDiff {
                    old_path: path("src/gone.ts"),
                    new_path: None,
                    added: Vec::new(),
                },
                FileDiff {
                    old_path: path("src/old name.ts"),
                    new_path: path("src/new name.ts"),
                    added: Vec::new(),
                },
                FileDiff {
                    old_path: path("img/logo.png"),
                    new_path: path("img/logo.png"),
                    added: Vec::new(),
                },
                FileDiff {
                    old_path: path("src/föo.ts"),
                    new_path: path("src/föo.ts"),
                    added: vec![lines(1, 1)],
                },
            ])
        );
    }

    #[test]
    /// A plain `diff -u` without `diff --git` lines splits files on `---`
    /// headers and ignores timestamps.
    fn plain_unified_diff_splits_on_file_headers() {
        let diff = concat!(
            "--- a/x.ts\t2026-09-25 10:00:00\n",
            "+++ b/x.ts\t2026-09-25 10:01:00\n",
            "@@ -1 +1,2 @@\n",
            " a\n",
            "+b\n",
            "--- a/y.ts\n",
            "+++ b/y.ts\n",
            "@@ -3,0 +4 @@\n",
            "+c\n",
        );
        assert_eq!(
            parse_unified_diff(diff),
            Ok(vec![
                FileDiff {
                    old_path: path("x.ts"),
                    new_path: path("x.ts"),
                    added: vec![lines(2, 2)],
                },
                FileDiff {
                    old_path: path("y.ts"),
                    new_path: path("y.ts"),
                    added: vec![lines(4, 4)],
                },
            ])
        );
    }

    #[test]
    /// A hunk-only provider patch coalesces adjacent added lines and treats a
    /// pure-deletion hunk as adding nothing.
    fn provider_patch_reports_coalesced_added_ranges() {
        let patch = concat!(
            "@@ -1,3 +1,5 @@\n",
            " a\n",
            "-b\n",
            "+B\n",
            "+C\n",
            " c\n",
            "+d\n",
            "@@ -10,2 +11 @@ fn tail()\n",
            " x\n",
            "-y\n",
            "\\ No newline at end of file\n",
        );
        assert_eq!(parse_hunks(patch), Ok(vec![lines(2, 3), lines(5, 5)]));
        assert_eq!(parse_hunks("@@ -4,1 +3,0 @@\n-gone\n"), Ok(Vec::new()));
    }

    #[test]
    /// Truncated, overlong, unordered, malformed, headerless, and unsafe-path
    /// input fails with the offending diff line.
    fn corrupt_diffs_fail_explicitly() {
        assert_eq!(
            parse_hunks("@@ -1,2 +1,2 @@\n a\n"),
            Err(DiffError::HunkLengthMismatch { line: 1 })
        );
        assert_eq!(
            parse_hunks("@@ -1 +1 @@\n a\n+extra\n"),
            Err(DiffError::UnexpectedLine { line: 3 })
        );
        assert_eq!(
            parse_hunks("@@ -1,2 +1,2 @@\n a\n*b\n"),
            Err(DiffError::UnexpectedLine { line: 3 })
        );
        assert_eq!(
            parse_hunks("@@ -5 +5 @@\n a\n@@ -1 +1 @@\n b\n"),
            Err(DiffError::UnorderedHunks { line: 3 })
        );
        assert_eq!(
            parse_hunks("@@ -1 +x @@\n"),
            Err(DiffError::InvalidHunkHeader { line: 1 })
        );
        assert_eq!(
            parse_unified_diff("@@ -1 +1 @@\n a\n"),
            Err(DiffError::HunkWithoutFile { line: 1 })
        );
        assert_eq!(
            parse_unified_diff("--- a/x.ts\n+++ b/x.ts\n@@ -1 +1 @@\n a\n+b\n"),
            Err(DiffError::UnexpectedLine { line: 5 })
        );
        assert_eq!(
            parse_unified_diff("--- a/x.ts\n+++ b/x.ts\n@@ -1,3 +1,3 @@\n a\ndiff --git a/y b/y\n"),
            Err(DiffError::UnexpectedLine { line: 5 })
        );
        assert!(matches!(
            parse_unified_diff("--- a/../x.ts\n+++ b/../x.ts\n"),
            Err(DiffError::InvalidPath { line: 1, .. })
        ));
        assert_eq!(
            parse_unified_diff("--- \"a/x\\q\"\n"),
            Err(DiffError::InvalidQuotedPath { line: 1 })
        );
        assert_eq!(
            parse_unified_diff("diff --git \"a/x y\" \"b/z\"\nold mode 100644\n"),
            Err(DiffError::MissingPath { line: 1 })
        );
    }
}
