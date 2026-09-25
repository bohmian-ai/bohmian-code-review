//! [`RelativePath`] — a validated repository-relative UTF-8 path.

use crate::limits::MAX_RELATIVE_PATH_BYTES;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A repository-relative source path in canonical forward-slash form.
///
/// Upstream paths are relative to the project root and normalized to
/// forward slashes (`normalizePath` in the frozen `src/utils.ts`); this
/// newtype validates that shape so identity-forming code never hashes an
/// absolute, escaping, or redundantly spelled path. Construction rejects
/// empty, absolute (leading `/`, Windows drive, or UNC),
/// backslash-containing, NUL-bearing, noncanonical (`.`, `..`, or empty
/// segment), and over-long paths; the byte ceiling is the locked persisted
/// limit [`MAX_RELATIVE_PATH_BYTES`].
///
/// Rejecting rather than normalizing is deliberate: identity, persistence,
/// and source access all construct through this one owner, so a file cannot
/// acquire two durable identities by being spelled two ways.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RelativePath(
    /// Validated UTF-8 path using repository-relative forward-slash syntax.
    String,
);

impl RelativePath {
    /// Validate and construct a repository-relative path.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidRelativePath`] when the path is empty, absolute
    /// (leading `/`, Windows drive, or UNC), contains a backslash or NUL,
    /// carries a `.`, `..`, or empty segment, or exceeds
    /// [`MAX_RELATIVE_PATH_BYTES`] bytes.
    pub fn new(raw: &str) -> Result<RelativePath, InvalidRelativePath> {
        if raw.is_empty() {
            return Err(InvalidRelativePath::Empty);
        }
        if raw.len() > MAX_RELATIVE_PATH_BYTES {
            return Err(InvalidRelativePath::TooLong { bytes: raw.len() });
        }
        if raw.starts_with('/') {
            return Err(InvalidRelativePath::Absolute);
        }
        if raw.starts_with('\\') {
            return Err(InvalidRelativePath::Absolute);
        }
        if is_windows_drive_prefix(raw) {
            return Err(InvalidRelativePath::Absolute);
        }
        if raw.contains('\0') {
            return Err(InvalidRelativePath::Nul);
        }
        if raw.contains('\\') {
            return Err(InvalidRelativePath::Backslash);
        }
        if raw
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        {
            return Err(InvalidRelativePath::NoncanonicalSegment);
        }
        Ok(RelativePath(raw.to_string()))
    }

    /// The validated path as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Detect a Windows drive or UNC prefix (`C:`, `//server`).
pub(crate) fn is_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes.get(2).is_none_or(|b| *b == b'/' || *b == b'\\'))
}

impl<'de> Deserialize<'de> for RelativePath {
    /// Deserializes through [`RelativePath::new`] so serde cannot mint a
    /// value the constructor would refuse.
    ///
    /// The wire shape stays a transparent string, so every canonical path
    /// round-trips byte-for-byte; only the invariant is added.
    ///
    /// # Errors
    ///
    /// Returns a serde error carrying the [`InvalidRelativePath`] reason when
    /// the serialized string is not a canonical repository-relative path.
    fn deserialize<D>(deserializer: D) -> Result<RelativePath, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        RelativePath::new(&raw).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for RelativePath {
    /// Implements the standard fmt boundary using the canonical wire spelling and validation rules.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Why a [`RelativePath`] construction failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InvalidRelativePath {
    /// The path was empty.
    #[error("relative path must not be empty")]
    Empty,
    /// The path was absolute (leading `/`, drive, or UNC prefix).
    #[error("relative path must not be absolute")]
    Absolute,
    /// The path exceeded [`MAX_RELATIVE_PATH_BYTES`].
    #[error("relative path exceeds {MAX_RELATIVE_PATH_BYTES} bytes (got {bytes})")]
    TooLong {
        /// Observed byte length.
        bytes: usize,
    },
    /// The path contained a NUL byte.
    #[error("relative path must not contain NUL")]
    Nul,
    /// The path contained a backslash (canonical form uses forward slashes).
    #[error("relative path must use forward slashes")]
    Backslash,
    /// The path contained a `.`, `..`, or empty segment.
    ///
    /// Canonical form names each file exactly once, so a spelling that walks
    /// through the current or parent directory — or repeats a separator — is
    /// rejected rather than normalized away.
    #[error("relative path must not contain `.`, `..`, or empty segments")]
    NoncanonicalSegment,
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Covers normalized repository-relative paths accepted at the source boundary.
    ///
    /// # Panics
    ///
    /// Panics if a path in the accepted fixture set is rejected.
    fn valid_paths_are_accepted() {
        for path in ["src/a.ts", "a", "nested/deep/file.rs", "a..b/c.rs"] {
            assert_eq!(RelativePath::new(path).unwrap().as_str(), path);
        }
    }

    #[test]
    /// Rejects every noncanonical segment spelling of an otherwise legal path.
    ///
    /// Parent, current-directory, repeated-separator, and trailing-separator
    /// spellings all denote a path the canonical form already spells another
    /// way; admitting them would let one file take several durable identities.
    ///
    /// # Panics
    ///
    /// Panics if any noncanonical spelling in the fixture set is accepted or
    /// is rejected for the wrong reason.
    fn noncanonical_segments_are_rejected() {
        for path in [
            "src/../lib/x.py",
            "..",
            "../a",
            "a/..",
            ".",
            "./a",
            "a/./b",
            "a//b",
            "a/",
            "/",
        ] {
            assert_eq!(
                RelativePath::new(path).err(),
                Some(match path {
                    "/" => InvalidRelativePath::Absolute,
                    _ => InvalidRelativePath::NoncanonicalSegment,
                }),
                "path {path} must be rejected"
            );
        }
    }

    #[test]
    /// Rejects absolute, empty, parent-escaping, and non-normalized repository paths.
    fn invalid_paths_are_rejected() {
        assert_eq!(RelativePath::new(""), Err(InvalidRelativePath::Empty));
        assert_eq!(
            RelativePath::new("/abs"),
            Err(InvalidRelativePath::Absolute)
        );
        assert_eq!(
            RelativePath::new("\\abs"),
            Err(InvalidRelativePath::Absolute)
        );
        assert_eq!(
            RelativePath::new("C:/x"),
            Err(InvalidRelativePath::Absolute)
        );
        assert_eq!(
            RelativePath::new("C:\\x"),
            Err(InvalidRelativePath::Absolute)
        );
        assert_eq!(
            RelativePath::new("a\\b"),
            Err(InvalidRelativePath::Backslash)
        );
        assert_eq!(RelativePath::new("a\0b"), Err(InvalidRelativePath::Nul));
        let long = "a".repeat(MAX_RELATIVE_PATH_BYTES + 1);
        assert!(matches!(
            RelativePath::new(&long),
            Err(InvalidRelativePath::TooLong { .. })
        ));
    }

    #[test]
    /// Pins newtype serialization to its validated inner string without wrapper fields.
    ///
    /// # Panics
    ///
    /// Panics if serialization fails or wraps the validated newtype instead of emitting its string.
    fn serde_is_transparent() {
        let path = RelativePath::new("src/a.ts").unwrap();
        assert_eq!(serde_json::to_string(&path).unwrap(), "\"src/a.ts\"");
        let back: RelativePath = serde_json::from_str("\"src/a.ts\"").unwrap();
        assert_eq!(back, path);
    }

    #[test]
    /// Serde input cannot bypass the canonical-path constructor.
    ///
    /// Every invalid class rejected by [`RelativePath::new`] must also be
    /// rejected when it arrives as serialized JSON, while a canonical path
    /// round-trips byte-for-byte.
    ///
    /// # Panics
    ///
    /// Panics if an invalid serialized path deserializes, or if a canonical
    /// path fails to round-trip.
    fn deserialization_enforces_the_canonical_invariant() {
        for raw in [
            "../escape.rs",
            "a//b",
            "/absolute.rs",
            "C:/windows.rs",
            "a\\b",
            "",
            &"a".repeat(MAX_RELATIVE_PATH_BYTES + 1),
        ] {
            let json = serde_json::to_string(raw).expect("string encodes");
            assert!(
                serde_json::from_str::<RelativePath>(&json).is_err(),
                "serialized `{raw}` must not deserialize"
            );
        }
        let canonical = RelativePath::new("src/a.ts").expect("canonical path");
        let json = serde_json::to_string(&canonical).expect("path encodes");
        assert_eq!(json, "\"src/a.ts\"");
        assert_eq!(
            serde_json::from_str::<RelativePath>(&json).expect("path decodes"),
            canonical
        );
    }
}
