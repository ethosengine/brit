use gix::{
    ObjectId, Result,
    bstr::{BStr, BString, ByteSlice},
    diff::tree_with_rewrites::Change as RewriteChange,
    error::{OptionExt, bail, message},
    object::bounded::Source,
    objs::{Kind, find::Buffer, tree::EntryKind},
    worktree::stack::state::attributes,
};

use super::diff::revisions::{Change, changes_matching};

pub(super) struct Pickaxe<'repo, 'pattern> {
    repo: &'repo gix::Repository,
    source: &'pattern Source<'repo>,
    pathspec: gix::Pathspec<'repo>,
    needle: &'pattern [u8],
    buffer: Buffer,
    renames: bool,
}

impl<'repo, 'pattern> Pickaxe<'repo, 'pattern> {
    pub(super) fn new(
        repo: &'repo gix::Repository,
        source: &'pattern Source<'repo>,
        paths: &[BString],
        needle: &'pattern [u8],
    ) -> Result<Self> {
        let pathspec = gix::Pathspec::new(repo, false, paths, false, || {
            let index = repo.index_or_load_from_head_or_empty()?;
            Ok(repo
                .attributes_only(&index, attributes::Source::WorktreeThenIdMapping)?
                .detach())
        })?;
        let renames = repo.config_snapshot().boolean("diff.renames").unwrap_or(true);
        Ok(Self {
            repo,
            source,
            pathspec,
            needle,
            buffer: Buffer::default(),
            renames,
        })
    }

    pub(super) fn treesame(&mut self, old_tree_id: ObjectId, new_tree_id: ObjectId) -> Result<bool> {
        Ok(old_tree_id == new_tree_id || self.changes(Some(old_tree_id), new_tree_id)?.is_empty())
    }

    fn changes(&mut self, old_tree_id: Option<ObjectId>, new_tree_id: ObjectId) -> Result<Vec<Change>> {
        changes_matching(self.repo, old_tree_id, Some(new_tree_id), |path, gitlink| {
            self.pathspec.is_included(path, Some(gitlink))
        })
    }

    pub(super) fn matches(&mut self, old_tree_id: Option<ObjectId>, new_tree_id: ObjectId) -> Result<bool> {
        if old_tree_id == Some(new_tree_id) {
            return Ok(false);
        }
        let changes = self.changes(old_tree_id, new_tree_id)?;
        // Diffcore runs rename detection before pickaxe. It can only pair a deletion with an
        // addition. Treat a rename whose other endpoint lies outside the pathspec as a one-sided
        // change to the selected path.
        if self.renames
            && changes.iter().any(|change| change.old.is_none())
            && changes.iter().any(|change| change.new.is_none())
        {
            return self.matches_with_renames(old_tree_id, new_tree_id);
        }
        for change in changes {
            if self.count_changed(change.old, change.new)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn matches_with_renames(&mut self, old_tree_id: Option<ObjectId>, new_tree_id: ObjectId) -> Result<bool> {
        let changes = self.repo.diff_tree_to_tree_ids(old_tree_id, Some(new_tree_id), None)?;
        let side = |mode: gix::objs::tree::EntryMode, id| (!mode.is_tree()).then(|| (u32::from(mode.value()), id));
        for change in changes {
            let (old, new) = match change {
                RewriteChange::Addition {
                    location,
                    entry_mode,
                    id,
                    ..
                } if self.included(location.as_bstr(), entry_mode.kind()) => (None, side(entry_mode, id)),
                RewriteChange::Deletion {
                    location,
                    entry_mode,
                    id,
                    ..
                } if self.included(location.as_bstr(), entry_mode.kind()) => (side(entry_mode, id), None),
                RewriteChange::Modification {
                    location,
                    previous_entry_mode,
                    previous_id,
                    entry_mode,
                    id,
                } if self.included(location.as_bstr(), entry_mode.kind()) => {
                    (side(previous_entry_mode, previous_id), side(entry_mode, id))
                }
                RewriteChange::Rewrite {
                    source_location,
                    source_entry_mode,
                    source_id,
                    location,
                    entry_mode,
                    id,
                    ..
                } => (
                    self.included(source_location.as_bstr(), source_entry_mode.kind())
                        .then(|| side(source_entry_mode, source_id))
                        .flatten(),
                    self.included(location.as_bstr(), entry_mode.kind())
                        .then(|| side(entry_mode, id))
                        .flatten(),
                ),
                _ => continue,
            };
            if self.count_changed(old, new)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn included(&mut self, path: &BStr, kind: EntryKind) -> bool {
        self.pathspec.is_included(path, Some(kind == EntryKind::Commit))
    }

    fn count_changed(&mut self, old: Option<(u32, ObjectId)>, new: Option<(u32, ObjectId)>) -> Result<bool> {
        if old.map(|(_, id)| id) == new.map(|(_, id)| id) {
            return Ok(false);
        }
        let before = self.count(old)?;
        let after = self.count(new)?;
        if let (Some((old_mode, _)), Some((new_mode, _))) = (old, new)
            && old_mode & 0o170_000 != new_mode & 0o170_000
        {
            // A type change is a deletion and addition for pickaxe, even at the same path.
            return Ok(before != 0 || after != 0);
        }
        Ok(before != after)
    }

    fn count(&mut self, side: Option<(u32, ObjectId)>) -> Result<usize> {
        let Some((mode, blob_id)) = side else { return Ok(0) };
        if mode == 0o160_000 {
            return Ok(0);
        }
        let blob = self.source.load(&blob_id, &mut self.buffer)?;
        if blob.kind != Kind::Blob {
            bail!(message!("Expected blob {blob_id}").corrupted());
        }
        let count = blob.data.find_iter(self.needle).count();
        self.buffer.clear();
        Ok(count)
    }

    pub(super) fn tree_id(&mut self, commit_id: ObjectId) -> Result<ObjectId> {
        let tree_id = self
            .source
            .load(&commit_id, &mut self.buffer)?
            .try_into_commit_iter()
            .ok_or_raise(|| message!("Expected commit {commit_id}").corrupted())?
            .tree_id()?;
        self.buffer.clear();
        Ok(tree_id)
    }
}
