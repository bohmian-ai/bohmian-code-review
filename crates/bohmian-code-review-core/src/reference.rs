//! The extracted [`RawReference`] value — an unresolved reference emitted by
//! a walker, mirroring the upstream `UnresolvedReference` interface.

use crate::edge::ReferenceKind;
use crate::language::Language;
use serde::{Deserialize, Serialize};

/// A reference that could not be resolved during extraction.
///
/// The kernel wire format carries `fromNodeId`, `referenceName`,
/// `referenceKind`, `line`, `column`, and an optional NUL-joined candidates
/// list; the optional `filePath`/`language` denormalized fields follow the
/// upstream contract (the native walkers set `file_path` only for the
/// flagged mixin/trait `implements` refs of ruby/php, matching the frozen
/// kernel's `REF_FLAG_FILE_PATH` semantics).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawReference {
    /// ID of the node containing the reference.
    pub from_node_id: String,
    /// Name being referenced.
    pub reference_name: String,
    /// Kind of reference (call, type, import, function value, …).
    pub reference_kind: ReferenceKind,
    /// Line of the reference (1-based).
    pub line: u32,
    /// Column of the reference (0-based).
    pub column: u32,
    /// File path where the reference occurs (denormalized; only the
    /// ruby/php flagged `implements` refs carry it from the walkers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    /// Language of the source file (denormalized; never emitted by the
    /// native kernel, preserved for upstream contract fidelity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
    /// Possible qualified names it might resolve to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidates: Option<Vec<String>>,
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins serialized enum and field names to the frozen upstream JSON contract.
    ///
    /// # Panics
    ///
    /// Panics if JSON serialization fails or produces a spelling outside the asserted wire contract.
    fn serde_uses_upstream_names() {
        let reference = RawReference {
            from_node_id: "method:abc".into(),
            reference_name: "helper".into(),
            reference_kind: ReferenceKind::Edge(crate::edge::EdgeKind::Calls),
            line: 7,
            column: 3,
            file_path: None,
            language: None,
            candidates: Some(vec!["a.helper".into(), "b.helper".into()]),
        };
        let wire = serde_json::to_value(&reference).unwrap();
        assert_eq!(wire["fromNodeId"], "method:abc");
        assert_eq!(wire["referenceName"], "helper");
        assert_eq!(wire["referenceKind"], "calls");
        assert_eq!(wire["candidates"][0], "a.helper");
        assert!(wire.get("filePath").is_none());
        assert!(wire.get("language").is_none());
        let back: RawReference = serde_json::from_value(wire).unwrap();
        assert_eq!(back, reference);
    }

    #[test]
    /// Shows optional reference path, language, and candidate fields are omitted or serialized exactly as populated.
    ///
    /// # Panics
    ///
    /// Panics if reference serialization fails or omits a populated optional field.
    fn optional_denormalized_fields_serialize_when_present() {
        let reference = RawReference {
            from_node_id: "file:x.rb".into(),
            reference_name: "Mixin".into(),
            reference_kind: ReferenceKind::Edge(crate::edge::EdgeKind::Implements),
            line: 1,
            column: 0,
            file_path: Some("x.rb".into()),
            language: Some(Language::Ruby),
            candidates: None,
        };
        let wire = serde_json::to_value(&reference).unwrap();
        assert_eq!(wire["filePath"], "x.rb");
        assert_eq!(wire["language"], "ruby");
    }
}
