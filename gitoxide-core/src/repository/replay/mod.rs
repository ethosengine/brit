//! Durable local replay with bounded Linux command dispatch.
//!
//! Read-only selection resolves the range from the merge base and the chain of
//! single-parent commits above it, so its cost is proportional to the range, not
//! to the history beneath it. Linux sessions preserve the original staging and
//! worktree distinction through separately journaled decisions.
//!
//! What a rebase reads and keeps is bounded by its *floor*: the merge base `p` and
//! the upstream `U`, both kept reachable by refs the user owns (the original branch
//! reaches `p`; the user's upstream ref, and once published the new branch, reach
//! `U`). No walk, verification or retained copy goes below the floor (see
//! `validation`). `Selection::prepare` verifies what a replay reads before it
//! returns; a session defers that verification until preparation's index- and
//! tree-only refusals have run, and verifies before its journal exists.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use gix::bstr::{BStr, BString, ByteSlice};

#[cfg(target_os = "linux")]
mod cherry_pick;
#[cfg(target_os = "linux")]
pub use cherry_pick::execute as cherry_pick;

/// Apply one commit to the checked-out branch while holding the writer guards.
#[cfg(not(target_os = "linux"))]
pub fn cherry_pick(_: &gix::Repository, revision: &BStr) -> Result<()> {
    // Elsewhere this stays a Git bridge: name the exact command, as the bridge did before.
    anyhow::bail!("native cherry-pick requires Linux; run: git cherry-pick {revision}")
}

mod validation;
use validation::Validation;
#[cfg(target_os = "linux")]
use validation::validate_tree;

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
    /// A null ID represents a root input to cherry-pick.
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
    /// The single merge base of the original tip and the upstream: the parent of
    /// the first candidate, or the original tip itself when nothing is replayed.
    pub base: gix::ObjectId,
    /// The commits no walk of this replay goes below: `base` and `upstream`.
    pub floor: BTreeSet<gix::ObjectId>,
    /// Hash-verified objects the replayed chain holds above the floor (its custody;
    /// see `validation`), filled in by verification.
    pub closure: BTreeSet<gix::ObjectId>,
}

pub(crate) fn validate_execution_policy(repo: &gix::Repository) -> Result<()> {
    ensure!(
        !repo.config_snapshot().try_boolean("commit.gpgSign")?.unwrap_or(false),
        "required signing is unsupported"
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

fn validate_selection_repository(repo: &gix::Repository) -> Result<()> {
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
    selection_interrupt()
}

impl Selection {
    /// Resolve only explicit, local revisions, prove the selected linear chain and
    /// verify what a replay of it reads. This function is read-only, including for
    /// no-op and fast-forward results.
    pub fn prepare(repo: &gix::Repository, upstream: &BStr) -> Result<Self> {
        let mut selection = Self::prepare_inner(repo, upstream, None)?;
        selection.verify(repo)?;
        Ok(selection)
    }

    /// [`Self::prepare`] for a replay session whose journal the scratch `limit`
    /// meters. Before any object is verified or spooled, it refuses a rebase whose
    /// journal could not fit. `interactive` says whether a no-op selection would
    /// still replay its own commits. Verification is left to [`Self::verify`], which
    /// the session runs once its index- and tree-only refusals have passed.
    pub fn prepare_for_session(repo: &gix::Repository, upstream: &BStr, interactive: bool, limit: u64) -> Result<Self> {
        Self::prepare_inner(repo, upstream, Some((interactive, limit)))
    }

    fn prepare_inner(repo: &gix::Repository, upstream: &BStr, bound: Option<(bool, u64)>) -> Result<Self> {
        ensure!(!gix::interrupt::is_triggered(), "replay selection interrupted");
        ensure!(!upstream.is_empty(), "an explicit local upstream is required");
        validate_selection_repository(repo)?;
        let original = head_commit_id(repo)?;
        let attachment = repo.head_name()?.map(|name| name.as_bstr().to_owned());
        let upstream_id = resolve_upstream(repo, upstream)?;
        selection_interrupt()?;
        let bases = repo.merge_bases_many(original, &[upstream_id])?;
        selection_interrupt()?;
        ensure!(bases.len() == 1, "replay requires exactly one merge base");
        let base = bases[0].detach();
        let candidates = chain(repo, original, base)?;
        let kind = if base == original && original != upstream_id {
            SelectionKind::FastForward
        } else if base == upstream_id {
            SelectionKind::Noop
        } else {
            SelectionKind::Replay
        };
        if let Some((interactive, limit)) = bound {
            copy_bound::preflight(repo, original, upstream_id, &candidates, interactive, limit)?;
        }
        let selection = Self {
            original,
            upstream: upstream_id,
            attachment,
            argument: upstream.to_owned(),
            candidates,
            kind,
            base,
            floor: BTreeSet::from([base, upstream_id]),
            closure: BTreeSet::new(),
        };
        if selection.kind != SelectionKind::Noop || !selection.candidates.is_empty() {
            let commits = [selection.original, selection.upstream]
                .into_iter()
                .chain(selection.candidates.iter().flat_map(|c| [c.parent, c.commit]));
            let trees = collect_trees(repo, commits)?;
            refuse_external_filters(repo, trees, &selection.footprint(repo)?)?;
        }
        ensure!(!gix::interrupt::is_triggered(), "replay selection interrupted");
        Ok(selection)
    }

    /// Verify, once, every object a replay of this selection reads or keeps: the
    /// custody of the replayed chain above the floor, and the upstream's and the
    /// base's trees and blobs along the footprint. Nothing below the floor is read.
    pub fn verify(&mut self, repo: &gix::Repository) -> Result<()> {
        let mut validation = Validation::default();
        let mut closure = BTreeSet::new();
        validation.custody(repo, self.original, &self.floor, &mut closure)?;
        let footprint = self.footprint(repo)?;
        for commit in [self.upstream, self.base] {
            validation.along(repo, commit, &footprint)?;
        }
        selection_interrupt()?;
        self.closure = closure;
        Ok(())
    }

    /// An attachment change is foreign movement even if the commit stays equal.
    pub fn validate_head(&self, repo: &gix::Repository) -> Result<()> {
        ensure!(head_commit_id(repo)? == self.original, "original HEAD moved");
        ensure!(
            repo.head_name()?.map(|name| name.as_bstr().to_owned()) == self.attachment,
            "original HEAD attachment changed"
        );
        Ok(())
    }

    /// Conservative initial preservation exclusion set. Merge outcomes may
    /// enlarge it, but never shrink away a selected input's changed path.
    pub fn footprint(&self, repo: &gix::Repository) -> Result<Vec<BString>> {
        selection_interrupt()?;
        let mut paths = BTreeSet::new();
        changed_paths(repo, self.original, self.upstream, &mut paths)?;
        for candidate in &self.candidates {
            selection_interrupt()?;
            changed_paths(repo, candidate.parent, candidate.commit, &mut paths)?;
            selection_checkpoint("replay-footprint-candidate")?;
        }
        collect_footprint(paths)
    }
}

fn collect_footprint(paths: BTreeSet<BString>) -> Result<Vec<BString>> {
    selection_interrupt()?;
    let mut footprint = Vec::new();
    for path in paths {
        selection_interrupt()?;
        footprint.push(path);
        selection_checkpoint("replay-footprint-path")?;
    }
    selection_interrupt()?;
    Ok(footprint)
}

// Keep test injection separate from the real poll: a checkpoint must return here.
fn selection_interrupt() -> Result<()> {
    ensure!(!gix::interrupt::is_triggered(), "replay selection interrupted");
    Ok(())
}

fn selection_checkpoint(point: &str) -> Result<()> {
    #[cfg(test)]
    tests::completed(point);
    #[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
    cancellation::checkpoint(point)?;
    let _ = point;
    selection_interrupt()
}

/// The commits from `original` down to (not including) `base`, oldest first. Each
/// must have exactly one parent; the walk reads nothing below `base`. Because
/// `base` is the only best common ancestor, none of these is reachable from the
/// upstream, and every commit only the original reaches is one of them.
fn chain(repo: &gix::Repository, original: gix::ObjectId, base: gix::ObjectId) -> Result<Vec<Candidate>> {
    selection_interrupt()?;
    let mut candidates = Vec::new();
    let mut current = original;
    let mut buffer = gix::objs::find::Buffer::default();
    while current != base {
        // Each commit is selected and chained in the same step.
        selection_checkpoint("replay-selected-commit")?;
        let commit = read_commit(repo, current, &mut buffer)?;
        let parents: Vec<_> = commit.parents().collect();
        drop(commit);
        ensure!(
            parents.len() == 1,
            "merge-bearing or root replay selection is unsupported"
        );
        candidates.push(Candidate {
            commit: current,
            parent: parents[0],
        });
        current = parents[0];
        selection_checkpoint("replay-candidate-chain")?;
    }
    candidates.reverse();
    selection_interrupt()?;
    Ok(candidates)
}

fn collect_trees(
    repo: &gix::Repository,
    commits: impl IntoIterator<Item = gix::ObjectId>,
) -> Result<BTreeSet<gix::ObjectId>> {
    selection_interrupt()?;
    let mut trees = BTreeSet::new();
    for commit in commits {
        selection_interrupt()?;
        trees.insert(commit_tree(repo, commit)?);
        selection_checkpoint("replay-selection-tree")?;
    }
    selection_interrupt()?;
    Ok(trees)
}

fn commit_tree(repo: &gix::Repository, commit: gix::ObjectId) -> Result<gix::ObjectId> {
    selection_interrupt()?;
    let tree = repo.tree_id_of_commit(commit)?.detach();
    selection_interrupt()?;
    Ok(tree)
}

fn read_commit<'a>(
    repo: &gix::Repository,
    id: gix::ObjectId,
    buffer: &'a mut gix::objs::find::Buffer,
) -> Result<gix::objs::CommitRef<'a>> {
    let source = gix::object::bounded::Source::deferred(repo);
    let data = source.load(&id, buffer)?;
    ensure!(data.kind == gix::objs::Kind::Commit, "expected commit {id}");
    Ok(gix::objs::CommitRef::from_bytes(data.data, repo.object_hash())?)
}

fn head_commit_id(repo: &gix::Repository) -> Result<gix::ObjectId> {
    let id = repo.head_id()?.detach();
    let mut buffer = gix::objs::find::Buffer::default();
    read_commit(repo, id, &mut buffer)?;
    Ok(id)
}

mod copy_bound;

#[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
mod cancellation;
#[cfg(test)]
mod tests;

/// Refuse when a configured external filter driver could convert one of `paths` under the attributes
/// of any of `trees`, the trees whose files a replay reads, merges or writes.
fn refuse_external_filters(
    repo: &gix::Repository,
    trees: impl IntoIterator<Item = gix::ObjectId>,
    paths: &[BString],
) -> Result<()> {
    selection_interrupt()?;
    let no_filters = paths.is_empty() || gix::filter::Pipeline::options(repo)?.drivers.is_empty();
    selection_interrupt()?;
    if no_filters {
        return Ok(());
    }
    for tree in trees {
        selection_interrupt()?;
        let index = repo.index_from_tree(&tree)?;
        selection_interrupt()?;
        let mut attributes =
            repo.attributes_only(&index, gix::worktree::stack::state::attributes::Source::IdMapping)?;
        selection_interrupt()?;
        super::index::authoring::refuse_external_filters(repo, &mut attributes, paths.iter().map(|p| p.as_bstr()))?;
        selection_checkpoint("replay-filter-tree")?;
    }
    selection_interrupt()
}

fn changed_paths(
    repo: &gix::Repository,
    before: gix::ObjectId,
    after: gix::ObjectId,
    paths: &mut BTreeSet<BString>,
) -> Result<()> {
    selection_interrupt()?;
    let before = if before.is_null() {
        gix::index::State::new(repo.object_hash())
    } else {
        let before_tree = commit_tree(repo, before)?;
        selection_interrupt()?;
        repo.index_from_tree(&before_tree)?.into()
    };
    selection_interrupt()?;
    let after_tree = commit_tree(repo, after)?;
    selection_interrupt()?;
    let after = repo.index_from_tree(&after_tree)?;
    selection_interrupt()?;
    changed_index_paths(&before, &after, paths)
}

fn changed_index_paths(
    before: &gix::index::State,
    after: &gix::index::State,
    paths: &mut BTreeSet<BString>,
) -> Result<()> {
    collect_changed_index_paths(before, after, paths, false)
}

fn collect_changed_index_paths(
    before: &gix::index::State,
    after: &gix::index::State,
    paths: &mut BTreeSet<BString>,
    allow_gitlinks: bool,
) -> Result<()> {
    let entries = |index: &gix::index::State, point| -> Result<BTreeMap<_, _>> {
        selection_interrupt()?;
        let mut entries = BTreeMap::new();
        for entry in index.entries() {
            selection_interrupt()?;
            entries.insert(entry.path(index).to_owned(), (entry.id, entry.mode));
            selection_checkpoint(point)?;
        }
        selection_interrupt()?;
        Ok(entries)
    };
    let before = entries(before, "replay-index-before")?;
    let after = entries(after, "replay-index-after")?;
    for path in before.keys().chain(after.keys()) {
        selection_interrupt()?;
        if before.get(path) != after.get(path) {
            ensure!(
                allow_gitlinks
                    || ![before.get(path), after.get(path)]
                        .into_iter()
                        .flatten()
                        .any(|(_, mode)| *mode == gix::index::entry::Mode::COMMIT),
                "gitlink {path} is changed by this rebase; affected gitlinks are unsupported by brit rebase, and \
                 nothing was changed; git rebase is the fallback"
            );
            paths.insert(path.clone());
        }
        selection_checkpoint("replay-changed-path")?;
    }
    selection_interrupt()
}

fn resolve_upstream(repo: &gix::Repository, argument: &BStr) -> Result<gix::ObjectId> {
    if argument.len() == repo.object_hash().len_in_hex() && argument.iter().all(u8::is_ascii_hexdigit) {
        let id = gix::ObjectId::from_hex(argument)?;
        read_commit(repo, id, &mut gix::objs::find::Buffer::default())?;
        return Ok(id);
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
            let source = gix::object::bounded::Source::deferred(repo);
            matches.push(source.peel(reference.peel_to_id()?.detach(), gix::objs::Kind::Commit)?);
        }
    }
    ensure!(
        matches.len() == 1,
        "upstream must resolve to one unambiguous local reference"
    );
    Ok(matches[0])
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReplayMode {
    Rebase,
    CherryPick,
}

impl Step {
    pub fn merge(repo: &gix::Repository, candidate: &Candidate, tip: gix::ObjectId) -> Result<Self> {
        Self::merge_commit(
            repo,
            candidate.commit,
            (!candidate.parent.is_null()).then_some(candidate.parent),
            tip,
            ReplayMode::Rebase,
        )
    }

    fn merge_commit(
        repo: &gix::Repository,
        commit: gix::ObjectId,
        parent: Option<gix::ObjectId>,
        tip: gix::ObjectId,
        mode: ReplayMode,
    ) -> Result<Self> {
        selection_interrupt()?;
        validate_execution_policy(repo)?;
        let mut buffer = gix::objs::find::Buffer::default();
        let original = read_commit(repo, commit, &mut buffer)?;
        ensure!(
            original.parents().eq(parent),
            "candidate parent does not match pinned selection"
        );
        let theirs = original.tree();
        drop(original);
        buffer.clear();
        let base = match parent {
            Some(parent) => commit_tree(repo, parent)?,
            None => gix::ObjectId::empty_tree(repo.object_hash()),
        };
        let ours = commit_tree(repo, tip)?;
        selection_interrupt()?;
        let options = repo.tree_merge_options()?;
        selection_interrupt()?;
        let result = repo.merge_trees(base, ours, theirs, Default::default(), options)?;
        selection_interrupt()?;
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
        let cherry_pick = mode == ReplayMode::CherryPick;
        if cherry_pick && !conflicts.is_empty() {
            let paths: Vec<_> = conflicts.iter().map(|path| format!("{path:?}")).collect();
            anyhow::bail!("cherry-pick conflicts: {}", paths.join(", "));
        }
        selection_interrupt()?;
        let tree = result
            .tree
            .clone()
            .detach()
            .write(|tree| repo.write_object(tree).map(gix::Id::detach))?;
        selection_interrupt()?;
        let tree_index = repo.index_from_tree(&tree)?;
        selection_interrupt()?;
        let mut footprint = conflicts.clone();
        {
            let base_index = if parent.is_none() {
                gix::index::State::new(repo.object_hash())
            } else {
                repo.index_from_tree(&base)?.into()
            };
            selection_interrupt()?;
            let theirs_index = repo.index_from_tree(&theirs)?;
            selection_interrupt()?;
            collect_changed_index_paths(&base_index, &theirs_index, &mut footprint, cherry_pick)?;
        }
        {
            selection_interrupt()?;
            let ours_index = repo.index_from_tree(&ours)?;
            selection_interrupt()?;
            collect_changed_index_paths(&ours_index, &tree_index, &mut footprint, cherry_pick)?;
        }
        // The merged tree's attributes govern the files this step will write.
        let footprint = collect_footprint(footprint)?;
        refuse_external_filters(repo, [tree], &footprint)?;
        drop(tree_index);
        // The merge records its own index, with the unmerged stages `how` selects in place of
        // the entries they replace.
        let index = result.index_file(how, None)?;
        selection_interrupt()?;
        index.verify_entries()?;
        selection_interrupt()?;
        Ok(Self {
            original: commit,
            parent: tip,
            tree,
            index,
            conflicts: conflicts.into_iter().collect(),
            footprint,
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
        let mut buffer = gix::objs::find::Buffer::default();
        let original = read_commit(repo, author_source, &mut buffer)?;
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
pub use journal::{Advance, Retired, Session};
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

/// Bounded public command orchestration (dispatch qualification is separate).
pub mod command;
