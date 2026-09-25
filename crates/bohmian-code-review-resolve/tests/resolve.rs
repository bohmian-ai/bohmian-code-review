//! Resolver tests across the real index boundary.

use bohmian_code_review_core::{
    EdgeKind, Language, Node, NodeKind, RawReference, ReferenceKind, RelativePath,
};
use bohmian_code_review_index::SymbolIndex;
use bohmian_code_review_resolve::{declaration_names, reference_names, resolve};

/// Builds a TypeScript declaration fact.
fn node(file: &str, name: &str, qualified: &str, line: u32) -> Node {
    Node {
        id: format!("function:{file}:{line}"),
        kind: NodeKind::Function,
        name: name.to_owned(),
        qualified_name: qualified.to_owned(),
        file_path: file.to_owned(),
        language: Language::Typescript,
        start_line: line,
        end_line: line,
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

/// Builds a call reference with optional extractor-provided candidates.
fn reference(name: &str, candidates: Option<&[&str]>) -> RawReference {
    RawReference {
        from_node_id: "function:handler".to_owned(),
        reference_name: name.to_owned(),
        reference_kind: ReferenceKind::Edge(EdgeKind::Calls),
        line: 2,
        column: 4,
        file_path: None,
        language: None,
        candidates: candidates.map(|list| list.iter().map(|&name| name.to_owned()).collect()),
    }
}

/// Indexes one file.
///
/// # Panics
///
/// Panics if `file` is not a valid repository-relative path.
fn add(index: &mut SymbolIndex, file: &str, nodes: Vec<Node>) {
    index.upsert(RelativePath::new(file).expect("valid path"), 1, nodes);
}

/// Renders results as `(name, [path:line])`.
fn render(index: &SymbolIndex, names: Vec<&str>) -> Vec<(String, Vec<String>)> {
    resolve(index, names)
        .into_iter()
        .map(|found| {
            let hits = found
                .hits
                .iter()
                .map(|hit| format!("{}:{}", hit.path.as_str(), hit.node.start_line))
                .collect();
            (found.name, hits)
        })
        .collect()
}

#[test]
/// Reference names, extractor candidates, and declaration names resolve to
/// every exact hit; ambiguity is kept, repeats collapse, unknowns vanish.
fn all_name_sources_resolve_to_every_plausible_hit() {
    let mut index = SymbolIndex::new();
    add(
        &mut index,
        "src/user.ts",
        vec![
            node("src/user.ts", "lookupUser", "lookupUser", 1),
            node("src/user.ts", "save", "UserRepo.save", 5),
        ],
    );
    add(
        &mut index,
        "src/legacy.ts",
        vec![node("src/legacy.ts", "lookupUser", "lookupUser", 3)],
    );
    add(
        &mut index,
        "src/format.ts",
        vec![node("src/format.ts", "formatUser", "formatUser", 2)],
    );

    let simple = reference("lookupUser", None);
    let repeated = reference("lookupUser", None);
    let qualified = reference("save", Some(&["UserRepo.save"]));
    let unknown = reference("missingThing", Some(&["Nope.missingThing"]));
    let changed = node("src/handler.ts", "formatUser", "formatUser", 7);

    let mut names: Vec<&str> = Vec::new();
    for reference in [&simple, &repeated, &qualified, &unknown] {
        names.extend(reference_names(reference));
    }
    names.extend(declaration_names(&changed));

    assert_eq!(
        render(&index, names),
        vec![
            ("UserRepo.save".to_owned(), vec!["src/user.ts:5".to_owned()]),
            ("formatUser".to_owned(), vec!["src/format.ts:2".to_owned()]),
            (
                "lookupUser".to_owned(),
                vec!["src/legacy.ts:3".to_owned(), "src/user.ts:1".to_owned()]
            ),
            ("save".to_owned(), vec!["src/user.ts:5".to_owned()]),
        ]
    );
    assert!(resolve(&index, ["LookupUser"]).is_empty());
}
