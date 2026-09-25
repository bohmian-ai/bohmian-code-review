//! Decode the walkers' flat-buffer tables into native facts.
//!
//! Byte-for-byte mirror of the frozen upstream `src/extraction/kernel/
//! decode.ts`: reads the five `EmitOut` tables (meta/nodes/edges/refs/arena)
//! and materializes [`Node`]/[`Edge`]/[`RawReference`]/diagnostic values
//! with the exact upstream optionality, kind mapping, `extraJson` merge, and
//! `function_ref` handling.

use crate::buffers::{
    EDGE_ROW_SIZE, EmitOut, KERNEL_ABI_VERSION, META_SIZE, NODE_ROW_SIZE, NONE, PROVENANCES,
    REF_FLAG_FILE_PATH, REF_ROW_SIZE, VISIBILITIES, edge as edge_off, flag as flag_off,
    meta as meta_off, node as node_off, reference as ref_off,
};
use crate::error::ExtractionError;
use crate::result::ExtractionResult;
use bohmian_code_review_core::{
    Edge, EdgeKind, ExtractionDiagnostic, Language, Node, NodeKind, RawReference, ReferenceKind,
};

/// Read an `(offset, len)` arena string; `None` when absent.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when the row lacks the offset/length pair,
/// the referenced arena range is out of bounds, or the bytes are not UTF-8.
fn str(arena: &[u8], row: &[u8], at: usize) -> Result<Option<String>, ExtractionError> {
    let off = read_u32(row, at)?;
    if off == NONE {
        return Ok(None);
    }
    let len = read_u32(row, at + 4)? as usize;
    let end = off as usize + len;
    let bytes = arena
        .get(off as usize..end)
        .ok_or_else(|| ExtractionError::Decode {
            message: "arena string out of bounds".to_string(),
        })?;
    std::str::from_utf8(bytes)
        .map(|value| Some(value.to_string()))
        .map_err(|error| ExtractionError::Decode {
            message: format!("arena string is not UTF-8: {error}"),
        })
}

/// NUL-joined list field; `None` when absent.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] under the same malformed row, arena-range,
/// or UTF-8 conditions as [`str`].
fn str_list(arena: &[u8], row: &[u8], at: usize) -> Result<Option<Vec<String>>, ExtractionError> {
    match str(arena, row, at)? {
        Some(joined) if !joined.is_empty() => {
            Ok(Some(joined.split('\0').map(str::to_string).collect()))
        }
        Some(_) => Ok(Some(Vec::new())),
        None => Ok(None),
    }
}

/// Tri-state boolean from a `(present, value)` bit pair.
fn flag(flags: u16, pair: u16) -> Option<bool> {
    if flags & (1 << (pair * 2)) == 0 {
        return None;
    }
    Some(flags & (1 << (pair * 2 + 1)) != 0)
}

/// A `u32` at `at` in `row`; out-of-range reads are malformed buffers.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when fewer than four bytes remain at `at`.
///
/// # Panics
///
/// Panics only if the four-byte slice accepted by the bounds check cannot be
/// converted to `[u8; 4]`, which would violate the standard slice conversion contract.
fn read_u32(row: &[u8], at: usize) -> Result<u32, ExtractionError> {
    let bytes = row.get(at..at + 4).ok_or_else(|| ExtractionError::Decode {
        message: format!("row too short for u32 at offset {at}"),
    })?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("four bytes")))
}

/// A `u8` at `at` in `row`.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when `at` lies outside the row.
fn read_u8(row: &[u8], at: usize) -> Result<u8, ExtractionError> {
    row.get(at).copied().ok_or_else(|| ExtractionError::Decode {
        message: format!("row too short for u8 at offset {at}"),
    })
}

/// A `u16` at `at` in `row`.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when fewer than two bytes remain at `at`.
///
/// # Panics
///
/// Panics only if the two-byte slice accepted by the bounds check cannot be
/// converted to `[u8; 2]`, which would violate the standard slice conversion contract.
fn read_u16(row: &[u8], at: usize) -> Result<u16, ExtractionError> {
    let bytes = row.get(at..at + 2).ok_or_else(|| ExtractionError::Decode {
        message: format!("row too short for u16 at offset {at}"),
    })?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("two bytes")))
}

/// A `u32` read as optional: `NONE` maps to `None`.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when the underlying four-byte read is truncated.
fn u32_opt(row: &[u8], at: usize) -> Result<Option<u32>, ExtractionError> {
    let value = read_u32(row, at)?;
    Ok(if value == NONE { None } else { Some(value) })
}

/// The fixed-width row starting at `index` within `table`.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when `index * row_size` overflows or
/// the table does not contain the complete requested row.
fn row(table: &[u8], index: usize, row_size: usize) -> Result<&[u8], ExtractionError> {
    let start = index
        .checked_mul(row_size)
        .ok_or_else(|| ExtractionError::Decode {
            message: "row index overflow".to_string(),
        })?;
    table
        .get(start..start + row_size)
        .ok_or_else(|| ExtractionError::Decode {
            message: format!("table too short for row {index}"),
        })
}

/// Decode one `EmitOut` into native facts.
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when the meta block is shorter than
/// [`META_SIZE`], the ABI version differs from [`KERNEL_ABI_VERSION`], a
/// table is too short for its declared row count, a kind index is out of
/// range, or an arena string is out of bounds.
pub fn decode_buffers(
    buffers: &EmitOut,
    file_path: &str,
    language: Language,
) -> Result<ExtractionResult, ExtractionError> {
    let meta = buffers.meta.as_slice();
    if meta.len() < META_SIZE {
        return Err(ExtractionError::Decode {
            message: format!("meta too short: {}", meta.len()),
        });
    }
    if meta[meta_off::VERSION] != KERNEL_ABI_VERSION {
        return Err(ExtractionError::Decode {
            message: format!(
                "buffer ABI {} != expected {KERNEL_ABI_VERSION}",
                meta[meta_off::VERSION]
            ),
        });
    }
    let node_count = read_u32(meta, meta_off::NODE_COUNT)? as usize;
    let edge_count = read_u32(meta, meta_off::EDGE_COUNT)? as usize;
    let ref_count = read_u32(meta, meta_off::REF_COUNT)? as usize;
    let arena = buffers.arena.as_slice();

    // Node-table row index → node id, for edge/ref endpoint resolution.
    let mut nodes = Vec::with_capacity(node_count);
    let mut id_by_row: Vec<String> = Vec::with_capacity(node_count);
    for index in 0..node_count {
        let row_bytes = row(&buffers.nodes, index, NODE_ROW_SIZE)?;
        let id = str(arena, row_bytes, node_off::ID)?.ok_or_else(|| ExtractionError::Decode {
            message: format!("node row {index} has no id"),
        })?;
        id_by_row.push(id.clone());
        let flags = read_u16(row_bytes, node_off::FLAGS)?;
        let kind_index = read_u8(row_bytes, node_off::KIND)?;
        let kind =
            NodeKind::from_wire_index(kind_index).ok_or_else(|| ExtractionError::Decode {
                message: format!("unknown node kind index {kind_index}"),
            })?;
        let mut node = Node {
            id,
            kind,
            name: str(arena, row_bytes, node_off::NAME)?.unwrap_or_default(),
            qualified_name: str(arena, row_bytes, node_off::QUALIFIED_NAME)?.unwrap_or_default(),
            file_path: file_path.to_string(),
            language,
            start_line: read_u32(row_bytes, node_off::START_LINE)?,
            end_line: read_u32(row_bytes, node_off::END_LINE)?,
            start_column: read_u32(row_bytes, node_off::START_COLUMN)?,
            end_column: read_u32(row_bytes, node_off::END_COLUMN)?,
            docstring: str(arena, row_bytes, node_off::DOCSTRING)?,
            signature: str(arena, row_bytes, node_off::SIGNATURE)?,
            visibility: VISIBILITIES
                .get(read_u8(row_bytes, node_off::VISIBILITY)? as usize)
                .copied()
                .flatten(),
            is_exported: flag(flags, flag_off::IS_EXPORTED),
            is_async: flag(flags, flag_off::IS_ASYNC),
            is_static: flag(flags, flag_off::IS_STATIC),
            is_abstract: flag(flags, flag_off::IS_ABSTRACT),
            decorators: str_list(arena, row_bytes, node_off::DECORATORS)?,
            type_parameters: str_list(arena, row_bytes, node_off::TYPE_PARAMETERS)?,
            return_type: str(arena, row_bytes, node_off::RETURN_TYPE)?,
            extra: None,
        };
        if let Some(extra_json) = str(arena, row_bytes, node_off::EXTRA_JSON)? {
            node.extra = Some(serde_json::from_str(&extra_json).map_err(|error| {
                ExtractionError::Decode {
                    message: format!("invalid extraJson: {error}"),
                }
            })?);
        }
        nodes.push(node);
    }

    let mut edges = Vec::with_capacity(edge_count);
    for index in 0..edge_count {
        let row_bytes = row(&buffers.edges, index, EDGE_ROW_SIZE)?;
        let source_idx = read_u32(row_bytes, edge_off::SOURCE_IDX)?;
        let target_idx = read_u32(row_bytes, edge_off::TARGET_IDX)?;
        let kind_index = read_u8(row_bytes, edge_off::KIND)?;
        let kind =
            EdgeKind::from_wire_index(kind_index).ok_or_else(|| ExtractionError::Decode {
                message: format!("unknown edge kind index {kind_index}"),
            })?;
        let mut edge = Edge {
            source: if source_idx == NONE {
                str(arena, row_bytes, edge_off::SOURCE_ID_STR)?.ok_or_else(|| {
                    ExtractionError::Decode {
                        message: format!("edge row {index} has neither source index nor id"),
                    }
                })?
            } else {
                id_by_row.get(source_idx as usize).cloned().ok_or_else(|| {
                    ExtractionError::Decode {
                        message: format!("edge row {index} source index {source_idx} out of range"),
                    }
                })?
            },
            target: if target_idx == NONE {
                str(arena, row_bytes, edge_off::TARGET_ID_STR)?.ok_or_else(|| {
                    ExtractionError::Decode {
                        message: format!("edge row {index} has neither target index nor id"),
                    }
                })?
            } else {
                id_by_row.get(target_idx as usize).cloned().ok_or_else(|| {
                    ExtractionError::Decode {
                        message: format!("edge row {index} target index {target_idx} out of range"),
                    }
                })?
            },
            kind,
            metadata: None,
            line: u32_opt(row_bytes, edge_off::LINE)?,
            column: u32_opt(row_bytes, edge_off::COLUMN)?,
            provenance: PROVENANCES
                .get(read_u8(row_bytes, edge_off::PROVENANCE)? as usize)
                .copied()
                .flatten(),
        };
        if let Some(metadata_json) = str(arena, row_bytes, edge_off::METADATA_JSON)? {
            edge.metadata = Some(serde_json::from_str(&metadata_json).map_err(|error| {
                ExtractionError::Decode {
                    message: format!("invalid metadataJson: {error}"),
                }
            })?);
        }
        edges.push(edge);
    }

    let mut references = Vec::with_capacity(ref_count);
    for index in 0..ref_count {
        let row_bytes = row(&buffers.refs, index, REF_ROW_SIZE)?;
        let from_idx = read_u32(row_bytes, ref_off::FROM_IDX)?;
        let kind_byte = read_u8(row_bytes, ref_off::KIND)?;
        let reference_kind =
            ReferenceKind::from_wire_byte(kind_byte).ok_or_else(|| ExtractionError::Decode {
                message: format!("unknown reference kind byte {kind_byte}"),
            })?;
        let mut reference = RawReference {
            from_node_id: if from_idx == NONE {
                str(arena, row_bytes, ref_off::FROM_ID_STR)?.ok_or_else(|| {
                    ExtractionError::Decode {
                        message: format!("ref row {index} has neither from index nor id"),
                    }
                })?
            } else {
                id_by_row.get(from_idx as usize).cloned().ok_or_else(|| {
                    ExtractionError::Decode {
                        message: format!("ref row {index} from index {from_idx} out of range"),
                    }
                })?
            },
            reference_name: str(arena, row_bytes, ref_off::REFERENCE_NAME)?.unwrap_or_default(),
            reference_kind,
            line: read_u32(row_bytes, ref_off::LINE)?,
            column: read_u32(row_bytes, ref_off::COLUMN)?,
            file_path: None,
            language: None,
            candidates: str_list(arena, row_bytes, ref_off::CANDIDATES)?,
        };
        if read_u8(row_bytes, ref_off::FLAGS)? & REF_FLAG_FILE_PATH != 0 {
            reference.file_path = Some(file_path.to_string());
        }
        references.push(reference);
    }

    let mut diagnostics = Vec::new();
    let errors_off = read_u32(meta, meta_off::ERRORS_OFF)?;
    if errors_off != NONE {
        let errors_len = read_u32(meta, meta_off::ERRORS_LEN)? as usize;
        let end = errors_off as usize + errors_len;
        let bytes = arena
            .get(errors_off as usize..end)
            .ok_or_else(|| ExtractionError::Decode {
                message: "errors JSON out of bounds".to_string(),
            })?;
        diagnostics = serde_json::from_slice(bytes).map_err(|error| ExtractionError::Decode {
            message: format!("invalid errors JSON: {error}"),
        })?;
    }

    Ok(ExtractionResult {
        nodes,
        edges,
        references,
        diagnostics,
    })
}

/// Decode diagnostics JSON bytes into typed diagnostics (test helper path).
#[doc(hidden)]
///
/// # Errors
///
/// Returns [`ExtractionError::Decode`] when `bytes` is not valid diagnostics JSON
/// or does not match the typed diagnostic schema.
pub fn parse_diagnostics(bytes: &[u8]) -> Result<Vec<ExtractionDiagnostic>, ExtractionError> {
    serde_json::from_slice(bytes).map_err(|error| ExtractionError::Decode {
        message: format!("invalid errors JSON: {error}"),
    })
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;
    use crate::buffers::{
        Arena, BoolFlags, EdgeRow, FUNCTION_REF_CODE, NONE_STR, NodeRow, RefRow, Tables,
        build_meta, edge_kind_index, node_kind_index,
    };
    use bohmian_code_review_core::{Provenance, Severity, Visibility};

    /// Encode one small synthetic table set and decode it back.
    ///
    /// # Panics
    ///
    /// Panics if a hard-coded ABI kind is absent, fixture JSON cannot serialize, or fixture decoding fails.
    fn round_trip_emit() -> (EmitOut, Node, Edge, RawReference) {
        let mut tables = Tables::default();
        let mut arena = Arena::default();
        let name = arena.put("main");
        let qn = arena.put("pkg::main");
        let id = arena.put("function:abcdef");
        let node_row = NodeRow {
            kind: node_kind_index("function").expect("function kind"),
            visibility: 1,
            flags: BoolFlags::default(),
            start_line: 1,
            end_line: 2,
            start_column: 0,
            end_column: 5,
            name,
            qualified_name: qn,
            id,
            docstring: NONE_STR,
            signature: NONE_STR,
            decorators: NONE_STR,
            type_parameters: NONE_STR,
            return_type: NONE_STR,
            extra_json: NONE_STR,
        };
        let node_idx = tables.push_node(&node_row);
        tables.push_edge(&EdgeRow {
            source_idx: node_idx,
            target_idx: node_idx,
            kind: edge_kind_index("calls").expect("calls kind"),
            provenance: 1,
            line: 3,
            column: 4,
            metadata_json: NONE_STR,
            source_id_str: NONE_STR,
            target_id_str: NONE_STR,
        });
        tables.push_ref(&RefRow {
            from_idx: node_idx,
            kind: FUNCTION_REF_CODE,
            line: 5,
            column: 0,
            reference_name: arena.put("callback"),
            candidates: NONE_STR,
            from_id_str: NONE_STR,
        });
        let meta = build_meta(&tables, arena.len(), NONE_STR, 0.0);
        let emit = EmitOut {
            meta,
            nodes: tables.nodes,
            edges: tables.edges,
            refs: tables.refs,
            arena: arena.into_vec(),
        };
        let node = Node {
            id: "function:abcdef".into(),
            kind: NodeKind::Function,
            name: "main".into(),
            qualified_name: "pkg::main".into(),
            file_path: "a.rs".into(),
            language: Language::Rust,
            start_line: 1,
            end_line: 2,
            start_column: 0,
            end_column: 5,
            docstring: None,
            signature: None,
            visibility: Some(Visibility::Public),
            is_exported: None,
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: None,
            extra: None,
        };
        let edge = Edge {
            source: "function:abcdef".into(),
            target: "function:abcdef".into(),
            kind: EdgeKind::Calls,
            metadata: None,
            line: Some(3),
            column: Some(4),
            provenance: Some(Provenance::TreeSitter),
        };
        let reference = RawReference {
            from_node_id: "function:abcdef".into(),
            reference_name: "callback".into(),
            reference_kind: ReferenceKind::FunctionRef,
            line: 5,
            column: 0,
            file_path: None,
            language: None,
            candidates: None,
        };
        (emit, node, edge, reference)
    }

    #[test]
    /// Decodes a representative buffer into the expected nodes, edges, and unresolved references.
    ///
    /// # Panics
    ///
    /// Panics if the synthetic ABI fixture cannot be built or decoded, or a decoded fact differs.
    fn decodes_expected_facts() {
        let (emit, node, edge, reference) = round_trip_emit();
        let result = decode_buffers(&emit, "a.rs", Language::Rust).unwrap();
        assert_eq!(result.nodes, vec![node]);
        assert_eq!(result.edges, vec![edge]);
        assert_eq!(result.references, vec![reference]);
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    /// Rejects metadata written with an unsupported kernel ABI version.
    fn rejects_wrong_abi() {
        let (mut emit, _, _, _) = round_trip_emit();
        emit.meta[meta_off::VERSION] = 99;
        assert!(matches!(
            decode_buffers(&emit, "a.rs", Language::Rust),
            Err(ExtractionError::Decode { .. })
        ));
    }

    #[test]
    /// Rejects metadata shorter than the fixed ABI header.
    fn rejects_truncated_meta() {
        let (mut emit, _, _, _) = round_trip_emit();
        emit.meta.truncate(10);
        assert!(matches!(
            decode_buffers(&emit, "a.rs", Language::Rust),
            Err(ExtractionError::Decode { .. })
        ));
    }

    #[test]
    /// Rejects node buffers whose byte length cannot contain the declared rows.
    fn rejects_truncated_node_table() {
        let (mut emit, _, _, _) = round_trip_emit();
        emit.nodes.truncate(emit.nodes.len() - 1);
        assert!(matches!(
            decode_buffers(&emit, "a.rs", Language::Rust),
            Err(ExtractionError::Decode { .. })
        ));
    }

    #[test]
    /// Rejects wire kind ordinals outside the closed registry.
    fn rejects_unknown_kind_index() {
        let (mut emit, _, _, _) = round_trip_emit();
        emit.nodes[0] = 200; // kind byte out of range
        assert!(matches!(
            decode_buffers(&emit, "a.rs", Language::Rust),
            Err(ExtractionError::Decode { .. })
        ));
    }

    #[test]
    /// Decodes serialized parser diagnostics without losing severity, message, or source range.
    ///
    /// # Panics
    ///
    /// Panics if diagnostic JSON serialization or typed decoding fails.
    fn parse_diagnostics_round_trip() {
        let json = r#"[{"message":"boom","filePath":"a.rs","severity":"warning","code":"W1"}]"#;
        let diagnostics = parse_diagnostics(json.as_bytes()).unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "boom");
        assert_eq!(diagnostics[0].severity, Severity::Warning);
    }
}
