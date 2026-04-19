//! brit-graph — EPR-native graph engine.
//!
//! Provides DAG construction with BritCid-keyed nodes, affected tracking
//! with provenance, content fingerprinting, and topological planning.
//! Pure computation — no IO, no git, no network.
//!
//! Any type implementing `ContentNode` (from brit-epr) can be a graph node.

#![deny(missing_docs, rust_2018_idioms)]
#![forbid(unsafe_code)]

pub mod graph;
pub mod traits;
pub mod affected;
pub mod fingerprint;
pub mod topo;
