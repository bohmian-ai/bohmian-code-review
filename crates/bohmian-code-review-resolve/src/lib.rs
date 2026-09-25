//! Ambiguity-preserving candidate lookup from reference names to indexed symbols.
//!
//! Names come from three approved sources: an extracted reference's exact
//! `reference_name`, its extractor-provided qualified `candidates`, and a
//! changed declaration's simple and qualified names. Each distinct name is
//! looked up exactly in [`SymbolIndex`]; every hit is kept. A hit is
//! candidate context, never a proven semantic definition, and the result
//! covers only files the caller indexed.

use bohmian_code_review_core::{Node, RawReference};
use bohmian_code_review_index::{SymbolHit, SymbolIndex};
use std::collections::BTreeSet;

/// Every index hit for one distinct query name.
#[derive(Debug, Clone, PartialEq)]
pub struct NameCandidates<'a> {
    /// The exact query name.
    pub name: String,
    /// All hits in the index's canonical path/location/ID order; more than
    /// one hit means the name is ambiguous.
    pub hits: Vec<SymbolHit<'a>>,
}

/// The query names an extracted reference contributes: its exact
/// `reference_name` followed by any extractor-provided candidates.
pub fn reference_names(reference: &RawReference) -> impl Iterator<Item = &str> {
    std::iter::once(reference.reference_name.as_str())
        .chain(reference.candidates.iter().flatten().map(String::as_str))
}

/// The query names a changed declaration contributes: its simple and
/// qualified names.
pub fn declaration_names(node: &Node) -> impl Iterator<Item = &str> {
    [node.name.as_str(), node.qualified_name.as_str()].into_iter()
}

/// Looks up each distinct name exactly and returns those with at least one
/// hit, ordered by name.
///
/// Repeated names collapse to one entry; unknown names produce no entry
/// rather than a fabricated target.
#[must_use]
pub fn resolve<'i, 'n>(
    index: &'i SymbolIndex,
    names: impl IntoIterator<Item = &'n str>,
) -> Vec<NameCandidates<'i>> {
    names
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter_map(|name| {
            let hits = index.lookup(name);
            (!hits.is_empty()).then(|| NameCandidates {
                name: name.to_owned(),
                hits,
            })
        })
        .collect()
}
