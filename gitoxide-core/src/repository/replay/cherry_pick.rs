//! One-shot replay onto the current branch.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek},
    path::Path,
    sync::Arc,
};

use anyhow::{Context, Result, bail, ensure};
use gix::{
    bstr::{BStr, ByteSlice},
    index::entry::{Flags, Mode, Stat},
    objs::{FindStream, Kind, Write},
    refs::transaction::{PreviousValue, RefEdit},
    utils::stream::{self, Budget, Spool},
};

use super::{ReplayMode, Step, read_commit};
use crate::repository::{
    index::authoring::{read_index, worktree_entry_spooled},
    switch::{self, Change, State, payload},
};

/// Apply one local commit. The caller holds the repository and worktree writer guards.
pub fn execute(repo: &gix::Repository, revision: &BStr) -> Result<()> {
    let root = repo.workdir().context("cherry-pick requires a worktree")?;
    let index_path = repo.index_path();
    ensure!(
        index_path == repo.current_dir().join(repo.git_dir()).join("index"),
        "alternate indexes require Git"
    );
    ensure!(repo.namespace().is_none(), "reference namespaces require Git");
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "BISECT_START",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        match fs::symlink_metadata(repo.git_dir().join(marker)) {
            Ok(_) => bail!("repository operation in progress: {marker}"),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
    }

    let branch = repo.head_name()?.context("cherry-pick requires a checked-out branch")?;
    ensure!(
        branch.as_bstr().starts_with(b"refs/heads/")
            && repo.find_reference("HEAD")?.target().try_name() == Some(branch.as_ref()),
        "cherry-pick requires HEAD to name a local branch directly"
    );
    let tip = super::head_commit_id(repo)?;
    let commit =
        gix::object::bounded::Source::deferred(repo).peel(repo.rev_parse_single(revision)?.detach(), Kind::Commit)?;
    let mut buffer = gix::objs::find::Buffer::default();
    let original = read_commit(repo, commit, &mut buffer)?;
    let mut parents = original.parents();
    let parent = parents.next();
    ensure!(
        parents.next().is_none(),
        "cherry-pick of a merge commit requires a mainline"
    );

    let step = Step::merge_commit(repo, commit, parent, tip, ReplayMode::CherryPick)?;
    ensure!(
        !step.initially_empty && !step.newly_empty,
        "cherry-pick would create an empty commit"
    );
    let proposal = step.propose(repo)?;
    let head = repo.index_from_tree(&repo.tree_id_of_commit(tip)?)?;
    let lock_mode = gix::lock::acquire::Fail::Immediately;
    let _index_lock = gix::lock::Marker::acquire_to_hold_resource(&index_path, lock_mode, None, 0)?;
    let index = read_index(repo)?;
    let scratch = tempfile::Builder::new()
        .prefix("brit-pick-")
        .tempdir_in(repo.git_dir())?;
    let staging = scratch.path();
    let budget = Budget::new(staging)?;
    let (new_index, mut work) = prepare_checkout(repo, root, &head, &index, &step, &budget, staging)?;
    switch::prepare_directories(staging, &mut work)?;
    work.sort_by_key(|change| std::cmp::Reverse(change.path.len()));

    let before = capture(staging, &budget, &index_path)?;
    let mode = match &before {
        State::StoredFile { mode, .. } => *mode,
        _ => switch::creation_mode(0o666)?,
    };
    let mut bytes = budget.spool()?;
    gix::index::File::from_state(new_index, &index_path).write_to(&mut bytes, Default::default())?;
    let index_change = Change {
        path: b"index".to_vec(),
        before,
        after: store(staging, bytes, mode)?,
    };
    ensure!(
        repo.write_buf(Kind::Commit, &proposal.bytes)? == proposal.commit,
        "pick commit hash changed"
    );
    let committer = repo.committer().context("committer identity required")??;
    let mut edit = RefEdit::update(
        "HEAD".try_into()?,
        proposal.commit,
        PreviousValue::MustExistAndMatch(tip.into()),
        "cherry-pick",
    );
    edit.deref = true;
    let transaction = repo.refs.transaction().prepare([edit], lock_mode, lock_mode)?;
    ensure!(
        repo.head_name()?.as_ref() == Some(&branch),
        "HEAD attachment changed during cherry-pick"
    );
    super::selection_interrupt()?;
    for change in &work {
        let path = gix::path::from_bstr(change.path.as_bstr())?;
        ensure!(
            payload::fingerprint(&root.join(path.as_ref()))? == change.before,
            "worktree changed during cherry-pick: {}",
            change.path.as_bstr()
        );
    }

    let mut started = BTreeSet::new();
    let mut index_started = false;
    let mut commit_started = false;
    let published = (|| -> Result<()> {
        apply_work(staging, root, &work, &mut started, false)?;
        index_started = true;
        switch::publish_file_from(staging, staging, repo.git_dir(), &index_change, &index_change.after)?;
        commit_started = true;
        transaction.commit(committer)?;
        Ok(())
    })();
    if let Err(error) = published {
        if commit_started {
            match repo.find_reference(branch.as_ref()) {
                Ok(reference) if reference.target().try_id() == Some(proposal.commit.as_ref()) => {
                    return Err(error.context("cherry-pick advanced the branch before reporting a publication error"));
                }
                Ok(reference) if reference.target().try_id() == Some(tip.as_ref()) => {}
                _ => {
                    let path = scratch.keep();
                    return Err(error.context(format!(
                        "could not confirm the branch after the publication error; checkout backups at {}",
                        path.display()
                    )));
                }
            }
        }
        let restored_index = if index_started {
            switch::publish_file_from(staging, staging, repo.git_dir(), &index_change, &index_change.before)
        } else {
            Ok(())
        };
        let restored_work = apply_work(staging, root, &work, &mut started, true);
        if let Err(rollback) = restored_index.and(restored_work) {
            let path = scratch.keep();
            return Err(error.context(format!(
                "cherry-pick rollback failed ({rollback:#}); checkout backups at {}",
                path.display()
            )));
        }
        return Err(error);
    }
    Ok(())
}

fn prepare_checkout(
    repo: &gix::Repository,
    root: &Path,
    head: &gix::index::State,
    index: &gix::index::State,
    step: &Step,
    budget: &Arc<Budget>,
    staging: &Path,
) -> Result<(gix::index::State, Vec<Change>)> {
    let destination = &step.index;
    let entry = |index: &gix::index::State, path: &BStr| index.entry_by_path(path).map(|e| (e.mode, e.id));
    let changed: BTreeSet<_> = step
        .footprint
        .iter()
        .map(|path| path.as_bstr())
        .filter(|path| entry(head, path) != entry(destination, path))
        .collect();
    ensure!(
        repo.config_snapshot().try_boolean("core.ignorecase")? != Some(true),
        "case-insensitive index semantics require Git"
    );
    let overlaps = |a: &BStr, b: &BStr| a.starts_with(b) && a.get(b.len()) == Some(&b'/');
    let paths = |index: &gix::index::State| index.entries().iter().map(|e| e.path(index).to_vec()).collect();
    for path in [head, index]
        .into_iter()
        .flat_map(|index| index.entries().iter().map(|e| e.path(index)))
    {
        if entry(head, path) != entry(index, path) {
            ensure!(
                !changed
                    .iter()
                    .any(|name| *name == path || overlaps(path, name) || overlaps(name, path)),
                "staged changes overlap cherry-pick: {path}"
            );
        }
    }
    let mut new_index = gix::index::State::new(repo.object_hash());
    for e in index.entries() {
        let name = e.path(index);
        if !changed.contains(name) {
            new_index.dangerously_push_entry(e.stat, e.id, e.flags - Flags::FSMONITOR_VALID, e.mode, name);
        }
    }
    for name in &changed {
        if let Some((mode, id)) = entry(destination, name) {
            new_index.dangerously_push_entry(Stat::default(), id, Flags::empty(), mode, name);
        }
    }
    if let Some(mut undo) = index.resolve_undo().cloned() {
        undo.retain(|record| !changed.contains(record.name.as_bstr()));
        new_index.set_resolve_undo(undo);
    }
    new_index.sort_entries();
    new_index.verify_entries()?;

    let mut attrs = repo.attributes_only(destination, gix::worktree::stack::state::attributes::Source::IdMapping)?;
    crate::repository::index::authoring::refuse_external_filters(repo, &mut attrs, changed.iter().copied())?;
    let (mut pipeline, mut attrs) = gix::filter::Pipeline::new(repo, attrs.detach())?.into_parts();
    let mut work = BTreeMap::new();
    for name in changed {
        switch::validate_path(name)?;
        let relative = gix::path::from_bstr(name)?;
        let absolute = root.join(relative.as_ref());
        for ancestor in relative.ancestors().skip(1).take_while(|p| !p.as_os_str().is_empty()) {
            let ancestor = root.join(ancestor);
            ensure!(
                !ancestor.is_symlink() && !ancestor.join(".git").exists(),
                "unsafe worktree ancestor of {name}"
            );
        }
        let old = entry(head, name);
        let new = entry(destination, name);
        if old.is_some_and(|(mode, _)| mode == Mode::COMMIT) || new.is_some_and(|(mode, _)| mode == Mode::COMMIT) {
            ensure!(
                matches!((old, new), (Some((Mode::COMMIT, _)), Some((Mode::COMMIT, _)))),
                "only gitlink pin moves are supported: {name}"
            );
            ensure!(
                matches!(payload::fingerprint(&absolute)?, State::Absent)
                    || (absolute.is_dir() && !absolute.is_symlink() && fs::read_dir(&absolute)?.next().is_none()),
                "gitlink path is populated: {name}"
            );
            continue;
        }
        let before = capture(staging, budget, &absolute)?;
        if let Some((mode, blob_id)) = old {
            let actual = worktree_entry_spooled(repo, index, &name.to_owned(), budget)
                .with_context(|| format!("inspect worktree path {name}"))?;
            let (kind, expected) = repo
                .objects
                .find_stream(&blob_id, budget, &gix::interrupt::IS_INTERRUPTED)?;
            ensure!(kind == Kind::Blob, "expected a blob for {name}");
            let clean = match actual {
                Some((actual_mode, bytes)) => {
                    actual_mode == mode && stream::equal(bytes, expected, &gix::interrupt::IS_INTERRUPTED)?
                }
                None => false,
            };
            ensure!(clean, "unstaged changes overlap cherry-pick: {name}");
        } else {
            ensure!(
                matches!(before, State::Absent | State::Directory { .. }),
                "untracked or ignored collision: {name}"
            );
        }
        let after = if let Some((mode, blob_id)) = new {
            let (kind, mut source) = repo
                .objects
                .find_stream(&blob_id, budget, &gix::interrupt::IS_INTERRUPTED)?;
            ensure!(kind == Kind::Blob, "expected a blob for {name}");
            if mode == Mode::SYMLINK {
                ensure!(
                    repo.config_snapshot().try_boolean("core.symlinks")? != Some(false),
                    "emulated symlinks are unsupported"
                );
                let mut bytes = Vec::new();
                source.read_to_end(&mut bytes)?;
                ensure!(
                    !bytes.is_empty() && !bytes.contains(&0),
                    "invalid symlink target: {name}"
                );
                State::Symlink(bytes)
            } else {
                ensure!(
                    matches!(mode, Mode::FILE | Mode::FILE_EXECUTABLE),
                    "unsupported file mode for {name}"
                );
                let attributes = attrs.at_entry(name, None, &repo.objects)?;
                let gix::filter::plumbing::pipeline::stream::Outcome::Ready(source) = pipeline
                    .convert_to_worktree_stream(
                        source,
                        name,
                        &mut |_, attrs| {
                            attributes.matching_attributes(attrs);
                        },
                        Default::default(),
                        budget,
                        &gix::interrupt::IS_INTERRUPTED,
                    )?
                else {
                    bail!("checkout filter delayed {name}");
                };
                let mode = switch::creation_mode(if mode == Mode::FILE_EXECUTABLE { 0o777 } else { 0o666 })?;
                store(staging, source, mode)?
            }
        } else {
            State::Absent
        };
        work.insert(
            name.to_vec(),
            Change {
                path: name.to_vec(),
                before,
                after,
            },
        );
    }
    switch::complete_directory_ledger(root, &paths(head), &paths(destination), &mut work)?;
    Ok((
        new_index,
        work.into_values()
            .filter(|change| change.before != change.after)
            .collect(),
    ))
}

fn capture(staging: &Path, budget: &Arc<Budget>, path: &Path) -> Result<State> {
    let state = payload::fingerprint(path)?;
    if let State::StoredFile { mode, .. } = state {
        let mut bytes = budget.spool()?;
        stream::copy(&mut fs::File::open(path)?, &mut bytes, &gix::interrupt::IS_INTERRUPTED)?;
        ensure!(
            store(staging, bytes, mode)? == state,
            "file changed while saving {}",
            path.display()
        );
    }
    Ok(state)
}

fn store(staging: &Path, mut bytes: Spool, mode: u32) -> Result<State> {
    let size = bytes.len();
    bytes.rewind()?;
    let checksum = payload::checksum(&mut bytes, size)?;
    bytes.rewind()?;
    payload::persist(staging, bytes, size, &checksum)?;
    Ok(State::StoredFile { mode, size, checksum })
}

fn apply_work<'a>(
    staging: &Path,
    root: &Path,
    work: &'a [Change],
    started: &mut BTreeSet<&'a [u8]>,
    rollback: bool,
) -> Result<()> {
    let target = |change: &'a Change| if rollback { &change.before } else { &change.after };
    for change in work {
        if rollback && !started.contains(change.path.as_slice()) {
            continue;
        }
        let path = gix::path::from_bstr(change.path.as_bstr())?;
        let actual = payload::fingerprint(&root.join(path.as_ref()))?;
        let desired = target(change);
        if actual != State::Absent
            && (desired == &State::Absent
                || matches!(desired, State::Directory { .. }) != matches!(actual, State::Directory { .. }))
        {
            started.insert(&change.path);
            switch::publish_file_from(staging, staging, root, change, &State::Absent)?;
        }
    }
    for change in work.iter().rev() {
        if (!rollback || started.contains(change.path.as_slice())) && target(change) != &State::Absent {
            started.insert(&change.path);
            switch::publish_file_from(staging, staging, root, change, target(change))?;
        }
    }
    Ok(())
}
