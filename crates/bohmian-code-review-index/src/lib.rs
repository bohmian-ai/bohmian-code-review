//! In-memory symbol-to-file postings over extracted facts.
//!
//! [`SymbolIndex`] holds only what callers supplied: per-file declaration
//! facts and byte lengths, never source bytes. It describes the supplied
//! files alone and makes no repository coverage claim. Lookup is exact and
//! case-sensitive over simple and qualified names and returns every hit, so
//! ambiguity stays visible to later stages.

use bohmian_code_review_core::{Node, NodeKind, RelativePath};
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
fn is_declaration(kind: NodeKind) -> bool {
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
