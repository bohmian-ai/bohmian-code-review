//! Path-only repository eligibility applied to a pinned tree listing.
//!
//! Eligibility runs before any source is downloaded, so it can only use what
//! the tree listing and the root `.gitattributes` say: mode, path, reported
//! size, and linguist overrides. Content-dependent skips such as LFS
//! pointers happen later, during ingest. Every excluded entry is reported
//! with a reason so coverage can count exclusions separately from eligible
//! files that were not indexed.

use crate::github::{TreeEntry, TreeEntryMode};
use bohmian_code_review::{RelativePath, SymbolicContext};
use serde::{Deserialize, Serialize};

/// Largest eligible file in bytes; larger blobs are excluded before download.
pub const MAX_ELIGIBLE_FILE_BYTES: u64 = 1024 * 1024;

/// Path segments that mark vendored or generated trees.
const EXCLUDED_SEGMENTS: [&str; 6] = [
    "node_modules",
    "vendor",
    "third_party",
    "dist",
    "build",
    ".git",
];

/// Why a tree entry is not eligible for indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ExclusionReason {
    /// `120000` symbolic link.
    Symlink,
    /// `160000` submodule commit.
    Submodule,
    /// No supported language routes from the path.
    Unsupported,
    /// A path segment names a vendored or build tree.
    VendoredSegment,
    /// `.min.js` or `.min.css` output.
    MinifiedOutput,
    /// Root `.gitattributes` marks the path `linguist-generated`.
    GeneratedAttribute,
    /// Root `.gitattributes` marks the path `linguist-vendored`.
    VendoredAttribute,
    /// Reported size exceeds the file bound.
    Oversize,
}

/// One tree entry excluded from ingest, with its reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exclusion {
    /// The entry's path.
    pub path: RelativePath,
    /// Why it was excluded.
    pub reason: ExclusionReason,
}

/// The eligible blobs of one tree and everything it left out.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EligibleTree {
    /// Regular blobs the pipeline may fetch and index, in listing order.
    pub eligible: Vec<TreeEntry>,
    /// Entries left out, each with its reason; directories are not listed.
    pub excluded: Vec<Exclusion>,
}

/// Parsed root `.gitattributes` linguist overrides.
///
/// Only `linguist-generated` and `linguist-vendored` are read. Rules apply in
/// file order and the last matching rule for each attribute wins, as Git
/// resolves attributes. Nested `.gitattributes` files are not consulted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RootAttributes {
    rules: Vec<AttributeRule>,
}

/// One `.gitattributes` line that sets or unsets a linguist attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AttributeRule {
    pattern: String,
    generated: Option<bool>,
    vendored: Option<bool>,
}

impl RootAttributes {
    /// Parse `.gitattributes` text, ignoring comments, blank lines, macros,
    /// and attributes other than the two linguist overrides.
    ///
    /// Invalid UTF-8 yields no rules rather than an error: a malformed
    /// attributes file cannot make source ineligible.
    pub fn parse(bytes: &[u8]) -> RootAttributes {
        let text = String::from_utf8_lossy(bytes);
        let rules = text
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                let pattern = words.next()?;
                if pattern.starts_with('#') || pattern.starts_with('[') {
                    return None;
                }
                let mut rule = AttributeRule {
                    pattern: pattern.to_owned(),
                    generated: None,
                    vendored: None,
                };
                for word in words {
                    let (name, value) = parse_attribute(word);
                    match name {
                        "linguist-generated" => rule.generated = Some(value),
                        "linguist-vendored" => rule.vendored = Some(value),
                        _ => {}
                    }
                }
                (rule.generated.is_some() || rule.vendored.is_some()).then_some(rule)
            })
            .collect();
        RootAttributes { rules }
    }

    /// Effective `(linguist-generated, linguist-vendored)` for `path`;
    /// `None` when no rule mentions the attribute.
    fn resolve(&self, path: &str) -> (Option<bool>, Option<bool>) {
        let mut generated = None;
        let mut vendored = None;
        for rule in &self.rules {
            if pattern_matches(&rule.pattern, path) {
                generated = rule.generated.or(generated);
                vendored = rule.vendored.or(vendored);
            }
        }
        (generated, vendored)
    }
}

/// Split one attribute word into its name and effective boolean:
/// `attr` and `attr=true` set it, `-attr`, `!attr`, and `attr=false` unset it.
fn parse_attribute(word: &str) -> (&str, bool) {
    if let Some(name) = word.strip_prefix('-').or_else(|| word.strip_prefix('!')) {
        return (name, false);
    }
    match word.split_once('=') {
        Some((name, value)) => (name, value != "false"),
        None => (word, true),
    }
}

/// Match a `.gitattributes` pattern against a repository-relative path.
///
/// A pattern without `/` matches the basename anywhere; one with `/` is
/// anchored at the root. `*` and `?` never cross `/`, `**` does.
///
/// ponytail: no `[class]` support; extend `glob_matches` if a real repository
/// needs it.
fn pattern_matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.trim_end_matches('/');
    if pattern.contains('/') {
        glob_matches(pattern.trim_start_matches('/'), path)
    } else {
        glob_matches(pattern, path.rsplit('/').next().unwrap_or(path))
    }
}

/// Recursive glob match over bytes with `*`, `?`, and `**` semantics; a
/// trailing `**` matches everything below its directory.
fn glob_matches(pattern: &str, text: &str) -> bool {
    let (p, t) = (pattern.as_bytes(), text.as_bytes());
    fn go(p: &[u8], t: &[u8]) -> bool {
        match p {
            [] => t.is_empty(),
            [b'*', b'*'] => true,
            [b'*', b'*', rest @ ..] => {
                let rest = rest.strip_prefix(b"/").unwrap_or(rest);
                (0..=t.len()).any(|i| (i == 0 || t[i - 1] == b'/') && go(rest, &t[i..]))
            }
            [b'*', rest @ ..] => (0..=t.len())
                .take_while(|&i| i == 0 || t[i - 1] != b'/')
                .any(|i| go(rest, &t[i..])),
            [b'?', rest @ ..] => !t.is_empty() && t[0] != b'/' && go(rest, &t[1..]),
            [c, rest @ ..] => !t.is_empty() && t[0] == *c && go(rest, &t[1..]),
        }
    }
    go(p, t)
}

/// Classify every entry of a tree listing as eligible or excluded.
///
/// `context` answers the path-only language query; `attributes` supplies
/// root `.gitattributes` overrides, where an explicit `false` overrides the
/// segment and minified defaults. Directory entries are dropped silently
/// because they are not blobs.
pub fn classify(
    entries: &[TreeEntry],
    attributes: &RootAttributes,
    context: &SymbolicContext,
    max_file_bytes: u64,
) -> EligibleTree {
    let mut tree = EligibleTree::default();
    for entry in entries {
        match exclusion(entry, attributes, context, max_file_bytes) {
            Ok(true) => tree.eligible.push(entry.clone()),
            Ok(false) => {}
            Err(reason) => tree.excluded.push(Exclusion {
                path: entry.path.clone(),
                reason,
            }),
        }
    }
    tree
}

/// `Ok(true)` when eligible, `Ok(false)` for a directory, `Err` with the reason otherwise.
fn exclusion(
    entry: &TreeEntry,
    attributes: &RootAttributes,
    context: &SymbolicContext,
    max_file_bytes: u64,
) -> Result<bool, ExclusionReason> {
    match entry.mode {
        TreeEntryMode::Directory => return Ok(false),
        TreeEntryMode::Symlink => return Err(ExclusionReason::Symlink),
        TreeEntryMode::Submodule => return Err(ExclusionReason::Submodule),
        TreeEntryMode::File | TreeEntryMode::Executable => {}
    }
    let path = entry.path.as_str();
    if context.path_language(path).ok().flatten().is_none() {
        return Err(ExclusionReason::Unsupported);
    }
    let (generated, vendored) = attributes.resolve(path);
    if generated == Some(true) {
        return Err(ExclusionReason::GeneratedAttribute);
    }
    if vendored == Some(true) {
        return Err(ExclusionReason::VendoredAttribute);
    }
    if vendored.is_none() && path.split('/').any(|seg| EXCLUDED_SEGMENTS.contains(&seg)) {
        return Err(ExclusionReason::VendoredSegment);
    }
    if generated.is_none() && (path.ends_with(".min.js") || path.ends_with(".min.css")) {
        return Err(ExclusionReason::MinifiedOutput);
    }
    if entry.size.is_some_and(|size| size > max_file_bytes) {
        return Err(ExclusionReason::Oversize);
    }
    Ok(true)
}

#[cfg(test)]
/// Owner tests for attribute parsing, glob matching, and entry classification.
mod tests {
    use super::*;
    use crate::identity::ObjectSha;

    /// A tree entry at `path` with `mode` and `size`.
    fn entry(path: &str, mode: TreeEntryMode, size: Option<u64>) -> TreeEntry {
        TreeEntry {
            path: RelativePath::new(path).expect("path"),
            mode,
            sha: ObjectSha::new(&"a".repeat(40)).expect("sha"),
            size,
        }
    }

    /// Basename patterns match anywhere, anchored patterns from the root,
    /// `*` stops at `/`, `**` crosses it, and the last rule wins per attribute.
    #[test]
    fn attributes_resolve_like_git() {
        let attributes = RootAttributes::parse(
            b"# comment\n*.pb.go linguist-generated=true\n/gen/** linguist-generated\ngen/keep.ts -linguist-generated\nsrc/* linguist-vendored=false\nvendor/lib.js linguist-vendored=false diff\n[attr]macro linguist-generated\n",
        );
        assert_eq!(attributes.resolve("a/b/x.pb.go"), (Some(true), None));
        assert_eq!(attributes.resolve("gen/a/b.ts"), (Some(true), None));
        assert_eq!(attributes.resolve("gen/keep.ts"), (Some(false), None));
        assert_eq!(attributes.resolve("src/a.ts"), (None, Some(false)));
        assert_eq!(attributes.resolve("src/a/b.ts"), (None, None));
        assert_eq!(attributes.resolve("vendor/lib.js"), (None, Some(false)));
        assert_eq!(attributes.resolve("other.ts"), (None, None));
        assert!(glob_matches("a?c", "abc"));
        assert!(!glob_matches("a?c", "a/c"));
    }

    /// Every exclusion reason is reported once; attribute `false` overrides
    /// the segment default; directories are dropped without a reason.
    #[test]
    fn classification_reports_each_reason() {
        let context = SymbolicContext::new();
        let attributes = RootAttributes::parse(
            b"generated/* linguist-generated\nvendored/* linguist-vendored\nvendor/keep.ts linguist-vendored=false\n",
        );
        let entries = [
            entry("src/ok.ts", TreeEntryMode::File, Some(10)),
            entry("bin/run.py", TreeEntryMode::Executable, None),
            entry("src", TreeEntryMode::Directory, None),
            entry("link.ts", TreeEntryMode::Symlink, Some(3)),
            entry("sub", TreeEntryMode::Submodule, None),
            entry("README.md", TreeEntryMode::File, Some(1)),
            entry("node_modules/a.ts", TreeEntryMode::File, Some(1)),
            entry("vendor/keep.ts", TreeEntryMode::File, Some(1)),
            entry("app.min.js", TreeEntryMode::File, Some(1)),
            entry("generated/a.ts", TreeEntryMode::File, Some(1)),
            entry("vendored/a.ts", TreeEntryMode::File, Some(1)),
            entry(
                "big.ts",
                TreeEntryMode::File,
                Some(MAX_ELIGIBLE_FILE_BYTES + 1),
            ),
        ];
        let tree = classify(&entries, &attributes, &context, MAX_ELIGIBLE_FILE_BYTES);
        let eligible: Vec<&str> = tree.eligible.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(eligible, ["src/ok.ts", "bin/run.py", "vendor/keep.ts"]);
        let excluded: Vec<(&str, ExclusionReason)> = tree
            .excluded
            .iter()
            .map(|e| (e.path.as_str(), e.reason))
            .collect();
        assert_eq!(
            excluded,
            [
                ("link.ts", ExclusionReason::Symlink),
                ("sub", ExclusionReason::Submodule),
                ("README.md", ExclusionReason::Unsupported),
                ("node_modules/a.ts", ExclusionReason::VendoredSegment),
                ("app.min.js", ExclusionReason::MinifiedOutput),
                ("generated/a.ts", ExclusionReason::GeneratedAttribute),
                ("vendored/a.ts", ExclusionReason::VendoredAttribute),
                ("big.ts", ExclusionReason::Oversize),
            ]
        );
    }
}
