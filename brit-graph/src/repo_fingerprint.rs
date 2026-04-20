//! Repo-aware fingerprint constructor (feature: `repo`).
//!
//! Builds a `ContentFingerprint` from file contents resolved through glob
//! patterns against a git tree at a specific commit. Reads blobs from the
//! tree, NOT the working tree — fingerprints are reproducible across machines
//! given the same commit and patterns.

use std::collections::{BTreeMap, VecDeque};

use globset::{Glob, GlobSetBuilder};
use gix::bstr::{BStr, BString, ByteSlice, ByteVec};

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
            path: BString::default(),
            path_deque: VecDeque::new(),
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
///
/// Path tracking mirrors `gix`'s own `Recorder` — a deque holds the full
/// path snapshot for each pending subtree so that breadth-first traversal
/// can restore the correct prefix when it descends into a subdirectory.
struct TreeCollector<'a> {
    repo: &'a gix::Repository,
    globset: &'a globset::GlobSet,
    inputs: &'a mut BTreeMap<String, Vec<u8>>,
    errors: &'a mut Vec<String>,
    /// The current path prefix (built up as we traverse).
    path: BString,
    /// Snapshot queue: one entry per pending subtree (breadth-first).
    path_deque: VecDeque<BString>,
}

impl<'a> TreeCollector<'a> {
    fn push_element(&mut self, component: &BStr) {
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(component);
    }

    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }
}

impl<'a> gix::traverse::tree::Visit for TreeCollector<'a> {
    /// Restore path from the back of the deque (depth-first; not used here).
    fn pop_back_tracked_path_and_set_current(&mut self) {
        self.path = self.path_deque.pop_back().unwrap_or_default();
    }

    /// Restore path from the front of the deque (breadth-first descent).
    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every push_back_tracked_path_component has a matching pop_front");
    }

    /// Called for tree entries that will be queued for later traversal.
    /// Save the current path (with the directory component) so it can be
    /// restored when we descend into this subtree.
    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    /// Called for every entry (tree or nontree) before visit_*.
    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    /// Called after every entry (tree or nontree).
    fn pop_path_component(&mut self) {
        self.pop_element();
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

        // At this point self.path already contains the full path (push_path_component
        // was called with entry.filename before visit_nontree).
        let path_str = match std::str::from_utf8(&self.path) {
            Ok(s) => s.to_string(),
            Err(_) => {
                self.errors
                    .push(format!("non-utf8 path: {:?}", self.path.as_bstr()));
                return std::ops::ControlFlow::Continue(true);
            }
        };

        // Glob match against the full path
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
