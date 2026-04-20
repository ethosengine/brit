//! Library facade for the brit-test-page binary.
//!
//! Exposes internal modules so they can be exercised by integration tests
//! under tests/. The main.rs binary wires these into its CLI surface.

pub mod coverage;
pub mod diff;
pub mod discover;
pub mod format;
pub mod normalize;
