//! IO-free extraction facts and validated boundary values.
//!
//! This crate owns the facts every later stage shares: the [`Language`]
//! contract, extracted graph facts ([`Node`], [`Edge`], [`RawReference`]),
//! extraction diagnostics, one-based source positions, and the canonical
//! repository-relative [`RelativePath`]. Values are ported unchanged from the
//! donor extractor so downstream crates consume one definition instead of
//! parallel representations. The crate performs no IO.

/// Contains structured extraction diagnostics shared by parser consumers.
pub mod diagnostic;
/// Contains extracted edge facts and reference kinds.
pub mod edge;
/// Contains the closed language contract.
pub mod language;
/// Contains the path byte limit enforced by [`RelativePath`].
pub mod limits;
/// Contains extracted node facts.
pub mod node;
/// Contains one-based line source coordinates.
pub mod position;
/// Contains unresolved raw reference facts.
pub mod reference;
/// Contains the canonical repository-relative path value.
pub mod relative_path;

pub use diagnostic::{ExtractionDiagnostic, Severity};
pub use edge::{Edge, EdgeKind, Provenance, ReferenceKind};
pub use language::{LANGUAGE_NAMES, Language, NATIVE_LANGUAGES};
pub use node::{Node, NodeKind, Visibility};
pub use position::{SourcePosition, SourceRange};
pub use reference::RawReference;
pub use relative_path::{InvalidRelativePath, RelativePath};
