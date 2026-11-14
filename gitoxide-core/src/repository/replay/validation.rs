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
        let mut buffer = gix::objs::find::Buffer::default();
        while let Some(commit_id) = pending.pop() {
            ensure!(!gix::interrupt::is_triggered(), "replay history validation interrupted");
            if commits.contains_key(&commit_id) {
                continue;
            }
            self.object(repo, commit_id, gix::objs::Kind::Commit, closure)?;
            let commit = super::read_commit(repo, commit_id, &mut buffer)?;
            let parents: Vec<_> = commit.parents().collect();
            let tree = commit.tree();
            drop(commit);
            buffer.clear();
            self.tree(repo, tree, closure)?;
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
        let source = gix::object::bounded::Source::deferred(repo);
        let mut buffer = gix::objs::find::Buffer::default();
        while let Some(tree_id) = pending.pop() {
            if !self.object(repo, tree_id, gix::objs::Kind::Tree, closure)? {
                continue;
            }
            let tree = source.load(&tree_id, &mut buffer)?;
            ensure!(tree.kind == gix::objs::Kind::Tree, "pinned object has wrong type");
            for entry in gix::objs::TreeRefIter::from_bytes(tree.data, repo.object_hash()) {
                ensure!(!gix::interrupt::is_triggered(), "replay tree validation interrupted");
                let entry = entry?;
                if entry.mode.is_tree() {
                    pending.push(entry.oid.to_owned());
                } else if !entry.mode.is_commit() {
                    self.object(repo, entry.oid.to_owned(), gix::objs::Kind::Blob, closure)?;
                }
            }
        }
        Ok(())
    }
}

/// Verify blobs by streaming and metadata through bounded storage.
pub(super) fn verify_object(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::objs::Kind> {
    use gix::objs::FindStream;
    ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
    let mut buffer = gix::objs::find::Buffer::default();
    let source = gix::object::bounded::Source::deferred(repo);
    let kind = repo.find_header(id)?.kind();
    let (kind, size, mut bytes): (_, _, Box<dyn std::io::Read + '_>) = if kind == gix::objs::Kind::Blob {
        let budget = gix::utils::stream::Budget::new(&std::env::temp_dir())?;
        let (kind, bytes) = repo
            .objects
            .find_stream(&id, &budget, &gix::interrupt::IS_INTERRUPTED)
            .with_context(|| format!("verify pinned replay object {id}"))?;
        (kind, bytes.len(), Box::new(bytes))
    } else {
        let object = source.load(&id, &mut buffer)?;
        (
            object.kind,
            object.data.len() as u64,
            Box::new(std::io::Cursor::new(object.data)),
        )
    };
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
