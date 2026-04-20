//! Repo-aware fingerprint constructor (feature: `repo`).
//!
//! Builds a `ContentFingerprint` from file contents resolved through glob
//! patterns against a git tree at a specific commit. Reads blobs from the
//! tree, NOT the working tree — fingerprints are reproducible across machines
//! given the same commit and patterns.

use std::collections::BTreeMap;

use globset::{Glob, GlobSetBuilder};

use crate::fingerprint::{ContentFingerprint, FingerprintError};

impl ContentFingerprint {
    /// Compute a fingerprint by reading files from a git tree at a specific
    /// commit, matching against the given glob patterns.
    ///
    /// Files are read from the git tree (not the working tree) for
    /// reproducibility. Same commit + same patterns = same fingerprint,
    /// regardless of working-tree state.
    ///
    /// Submodule entries and symlinks in the tree are skipped (only regular
    /// blobs and executable blobs are included). Empty pattern set or no
    /// matching files produces a stable empty-input fingerprint.
    pub fn from_repo_globs(
        repo: &gix::Repository,
        commit_id: gix::ObjectId,
        patterns: &[String],
    ) -> Result<Self, FingerprintError> {
        // Step A: build the GlobSet
        let mut builder = GlobSetBuilder::new();
        for pattern in patterns {
            let glob = Glob::new(pattern).map_err(|e| FingerprintError::InvalidGlob {
                pattern: pattern.clone(),
                message: e.to_string(),
            })?;
            builder.add(glob);
        }
        let globset = builder
            .build()
            .map_err(|e| FingerprintError::InvalidGlob {
                pattern: patterns.join(", "),
                message: e.to_string(),
            })?;

        // Step B: open the tree at the commit
        let object = repo
            .find_object(commit_id)
            .map_err(|e| FingerprintError::CommitResolve {
                commit: commit_id.to_hex().to_string(),
                message: e.to_string(),
            })?;
        let commit = object
            .try_into_commit()
            .map_err(|e| FingerprintError::CommitResolve {
                commit: commit_id.to_hex().to_string(),
                message: format!("not a commit: {e}"),
            })?;
        let tree = commit
            .tree()
            .map_err(|e| FingerprintError::TreeWalk(e.to_string()))?;

        // Step C: walk the tree, collect matching (path, blob_bytes)
        let mut inputs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut walk_errors: Vec<String> = Vec::new();

        let mut visitor = TreeCollector {
            repo,
            globset: &globset,
            inputs: &mut inputs,
            errors: &mut walk_errors,
            path_prefix: Vec::new(),
        };

        tree.traverse()
            .breadthfirst(&mut visitor)
            .map_err(|e| FingerprintError::TreeWalk(e.to_string()))?;

        if !walk_errors.is_empty() {
            return Err(FingerprintError::TreeWalk(walk_errors.join("; ")));
        }

        // Step D: delegate to existing pure compute
        Ok(Self::compute(&inputs))
    }
}

/// Visitor that walks a tree, matches paths against globs, and collects
/// blob contents for matching files.
struct TreeCollector<'a> {
    repo: &'a gix::Repository,
    globset: &'a globset::GlobSet,
    inputs: &'a mut BTreeMap<String, Vec<u8>>,
    errors: &'a mut Vec<String>,
    path_prefix: Vec<u8>,
}

impl<'a> gix::traverse::tree::Visit for TreeCollector<'a> {
    fn pop_back_tracked_path_and_set_current(&mut self) {}
    fn pop_front_tracked_path_and_set_current(&mut self) {}
    fn push_back_tracked_path_component(&mut self, _component: &gix::bstr::BStr) {}
    fn push_path_component(&mut self, component: &gix::bstr::BStr) {
        if !self.path_prefix.is_empty() {
            self.path_prefix.push(b'/');
        }
        self.path_prefix.extend_from_slice(component);
    }
    fn pop_path_component(&mut self) {
        if let Some(slash_pos) = self.path_prefix.iter().rposition(|&b| b == b'/') {
            self.path_prefix.truncate(slash_pos);
        } else {
            self.path_prefix.clear();
        }
    }

    fn visit_tree(
        &mut self,
        _entry: &gix::objs::tree::EntryRef<'_>,
    ) -> gix::traverse::tree::visit::Action {
        // Continue(true) = descend into this subtree
        std::ops::ControlFlow::Continue(true)
    }

    fn visit_nontree(
        &mut self,
        entry: &gix::objs::tree::EntryRef<'_>,
    ) -> gix::traverse::tree::visit::Action {
        // Skip submodules and symlinks
        if !matches!(
            entry.mode.kind(),
            gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable
        ) {
            return std::ops::ControlFlow::Continue(true);
        }

        // Build full path
        let mut full_path = self.path_prefix.clone();
        if !full_path.is_empty() {
            full_path.push(b'/');
        }
        full_path.extend_from_slice(entry.filename);

        // UTF-8 conversion
        let path_str = match std::str::from_utf8(&full_path) {
            Ok(s) => s.to_string(),
            Err(_) => {
                self.errors.push(format!("non-utf8 path: {:?}", full_path));
                return std::ops::ControlFlow::Continue(true);
            }
        };

        // Glob match against the path
        if !self.globset.is_match(&path_str) {
            return std::ops::ControlFlow::Continue(true);
        }

        // Read the blob
        match self.repo.find_object(entry.oid) {
            Ok(obj) => {
                let bytes = obj.data.clone();
                self.inputs.insert(path_str, bytes);
            }
            Err(e) => {
                self.errors.push(format!("read {}: {e}", path_str));
            }
        }

        std::ops::ControlFlow::Continue(true)
    }
}
