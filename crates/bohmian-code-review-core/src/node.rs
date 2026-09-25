//! The extracted [`Node`] value and its closed [`NodeKind`]/[`Visibility`]
//! contracts, mirroring the frozen upstream `Node` interface and
//! `NODE_KINDS` array in `src/types.ts`.

use crate::language::Language;
use serde::{Deserialize, Serialize};

/// The closed node-kind contract (22 values, upstream `NODE_KINDS` order).
///
/// The array order is the native-kernel wire contract: kinds cross the
/// buffer boundary as indexes into `NODE_KINDS`, so this order is append-only
/// (see `bohmian-code-review-extract::buffers`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    /// File.
    File,
    /// Module.
    Module,
    /// Class.
    Class,
    /// Struct.
    Struct,
    /// Interface.
    Interface,
    /// Trait.
    Trait,
    /// Protocol.
    Protocol,
    /// Function.
    Function,
    /// Method.
    Method,
    /// Property.
    Property,
    /// Field.
    Field,
    /// Variable.
    Variable,
    /// Constant.
    Constant,
    /// Enum.
    Enum,
    /// Enum member.
    #[serde(rename = "enum_member")]
    EnumMember,
    /// Type alias.
    #[serde(rename = "type_alias")]
    TypeAlias,
    /// Namespace.
    Namespace,
    /// Parameter.
    Parameter,
    /// Import.
    Import,
    /// Export.
    Export,
    /// Route.
    Route,
    /// Component.
    Component,
}

impl NodeKind {
    /// Upstream wire names in contract (index) order.
    pub const WIRE_NAMES: [&str; 22] = [
        "file",
        "module",
        "class",
        "struct",
        "interface",
        "trait",
        "protocol",
        "function",
        "method",
        "property",
        "field",
        "variable",
        "constant",
        "enum",
        "enum_member",
        "type_alias",
        "namespace",
        "parameter",
        "import",
        "export",
        "route",
        "component",
    ];

    /// The wire index of this kind (index into [`NodeKind::WIRE_NAMES`]).
    pub fn wire_index(self) -> Option<u8> {
        NodeKind::WIRE_NAMES
            .iter()
            .position(|name| *name == self.wire_name())
            .map(|index| index as u8)
    }

    /// Resolve a wire index to a kind, or `None` when out of range.
    pub fn from_wire_index(index: u8) -> Option<NodeKind> {
        NodeKind::ALL.get(index as usize).copied()
    }

    /// The upstream wire name (`"enum_member"`, `"type_alias"`, …).
    pub fn wire_name(self) -> &'static str {
        NodeKind::WIRE_NAMES[self as usize]
    }

    /// All 22 kinds in contract order.
    pub const ALL: [NodeKind; 22] = [
        NodeKind::File,
        NodeKind::Module,
        NodeKind::Class,
        NodeKind::Struct,
        NodeKind::Interface,
        NodeKind::Trait,
        NodeKind::Protocol,
        NodeKind::Function,
        NodeKind::Method,
        NodeKind::Property,
        NodeKind::Field,
        NodeKind::Variable,
        NodeKind::Constant,
        NodeKind::Enum,
        NodeKind::EnumMember,
        NodeKind::TypeAlias,
        NodeKind::Namespace,
        NodeKind::Parameter,
        NodeKind::Import,
        NodeKind::Export,
        NodeKind::Route,
        NodeKind::Component,
    ];
}

/// Visibility modifier byte contract: 0 = absent, 1 = public, 2 = private,
/// 3 = protected, 4 = internal (upstream `VISIBILITIES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Public.
    Public,
    /// Private.
    Private,
    /// Protected.
    Protected,
    /// Internal.
    Internal,
}

impl Visibility {
    /// The wire byte for this visibility (1–4).
    pub fn wire_byte(self) -> u8 {
        match self {
            Visibility::Public => 1,
            Visibility::Private => 2,
            Visibility::Protected => 3,
            Visibility::Internal => 4,
        }
    }

    /// Resolve a wire byte (1–4) to a visibility; 0 (absent) is `None`.
    pub fn from_wire_byte(byte: u8) -> Option<Visibility> {
        match byte {
            1 => Some(Visibility::Public),
            2 => Some(Visibility::Private),
            3 => Some(Visibility::Protected),
            4 => Some(Visibility::Internal),
            _ => None,
        }
    }
}

impl Node {
    /// Encodes resolver-visible node fields into the canonical PostgreSQL
    /// `nodes.metadata` object. Existing `extra` object members are retained,
    /// while typed fields win for their reserved upstream keys so reloads
    /// cannot silently lose matcher inputs.
    #[must_use]
    pub fn canonical_metadata(&self) -> Option<serde_json::Value> {
        let mut object = match self.extra.clone() {
            Some(serde_json::Value::Object(value)) => value,
            Some(extra) => {
                let mut object = serde_json::Map::new();
                object.insert("extra".to_owned(), extra);
                object
            }
            None => serde_json::Map::new(),
        };
        macro_rules! insert {
            ($key:literal, $value:expr) => {
                if let Some(value) = $value {
                    object.insert($key.to_owned(), serde_json::json!(value));
                }
            };
        }
        insert!("docstring", self.docstring.as_deref());
        insert!("signature", self.signature.as_deref());
        insert!(
            "visibility",
            self.visibility.map(|value| match value {
                Visibility::Public => "public",
                Visibility::Private => "private",
                Visibility::Protected => "protected",
                Visibility::Internal => "internal",
            })
        );
        insert!("isExported", self.is_exported);
        insert!("isAsync", self.is_async);
        insert!("isStatic", self.is_static);
        insert!("isAbstract", self.is_abstract);
        insert!("decorators", self.decorators.as_ref());
        insert!("typeParameters", self.type_parameters.as_ref());
        insert!("returnType", self.return_type.as_deref());
        (!object.is_empty()).then_some(serde_json::Value::Object(object))
    }
}

/// One extracted symbol node, mirroring the upstream `Node` interface.
///
/// `file_path` is the repository-relative path as given at extraction time;
/// node identity (`id`) is upstream-compatible and formed by the extracting
/// walkers. Coordinates are upstream semantics: lines are 1-based, columns
/// 0-based (UTF-16 code units where the upstream walker computes them).
/// `extra` carries the wire escape-hatch `extraJson` payload verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    /// Upstream-compatible node id (`"kind:hash32"` or `"file:<path>"`).
    pub id: String,
    /// Node kind.
    pub kind: NodeKind,
    /// Simple name (e.g. `"calculateTotal"`).
    pub name: String,
    /// Fully qualified name.
    pub qualified_name: String,
    /// File path relative to the project root.
    pub file_path: String,
    /// Source language.
    pub language: Language,
    /// Starting line (1-based).
    pub start_line: u32,
    /// Ending line (1-based).
    pub end_line: u32,
    /// Starting column (0-based).
    pub start_column: u32,
    /// Ending column (0-based).
    pub end_column: u32,
    /// Documentation string when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docstring: Option<String>,
    /// Function/method signature when captured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Visibility modifier when declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<Visibility>,
    /// Whether the symbol is exported.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_exported: Option<bool>,
    /// Whether the symbol is async.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_async: Option<bool>,
    /// Whether the symbol is static.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_static: Option<bool>,
    /// Whether the symbol is abstract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_abstract: Option<bool>,
    /// Decorators/annotations when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decorators: Option<Vec<String>>,
    /// Generic type parameters when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_parameters: Option<Vec<String>>,
    /// Normalized return/result type name when captured.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    /// Raw wire `extraJson` payload (merged onto the node by the upstream
    /// decoder). No walker currently emits it; kept for contract fidelity.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins every node-kind wire name to the frozen node registry.
    ///
    /// # Panics
    ///
    /// Panics if a node-kind lookup or wire-byte conversion fails for a supposedly closed registry entry.
    fn wire_names_match_upstream_node_kinds() {
        for (kind, name) in NodeKind::ALL.iter().zip(NodeKind::WIRE_NAMES.iter()) {
            assert_eq!(kind.wire_name(), *name);
            assert_eq!(
                NodeKind::from_wire_index(kind.wire_index().unwrap()),
                Some(*kind)
            );
        }
        assert_eq!(NodeKind::ALL.len(), 22);
    }

    #[test]
    /// Round-trips every closed node kind through its ABI byte without loss.
    fn wire_byte_round_trip() {
        assert_eq!(Visibility::from_wire_byte(0), None);
        for visibility in [
            Visibility::Public,
            Visibility::Private,
            Visibility::Protected,
            Visibility::Internal,
        ] {
            assert_eq!(
                Visibility::from_wire_byte(visibility.wire_byte()),
                Some(visibility)
            );
        }
        assert_eq!(Visibility::from_wire_byte(9), None);
    }

    #[test]
    /// Pins serialized enum and field names to the frozen upstream JSON contract.
    ///
    /// # Panics
    ///
    /// Panics if JSON serialization fails or produces a spelling outside the asserted wire contract.
    fn serde_uses_upstream_names() {
        let kind = NodeKind::EnumMember;
        assert_eq!(serde_json::to_string(&kind).unwrap(), "\"enum_member\"");
        let node = Node {
            id: "file:a.ts".into(),
            kind: NodeKind::File,
            name: "a.ts".into(),
            qualified_name: "a.ts".into(),
            file_path: "a.ts".into(),
            language: Language::Typescript,
            start_line: 1,
            end_line: 3,
            start_column: 0,
            end_column: 2,
            docstring: None,
            signature: None,
            visibility: None,
            is_exported: Some(false),
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: None,
            extra: None,
        };
        let wire = serde_json::to_value(&node).unwrap();
        assert_eq!(wire["filePath"], "a.ts");
        assert_eq!(wire["qualifiedName"], "a.ts");
        assert_eq!(wire["startLine"], 1);
        assert_eq!(wire["isExported"], false);
        assert_eq!(wire["kind"], "file");
        assert!(
            wire.get("updatedAt").is_none(),
            "timestamps are not kernel facts"
        );
        let back: Node = serde_json::from_value(wire).unwrap();
        assert_eq!(back, node);
    }

    #[test]
    /// Preserves every resolver-visible field in the PostgreSQL metadata contract.
    ///
    /// # Panics
    ///
    /// Panics if the canonical metadata object is unexpectedly absent.
    fn canonical_metadata_contains_all_projection_fields() {
        let node = Node {
            id: "method:1".into(),
            kind: NodeKind::Method,
            name: "run".into(),
            qualified_name: "Widget.run".into(),
            file_path: "src/widget.ts".into(),
            language: Language::Typescript,
            start_line: 1,
            end_line: 3,
            start_column: 0,
            end_column: 1,
            docstring: Some("docs".into()),
            signature: Some("run(input: string): Promise<void>".into()),
            visibility: Some(Visibility::Protected),
            is_exported: Some(true),
            is_async: Some(true),
            is_static: Some(false),
            is_abstract: Some(false),
            decorators: Some(vec!["Get".into()]),
            type_parameters: Some(vec!["T".into()]),
            return_type: Some("Promise<void>".into()),
            extra: Some(serde_json::json!({"framework":"nest"})),
        };
        let metadata = node.canonical_metadata().expect("metadata");
        assert_eq!(metadata["docstring"], "docs");
        assert_eq!(metadata["signature"], "run(input: string): Promise<void>");
        assert_eq!(metadata["visibility"], "protected");
        assert_eq!(metadata["isExported"], true);
        assert_eq!(metadata["isAsync"], true);
        assert_eq!(metadata["isStatic"], false);
        assert_eq!(metadata["isAbstract"], false);
        assert_eq!(metadata["decorators"], serde_json::json!(["Get"]));
        assert_eq!(metadata["typeParameters"], serde_json::json!(["T"]));
        assert_eq!(metadata["returnType"], "Promise<void>");
        assert_eq!(metadata["framework"], "nest");
    }

    #[test]
    /// Rejects node-kind ordinals outside the closed ABI registry.
    fn unknown_kind_index_is_rejected() {
        assert_eq!(NodeKind::from_wire_index(22), None);
        assert_eq!(NodeKind::from_wire_index(255), None);
    }
}
