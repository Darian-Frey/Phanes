//! Phanes — index, search, and surface relationships across a folder of
//! project-idea markdown files.
//!
//! Two rules hold the design together:
//!   1. The enrichment model runs at *index time only*; queries are always
//!      deterministic and offline (see [`indexer`]).
//!   2. Model output is *proposed*, never canonical; it fills gaps and never
//!      overwrites asserted facts (see [`model::Provenance`]).

pub mod cli;
pub mod graph;
pub mod indexer;
pub mod model;
pub mod parser;
pub mod query;
pub mod scaffold;
pub mod store;

#[cfg(feature = "enrich")]
pub mod enrich;

#[cfg(feature = "enrich")]
pub mod embed;
