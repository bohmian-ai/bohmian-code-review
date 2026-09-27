//! In-memory symbol-to-file postings over extracted facts.
//!
//! [`SymbolIndex`] holds only what callers supplied: per-file declaration
//! facts and byte lengths, never source bytes. It describes the supplied
//! files alone and makes no repository coverage claim. Lookup is exact and
//! case-sensitive over simple and qualified names and returns every hit, so
//! ambiguity stays visible to later stages.

use bohmian_code_review_core::{
    DeclarationFact, IndexedFileFacts, InvalidFact, Node, NodeKind, RelativePath,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// One declaration matching an exact lookup name.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SymbolHit<'a> {
    /// Canonical path of the file declaring the symbol.
    pub path: &'a RelativePath,
    /// The extracted declaration fact (kind, language, span, ID).
    pub node: &'a Node,
}

/// The facts retained for one indexed file.
#[derive(Debug)]
struct IndexedFile {
    /// Byte length of the source the facts were extracted from.
    byte_len: usize,
    /// Extracted node facts in extractor order.
    nodes: Vec<Node>,
}

impl IndexedFile {
    /// This file's declaration facts at `path`, stamped with
    /// `extraction_version`; non-declaration nodes are dropped.
    fn export(&self, path: &RelativePath, extraction_version: &str) -> IndexedFileFacts {
        IndexedFileFacts {
            path: path.clone(),
            extraction_version: extraction_version.to_owned(),
            byte_len: self.byte_len,
            declarations: self
                .nodes
                .iter()
                .filter(|node| is_declaration(node.kind))
                .map(DeclarationFact::from_node)
                .collect(),
        }
    }
}

/// Canonical posting order: path, start line, start column, ID, then the
/// node's slot in its file so equal keys stay distinct.
type Posting = (RelativePath, u32, u32, String, usize);

/// Ephemeral in-memory symbol index keyed by canonical path.
///
/// Every mutation takes `&mut self` and completes before returning, so a
/// reader never observes a file with half-replaced postings.
#[derive(Debug, Default)]
pub struct SymbolIndex {
    /// Retained facts per file.
    files: BTreeMap<RelativePath, IndexedFile>,
    /// Exact simple and qualified names to ordered postings.
    postings: HashMap<String, BTreeSet<Posting>>,
}

impl SymbolIndex {
    /// Builds an empty index.
    #[must_use]
    pub fn new() -> Self {
        SymbolIndex::default()
    }

    /// Replaces every fact and posting for `path` with `nodes`.
    ///
    /// Old postings are removed before the new ones are inserted within this
    /// call; other files are untouched. Only `nodes` and `byte_len` are kept.
    pub fn upsert(&mut self, path: RelativePath, byte_len: usize, nodes: Vec<Node>) {
        self.remove(&path);
        for (slot, node) in nodes.iter().enumerate() {
            if !is_declaration(node.kind) {
                continue;
            }
            let posting = (
                path.clone(),
                node.start_line,
                node.start_column,
                node.id.clone(),
                slot,
            );
            for name in names(node) {
                self.postings
                    .entry(name.to_owned())
                    .or_default()
                    .insert(posting.clone());
            }
        }
        self.files.insert(path, IndexedFile { byte_len, nodes });
    }

    /// Removes every fact and posting for `path`.
    ///
    /// Idempotent: returns `false` when `path` was not indexed.
    pub fn remove(&mut self, path: &RelativePath) -> bool {
        let Some(file) = self.files.remove(path) else {
            return false;
        };
        for node in file.nodes.iter().filter(|node| is_declaration(node.kind)) {
            for name in names(node) {
                if let Some(set) = self.postings.get_mut(name) {
                    set.retain(|posting| &posting.0 != path);
                    if set.is_empty() {
                        self.postings.remove(name);
                    }
                }
            }
        }
        true
    }

    /// Returns every declaration whose simple or qualified name equals
    /// `name` exactly, in canonical path/location/ID order.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Vec<SymbolHit<'_>> {
        let Some(set) = self.postings.get(name) else {
            return Vec::new();
        };
        set.iter()
            .filter_map(|(path, _, _, _, slot)| {
                let (path, file) = self.files.get_key_value(path)?;
                Some(SymbolHit {
                    path,
                    node: file.nodes.get(*slot)?,
                })
            })
            .collect()
    }

    /// Exports every file's source-free declaration facts in canonical path
    /// order, stamped with `extraction_version`.
    ///
    /// Only declarations are exported; file, import, export, and parameter
    /// facts never post and are dropped. Loading the result with
    /// [`SymbolIndex::load`] reproduces lookup exactly.
    #[must_use]
    pub fn export(&self, extraction_version: &str) -> Vec<IndexedFileFacts> {
        self.files
            .iter()
            .map(|(path, file)| file.export(path, extraction_version))
            .collect()
    }

    /// Exports `path`'s source-free declaration facts exactly as
    /// [`SymbolIndex::export`] would, or `None` when `path` is not indexed.
    #[must_use]
    pub fn export_file(
        &self,
        path: &RelativePath,
        extraction_version: &str,
    ) -> Option<IndexedFileFacts> {
        let (path, file) = self.files.get_key_value(path)?;
        Some(file.export(path, extraction_version))
    }

    /// Validates `facts` and then replaces `facts.path`'s indexed facts with
    /// them, exactly like [`SymbolIndex::upsert`].
    ///
    /// Validation completes before the index is touched, so a rejected
    /// record leaves every file, including `facts.path`, unchanged. The
    /// extraction version is not checked here; the caller that knows its
    /// extractor compares it first.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidFact`] for an empty identity or a span that is not
    /// one-based and ascending.
    pub fn load(&mut self, facts: IndexedFileFacts) -> Result<(), InvalidFact> {
        facts.validate()?;
        let nodes = facts
            .declarations
            .into_iter()
            .map(|fact| fact.into_node(&facts.path))
            .collect();
        self.upsert(facts.path, facts.byte_len, nodes);
        Ok(())
    }

    /// Byte length retained for `path`, if indexed.
    #[must_use]
    pub fn byte_len(&self, path: &RelativePath) -> Option<usize> {
        self.files.get(path).map(|file| file.byte_len)
    }

    /// Node facts retained for `path`, if indexed.
    #[must_use]
    pub fn nodes(&self, path: &RelativePath) -> Option<&[Node]> {
        self.files.get(path).map(|file| file.nodes.as_slice())
    }

    /// Number of caller-supplied files currently indexed.
    #[must_use]
    pub fn file_count(&self) -> usize {
        self.files.len()
    }
}

/// Whether a node kind declares a symbol other files can reference.
///
/// File, import, export, and parameter nodes name uses or locals, not
/// declarations, so posting them would fabricate candidate files.
pub fn is_declaration(kind: NodeKind) -> bool {
    !matches!(
        kind,
        NodeKind::File | NodeKind::Import | NodeKind::Export | NodeKind::Parameter
    )
}

/// The distinct exact names a declaration is posted under.
fn names(node: &Node) -> impl Iterator<Item = &str> {
    let qualified = (node.qualified_name != node.name).then_some(node.qualified_name.as_str());
    std::iter::once(node.name.as_str()).chain(qualified)
}

#[cfg(test)]
/// Owner tests for exact postings, deterministic order, and per-file state.
mod tests {
    use super::*;
    use bohmian_code_review_core::Language;

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
    /// Export drops non-declaration facts and orders files by path; loading
    /// the export into a fresh index reproduces lookup and byte lengths, and
    /// an invalid record is rejected before any postings change.
    fn export_round_trips_through_load_and_rejects_invalid_records() {
        let mut index = SymbolIndex::new();
        index.upsert(
            path("src/z.ts"),
            10,
            vec![
                node("src/z.ts", NodeKind::File, "z.ts", "src/z.ts", 1, "file:z"),
                node(
                    "src/z.ts",
                    NodeKind::Function,
                    "load",
                    "load",
                    1,
                    "function:z",
                ),
            ],
        );
        index.upsert(
            path("src/a.ts"),
            20,
            vec![node(
                "src/a.ts",
                NodeKind::Method,
                "load",
                "Repo::load",
                9,
                "method:a",
            )],
        );
        let exported = index.export("v1");
        let summary: Vec<(&str, &str, usize, usize)> = exported
            .iter()
            .map(|facts| {
                (
                    facts.path.as_str(),
                    facts.extraction_version.as_str(),
                    facts.byte_len,
                    facts.declarations.len(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            [("src/a.ts", "v1", 20, 1), ("src/z.ts", "v1", 10, 1)]
        );

        let mut loaded = SymbolIndex::new();
        for facts in exported.clone() {
            loaded.load(facts).expect("valid facts");
        }
        assert_eq!(keys(&loaded, "load"), keys(&index, "load"));
        assert_eq!(keys(&loaded, "Repo::load"), keys(&index, "Repo::load"));
        assert_eq!(loaded.byte_len(&path("src/z.ts")), Some(10));
        assert_eq!(loaded.export("v1"), exported);

        let mut invalid = exported[1].clone();
        invalid.declarations[0].end_line = 0;
        assert!(matches!(
            loaded.load(invalid),
            Err(InvalidFact::InvalidSpan { .. })
        ));
        assert_eq!(keys(&loaded, "load"), keys(&index, "load"));
        assert_eq!(loaded.file_count(), 2);
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
}
