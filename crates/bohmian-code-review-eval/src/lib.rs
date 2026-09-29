//! Context-relevance evaluation of caller-named public rankers: select each
//! caller-supplied repository's merged-PR sample, read its time-eligible
//! co-change history through the pipeline, and score leave-one-out trials
//! of every arm by hit@10 with a seeded paired bootstrap against the
//! caller's baseline arm.
//!
//! Every arm is a public [`bohmian_code_review::Ranker`] run through
//! [`bohmian_code_review::SymbolicContext::rank_with`] over the persisted
//! facts of a real base snapshot; nothing here ranks on its own. Provider
//! reads go through pipeline types only. The crate compiles only with its
//! `eval` feature.

#![cfg(feature = "eval")]

pub mod live;
pub mod report;
pub mod sample;
pub mod trial;
