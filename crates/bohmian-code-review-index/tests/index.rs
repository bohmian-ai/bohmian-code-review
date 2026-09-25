//! Owner tests for exact postings, deterministic order, and per-file state.

use bohmian_code_review_core::{Language, Node, NodeKind, RelativePath};
use bohmian_code_review_index::SymbolIndex;

/// Builds a canonical path.
///
/// # Panics
///
/// Panics if `raw` is not a valid repository-relative path.
fn path(raw: &str) -> RelativePath {
    RelativePath::new(raw).expect("valid path")
}

/// Builds a TypeScript declaration fact with the fields lookup exposes.
fn node(file: &str, kind: NodeKind, name: &str, qualified: &str, line: u32, id: &str) -> Node {
    Node {
        id: id.to_owned(),
        kind,
        name: name.to_owned(),
        qualified_name: qualified.to_owned(),
        file_path: file.to_owned(),
        language: Language::Typescript,
        start_line: line,
        end_line: line + 2,
        start_column: 0,
        end_column: 1,
        docstring: None,
        signature: None,
        visibility: None,
        is_exported: None,
        is_async: None,
        is_static: None,
        is_abstract: None,
        decorators: None,
        type_parameters: None,
        return_type: None,
        extra: None,
    }
}

/// Renders hits as `(path, line, id)` for order assertions.
fn keys(index: &SymbolIndex, name: &str) -> Vec<(String, u32, String)> {
    index
        .lookup(name)
        .iter()
        .map(|hit| {
            (
                hit.path.as_str().to_owned(),
                hit.node.start_line,
                hit.node.id.clone(),
            )
        })
        .collect()
}

#[test]
/// Simple and qualified lookup is exact and case-sensitive, returns every
/// duplicate in path/location/ID order regardless of insertion order, and
/// skips non-declaration facts; unknown names are empty.
fn exact_lookup_returns_all_duplicates_in_canonical_order() {
    let mut index = SymbolIndex::new();
    index.upsert(
        path("src/z.ts"),
        10,
        vec![node(
            "src/z.ts",
            NodeKind::Function,
            "lookupUser",
            "lookupUser",
            1,
            "function:z",
        )],
    );
    index.upsert(
        path("src/a.ts"),
        20,
        vec![
            node(
                "src/a.ts",
                NodeKind::File,
                "a.ts",
                "src/a.ts",
                1,
                "file:src/a.ts",
            ),
            node(
                "src/a.ts",
                NodeKind::Import,
                "lookupUser",
                "lookupUser",
                1,
                "import:a",
            ),
            node(
                "src/a.ts",
                NodeKind::Method,
                "lookupUser",
                "Repo::lookupUser",
                9,
                "method:a2",
            ),
            node(
                "src/a.ts",
                NodeKind::Function,
                "lookupUser",
                "lookupUser",
                3,
                "function:a1",
            ),
            node(
                "src/a.ts",
                NodeKind::Function,
                "lookupUser",
                "lookupUser",
                3,
                "function:a0",
            ),
        ],
    );

    assert_eq!(
        keys(&index, "lookupUser"),
        vec![
            ("src/a.ts".to_owned(), 3, "function:a0".to_owned()),
            ("src/a.ts".to_owned(), 3, "function:a1".to_owned()),
            ("src/a.ts".to_owned(), 9, "method:a2".to_owned()),
            ("src/z.ts".to_owned(), 1, "function:z".to_owned()),
        ]
    );
    assert_eq!(
        keys(&index, "Repo::lookupUser"),
        vec![("src/a.ts".to_owned(), 9, "method:a2".to_owned())]
    );
    assert!(index.lookup("lookupuser").is_empty());
    assert!(index.lookup("a.ts").is_empty());
    assert!(index.lookup("missing").is_empty());
}

#[test]
/// Upsert replaces a file's postings completely, leaves other files intact,
/// and removal is idempotent.
fn upsert_replaces_postings_and_removal_is_idempotent() {
    let mut index = SymbolIndex::new();
    let user = path("src/user.ts");
    let other = path("src/other.ts");
    index.upsert(
        user.clone(),
        30,
        vec![node(
            "src/user.ts",
            NodeKind::Function,
            "oldName",
            "oldName",
            1,
            "function:old",
        )],
    );
    index.upsert(
        other.clone(),
        40,
        vec![node(
            "src/other.ts",
            NodeKind::Function,
            "oldName",
            "oldName",
            2,
            "function:other",
        )],
    );
    index.upsert(
        user.clone(),
        35,
        vec![node(
            "src/user.ts",
            NodeKind::Function,
            "newName",
            "newName",
            4,
            "function:new",
        )],
    );

    assert_eq!(
        keys(&index, "oldName"),
        vec![("src/other.ts".to_owned(), 2, "function:other".to_owned())]
    );
    assert_eq!(
        keys(&index, "newName"),
        vec![("src/user.ts".to_owned(), 4, "function:new".to_owned())]
    );
    assert_eq!(index.byte_len(&user), Some(35));

    assert!(index.remove(&user));
    assert!(!index.remove(&user));
    assert!(index.lookup("newName").is_empty());
    assert_eq!(index.byte_len(&user), None);
    assert_eq!(index.byte_len(&other), Some(40));
    assert_eq!(index.file_count(), 1);
}

#[test]
/// Successive one-file upserts retain every file's facts and byte length.
fn successive_upserts_retain_facts_and_byte_lengths() {
    let mut index = SymbolIndex::new();
    for number in 0..50_u32 {
        let raw = format!("src/f{number}.ts");
        let name = format!("f{number}");
        let source = format!("export function {name}() {{}}\n");
        let nodes = vec![node(
            &raw,
            NodeKind::Function,
            &name,
            &name,
            1,
            &format!("function:{number}"),
        )];
        index.upsert(path(&raw), source.len(), nodes);
    }

    assert_eq!(index.file_count(), 50);
    for number in 0..50_u32 {
        let raw = format!("src/f{number}.ts");
        let name = format!("f{number}");
        let expected = format!("export function {name}() {{}}\n").len();
        assert_eq!(index.byte_len(&path(&raw)), Some(expected));
        assert_eq!(index.nodes(&path(&raw)).map(<[Node]>::len), Some(1));
        assert_eq!(keys(&index, &name).len(), 1);
    }
}
