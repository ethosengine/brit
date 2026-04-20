//! cli-journey — CLI integration test infrastructure.
//!
//! Provides reusable helpers for testing the brit-workspace binaries:
//!   - `TestRepo` — temp git repo with deterministic commit history
//!   - `MockRemote` — bare git repo at temp path, file:// URL
//!   - `Normalizer` — redacts variable output (tempdirs, SHAs, timestamps)
//!   - `BritInvocation` — process invocation + capture + normalization
//!
//! Tests under `tests/` use these helpers to exercise rakia, brit-verify,
//! and brit-build-ref subcommands. The cli-test-page runner reads the
//! captured outputs from the staging directory.

pub mod support;
