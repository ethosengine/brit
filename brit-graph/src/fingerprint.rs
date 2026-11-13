//! Content fingerprinting — deterministic hash over named inputs.
//!
//! A fingerprint is a `BritCid` computed from a sorted map of named inputs.
//! Same inputs always produce the same fingerprint, regardless of insertion order.

use std::collections::BTreeMap;

use brit_epr::BritCid;

/// Errors produced by repo-aware fingerprint construction.
///
/// Exported regardless of the `repo` feature so downstream code can match
/// on it without conditional compilation. The variants only get instantiated
/// when the `repo` feature is enabled and `ContentFingerprint::from_repo_globs`
/// is called.
#[derive(Debug, thiserror::Error)]
pub enum FingerprintError {
    /// Glob pattern compilation failed.
    #[error("invalid glob pattern '{pattern}': {message}")]
    InvalidGlob {
        /// The pattern that failed to compile.
        pattern: String,
        /// The underlying error message.
        message: String,
    },
    /// Resolving the commit ref failed.
    #[error("failed to resolve commit '{commit}': {message}")]
    CommitResolve {
        /// The ref or SHA being resolved.
        commit: String,
        /// The underlying error message.
        message: String,
    },
    /// Walking the git tree failed.
    #[error("tree walk failed: {0}")]
    TreeWalk(String),
    /// Reading a blob's bytes failed.
    #[error("failed to read blob at '{path}': {message}")]
    BlobRead {
        /// The repo-relative path whose blob couldn't be read.
        path: String,
        /// The underlying error message.
        message: String,
    },
    /// Path was not valid UTF-8.
    #[error("non-UTF-8 path in tree: {0:?}")]
    NonUtf8Path(Vec<u8>),
}

/// A deterministic content fingerprint over named inputs.
#[derive(Debug, Clone)]
pub struct ContentFingerprint {
    /// The overall fingerprint CID.
    pub cid: BritCid,
    /// Individual input hashes (name -> CID of that input's bytes).
    pub inputs: BTreeMap<String, BritCid>,
}

impl ContentFingerprint {
    /// Compute a fingerprint from a map of named inputs.
    ///
    /// Keys are sorted (BTreeMap guarantees this). Each input's bytes are
    /// individually hashed, then all hashes are concatenated with their keys
    /// and hashed again to produce the overall fingerprint.
    #[must_use]
    pub fn compute(inputs: &BTreeMap<String, Vec<u8>>) -> Self {
        Self::from_input_cids(
            inputs
                .iter()
                .map(|(name, bytes)| (name.clone(), BritCid::compute(bytes)))
                .collect(),
        )
    }

    /// Complete the same fingerprint after each input's content has been hashed and released.
    pub(crate) fn from_input_cids(individual: BTreeMap<String, BritCid>) -> Self {
        let mut combined = Vec::new();

        for (name, input_cid) in &individual {
            combined.extend_from_slice(name.as_bytes());
            combined.push(0);
            combined.extend_from_slice(input_cid.to_string().as_bytes());
            combined.push(0);
        }

        let cid = BritCid::compute(&combined);
        ContentFingerprint {
            cid,
            inputs: individual,
        }
    }
}
