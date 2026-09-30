//! The extracted [`Edge`] value and the closed [`EdgeKind`],
//! [`ReferenceKind`], and [`Provenance`] contracts, mirroring the frozen
//! upstream `Edge` interface, `EDGE_KINDS`, `ReferenceKind`, and
//! `PROVENANCES` in `src/types.ts`.

use serde::{Deserialize, Serialize};

/// The closed edge-kind contract (12 values, upstream `EDGE_KINDS` order).
///
/// The array order is the native-kernel wire contract: kinds cross the
/// buffer boundary as indexes into `EDGE_KINDS`, so this order is append-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Parent contains child (file → class, class → method).
    Contains,
    /// Function/method calls another.
    Calls,
    /// File imports from another.
    Imports,
    /// File exports a symbol.
    Exports,
    /// Class/interface extends another.
    Extends,
    /// Class implements interface.
    Implements,
    /// Generic reference to another symbol.
    References,
    /// Variable/parameter has type.
    #[serde(rename = "type_of")]
    TypeOf,
    /// Function returns type.
    Returns,
    /// Creates instance of class.
    Instantiates,
    /// Method overrides parent method.
    Overrides,
    /// Decorator applied to symbol.
    Decorates,
}

impl EdgeKind {
    /// Upstream wire names in contract (index) order.
    pub const WIRE_NAMES: [&str; 12] = [
        "contains",
        "calls",
        "imports",
        "exports",
        "extends",
        "implements",
        "references",
        "type_of",
        "returns",
        "instantiates",
        "overrides",
        "decorates",
    ];

    /// The wire index of this kind (index into [`EdgeKind::WIRE_NAMES`]).
    pub fn wire_index(self) -> Option<u8> {
        EdgeKind::WIRE_NAMES
            .iter()
            .position(|name| *name == self.wire_name())
            .map(|index| index as u8)
    }

    /// Resolve a wire index to a kind, or `None` when out of range.
    pub fn from_wire_index(index: u8) -> Option<EdgeKind> {
        EdgeKind::ALL.get(index as usize).copied()
    }

    /// The upstream wire name (`"type_of"`, …).
    pub fn wire_name(self) -> &'static str {
        EdgeKind::WIRE_NAMES[self as usize]
    }

    /// All 12 kinds in contract order.
    pub const ALL: [EdgeKind; 12] = [
        EdgeKind::Contains,
        EdgeKind::Calls,
        EdgeKind::Imports,
        EdgeKind::Exports,
        EdgeKind::Extends,
        EdgeKind::Implements,
        EdgeKind::References,
        EdgeKind::TypeOf,
        EdgeKind::Returns,
        EdgeKind::Instantiates,
        EdgeKind::Overrides,
        EdgeKind::Decorates,
    ];
}

/// The kind an unresolved reference can carry. `FunctionRef` is
/// internal-only — a function name used as a value (callback registration);
/// resolution maps it to a `references` edge targeting function/method nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceKind {
    /// One of the twelve edge kinds.
    Edge(EdgeKind),
    /// Internal-only `function_ref` (wire code 200).
    FunctionRef,
}

impl ReferenceKind {
    /// The wire byte for this kind: `EDGE_KINDS` index or the internal
    /// `function_ref` code (200).
    ///
    /// # Panics
    ///
    /// Panics if this closed edge kind is missing from [`EDGE_KINDS`], which
    /// would indicate internal registry drift.
    pub fn wire_byte(self) -> u8 {
        match self {
            ReferenceKind::Edge(kind) => kind.wire_index().expect("edge kind has a wire index"),
            ReferenceKind::FunctionRef => 200,
        }
    }

    /// Resolve a wire byte (including the 200 function-ref code) to a kind.
    pub fn from_wire_byte(byte: u8) -> Option<ReferenceKind> {
        match byte {
            200 => Some(ReferenceKind::FunctionRef),
            _ => EdgeKind::from_wire_index(byte).map(ReferenceKind::Edge),
        }
    }

    /// The upstream wire name (`"function_ref"` or the edge-kind name).
    pub fn wire_name(self) -> &'static str {
        match self {
            ReferenceKind::Edge(kind) => kind.wire_name(),
            ReferenceKind::FunctionRef => "function_ref",
        }
    }

    /// Resolve an upstream wire name into a kind.
    pub fn from_wire_name(name: &str) -> Option<ReferenceKind> {
        match name {
            "function_ref" => Some(ReferenceKind::FunctionRef),
            other => EdgeKind::WIRE_NAMES
                .iter()
                .position(|candidate| *candidate == other)
                .and_then(|index| EdgeKind::from_wire_index(index as u8))
                .map(ReferenceKind::Edge),
        }
    }
}

impl Serialize for ReferenceKind {
    /// Implements the standard serialize boundary using the canonical wire spelling and validation rules.
    ///
    /// # Errors
    ///
    /// Returns the serializer’s error if it cannot write the canonical edge-kind string.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.wire_name())
    }
}

impl<'de> Deserialize<'de> for ReferenceKind {
    /// Implements the standard deserialize boundary using the canonical wire spelling and validation rules.
    ///
    /// # Errors
    ///
    /// Returns a deserializer error when the input is not a string or names no
    /// variant in the closed edge-kind registry.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        ReferenceKind::from_wire_name(&name)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown reference kind `{name}`")))
    }
}

/// Edge provenance byte contract: 0 = absent, 1 = tree-sitter, 2 = scip,
/// 3 = heuristic (upstream `PROVENANCES`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provenance {
    /// Tree-sitter derived.
    #[serde(rename = "tree-sitter")]
    TreeSitter,
    /// SCIP derived.
    Scip,
    /// Heuristic.
    Heuristic,
}

impl Provenance {
    /// The wire byte for this provenance (1–3).
    pub fn wire_byte(self) -> u8 {
        match self {
            Provenance::TreeSitter => 1,
            Provenance::Scip => 2,
            Provenance::Heuristic => 3,
        }
    }

    /// Resolve a wire byte (1–3) to a provenance; 0 (absent) is `None`.
    pub fn from_wire_byte(byte: u8) -> Option<Provenance> {
        match byte {
            1 => Some(Provenance::TreeSitter),
            2 => Some(Provenance::Scip),
            3 => Some(Provenance::Heuristic),
            _ => None,
        }
    }
}

/// One relationship between two extracted nodes, mirroring the upstream
/// `Edge` interface. Coordinates (`line`/`column`) are optional and absent
/// when the walker did not record a call-site location.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Edge {
    /// Source node id.
    pub source: String,
    /// Target node id.
    pub target: String,
    /// Relationship kind.
    pub kind: EdgeKind,
    /// Additional context about the relationship.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Line where the relationship occurs (e.g. call site).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Column where the relationship occurs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// How this edge was created.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Pins every edge-kind wire name to the frozen edge registry.
    ///
    /// # Panics
    ///
    /// Panics if an edge-kind lookup or wire-byte conversion fails for a supposedly closed registry entry.
    fn wire_names_match_upstream_edge_kinds() {
        for (kind, name) in EdgeKind::ALL.iter().zip(EdgeKind::WIRE_NAMES.iter()) {
            assert_eq!(kind.wire_name(), *name);
            assert_eq!(
                EdgeKind::from_wire_index(kind.wire_index().unwrap()),
                Some(*kind)
            );
        }
        assert_eq!(EdgeKind::ALL.len(), 12);
    }

    #[test]
    /// Pins the internal function-reference discriminator outside the edge-kind range.
    ///
    /// # Panics
    ///
    /// Panics if the reserved function-reference byte unexpectedly maps to an ordinary edge kind.
    fn function_ref_wire_code() {
        assert_eq!(ReferenceKind::FunctionRef.wire_byte(), 200);
        assert_eq!(
            ReferenceKind::from_wire_byte(200),
            Some(ReferenceKind::FunctionRef)
        );
        assert_eq!(
            ReferenceKind::Edge(EdgeKind::Calls).wire_byte(),
            EdgeKind::Calls.wire_index().unwrap()
        );
        assert_eq!(ReferenceKind::from_wire_byte(255), None);
        assert_eq!(
            serde_json::to_string(&ReferenceKind::FunctionRef).unwrap(),
            "\"function_ref\""
        );
    }

    #[test]
    /// Round-trips every provenance value through its ABI byte and absent sentinel.
    ///
    /// # Panics
    ///
    /// Panics if a closed provenance value cannot round-trip through its ABI byte.
    fn provenance_wire_round_trip() {
        assert_eq!(Provenance::from_wire_byte(0), None);
        for provenance in [
            Provenance::TreeSitter,
            Provenance::Scip,
            Provenance::Heuristic,
        ] {
            assert_eq!(
                Provenance::from_wire_byte(provenance.wire_byte()),
                Some(provenance)
            );
        }
        assert_eq!(
            serde_json::to_string(&Provenance::TreeSitter).unwrap(),
            "\"tree-sitter\""
        );
    }

    #[test]
    /// Pins serialized enum and field names to the frozen upstream JSON contract.
    ///
    /// # Panics
    ///
    /// Panics if JSON serialization fails or produces a spelling outside the asserted wire contract.
    fn serde_uses_upstream_names() {
        let edge = Edge {
            source: "file:a.ts".into(),
            target: "function:abc".into(),
            kind: EdgeKind::Calls,
            metadata: Some(serde_json::json!({"valueRef": true})),
            line: Some(4),
            column: Some(9),
            provenance: None,
        };
        let wire = serde_json::to_value(&edge).unwrap();
        assert_eq!(wire["source"], "file:a.ts");
        assert_eq!(wire["kind"], "calls");
        assert_eq!(wire["line"], 4);
        assert_eq!(wire["metadata"]["valueRef"], true);
        assert!(wire.get("provenance").is_none());
        let back: Edge = serde_json::from_value(wire).unwrap();
        assert_eq!(back, edge);
    }

    #[test]
    /// Pins the `type_of` edge spelling that differs from its Rust variant name.
    ///
    /// # Panics
    ///
    /// Panics if serialization fails or the `type_of` edge spelling changes.
    fn type_of_wire_name() {
        assert_eq!(EdgeKind::TypeOf.wire_name(), "type_of");
        assert_eq!(
            serde_json::to_string(&EdgeKind::TypeOf).unwrap(),
            "\"type_of\""
        );
    }
}
