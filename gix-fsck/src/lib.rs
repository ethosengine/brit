//! A library for performing object database integrity and connectivity checks
#![deny(unsafe_code, missing_docs)]

use gix_error::{OptionExt, Result, ResultExt, corruption, message};
use std::collections::VecDeque;

use gix_hash::ObjectId;
use gix_hashtable::HashSet;
use gix_object::{
    Exists, Kind,
    find::{Buffer, Source},
    tree::EntryKind,
};

/// Perform a connectivity check.
pub struct Connectivity<T, F>
where
    T: Source + Exists,
    F: FnMut(&ObjectId, Kind),
{
    /// ODB handle to use for the check
    db: T,
    /// Closure to invoke when a missing object is encountered
    missing_cb: F,
    /// Set of Object IDs already (or about to be) scanned during the check
    seen: HashSet,
    /// A buffer to keep a single object at a time.
    buf: Buffer,
}

impl<T, F> Connectivity<T, F>
where
    T: Source + Exists,
    F: FnMut(&ObjectId, Kind),
{
    /// Instantiate a connectivity check.
    pub fn new(db: T, missing_cb: F) -> Connectivity<T, F> {
        Connectivity {
            db,
            missing_cb,
            seen: HashSet::default(),
            buf: Default::default(),
        }
    }

    /// Run the connectivity check on the provided commit `oid`.
    ///
    /// ### Algorithm
    ///
    /// Walk the trees and blobs referenced by the commit and verify they exist in the ODB.
    /// Any objects previously encountered by this instance will be skipped silently.
    /// Missing referenced trees or blobs invoke `missing_cb`; a missing commit returns an error.
    /// Read and decode failures return their original error instead of reporting missing objects.
    /// After an error the visited cache is cleared, so retrying cannot accept an incomplete scan.
    pub fn check_commit(&mut self, oid: &ObjectId) -> Result {
        let result = self.check_commit_inner(oid);
        if result.is_err() {
            self.seen = HashSet::default();
            self.buf.clear();
        }
        result
    }

    fn check_commit_inner(&mut self, oid: &ObjectId) -> Result {
        // Attempt to insert the commit ID in the set, and if already present, return immediately
        if !self.seen.insert(*oid) {
            return Ok(());
        }
        // Obtain the commit's tree ID
        let tree_id = {
            let mut commit = self.db.find_commit(oid, &mut self.buf)?;
            let tree = commit
                .tree_id()
                .or_raise(|| corruption(format!("Could not decode commit {oid}")))?;
            // Retain full commit validation without retaining decoded parent/header vectors.
            for token in commit {
                token.or_raise(|| corruption(format!("Could not decode commit {oid}")))?;
            }
            tree
        };

        let mut tree_ids = VecDeque::from_iter(Some(tree_id));
        while let Some(tree_id) = tree_ids.pop_front() {
            if self.seen.insert(tree_id) {
                self.check_tree(&tree_id, &mut tree_ids)?;
            }
        }

        Ok(())
    }

    /// Blobs are checked right away, trees are stored in `tree_ids` for the parent to iterate them, and only
    /// if they have not been `seen` yet.
    fn check_tree(&mut self, oid: &ObjectId, tree_ids: &mut VecDeque<ObjectId>) -> Result {
        let Some(data) = self.db.try_find(oid, &mut self.buf)? else {
            (self.missing_cb)(oid, Kind::Tree);
            return Ok(());
        };
        let tree = data.try_into_tree_iter().ok_or_raise(|| {
            message!(
                "Expected object of kind tree but got {actual} at {oid}",
                actual = data.kind
            )
            .validation()
        })?;

        for entry_ref in tree {
            let entry_ref = entry_ref.or_raise(|| corruption(format!("Could not decode tree {oid}")))?;
            match entry_ref.mode.kind() {
                EntryKind::Tree => {
                    let tree_id = entry_ref.oid.to_owned();
                    tree_ids.push_back(tree_id);
                }
                EntryKind::Blob | EntryKind::BlobExecutable | EntryKind::Link => {
                    let blob_id = entry_ref.oid.to_owned();
                    if self.seen.insert(blob_id) {
                        check_blob(&self.db, &blob_id, &mut self.missing_cb)?;
                    }
                }
                EntryKind::Commit => {
                    // Skip submodules as they wouldn't be in this repository!
                }
            }
        }
        Ok(())
    }
}

fn check_blob<F>(db: impl Exists, oid: &ObjectId, mut missing_cb: F) -> Result
where
    F: FnMut(&ObjectId, Kind),
{
    if !db.try_exists(oid)? {
        missing_cb(oid, Kind::Blob);
    }
    Ok(())
}
