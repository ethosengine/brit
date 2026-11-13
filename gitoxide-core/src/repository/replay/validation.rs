//! Verify each immutable object once per selection, retaining its expected type.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

#[derive(Default)]
pub(super) struct Validation {
    checked: BTreeMap<gix::ObjectId, gix::objs::Kind>,
}

impl Validation {
    fn object(
        &mut self,
        repo: &gix::Repository,
        object_id: gix::ObjectId,
        kind: gix::objs::Kind,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<bool> {
        ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
        #[cfg(target_os = "linux")]
        super::super::switch::checkpoint("replay-selection-object")?;
        if let Some(checked_kind) = self.checked.get(&object_id) {
            ensure!(*checked_kind == kind, "pinned object has wrong type");
            return Ok(false);
        }
        ensure!(verify_object(repo, object_id)? == kind, "pinned object has wrong type");
        #[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
        super::super::switch::checkpoint("replay-object-verified")?;
        ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
        self.checked.insert(object_id, kind);
        closure.insert(object_id);
        Ok(true)
    }

    pub(super) fn history(
        &mut self,
        repo: &gix::Repository,
        tip: gix::ObjectId,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>> {
        let mut commits = BTreeMap::new();
        let mut pending = vec![tip];
        while let Some(commit_id) = pending.pop() {
            ensure!(!gix::interrupt::is_triggered(), "replay history validation interrupted");
            if commits.contains_key(&commit_id) {
                continue;
            }
            self.object(repo, commit_id, gix::objs::Kind::Commit, closure)?;
            let commit = repo.find_commit(commit_id)?;
            let parents: Vec<_> = commit.parent_ids().map(gix::Id::detach).collect();
            self.tree(repo, commit.tree_id()?.detach(), closure)?;
            pending.extend(parents.iter().copied());
            commits.insert(commit_id, parents);
        }
        Ok(commits)
    }

    fn tree(
        &mut self,
        repo: &gix::Repository,
        tree_id: gix::ObjectId,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<()> {
        let mut pending = vec![tree_id];
        while let Some(tree_id) = pending.pop() {
            if !self.object(repo, tree_id, gix::objs::Kind::Tree, closure)? {
                continue;
            }
            let tree = repo.find_tree(tree_id)?;
            for entry in tree.iter() {
                ensure!(!gix::interrupt::is_triggered(), "replay tree validation interrupted");
                let entry = entry?;
                if entry.mode().is_tree() {
                    pending.push(entry.object_id());
                } else if !entry.mode().is_commit() {
                    self.object(repo, entry.object_id(), gix::objs::Kind::Blob, closure)?;
                }
            }
        }
        Ok(())
    }
}

/// Verify blob identity through the shared bounded reader; metadata keeps its
/// structural reader and configured allocation policy.
pub(super) fn verify_object(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::objs::Kind> {
    use gix::objs::FindStream;
    ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
    let header = repo.find_header(id)?;
    if header.kind() == gix::objs::Kind::Blob {
        let budget = gix::utils::stream::Budget::new(&std::env::temp_dir())?;
        let (kind, mut bytes) = repo
            .objects
            .find_stream(&id, &budget, &gix::interrupt::IS_INTERRUPTED)
            .with_context(|| format!("verify pinned replay blob {id}"))?;
        ensure!(kind == gix::objs::Kind::Blob, "pinned object has wrong type");
        let size = bytes.len();
        ensure!(
            gix::objs::compute_stream_hash(
                repo.object_hash(),
                kind,
                &mut bytes,
                size,
                &mut gix::progress::Discard,
                &gix::interrupt::IS_INTERRUPTED
            )? == id,
            "pinned object checksum mismatch"
        );
        Ok(kind)
    } else {
        let object = repo.find_object(id)?;
        ensure!(
            gix::objs::compute_hash(repo.object_hash(), object.kind, &object.data)? == id,
            "pinned object checksum mismatch"
        );
        ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
        Ok(object.kind)
    }
}

#[cfg(target_os = "linux")]
pub(super) fn history(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>> {
    Validation::default().history(repo, tip, closure)
}

#[cfg(target_os = "linux")]
pub(super) fn validate_tree(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<()> {
    Validation::default().tree(repo, tree_id, closure)
}
