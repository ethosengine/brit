//! Internal durable linear replay; public command dispatch remains disabled.
//!
//! Read-only selection proves pinned local history. Linux sessions preserve the
//! original staging/worktree distinction through separately journaled decisions.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use gix::bstr::{BStr, BString, ByteSlice};

#[cfg(target_os = "linux")]
mod hooks;

#[cfg(target_os = "linux")]
pub mod retained;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionKind {
    Noop,
    FastForward,
    Replay,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub commit: gix::ObjectId,
    pub parent: gix::ObjectId,
}

#[derive(Clone, Debug)]
pub struct Selection {
    pub original: gix::ObjectId,
    pub upstream: gix::ObjectId,
    pub attachment: Option<BString>,
    pub argument: BString,
    pub candidates: Vec<Candidate>,
    pub kind: SelectionKind,
    /// Native hash-validated object identities required by both pinned histories.
    pub closure: BTreeSet<gix::ObjectId>,
}

pub(crate) fn validate_execution_policy(repo: &gix::Repository) -> Result<()> {
    ensure!(
        !repo.config_snapshot().try_boolean("commit.gpgSign")?.unwrap_or(false),
        "required signing is unsupported"
    );
    ensure!(
        gix::filter::Pipeline::options(repo)?.drivers.is_empty(),
        "external filters are unsupported"
    );
    if let Some(sections) = repo.config_snapshot().plumbing().sections_by_name("merge") {
        ensure!(
            !sections
                .into_iter()
                .any(|section| section.header().subsection_name().is_some() && section.contains_value_name("driver")),
            "external merge drivers are unsupported"
        );
    }
    Ok(())
}

impl Selection {
    /// Resolve only explicit, local revisions and prove the selected linear chain.
    /// This function is read-only, including for no-op and fast-forward results.
    pub fn prepare(repo: &gix::Repository, upstream: &BStr) -> Result<Self> {
        ensure!(!upstream.is_empty(), "an explicit local upstream is required");
        validate_execution_policy(repo)?;
        #[cfg(target_os = "linux")]
        {
            super::switch::validate_local_history(repo)?;
            super::switch::validate_object_boundaries(repo)?;
        }
        super::switch::ensure_no_pending(repo)?;
        ensure!(repo.state().is_none(), "foreign operation in progress");
        ensure!(!repo.is_shallow()?, "shallow history cannot prove replay selection");
        ensure!(
            std::env::var_os("GIT_INDEX_FILE").is_none(),
            "alternate indexes are unsupported"
        );
        let original = repo.head_commit()?.id;
        let attachment = repo.head_name()?.map(|name| name.as_bstr().to_owned());
        let upstream_id = resolve_upstream(repo, upstream)?;
        let mut closure = BTreeSet::new();
        let ours = history(repo, original, &mut closure)?;
        let theirs = history(repo, upstream_id, &mut closure)?;
        let bases = repo.merge_bases_many(original, &[upstream_id])?;
        ensure!(bases.len() == 1, "replay requires exactly one merge base");
        let kind = if theirs.contains_key(&original) && original != upstream_id {
            SelectionKind::FastForward
        } else if ours.contains_key(&upstream_id) {
            SelectionKind::Noop
        } else {
            SelectionKind::Replay
        };
        let selected: BTreeSet<_> = ours.keys().filter(|id| !theirs.contains_key(*id)).copied().collect();
        let mut candidates = Vec::new();
        let mut current = original;
        while selected.contains(&current) {
            let parents = ours.get(&current).context("selected history disappeared")?;
            ensure!(
                parents.len() == 1,
                "merge-bearing or root replay selection is unsupported"
            );
            candidates.push(Candidate {
                commit: current,
                parent: parents[0],
            });
            current = parents[0];
        }
        ensure!(candidates.len() == selected.len(), "selection is not one linear chain");
        candidates.reverse();
        Ok(Self {
            original,
            upstream: upstream_id,
            attachment,
            argument: upstream.to_owned(),
            candidates,
            kind,
            closure,
        })
    }

    /// An attachment change is foreign movement even if the commit stays equal.
    pub fn validate_head(&self, repo: &gix::Repository) -> Result<()> {
        ensure!(repo.head_commit()?.id == self.original, "original HEAD moved");
        ensure!(
            repo.head_name()?.map(|name| name.as_bstr().to_owned()) == self.attachment,
            "original HEAD attachment changed"
        );
        Ok(())
    }

    /// Conservative initial preservation exclusion set. Merge outcomes may
    /// enlarge it, but never shrink away a selected input's changed path.
    pub fn footprint(&self, repo: &gix::Repository) -> Result<Vec<BString>> {
        let mut paths = BTreeSet::new();
        changed_paths(repo, self.original, self.upstream, &mut paths)?;
        for candidate in &self.candidates {
            changed_paths(repo, candidate.parent, candidate.commit, &mut paths)?;
        }
        Ok(paths.into_iter().collect())
    }
}

fn changed_paths(
    repo: &gix::Repository,
    before: gix::ObjectId,
    after: gix::ObjectId,
    paths: &mut BTreeSet<BString>,
) -> Result<()> {
    let before = repo.index_from_tree(&repo.find_commit(before)?.tree_id()?)?;
    let after = repo.index_from_tree(&repo.find_commit(after)?.tree_id()?)?;
    changed_index_paths(&before, &after, paths)
}

fn changed_index_paths(
    before: &gix::index::State,
    after: &gix::index::State,
    paths: &mut BTreeSet<BString>,
) -> Result<()> {
    let entries = |index: &gix::index::State| {
        index
            .entries()
            .iter()
            .map(|entry| (entry.path(index).to_owned(), (entry.id, entry.mode)))
            .collect::<BTreeMap<_, _>>()
    };
    let before = entries(before);
    let after = entries(after);
    for path in before.keys().chain(after.keys()) {
        if before.get(path) != after.get(path) {
            ensure!(
                ![before.get(path), after.get(path)]
                    .into_iter()
                    .flatten()
                    .any(|(_, mode)| *mode == gix::index::entry::Mode::COMMIT),
                "affected gitlinks are unsupported"
            );
            paths.insert(path.clone());
        }
    }
    Ok(())
}

fn resolve_upstream(repo: &gix::Repository, argument: &BStr) -> Result<gix::ObjectId> {
    if argument.len() == repo.object_hash().len_in_hex() && argument.iter().all(u8::is_ascii_hexdigit) {
        return Ok(repo.find_commit(gix::ObjectId::from_hex(argument)?)?.id);
    }
    let prefixes: &[&[u8]] = if argument.starts_with(b"refs/") {
        ensure!(
            [b"refs/heads/".as_slice(), b"refs/tags/", b"refs/remotes/"]
                .iter()
                .any(|prefix| argument.starts_with(prefix)),
            "upstream must name a branch, tag, remote-tracking ref or full object ID"
        );
        &[b""]
    } else {
        &[b"refs/heads/", b"refs/tags/", b"refs/remotes/"]
    };
    let mut matches = Vec::new();
    for prefix in prefixes {
        let mut name = prefix.to_vec();
        name.extend_from_slice(argument);
        if let Some(mut reference) = repo.try_find_reference(name.as_bstr())? {
            matches.push(reference.peel_to_commit()?.id);
        }
    }
    ensure!(
        matches.len() == 1,
        "upstream must resolve to one unambiguous local reference"
    );
    Ok(matches[0])
}

fn validate_object(
    repo: &gix::Repository,
    object_id: gix::ObjectId,
    kind: gix::objs::Kind,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<()> {
    let object = repo.find_object(object_id)?;
    ensure!(object.kind == kind, "pinned object has wrong type");
    ensure!(
        gix::objs::compute_hash(repo.object_hash(), object.kind, &object.data)? == object_id,
        "pinned object checksum mismatch"
    );
    closure.insert(object_id);
    Ok(())
}

fn history(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>> {
    let mut commits = BTreeMap::new();
    let mut pending = vec![tip];
    let mut trees = BTreeSet::new();
    while let Some(commit_id) = pending.pop() {
        if commits.contains_key(&commit_id) {
            continue;
        }
        validate_object(repo, commit_id, gix::objs::Kind::Commit, closure)?;
        let commit = repo.find_commit(commit_id)?;
        let parents: Vec<_> = commit.parent_ids().map(gix::Id::detach).collect();
        validate_tree(repo, commit.tree_id()?.detach(), &mut trees, closure)?;
        pending.extend(parents.iter().copied());
        commits.insert(commit_id, parents);
    }
    Ok(commits)
}

fn validate_tree(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    seen: &mut BTreeSet<gix::ObjectId>,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<()> {
    let mut pending = vec![tree_id];
    while let Some(tree_id) = pending.pop() {
        if !seen.insert(tree_id) {
            continue;
        }
        validate_object(repo, tree_id, gix::objs::Kind::Tree, closure)?;
        let tree = repo.find_tree(tree_id)?;
        for entry in tree.iter() {
            let entry = entry?;
            if entry.mode().is_tree() {
                pending.push(entry.object_id());
            } else if !entry.mode().is_commit() {
                validate_object(repo, entry.object_id(), gix::objs::Kind::Blob, closure)?;
            }
        }
    }
    Ok(())
}

/// A native three-way result. Objects may be written, but no live index, file or
/// reference is touched. The driver must protect and synchronize these objects
/// and persist the proposal before making a publication decision.
pub struct Step {
    pub original: gix::ObjectId,
    pub parent: gix::ObjectId,
    pub tree: gix::ObjectId,
    pub index: gix::index::File,
    pub conflicts: Vec<BString>,
    pub footprint: Vec<BString>,
    pub initially_empty: bool,
    pub newly_empty: bool,
}

impl Step {
    pub fn merge(repo: &gix::Repository, candidate: &Candidate, tip: gix::ObjectId) -> Result<Self> {
        validate_execution_policy(repo)?;
        let original = repo.find_commit(candidate.commit)?;
        let parents: Vec<_> = original.parent_ids().map(gix::Id::detach).collect();
        ensure!(
            parents == [candidate.parent],
            "candidate parent does not match pinned selection"
        );
        let base = repo.find_commit(candidate.parent)?.tree_id()?.detach();
        let ours = repo.find_commit(tip)?.tree_id()?.detach();
        let theirs = original.tree_id()?.detach();
        let result = repo.merge_trees(base, ours, theirs, Default::default(), repo.tree_merge_options()?)?;
        let how = gix::merge::tree::TreatAsUnresolved::default();
        let mut conflicts = BTreeSet::new();
        for conflict in result.conflicts.iter().filter(|conflict| conflict.is_unresolved(how)) {
            let (ours, theirs) = conflict.changes_in_resolution();
            for path in [
                ours.source_location(),
                ours.location(),
                theirs.source_location(),
                theirs.location(),
            ] {
                if !path.is_empty() {
                    conflicts.insert(path.to_owned());
                }
            }
        }
        let tree = result
            .tree
            .clone()
            .detach()
            .write(|tree| repo.write_object(tree).map(gix::Id::detach))?;
        let mut index = repo.index_from_tree(&tree)?;
        let mut footprint = conflicts.clone();
        changed_index_paths(
            &*repo.index_from_tree(&base)?,
            &*repo.index_from_tree(&theirs)?,
            &mut footprint,
        )?;
        changed_index_paths(&*repo.index_from_tree(&ours)?, &index, &mut footprint)?;
        result.index_changed_after_applying_conflicts(
            &mut index,
            how,
            gix::merge::tree::apply_index_entries::RemovalMode::Prune,
        );
        index.verify_entries()?;
        Ok(Self {
            original: candidate.commit,
            parent: tip,
            tree,
            index,
            conflicts: conflicts.into_iter().collect(),
            footprint: footprint.into_iter().collect(),
            initially_empty: base == theirs,
            newly_empty: base != theirs && tree == ours,
        })
    }

    /// Fix bytes and committer time once. Recovery must use these bytes rather
    /// than call this function again. The original raw author and encoding are
    /// preserved; signatures and other old extra headers are not copied.
    pub fn propose(&self, repo: &gix::Repository) -> Result<Proposal> {
        self.propose_message(repo, self.original, self.parent, None)
    }

    /// Compose an interactive proposal with the group's original author and
    /// parent, while retaining arbitrary message bytes without UTF-8 conversion.
    pub(crate) fn propose_message(
        &self,
        repo: &gix::Repository,
        author_source: gix::ObjectId,
        parent: gix::ObjectId,
        message: Option<&[u8]>,
    ) -> Result<Proposal> {
        use gix::objs::WriteTo;
        ensure!(self.conflicts.is_empty(), "unresolved replay conflicts");
        ensure!(
            !repo.config_snapshot().try_boolean("commit.gpgSign")?.unwrap_or(false),
            "required signing is unsupported"
        );
        let original = repo.find_commit(author_source)?;
        let original = original.decode()?;
        let mut committer = Vec::new();
        repo.committer()
            .context("committer identity required")??
            .write_to(&mut committer)?;
        let tree = self.tree.to_string();
        let parent = parent.to_string();
        let commit = gix::objs::CommitRef {
            tree: tree.as_bytes().as_bstr(),
            parents: [parent.as_bytes().as_bstr()].into_iter().collect(),
            author: original.author,
            committer: committer.as_bstr(),
            encoding: original.encoding,
            message: message.map_or(original.message, ByteSlice::as_bstr),
            extra_headers: Vec::new(),
        };
        let mut bytes = Vec::new();
        commit.write_to(&mut bytes)?;
        let commit = gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Commit, &bytes)?;
        Ok(Proposal { commit, bytes })
    }
}

#[derive(Clone, Debug)]
pub struct Proposal {
    pub commit: gix::ObjectId,
    pub bytes: Vec<u8>,
}

#[cfg(target_os = "linux")]
pub use journal::{Advance, Session};
#[cfg(target_os = "linux")]
pub(crate) use journal::{
    ChildAuthorization, active_nonce, protect_child_commit, validate_accepted_child, validate_child_binding,
};

#[cfg(target_os = "linux")]
mod journal;
#[cfg(target_os = "linux")]
mod transition;

#[cfg(target_os = "linux")]
mod cleanup;
#[cfg(target_os = "linux")]
pub mod editors;
#[cfg(target_os = "linux")]
pub(crate) use cleanup::ensure_cleanup_retired;
