//! Encode native facts back into the flat-buffer tables.
//!
//! The reverse of [`decode::decode_buffers`], used by the development NAPI
//! adapter to re-serialize [`ExtractionResult`] into the upstream wire
//! tables the TS loader decodes. Arena layout may differ from the walkers'
//! original emission (only the decoded facts matter), but every field,
//! optionality, and kind mapping round-trips exactly.

use crate::buffers::{
    Arena, BoolFlags, EdgeRow, EmitOut, NONE, NONE_STR, NodeRow, REF_FLAG_FILE_PATH, RefRow,
    StrRef, Tables, build_meta,
};
use crate::error::ExtractionError;
use crate::result::ExtractionResult;
use bohmian_code_review_core::Visibility;
use std::collections::HashMap;

/// Encode native facts into the wire tables for one file.
///
/// Node rows are emitted in `result.nodes` order; an `id → row index` map
/// resolves edge/ref endpoints so rows reference each other exactly like the
/// walkers' emission (falling back to id strings when a node id is absent).
/// Diagnostics serialize into the arena errors block; an empty diagnostic
/// list writes no errors block (offset [`NONE`]).
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] (reused as the codec error) when a
/// node/edge kind has no wire index, or when a serialization step fails.
pub fn encode_extraction_result(
    result: &ExtractionResult,
    file_path: &str,
) -> Result<EmitOut, ExtractionError> {
    let mut arena = Arena::default();
    let mut tables = Tables::default();

    let row_index: HashMap<&str, u32> = result
        .nodes
        .iter()
        .enumerate()
        .map(|(index, node)| (node.id.as_str(), index as u32))
        .collect();

    for node in &result.nodes {
        let mut flags = BoolFlags::default();
        set_flag(&mut flags, 0, node.is_exported);
        set_flag(&mut flags, 1, node.is_async);
        set_flag(&mut flags, 2, node.is_static);
        set_flag(&mut flags, 3, node.is_abstract);
        let kind = node
            .kind
            .wire_index()
            .ok_or_else(|| ExtractionError::Decode {
                message: format!("node kind `{:?}` has no wire index", node.kind),
            })?;
        let visibility = node.visibility.map_or(0, Visibility::wire_byte);
        let extra_json = node
            .extra
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| ExtractionError::Decode {
                message: format!("extra JSON serialization failed: {error}"),
            })?;
        tables.push_node(&NodeRow {
            kind,
            visibility,
            flags,
            start_line: node.start_line,
            end_line: node.end_line,
            start_column: node.start_column,
            end_column: node.end_column,
            name: arena.put(&node.name),
            qualified_name: arena.put(&node.qualified_name),
            id: arena.put(&node.id),
            docstring: opt_str(&mut arena, node.docstring.as_deref()),
            signature: opt_str(&mut arena, node.signature.as_deref()),
            decorators: opt_list(&mut arena, node.decorators.as_deref()),
            type_parameters: opt_list(&mut arena, node.type_parameters.as_deref()),
            return_type: opt_str(&mut arena, node.return_type.as_deref()),
            extra_json: opt_str(&mut arena, extra_json.as_deref()),
        });
    }

    for edge in &result.edges {
        let kind = edge
            .kind
            .wire_index()
            .ok_or_else(|| ExtractionError::Decode {
                message: format!("edge kind `{:?}` has no wire index", edge.kind),
            })?;
        let (source_idx, source_id_str) = endpoint(
            row_index.get(edge.source.as_str()),
            &mut arena,
            &edge.source,
        );
        let (target_idx, target_id_str) = endpoint(
            row_index.get(edge.target.as_str()),
            &mut arena,
            &edge.target,
        );
        let metadata_json = edge
            .metadata
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| ExtractionError::Decode {
                message: format!("metadata JSON serialization failed: {error}"),
            })?;
        tables.push_edge(&EdgeRow {
            source_idx,
            target_idx,
            kind,
            provenance: edge
                .provenance
                .map_or(0, bohmian_code_review_core::Provenance::wire_byte),
            line: edge.line.unwrap_or(NONE),
            column: edge.column.unwrap_or(NONE),
            metadata_json: opt_str(&mut arena, metadata_json.as_deref()),
            source_id_str,
            target_id_str,
        });
    }

    for reference in &result.references {
        let kind = reference.reference_kind.wire_byte();
        let flags = if reference.file_path.as_deref() == Some(file_path) {
            REF_FLAG_FILE_PATH
        } else {
            0
        };
        let (from_idx, from_id_str) = endpoint(
            row_index.get(reference.from_node_id.as_str()),
            &mut arena,
            &reference.from_node_id,
        );
        tables.push_ref_flagged(
            &RefRow {
                from_idx,
                kind,
                line: reference.line,
                column: reference.column,
                reference_name: arena.put(&reference.reference_name),
                candidates: opt_list(&mut arena, reference.candidates.as_deref()),
                from_id_str,
            },
            flags,
        );
    }

    let errors_ref = if result.diagnostics.is_empty() {
        NONE_STR
    } else {
        let json =
            serde_json::to_vec(&result.diagnostics).map_err(|error| ExtractionError::Decode {
                message: format!("diagnostics JSON serialization failed: {error}"),
            })?;
        let json = String::from_utf8(json).map_err(|error| ExtractionError::Decode {
            message: format!("diagnostics JSON is not UTF-8: {error}"),
        })?;
        arena.put(&json)
    };

    let meta = build_meta(&tables, arena.len(), errors_ref, 0.0);
    Ok(EmitOut {
        meta,
        nodes: tables.nodes,
        edges: tables.edges,
        refs: tables.refs,
        arena: arena.into_vec(),
    })
}

/// Set one `(present, value)` flag pair from an optional bool; `None` leaves
/// the pair unset so decode reads it as absent.
fn set_flag(flags: &mut BoolFlags, pair: u16, value: Option<bool>) {
    if let Some(value) = value {
        flags.set(pair, value);
    }
}

/// Resolve an endpoint: a row index when the node is in the table, else the
/// id string slot with a [`NONE`] index.
fn endpoint(row: Option<&u32>, arena: &mut Arena, id: &str) -> (u32, StrRef) {
    match row {
        Some(index) => (*index, NONE_STR),
        None => (NONE, arena.put(id)),
    }
}

/// Put an optional string; absent → [`NONE_STR`].
fn opt_str(arena: &mut Arena, value: Option<&str>) -> StrRef {
    match value {
        Some(value) => arena.put(value),
        None => NONE_STR,
    }
}

/// Put an optional NUL-joined list; empty or absent → [`NONE_STR`] (matching
/// the walkers' `put_list` semantics).
fn opt_list(arena: &mut Arena, items: Option<&[String]>) -> StrRef {
    match items {
        Some(items) if !items.is_empty() => {
            let joined = items.join("\0");
            arena.put(&joined)
        }
        _ => NONE_STR,
    }
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::buffers::KERNEL_ABI_VERSION;
    use crate::buffers::meta as meta_off;
    use crate::decode::decode_buffers;
    use bohmian_code_review_core::{
        Edge, EdgeKind, Language, Node, NodeKind, Provenance, RawReference, ReferenceKind,
        RelativePath,
    };

    /// A representative result with every optional field exercised.
    fn sample_result() -> (ExtractionResult, String) {
        let path = "src/sample.rs".to_string();
        let result = ExtractionResult {
            nodes: vec![
                Node {
                    id: "file:src/sample.rs".into(),
                    kind: NodeKind::File,
                    name: "sample.rs".into(),
                    qualified_name: "src/sample.rs".into(),
                    file_path: path.clone(),
                    language: Language::Rust,
                    start_line: 1,
                    end_line: 6,
                    start_column: 0,
                    end_column: 0,
                    docstring: Some("docs".into()),
                    signature: None,
                    visibility: Some(Visibility::Public),
                    is_exported: Some(true),
                    is_async: None,
                    is_static: Some(false),
                    is_abstract: None,
                    decorators: Some(vec!["derive".into()]),
                    type_parameters: None,
                    return_type: Some("u32".into()),
                    extra: None,
                },
                Node {
                    id: "function:abc".into(),
                    kind: NodeKind::Function,
                    name: "add".into(),
                    qualified_name: "src/sample.rs::add".into(),
                    file_path: path.clone(),
                    language: Language::Rust,
                    start_line: 3,
                    end_line: 5,
                    start_column: 0,
                    end_column: 2,
                    docstring: None,
                    signature: Some("fn add(a: u32) -> u32".into()),
                    visibility: None,
                    is_exported: None,
                    is_async: None,
                    is_static: None,
                    is_abstract: None,
                    decorators: None,
                    type_parameters: Some(vec!["T".into()]),
                    return_type: None,
                    extra: None,
                },
            ],
            edges: vec![Edge {
                source: "file:src/sample.rs".into(),
                target: "function:abc".into(),
                kind: EdgeKind::Contains,
                metadata: Some(serde_json::json!({"valueRef": true})),
                line: Some(3),
                column: None,
                provenance: Some(Provenance::TreeSitter),
            }],
            references: vec![RawReference {
                from_node_id: "function:abc".into(),
                reference_name: "helper".into(),
                reference_kind: ReferenceKind::Edge(EdgeKind::Calls),
                line: 4,
                column: 2,
                file_path: None,
                language: None,
                candidates: Some(vec!["a::helper".into()]),
            }],
            diagnostics: vec![bohmian_code_review_core::ExtractionDiagnostic {
                message: "note".into(),
                file_path: Some(path.clone()),
                line: None,
                column: None,
                severity: bohmian_code_review_core::Severity::Warning,
                code: None,
            }],
        };
        (result, path)
    }

    #[test]
    /// Converts decode round trip preserves facts using the frozen kernel wire layout.
    ///
    /// # Panics
    ///
    /// Panics if fixture construction, ABI encoding, decoding, or equality assertions fail.
    fn encode_decode_round_trip_preserves_facts() {
        let (result, path) = sample_result();
        let emit = encode_extraction_result(&result, &path).unwrap();
        // The codec must be a valid wire artifact: meta declares the tables.
        let decoded = decode_buffers(&emit, &path, Language::Rust).unwrap();
        assert_eq!(decoded, result, "facts must survive the round trip");
        assert_eq!(decoded.diagnostics.len(), 1);
        assert_eq!(
            decoded.diagnostics[0].severity,
            bohmian_code_review_core::Severity::Warning
        );
    }

    #[test]
    /// Checks encoder output lengths against the fixed metadata and row widths.
    ///
    /// # Panics
    ///
    /// Panics if fixture encoding fails or a table length differs from its fixed row width.
    fn encoded_buffers_have_expected_row_sizes() {
        let (result, path) = sample_result();
        let emit = encode_extraction_result(&result, &path).unwrap();
        assert_eq!(emit.nodes.len(), 2 * crate::buffers::NODE_ROW_SIZE);
        assert_eq!(emit.edges.len(), crate::buffers::EDGE_ROW_SIZE);
        assert_eq!(emit.refs.len(), crate::buffers::REF_ROW_SIZE);
        assert_eq!(emit.meta[meta_off::VERSION], KERNEL_ABI_VERSION);
    }

    #[test]
    /// Converts accepts any relative path input using the frozen kernel wire layout.
    ///
    /// # Panics
    ///
    /// Panics if fixture encoding fails for a path that the ABI treats as opaque text.
    fn encode_accepts_any_relative_path_input() {
        let (result, path) = sample_result();
        let relative = RelativePath::new(&path).unwrap();
        let emit = encode_extraction_result(&result, relative.as_str()).unwrap();
        assert!(!emit.meta.is_empty());
    }
}
