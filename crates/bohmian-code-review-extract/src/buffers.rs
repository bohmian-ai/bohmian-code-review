//! Flat buffer contract — the ONE boundary crossing per file.
//!
//! The kernel returns five Buffers: meta, nodes, edges, refs, arena. All rows
//! are fixed-width little-endian; every string is an (offset, len) pair into
//! the UTF-8 arena. `OFFSET == NONE (0xFFFF_FFFF)` means "field absent".
//!
//! THIS FILE AND `src/extraction/kernel/layout.ts` MUST MATCH BYTE FOR BYTE.
//! Any layout change bumps `KERNEL_ABI_VERSION` — the TS loader refuses a
//! version it doesn't know and falls back to the wasm path.
//!
//! Layout (v1):
//!
//! meta (36 bytes):
//!   0   u8   KERNEL_ABI_VERSION
//!   1   [3]  pad
//!   4   u32  node count
//!   8   u32  edge count
//!   12  u32  ref count
//!   16  u32  arena byte length
//!   20  u32  errors-JSON arena offset (NONE = no errors)
//!   24  u32  errors-JSON byte length
//!   28  f64  kernel-side wall duration (ms) — introspection only; the TS
//!            wrapper measures the ExtractionResult.durationMs it reports
//!
//! node row (96 bytes):
//!   0   u8   NodeKind index (NODE_KINDS order)
//!   1   u8   visibility (0 absent, 1 public, 2 private, 3 protected, 4 internal)
//!   2   u16  bool flags — bit pairs (present, value):
//!            0/1 isExported, 2/3 isAsync, 4/5 isStatic, 6/7 isAbstract
//!   4   u32  startLine (1-based)
//!   8   u32  endLine
//!   12  u32  startColumn (0-based)
//!   16  u32  endColumn
//!   20  str  name
//!   28  str  qualifiedName
//!   36  str  id (kernel-computed: "kind:hash32", or "file:<path>" for the file node)
//!   44  str  docstring
//!   52  str  signature
//!   60  str  decorators (NUL-joined list)
//!   68  str  typeParameters (NUL-joined list)
//!   76  str  returnType
//!   84  str  extraJson (escape hatch: JSON of any extra Node props)
//!   92  u32  metrics slot (reserved for Arc 3.2 per-node code metrics; 0)
//!
//! edge row (44 bytes):
//!   0   u32  source node row index (NONE → use sourceIdStr)
//!   4   u32  target node row index (NONE → use targetIdStr)
//!   8   u8   EdgeKind index (EDGE_KINDS order)
//!   9   u8   provenance (0 absent, 1 tree-sitter, 2 scip, 3 heuristic)
//!   10  u16  pad
//!   12  u32  line (NONE absent)
//!   16  u32  column (NONE absent)
//!   20  str  metadataJson
//!   28  str  sourceIdStr
//!   36  str  targetIdStr
//!
//! ref row (40 bytes):
//!   0   u32  fromNode row index (NONE → use fromNodeIdStr)
//!   4   u8   ReferenceKind (EDGE_KINDS index, or 200 = function_ref)
//!   5   u8   flags — bit 0: ref carries the extracting file's path (v2; the
//!            ruby/php visitNode hooks set `filePath: ctx.filePath` on their
//!            mixin/trait `implements` refs — decode re-attaches the decode
//!            call's own filePath, which is byte-identical)
//!   6   [2]  pad
//!   8   u32  line (1-based)
//!   12  u32  column (0-based)
//!   16  str  referenceName
//!   24  str  candidates (NUL-joined list)
//!   32  str  fromNodeIdStr

pub const KERNEL_ABI_VERSION: u8 = 2;
/// Pins NONE to the ABI or syntax value consumed by the buffers implementation.
pub const NONE: u32 = 0xFFFF_FFFF;

/// Pins META SIZE to the ABI or syntax value consumed by the buffers implementation.
pub const META_SIZE: usize = 36;
/// Pins NODE ROW SIZE to the ABI or syntax value consumed by the buffers implementation.
pub const NODE_ROW_SIZE: usize = 96;
/// Pins EDGE ROW SIZE to the ABI or syntax value consumed by the buffers implementation.
pub const EDGE_ROW_SIZE: usize = 44;
/// Pins REF ROW SIZE to the ABI or syntax value consumed by the buffers implementation.
pub const REF_ROW_SIZE: usize = 40;

// ---------------------------------------------------------------------------
// Byte offsets and small tables — mirror of `src/extraction/kernel/layout.ts`
// (`META`/`NODE`/`EDGE`/`REF`/`FLAG`/`VISIBILITIES`/`PROVENANCES`). These
// constants back the Rust decoder (`decode.rs`) and encoder (`encode.rs`) so
// no magic numbers appear in the codec; the layout must match the TS mirror
// byte for byte.
// ---------------------------------------------------------------------------

/// meta byte offsets (layout.ts `META`).
pub mod meta {
    /// ABI version byte.
    pub const VERSION: usize = 0;
    /// Node count (u32).
    pub const NODE_COUNT: usize = 4;
    /// Edge count (u32).
    pub const EDGE_COUNT: usize = 8;
    /// Ref count (u32).
    pub const REF_COUNT: usize = 12;
    /// Arena byte length (u32).
    pub const ARENA_LEN: usize = 16;
    /// Errors-JSON arena offset (u32, `NONE` = no errors).
    pub const ERRORS_OFF: usize = 20;
    /// Errors-JSON byte length (u32).
    pub const ERRORS_LEN: usize = 24;
    /// Kernel-side wall duration in ms (f64, introspection only).
    pub const DURATION_MS: usize = 28;
}

/// node row byte offsets (layout.ts `NODE`).
pub mod node {
    /// NodeKind index (u8, `NODE_KINDS` order).
    pub const KIND: usize = 0;
    /// Visibility byte (u8, `VISIBILITIES` index, 0 = absent).
    pub const VISIBILITY: usize = 1;
    /// Bool flags (u16, `(present, value)` bit pairs).
    pub const FLAGS: usize = 2;
    /// Start line (u32, 1-based).
    pub const START_LINE: usize = 4;
    /// End line (u32).
    pub const END_LINE: usize = 8;
    /// Start column (u32, 0-based).
    pub const START_COLUMN: usize = 12;
    /// End column (u32).
    pub const END_COLUMN: usize = 16;
    /// Name (str).
    pub const NAME: usize = 20;
    /// Qualified name (str).
    pub const QUALIFIED_NAME: usize = 28;
    /// Kernel-computed node id (str).
    pub const ID: usize = 36;
    /// Docstring (str).
    pub const DOCSTRING: usize = 44;
    /// Signature (str).
    pub const SIGNATURE: usize = 52;
    /// Decorators (str, NUL-joined list).
    pub const DECORATORS: usize = 60;
    /// Type parameters (str, NUL-joined list).
    pub const TYPE_PARAMETERS: usize = 68;
    /// Return type (str).
    pub const RETURN_TYPE: usize = 76;
    /// Extra JSON (str).
    pub const EXTRA_JSON: usize = 84;
    /// Metrics slot (u32, reserved).
    pub const METRICS: usize = 92;
}

/// edge row byte offsets (layout.ts `EDGE`).
pub mod edge {
    /// Source node row index (u32, `NONE` → use `SOURCE_ID_STR`).
    pub const SOURCE_IDX: usize = 0;
    /// Target node row index (u32, `NONE` → use `TARGET_ID_STR`).
    pub const TARGET_IDX: usize = 4;
    /// EdgeKind index (u8, `EDGE_KINDS` order).
    pub const KIND: usize = 8;
    /// Provenance byte (u8, `PROVENANCES` index, 0 = absent).
    pub const PROVENANCE: usize = 9;
    /// Line (u32, `NONE` absent).
    pub const LINE: usize = 12;
    /// Column (u32, `NONE` absent).
    pub const COLUMN: usize = 16;
    /// Metadata JSON (str).
    pub const METADATA_JSON: usize = 20;
    /// Source id string (str).
    pub const SOURCE_ID_STR: usize = 28;
    /// Target id string (str).
    pub const TARGET_ID_STR: usize = 36;
}

/// ref row byte offsets (layout.ts `REF`).
pub mod reference {
    /// From-node row index (u32, `NONE` → use `FROM_ID_STR`).
    pub const FROM_IDX: usize = 0;
    /// ReferenceKind byte (u8, `EDGE_KINDS` index or `FUNCTION_REF_CODE`).
    pub const KIND: usize = 4;
    /// Flags byte (u8, `REF_FLAGS` bits).
    pub const FLAGS: usize = 5;
    /// Line (u32, 1-based).
    pub const LINE: usize = 8;
    /// Column (u32, 0-based).
    pub const COLUMN: usize = 12;
    /// Reference name (str).
    pub const REFERENCE_NAME: usize = 16;
    /// Candidates (str, NUL-joined list).
    pub const CANDIDATES: usize = 24;
    /// From-node id string (str).
    pub const FROM_ID_STR: usize = 32;
}

/// Node bool-flag bit pairs: bit(2n) = present, bit(2n+1) = value
/// (layout.ts `FLAG`).
pub mod flag {
    /// `isExported` pair.
    pub const IS_EXPORTED: u16 = 0;
    /// `isAsync` pair.
    pub const IS_ASYNC: u16 = 1;
    /// `isStatic` pair.
    pub const IS_STATIC: u16 = 2;
    /// `isAbstract` pair.
    pub const IS_ABSTRACT: u16 = 3;
}

/// Visibility byte values, index 0 = absent (layout.ts `VISIBILITIES`).
pub const VISIBILITIES: [Option<bohmian_code_review_core::Visibility>; 5] = [
    None,
    Some(bohmian_code_review_core::Visibility::Public),
    Some(bohmian_code_review_core::Visibility::Private),
    Some(bohmian_code_review_core::Visibility::Protected),
    Some(bohmian_code_review_core::Visibility::Internal),
];

/// Provenance byte values, index 0 = absent (layout.ts `PROVENANCES`).
pub const PROVENANCES: [Option<bohmian_code_review_core::Provenance>; 4] = [
    None,
    Some(bohmian_code_review_core::Provenance::TreeSitter),
    Some(bohmian_code_review_core::Provenance::Scip),
    Some(bohmian_code_review_core::Provenance::Heuristic),
];

/// Mirror of NODE_KINDS in src/types.ts — order is the wire contract.
pub const NODE_KINDS: [&str; 22] = [
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

/// Mirror of EDGE_KINDS in src/types.ts — order is the wire contract.
pub const EDGE_KINDS: [&str; 12] = [
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

/// ReferenceKind code for the internal-only `function_ref` (#756).
pub const FUNCTION_REF_CODE: u8 = 200;

/// Ref-row flag bit 0: the ref carries `filePath` = the extracted file.
pub const REF_FLAG_FILE_PATH: u8 = 1;

/// Returns the frozen ABI ordinal for a node-kind wire spelling.
pub fn node_kind_index(kind: &str) -> Option<u8> {
    NODE_KINDS.iter().position(|k| *k == kind).map(|i| i as u8)
}

/// Returns the frozen ABI ordinal for an edge-kind wire spelling.
pub fn edge_kind_index(kind: &str) -> Option<u8> {
    EDGE_KINDS.iter().position(|k| *k == kind).map(|i| i as u8)
}

/// (offset, len) arena reference. `NONE_STR` encodes an absent field.
pub type StrRef = (u32, u32);
/// Pins NONE STR to the ABI or syntax value consumed by the buffers implementation.
pub const NONE_STR: StrRef = (NONE, 0);

/// UTF-8 string arena. Strings are appended verbatim; no dedup (per-file
/// buffers are transient and small — intern later if profiling says so).
#[derive(Default)]
pub struct Arena {
    /// Contiguous UTF-8 bytes owned by the string arena.
    buf: Vec<u8>,
}

impl Arena {
    /// Appends UTF-8 bytes to the arena and returns their stable offset and length.
    pub fn put(&mut self, s: &str) -> StrRef {
        let off = self.buf.len() as u32;
        self.buf.extend_from_slice(s.as_bytes());
        (off, s.len() as u32)
    }

    /// Not used by the seed emitter yet. Kept
    /// so the arena API is complete alongside the layout it feeds.
    pub fn put_opt(&mut self, s: Option<&str>) -> StrRef {
        match s {
            Some(s) => self.put(s),
            None => NONE_STR,
        }
    }

    /// NUL-joined list; absent when the list is empty (decorators,
    /// typeParameters, candidates.)
    pub fn put_list(&mut self, items: &[String]) -> StrRef {
        if items.is_empty() {
            return NONE_STR;
        }
        let joined = items.join("\0");
        self.put(&joined)
    }

    /// Returns the arena byte length encoded into the metadata header.
    pub fn len(&self) -> u32 {
        self.buf.len() as u32
    }

    /// Whether the arena holds no bytes.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Transfers ownership of the completed arena bytes to the extraction result.
    pub fn into_vec(self) -> Vec<u8> {
        self.buf
    }

    /// Returns the UTF-8 bytes referenced by a string ref.
    ///
    /// # Panics
    ///
    /// Panics when the string ref is the absent sentinel or its range falls
    /// outside the arena (both are walker-internal invariants).
    pub fn get(&self, r: StrRef) -> &str {
        assert_ne!(r.0, NONE, "arena get on the absent sentinel");
        let start = r.0 as usize;
        let end = start + r.1 as usize;
        std::str::from_utf8(&self.buf[start..end]).expect("arena strings are UTF-8")
    }
}

/// Tri-state booleans packed as (present, value) bit pairs.
#[derive(Default, Clone, Copy)]
pub struct BoolFlags(
    /// Packed `(present, value)` pairs in frozen kernel bit order.
    pub u16,
);

impl BoolFlags {
    /// Sets one optional-boolean presence/value pair without disturbing adjacent flag pairs.
    pub fn set(&mut self, pair: u16, value: bool) {
        self.0 |= 1 << (pair * 2);
        if value {
            self.0 |= 1 << (pair * 2 + 1);
        }
    }
}

/// Pins FLAG IS EXPORTED to the ABI or syntax value consumed by the buffers implementation.
pub const FLAG_IS_EXPORTED: u16 = 0;
/// Pins FLAG IS ASYNC to the ABI or syntax value consumed by the buffers implementation.
pub const FLAG_IS_ASYNC: u16 = 1;
/// Pins FLAG IS STATIC to the ABI or syntax value consumed by the buffers implementation.
pub const FLAG_IS_STATIC: u16 = 2;
/// Pins FLAG IS ABSTRACT to the ABI or syntax value consumed by the buffers implementation.
pub const FLAG_IS_ABSTRACT: u16 = 3;

/// Represents one fixed-width node ABI row before it is encoded into bytes.
pub struct NodeRow {
    /// Closed upstream kind tag controlling the emitted fact classification.
    pub kind: u8,
    /// Language visibility mapped to the closed wire-level visibility value.
    pub visibility: u8,
    /// Packed presence/value bit pairs for optional node booleans.
    pub flags: BoolFlags,
    /// One-based first source line encoded for the node.
    pub start_line: u32,
    /// One-based final source line encoded for the node.
    pub end_line: u32,
    /// Zero-based UTF-16 start column encoded for the node.
    pub start_column: u32,
    /// Zero-based UTF-16 end column encoded for the node.
    pub end_column: u32,
    /// Source spelling retained for symbol names, qualified names, or reference matching.
    pub name: StrRef,
    /// Scope-qualified symbol name used for stable lookup and display.
    pub qualified_name: StrRef,
    /// Upstream-compatible semantic identifier stored in the wire row.
    pub id: StrRef,
    /// Normalized leading documentation attached to the emitted symbol.
    pub docstring: StrRef,
    /// Source signature text preserved on the emitted symbol.
    pub signature: StrRef,
    /// NUL-joined decorator names stored in the string arena.
    pub decorators: StrRef,
    /// NUL-joined generic parameter text stored in the string arena.
    pub type_parameters: StrRef,
    /// Declared return type retained for the node payload and type references.
    pub return_type: StrRef,
    /// JSON escape-hatch payload preserved from the upstream node shape.
    pub extra_json: StrRef,
}

/// Represents one fixed-width edge ABI row with indexed or string endpoints.
pub struct EdgeRow {
    /// Source node-table index, or the absent sentinel when a string identifier is used.
    pub source_idx: u32,
    /// Target node-table index, or the absent sentinel when a string identifier is used.
    pub target_idx: u32,
    /// Closed upstream kind tag controlling the emitted fact classification.
    pub kind: u8,
    /// Closed provenance code identifying how the edge was derived.
    pub provenance: u8,
    /// One-based source line attached to the emitted fact.
    pub line: u32,
    /// Zero-based UTF-16 source column attached to the emitted fact.
    pub column: u32,
    /// Optional serialized edge metadata referenced through the arena.
    pub metadata_json: StrRef,
    /// Fallback source identifier used when no source row index exists.
    pub source_id_str: StrRef,
    /// Fallback target identifier used when no target row index exists.
    pub target_id_str: StrRef,
}

/// Represents one fixed-width unresolved-reference ABI row.
pub struct RefRow {
    /// Origin node-table index, or the absent sentinel for a string identifier.
    pub from_idx: u32,
    /// Closed upstream kind tag controlling the emitted fact classification.
    pub kind: u8,
    /// One-based source line attached to the emitted fact.
    pub line: u32,
    /// Zero-based UTF-16 source column attached to the emitted fact.
    pub column: u32,
    /// Unresolved source spelling supplied to the later resolver.
    pub reference_name: StrRef,
    /// Ordered candidate names retained for later reference resolution.
    pub candidates: StrRef,
    /// Fallback origin identifier used when no node row index exists.
    pub from_id_str: StrRef,
}

/// Encodes str ref into the current flat table while preserving insertion order and row indexes.
fn push_str_ref(buf: &mut Vec<u8>, r: StrRef) {
    buf.extend_from_slice(&r.0.to_le_bytes());
    buf.extend_from_slice(&r.1.to_le_bytes());
}

/// Owns all per-file ABI row buffers and their monotonically increasing counts.
pub struct Tables {
    /// Encoded node rows in the walkers original emission order.
    pub nodes: Vec<u8>,
    /// Encoded edge rows whose indexes refer to the node table.
    pub edges: Vec<u8>,
    /// Encoded unresolved-reference rows whose indexes refer to the node table.
    pub refs: Vec<u8>,
    /// Number of node rows written into the metadata header.
    pub node_count: u32,
    /// Number of edge rows written into the metadata header.
    pub edge_count: u32,
    /// Number of unresolved-reference rows written into the metadata header.
    pub ref_count: u32,
}

impl Default for Tables {
    /// Allocates empty row buffers with small per-file capacities and zero counts.
    fn default() -> Self {
        Tables {
            nodes: Vec::with_capacity(NODE_ROW_SIZE * 64),
            edges: Vec::with_capacity(EDGE_ROW_SIZE * 64),
            refs: Vec::with_capacity(REF_ROW_SIZE * 64),
            node_count: 0,
            edge_count: 0,
            ref_count: 0,
        }
    }
}

impl Tables {
    /// Encodes node into the current flat table while preserving insertion order and row indexes.
    pub fn push_node(&mut self, r: &NodeRow) -> u32 {
        let buf = &mut self.nodes;
        buf.push(r.kind);
        buf.push(r.visibility);
        buf.extend_from_slice(&r.flags.0.to_le_bytes());
        buf.extend_from_slice(&r.start_line.to_le_bytes());
        buf.extend_from_slice(&r.end_line.to_le_bytes());
        buf.extend_from_slice(&r.start_column.to_le_bytes());
        buf.extend_from_slice(&r.end_column.to_le_bytes());
        push_str_ref(buf, r.name);
        push_str_ref(buf, r.qualified_name);
        push_str_ref(buf, r.id);
        push_str_ref(buf, r.docstring);
        push_str_ref(buf, r.signature);
        push_str_ref(buf, r.decorators);
        push_str_ref(buf, r.type_parameters);
        push_str_ref(buf, r.return_type);
        push_str_ref(buf, r.extra_json);
        buf.extend_from_slice(&0u32.to_le_bytes()); // metrics slot (Arc 3.2)
        let idx = self.node_count;
        self.node_count += 1;
        idx
    }

    /// Encodes edge into the current flat table while preserving insertion order and row indexes.
    pub fn push_edge(&mut self, r: &EdgeRow) {
        let buf = &mut self.edges;
        buf.extend_from_slice(&r.source_idx.to_le_bytes());
        buf.extend_from_slice(&r.target_idx.to_le_bytes());
        buf.push(r.kind);
        buf.push(r.provenance);
        buf.extend_from_slice(&0u16.to_le_bytes()); // pad
        buf.extend_from_slice(&r.line.to_le_bytes());
        buf.extend_from_slice(&r.column.to_le_bytes());
        push_str_ref(buf, r.metadata_json);
        push_str_ref(buf, r.source_id_str);
        push_str_ref(buf, r.target_id_str);
        self.edge_count += 1;
    }

    /// Encodes ref into the current flat table while preserving insertion order and row indexes.
    pub fn push_ref(&mut self, r: &RefRow) {
        self.push_ref_flagged(r, 0);
    }

    /// Encodes ref flagged into the current flat table while preserving insertion order and row indexes.
    pub fn push_ref_flagged(&mut self, r: &RefRow, flags: u8) {
        let buf = &mut self.refs;
        buf.extend_from_slice(&r.from_idx.to_le_bytes());
        buf.push(r.kind);
        buf.push(flags);
        buf.extend_from_slice(&[0u8; 2]); // pad
        buf.extend_from_slice(&r.line.to_le_bytes());
        buf.extend_from_slice(&r.column.to_le_bytes());
        push_str_ref(buf, r.reference_name);
        push_str_ref(buf, r.candidates);
        push_str_ref(buf, r.from_id_str);
        self.ref_count += 1;
    }
}

/// One file's encoded tables, ready to hand across the JS boundary.
pub struct EmitOut {
    /// Fixed ABI metadata bytes containing row counts, arena length, diagnostics, and duration.
    pub meta: Vec<u8>,
    /// Encoded node rows in the walkers original emission order.
    pub nodes: Vec<u8>,
    /// Encoded edge rows whose indexes refer to the node table.
    pub edges: Vec<u8>,
    /// Encoded unresolved-reference rows whose indexes refer to the node table.
    pub refs: Vec<u8>,
    /// String arena that owns text referenced by the emitted fixed-width rows.
    pub arena: Vec<u8>,
}

/// Encodes ABI version, row counts, arena length, diagnostics location, and duration into the metadata header.
pub fn build_meta(t: &Tables, arena_len: u32, errors_json: StrRef, duration_ms: f64) -> Vec<u8> {
    let mut m = Vec::with_capacity(META_SIZE);
    m.push(KERNEL_ABI_VERSION);
    m.extend_from_slice(&[0u8; 3]);
    m.extend_from_slice(&t.node_count.to_le_bytes());
    m.extend_from_slice(&t.edge_count.to_le_bytes());
    m.extend_from_slice(&t.ref_count.to_le_bytes());
    m.extend_from_slice(&arena_len.to_le_bytes());
    m.extend_from_slice(&errors_json.0.to_le_bytes());
    m.extend_from_slice(&errors_json.1.to_le_bytes());
    m.extend_from_slice(&duration_ms.to_le_bytes());
    debug_assert_eq!(m.len(), META_SIZE);
    m
}

#[cfg(test)]
/// Contains focused regression tests for this module's frozen behavior and invariants.
mod tests {
    use super::*;

    #[test]
    /// Guards fixed-width ABI row sizes against accidental layout drift.
    fn row_sizes_match_constants() {
        let mut t = Tables::default();
        let mut a = Arena::default();
        let name = a.put("x");
        t.push_node(&NodeRow {
            kind: 0,
            visibility: 0,
            flags: BoolFlags::default(),
            start_line: 1,
            end_line: 1,
            start_column: 0,
            end_column: 0,
            name,
            qualified_name: name,
            id: name,
            docstring: NONE_STR,
            signature: NONE_STR,
            decorators: NONE_STR,
            type_parameters: NONE_STR,
            return_type: NONE_STR,
            extra_json: NONE_STR,
        });
        assert_eq!(t.nodes.len(), NODE_ROW_SIZE);
        t.push_edge(&EdgeRow {
            source_idx: 0,
            target_idx: 0,
            kind: 0,
            provenance: 0,
            line: NONE,
            column: NONE,
            metadata_json: NONE_STR,
            source_id_str: NONE_STR,
            target_id_str: NONE_STR,
        });
        assert_eq!(t.edges.len(), EDGE_ROW_SIZE);
        t.push_ref(&RefRow {
            from_idx: 0,
            kind: 1,
            line: 1,
            column: 0,
            reference_name: name,
            candidates: NONE_STR,
            from_id_str: NONE_STR,
        });
        assert_eq!(t.refs.len(), REF_ROW_SIZE);
        let meta = build_meta(&t, a.len(), NONE_STR, 0.0);
        assert_eq!(meta.len(), META_SIZE);
    }
}
