//! Brit adapter for `eprfs`.
//!
//! This crate maps git repository concepts into `eprfs-core` projection
//! manifests. It does not fetch bytes from the network, publish attestations,
//! or materialize files. Those concerns belong below this adapter.

mod project;
mod storage;

pub use project::{project_tree, project_tree_from_repo, BritEprfsError, Result};
pub use storage::GitObjectStorage;
