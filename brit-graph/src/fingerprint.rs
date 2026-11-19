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
    /// Gitlink (submodule) inputs: superproject path -> pinned commit id (hex).
    ///
    /// A gitlink is keyed by its pin, not by the submodule's content, so it can
    /// be computed without the submodule's objects. Kept apart from `inputs` so
    /// a gitlink can never be mistaken for a blob at the same path.
    pub gitlinks: BTreeMap<String, String>,
    /// Symlink inputs: path -> the link's own object id (hex).
    ///
    /// A symlink's blob holds its target path, so its id changes exactly when
    /// the link is retargeted. The link is not followed: the file it points
    /// to is an input only if a pattern matches that file itself.
    pub symlinks: BTreeMap<String, String>,
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

    /// Number of inputs the fingerprint covers: blobs, gitlinks and symlinks.
    #[must_use]
    pub fn input_count(&self) -> usize {
        self.inputs.len() + self.gitlinks.len() + self.symlinks.len()
    }

    /// Complete the same fingerprint after each input's content has been hashed and released.
    pub(crate) fn from_input_cids(individual: BTreeMap<String, BritCid>) -> Self {
        Self::from_inputs(individual, BTreeMap::new(), BTreeMap::new())
    }

    /// Complete a fingerprint over blob, gitlink and symlink inputs.
    ///
    /// Material: one `path NUL cid NUL` record per blob, in path order — the
    /// contract every fingerprint without gitlinks or symlinks keeps byte for
    /// byte. Then, for each non-empty kind in the order gitlink, symlink, a
    /// section that opens with a NUL byte (an empty name, which no record can
    /// have), its tag (`gitlink` or `symlink`) and NUL, followed by one
    /// `path NUL object-id-hex NUL` record per entry in path order.
    pub(crate) fn from_inputs(
        individual: BTreeMap<String, BritCid>,
        gitlinks: BTreeMap<String, String>,
        symlinks: BTreeMap<String, String>,
    ) -> Self {
        let mut combined = Vec::new();

        for (name, input_cid) in &individual {
            combined.extend_from_slice(name.as_bytes());
            combined.push(0);
            combined.extend_from_slice(input_cid.to_string().as_bytes());
            combined.push(0);
        }
        push_section(&mut combined, GITLINK_SECTION_TAG, &gitlinks);
        push_section(&mut combined, SYMLINK_SECTION_TAG, &symlinks);

        let cid = BritCid::compute(&combined);
        ContentFingerprint {
            cid,
            inputs: individual,
            gitlinks,
            symlinks,
        }
    }
}

/// Append a tagged section of `path NUL id NUL` records, or nothing when empty.
fn push_section(combined: &mut Vec<u8>, tag: &[u8], entries: &BTreeMap<String, String>) {
    if entries.is_empty() {
        return;
    }
    combined.push(0);
    combined.extend_from_slice(tag);
    combined.push(0);
    for (path, id) in entries {
        combined.extend_from_slice(path.as_bytes());
        combined.push(0);
        combined.extend_from_slice(id.as_bytes());
        combined.push(0);
    }
}

/// Domain tag of the gitlink section in fingerprint material.
const GITLINK_SECTION_TAG: &[u8] = b"gitlink";

/// Domain tag of the symlink section in fingerprint material.
const SYMLINK_SECTION_TAG: &[u8] = b"symlink";
