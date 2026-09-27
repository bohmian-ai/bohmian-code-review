//! Synchronous parsing and raw fact extraction for codegraph-rs.
//!
//! This crate ports the frozen upstream native extraction kernel
//! (`codegraph-kernel/` at `572d22bf`) into an ordinary synchronous Rust
//! `rlib`. The per-language tree-sitter walkers (`java`, `python`, `tsjs`,
//! …) emit the upstream flat-buffer tables verbatim ([`buffers`]), and
//! [`Extractor::extract`] decodes them into typed native facts
//! ([`ExtractionResult`]). Extraction semantics — node IDs, names, qualified
//! names, signatures, metadata, positions, edges, raw references, ordering,
//! optionality — are preserved
//! exactly. Only the NAPI surface is gone: a development-only adapter under
//! `tools/oracle` re-exposes conversion over the same `Extractor`.
//!
//! Parsing is synchronous by design: callers parallelize per file through a
//! bounded worker pool and never hold a database transaction during parsing.
//! Grammar revisions are frozen at the upstream pins (see `Cargo.toml` and
//! `build.rs`); changing them silently changes graph facts and fails the
//! grammar-parity gates.
//!
//! The codec ([`decode`]/[`encode`]) mirrors `src/extraction/kernel/decode.ts`
//! and the buffer writers in the frozen kernel; it is the wire contract
//! shared with the development NAPI adapter.

pub mod buffers;
/// Contains the cfnptr domain types and algorithms re-exported through this crate boundary.
pub mod cfnptr;
/// Contains the decode domain types and algorithms re-exported through this crate boundary.
pub mod decode;
/// Contains the encode domain types and algorithms re-exported through this crate boundary.
pub mod encode;
/// Contains the error domain types and algorithms re-exported through this crate boundary.
pub mod error;
/// Contains the langs domain types and algorithms re-exported through this crate boundary.
pub mod langs;

/// Contains the arkts domain types and algorithms re-exported through this crate boundary.
mod arkts;
/// Contains the ccpp domain types and algorithms re-exported through this crate boundary.
mod ccpp;
/// Contains the cfquery domain types and algorithms re-exported through this crate boundary.
mod cfquery;
/// Contains the cfscript domain types and algorithms re-exported through this crate boundary.
mod cfscript;
/// Contains the cobol domain types and algorithms re-exported through this crate boundary.
mod cobol;
/// Contains the composite/Twig extraction domain types and algorithms re-exported through this crate boundary.
pub mod composite;
/// Contains the config domain types and algorithms re-exported through this crate boundary.
mod config;
/// Contains the csharp domain types and algorithms re-exported through this crate boundary.
mod csharp;
/// Contains the dart domain types and algorithms re-exported through this crate boundary.
mod dart;
/// Contains the docstring domain types and algorithms re-exported through this crate boundary.
mod docstring;
/// Contains the erlang domain types and algorithms re-exported through this crate boundary.
mod erlang;
/// Contains the extractor domain types and algorithms re-exported through this crate boundary.
mod extractor;
/// Contains the go domain types and algorithms re-exported through this crate boundary.
mod go;
/// Contains the ids domain types and algorithms re-exported through this crate boundary.
mod ids;
/// Contains the java domain types and algorithms re-exported through this crate boundary.
mod java;
/// Contains the kotlin domain types and algorithms re-exported through this crate boundary.
mod kotlin;
/// Contains the lua domain types and algorithms re-exported through this crate boundary.
mod lua;
/// Contains the nix domain types and algorithms re-exported through this crate boundary.
mod nix;
/// Contains the objc domain types and algorithms re-exported through this crate boundary.
mod objc;
/// Contains the bounded parse pool domain types and algorithms re-exported through this crate boundary.
pub mod parse_pool;
/// Contains the pascal domain types and algorithms re-exported through this crate boundary.
mod pascal;
/// Contains the php domain types and algorithms re-exported through this crate boundary.
mod php;
/// Contains the python domain types and algorithms re-exported through this crate boundary.
mod python;
/// Contains the result domain types and algorithms re-exported through this crate boundary.
mod result;
/// Contains the rlang domain types and algorithms re-exported through this crate boundary.
mod rlang;
/// Contains the routing domain types and algorithms re-exported through this crate boundary.
mod routing;
/// Contains the ruby domain types and algorithms re-exported through this crate boundary.
mod ruby;
/// Contains the rustlang domain types and algorithms re-exported through this crate boundary.
mod rustlang;
/// Contains the scala domain types and algorithms re-exported through this crate boundary.
mod scala;
/// Contains the solidity domain types and algorithms re-exported through this crate boundary.
mod solidity;
/// Contains the source file domain types and algorithms re-exported through this crate boundary.
mod source_file;
/// Contains the swift domain types and algorithms re-exported through this crate boundary.
mod swift;
/// Contains the terraform domain types and algorithms re-exported through this crate boundary.
mod terraform;
/// Contains the textutil domain types and algorithms re-exported through this crate boundary.
mod textutil;
/// Contains the tsjs domain types and algorithms re-exported through this crate boundary.
mod tsjs;
/// Contains the vbnet domain types and algorithms re-exported through this crate boundary.
mod vbnet;

#[cfg(test)]
/// Verifies that corpus tests preserves the frozen extraction contract.
mod corpus_tests;

/// Identity of the extracted-fact contract produced by this crate.
///
/// Persisted facts are stamped with this value and refused when it differs.
/// Bump it whenever a grammar pin in `Cargo.toml`, a walker, the decoder, or
/// the fact projection changes what is extracted; the extraction fixtures
/// under `corpus_tests` are the evidence such a change occurred.
pub const EXTRACTION_VERSION: &str =
    "codegraph-kernel-572d22bf.tree-sitter-0.25.10.recovered-tree.facts-2";

pub use composite::{CompositeExtractor, CompositeKind, DiscoveredSource};
pub use config::ExtractionConfig;
pub use error::ExtractionError;
pub use extractor::Extractor;
pub use parse_pool::{ParseOutcome, ParsePool, ParseRequest};
pub use result::ExtractionResult;
pub use routing::LanguageRouter;
pub use source_file::SourceFile;

pub use bohmian_code_review_core::{ExtractionDiagnostic, Language};
