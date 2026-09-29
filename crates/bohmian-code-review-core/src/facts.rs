//! Source-free, persistable declaration facts for one indexed file.
//!
//! [`IndexedFileFacts`] is the projection an index exports and reloads
//! without reparsing. It keeps only what exact lookup, ranking reasons, and
//! candidate spans need: names, IDs, kinds, languages, and line/column
//! coordinates, plus the distinct names the file references, without their
//! positions. Docstrings, signatures, decorators, return types, and any
//! other verbatim source fragment are deliberately absent, so a persisted
//! record never carries source text.

use crate::language::Language;
use crate::node::{Node, NodeKind};
use crate::relative_path::RelativePath;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One declaration as retained for lookup, ranking, and span selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationFact {
    /// Stable extractor-assigned node ID.
    pub id: String,
    /// Declaration kind.
    pub kind: NodeKind,
    /// Simple name posted for exact lookup.
    pub name: String,
    /// Qualified name posted for exact lookup when it differs from `name`.
    pub qualified_name: String,
    /// Language the declaration was extracted as.
    pub language: Language,
    /// One-based first line of the declaration.
    pub start_line: u32,
    /// One-based last line, inclusive.
    pub end_line: u32,
    /// Zero-based start column, used only for canonical posting order.
    pub start_column: u32,
}

impl DeclarationFact {
    /// Projects an extracted node onto its source-free fact.
    #[must_use]
    pub fn from_node(node: &Node) -> Self {
        DeclarationFact {
            id: node.id.clone(),
            kind: node.kind,
            name: node.name.clone(),
            qualified_name: node.qualified_name.clone(),
            language: node.language,
            start_line: node.start_line,
            end_line: node.end_line,
            start_column: node.start_column,
        }
    }

    /// Rebuilds the node an index needs for `path`, leaving every
    /// source-derived optional field empty.
    #[must_use]
    pub fn into_node(self, path: &RelativePath) -> Node {
        Node {
            id: self.id,
            kind: self.kind,
            name: self.name,
            qualified_name: self.qualified_name,
            file_path: path.as_str().to_owned(),
            language: self.language,
            start_line: self.start_line,
            end_line: self.end_line,
            start_column: self.start_column,
            end_column: 0,
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

    /// Checks the invariants lookup and span selection rely on.
    ///
    /// # Errors
    ///
    /// Returns [`InvalidFact`] when the ID, name, or qualified name is
    /// empty, or the line span is not one-based and ascending.
    pub fn validate(&self) -> Result<(), InvalidFact> {
        if self.id.is_empty() || self.name.is_empty() || self.qualified_name.is_empty() {
            return Err(InvalidFact::EmptyIdentity {
                id: self.id.clone(),
            });
        }
        if self.start_line == 0 || self.start_line > self.end_line {
            return Err(InvalidFact::InvalidSpan {
                id: self.id.clone(),
                start_line: self.start_line,
                end_line: self.end_line,
            });
        }
        Ok(())
    }
}

/// The persistable facts of one indexed file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFileFacts {
    /// Canonical repository-relative path; deserialization revalidates it.
    pub path: RelativePath,
    /// Version of the extractor that produced `declarations`.
    pub extraction_version: String,
    /// Byte length of the source the facts were extracted from.
    pub byte_len: usize,
    /// Declarations in extractor order.
    pub declarations: Vec<DeclarationFact>,
    /// Distinct reference and extractor-candidate names the file uses, in
    /// ascending order; names only, never positions or source.
    pub references: Vec<String>,
}

impl IndexedFileFacts {
    /// Validates every declaration before any of them is trusted.
    ///
    /// # Errors
    ///
    /// Returns the first [`InvalidFact`] found.
    pub fn validate(&self) -> Result<(), InvalidFact> {
        self.declarations
            .iter()
            .try_for_each(DeclarationFact::validate)
    }
}

/// Why a persisted declaration fact was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InvalidFact {
    /// The ID, name, or qualified name is empty.
    #[error("declaration `{id}` has an empty id, name, or qualified name")]
    EmptyIdentity {
        /// The offending declaration's ID, possibly empty.
        id: String,
    },
    /// The line span is zero-based or reversed.
    #[error("declaration `{id}` span {start_line}..={end_line} is not one-based and ascending")]
    InvalidSpan {
        /// The offending declaration's ID.
        id: String,
        /// Supplied start line.
        start_line: u32,
        /// Supplied end line.
        end_line: u32,
    },
}

#[cfg(test)]
/// Owner tests for the source-free projection and its validation.
mod tests {
    use super::*;

    /// Builds a node carrying source fragments that must not survive export.
    fn node() -> Node {
        Node {
            id: "function:src/a.ts:load".to_owned(),
            kind: NodeKind::Function,
            name: "load".to_owned(),
            qualified_name: "Repo::load".to_owned(),
            file_path: "src/a.ts".to_owned(),
            language: Language::Typescript,
            start_line: 3,
            end_line: 5,
            start_column: 2,
            end_column: 1,
            docstring: Some("loads".to_owned()),
            signature: Some("load(): void".to_owned()),
            visibility: None,
            is_exported: Some(true),
            is_async: None,
            is_static: None,
            is_abstract: None,
            decorators: None,
            type_parameters: None,
            return_type: Some("void".to_owned()),
            extra: None,
        }
    }

    #[test]
    /// Projection keeps identity and coordinates, drops source fragments,
    /// and rebuilds a node bound to the supplied path.
    fn projection_is_source_free_and_rebuilds_a_node() {
        let fact = DeclarationFact::from_node(&node());
        let json = serde_json::to_string(&fact).expect("serializes");
        assert!(!json.contains("loads") && !json.contains("void"));
        let path = RelativePath::new("src/moved.ts").expect("valid path");
        let rebuilt = fact.clone().into_node(&path);
        assert_eq!(rebuilt.file_path, "src/moved.ts");
        assert_eq!((&rebuilt.docstring, &rebuilt.signature), (&None, &None));
        assert_eq!(DeclarationFact::from_node(&rebuilt), fact);
    }

    #[test]
    /// Empty identity fields and zero-based or reversed spans are rejected.
    fn validation_rejects_empty_identity_and_bad_spans() {
        let fact = DeclarationFact::from_node(&node());
        assert_eq!(fact.validate(), Ok(()));
        let mut empty = fact.clone();
        empty.qualified_name.clear();
        assert!(matches!(
            empty.validate(),
            Err(InvalidFact::EmptyIdentity { .. })
        ));
        let mut zero = fact.clone();
        zero.start_line = 0;
        assert!(matches!(
            zero.validate(),
            Err(InvalidFact::InvalidSpan { .. })
        ));
        let mut reversed = fact;
        reversed.end_line = 2;
        assert_eq!(
            reversed.validate(),
            Err(InvalidFact::InvalidSpan {
                id: "function:src/a.ts:load".to_owned(),
                start_line: 3,
                end_line: 2,
            })
        );
    }
}
