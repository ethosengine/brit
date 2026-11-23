use std::{
    fs,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
};

use gix::{
    NestedProgress, ObjectId, Repository, Result, Submodule,
    bstr::{BString, ByteSlice},
    error::{ErrorExt, OptionExt, ResultExt, bail, message},
    index::entry::{Mode, Stage},
    interrupt::IS_INTERRUPTED,
    progress::Discard,
    utils::stream::Budget,
    worktree::stack::state::attributes::Source,
};

use super::{config::edit_local_config, index::authoring, switch};

/// Update the named literal submodule paths to their stage-zero gitlinks.
pub fn update<P>(repo: Repository, paths: &[BString], init: bool, overrides: &[BString], mut progress: P) -> Result<()>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let index = repo.index_or_empty()?;
    let mut modules = repo.submodules()?.into_iter().flatten().collect::<Vec<_>>();
    let mut selected = Vec::new();
    for input in paths {
        let path = repo.normalize_path(input)?;
        if selected.iter().any(|(known, _, _)| known == &path) {
            continue;
        }
        let position = modules
            .iter()
            .position(|module| {
                module
                    .try_path()
                    .is_ok_and(|p| p.is_some_and(|p| p.as_bstr() == path.as_ref()))
            })
            .ok_or_raise(|| message!("Unknown submodule path {input:?}").not_found())?;
        let module = modules.remove(position);
        let commit_id = index
            .entry_by_path_and_stage(&path, Stage::Unconflicted)
            .filter(|entry| entry.mode == Mode::COMMIT)
            .map(|entry| entry.id)
            .ok_or_raise(|| message!("Submodule {path:?} has no gitlink in the index").validation())?;
        selected.push((path, module, commit_id));
    }

    for (path, module, commit_id) in selected {
        update_one(&repo, &module, commit_id, init, overrides, &mut progress)
            .or_raise(|| message!("Could not update submodule {path:?}"))?;
    }
    Ok(())
}

fn update_one<P>(
    parent: &Repository,
    module: &Submodule<'_>,
    commit_id: ObjectId,
    init: bool,
    overrides: &[BString],
    progress: &mut P,
) -> Result<()>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let name = module.validated_name()?;
    let work_dir = module
        .work_dir()?
        .ok_or_raise(|| message("Submodule update requires a worktree").validation())?;
    let work_dir = parent.current_dir().join(work_dir);
    let dot_git = work_dir.join(".git");
    match fs::symlink_metadata(&dot_git) {
        Ok(meta) if meta.is_file() || meta.is_dir() => {}
        Ok(_) => bail!("Submodule metadata is neither a regular gitfile nor a directory".validation()),
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(err).or_error(),
    }
    let state = module.state()?;
    let git_dir = if state.worktree_checkout {
        None
    } else {
        Some(unchecked_out_git_dir(parent, module)?)
    };
    let configured_url = parent
        .config_snapshot()
        .plumbing()
        .string_by("submodule", Some(name), "url");
    if init && configured_url.is_none() {
        edit_local_config(parent, |config| {
            if config.string_by("submodule", Some(name), "url").is_some() {
                return Ok(false);
            }
            let mut url = module.url()?.to_bstring();
            if url.starts_with(b"./") || url.starts_with(b"../") {
                url = module.resolved_url()?.to_bstring();
            }
            config.set_raw_value_by("submodule", Some(name), "url", url)?;
            Ok(true)
        })?;
    }

    if let Some(repo) = module.open()? {
        if fs::canonicalize(repo.common_dir()).or_error()? == fs::canonicalize(parent.common_dir()).or_error()? {
            bail!("The submodule's gitfile points to the superproject".validation());
        }
        if state.worktree_checkout {
            let configured = repo
                .workdir()
                .ok_or_raise(|| message("The submodule has no worktree"))?;
            if fs::canonicalize(configured).or_error()? != fs::canonicalize(&work_dir).or_error()? {
                bail!("The submodule repository has a different configured worktree".validation());
            }
        }
        let _guard =
            switch::worktree_writer_guard(&repo).map_err(|err| gix::Error::from_boxed(err.into_boxed_dyn_error()))?;
        let head = repo.git_dir().join("HEAD");
        let index = repo.index_path();
        let _head_lock = lock(&head)?;
        let _index_lock = lock(&index)?;
        let previous = if state.worktree_checkout {
            let index = authoring::read_index_for_staging(&repo)?;
            refuse_local_changes(&repo)?;
            if repo.head()?.is_detached() && repo.head_id()?.as_ref() == commit_id {
                return Ok(());
            }
            Some(index)
        } else {
            require_empty(&work_dir)?;
            None
        };
        fetch_pin(&repo, commit_id, progress)?;
        return checkout(&work_dir, commit_id, previous.as_ref(), git_dir.as_deref(), None, repo);
    }

    if !init && configured_url.is_none() {
        // Git skips unregistered submodules on update without --init.
        return Ok(());
    }
    if state.repository_exists || state.worktree_checkout {
        bail!("The existing submodule repository could not be opened".validation());
    }
    require_empty(&work_dir)?;
    clone_and_checkout(
        module,
        &work_dir,
        &git_dir.expect("an absent checkout has a Git directory"),
        commit_id,
        overrides,
        progress,
    )
}

fn unchecked_out_git_dir(parent: &Repository, module: &Submodule<'_>) -> Result<PathBuf> {
    let git_dir = parent.current_dir().join(module.git_dir()?);
    let parent_git_dir = parent.current_dir().join(parent.git_dir());
    let relative = git_dir
        .strip_prefix(&parent_git_dir)
        .or_raise(|| message("Submodule Git directory leaves the superproject's Git directory").validation())?;
    gix::fs::symlink::validate_relative_path(&parent_git_dir, relative)?;
    for ancestor in git_dir.ancestors().skip(1).take_while(|p| *p != parent_git_dir) {
        if ancestor.join("HEAD").is_file() {
            bail!("A submodule Git directory cannot be nested in another repository".validation());
        }
    }
    Ok(git_dir)
}

fn clone_and_checkout<P>(
    module: &Submodule<'_>,
    work_dir: &Path,
    git_dir: &Path,
    commit_id: ObjectId,
    overrides: &[BString],
    progress: &mut P,
) -> Result<()>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    let git_dir_parent = git_dir.parent().expect("a submodule Git directory has a parent");
    fs::create_dir_all(git_dir_parent).or_error()?;
    let clone_dir = scratch_directory(git_dir_parent)?;
    let mut options = gix::open::Options::default().config_overrides(overrides.iter().cloned());
    options.permissions.config.git_binary = true;
    let repo = gix::clone::PrepareFetch::new(
        module.resolved_url()?,
        clone_dir.path().join("clone"),
        gix::create::Kind::WithWorktree,
        gix::create::Options {
            object_hash: Some(commit_id.kind()),
            ..Default::default()
        },
        options,
    )?
    .fetch_then_checkout(&mut *progress, &IS_INTERRUPTED)?
    .0
    .persist();

    fetch_pin(&repo, commit_id, progress)?;
    edit_local_config(&repo, |config| {
        let relative = gix::path::relativize_with_prefix(work_dir, git_dir);
        config.set_raw_value("core.worktree", git_path(&relative)?)?;
        Ok(true)
    })?;
    checkout(work_dir, commit_id, None, Some(git_dir), Some(clone_dir), repo)
}

fn lock(path: &Path) -> Result<gix::lock::Marker> {
    gix::lock::Marker::acquire_to_hold_resource(path, gix::lock::acquire::Fail::Immediately, None, 0).or_error()
}

fn fetch_pin<P>(repo: &Repository, commit_id: ObjectId, progress: &mut P) -> Result<()>
where
    P: NestedProgress,
    P::SubProgress: 'static,
{
    use gix::remote::{Direction, ref_map::Options};

    if repo.try_find_header(commit_id)?.is_none() {
        let mut remote = repo.find_fetch_remote(None)?;
        remote
            .connect(Direction::Fetch)?
            .prepare_fetch(&mut *progress, Options::default())?
            .receive(&mut *progress, &IS_INTERRUPTED)?;
        if repo.try_find_header(commit_id)?.is_none() {
            let pin = commit_id.to_string();
            remote.replace_refspecs([pin.as_bytes().as_bstr()], Direction::Fetch)?;
            remote
                .connect(Direction::Fetch)?
                .prepare_fetch(&mut *progress, Options::default())?
                .receive(progress, &IS_INTERRUPTED)?;
        }
    }
    Ok(())
}

fn refuse_local_changes(repo: &Repository) -> Result<()> {
    use gix::status::Submodule as SubmoduleStatus;
    let walk = repo
        .dirwalk_options()?
        .emit_untracked(gix::dir::walk::EmissionMode::Matching);
    let mut status = repo
        .status(Discard)?
        .index_worktree_options_mut(|opts| opts.dirwalk_options = Some(walk))
        .index_worktree_submodules(SubmoduleStatus::Given {
            ignore: gix::submodule::config::Ignore::None,
            check_dirty: true,
        })
        .should_interrupt_shared(&IS_INTERRUPTED)
        .into_iter(None)?;
    if status.next().transpose()?.is_some() {
        bail!("The submodule has local modifications".conflict());
    }
    gix::utils::stream::check(&IS_INTERRUPTED).or_error()
}

fn require_empty(path: &Path) -> Result<()> {
    match fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().transpose().or_error()?.is_some() {
                bail!("The submodule has local modifications at {}", path.display());
            }
            Ok(())
        }
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).or_error(),
    }
}

fn checkout(
    work_dir: &Path,
    commit_id: ObjectId,
    previous: Option<&gix::index::File>,
    link: Option<&Path>,
    clone_dir: Option<tempfile::TempDir>,
    repo: Repository,
) -> Result<()> {
    let existing_parent = work_dir
        .ancestors()
        .skip(1)
        .find(|p| p.is_dir())
        .ok_or_raise(|| message("The submodule checkout has no existing ancestor"))?;
    let scratch = scratch_directory(existing_parent)?;
    let metadata_parent = clone_dir
        .as_ref()
        .map_or_else(|| repo.git_dir(), tempfile::TempDir::path);
    let metadata = scratch_directory(metadata_parent)?;
    let staged = scratch.path().join("checkout");
    let backup = scratch.path().join("backup");
    fs::create_dir(&staged).or_error()?;
    let mut next = stage_checkout(&repo, commit_id, &staged, scratch.path())?;
    let next_index = metadata.path().join("index");
    next.set_path(next_index.clone());
    next.write(Default::default())?;
    let next_head = metadata.path().join("HEAD");
    fs::write(&next_head, format!("{commit_id}\n")).or_error()?;
    let gitfile = staged.join(".git");
    if let Some(git_dir) = link {
        let relative = gix::path::relativize_with_prefix(git_dir, work_dir);
        let mut bytes = b"gitdir: ".to_vec();
        bytes.extend_from_slice(&git_path(&relative)?);
        bytes.push(b'\n');
        fs::write(&gitfile, bytes).or_error()?;
    }

    let repo_git_dir = repo.git_dir().to_owned();
    let index_path = repo.index_path();
    let mut publication = Publication::default();
    let result = (|| -> Result<()> {
        publication.mkdir(work_dir)?;
        if let Some(previous) = previous {
            refuse_local_changes(&repo)?;
            publication.back_up_checkout(work_dir, &backup, previous)?;
        } else {
            require_empty(work_dir)?;
        }
        // Windows requires the clone's open handles and pack mappings to be released
        // before its Git directory can be renamed during publication or rollback.
        drop(repo);
        for entry in next.entries() {
            let path = gix::path::from_bstr(entry.path(&next))?;
            let destination = checked_parent(work_dir, &path)?;
            if entry.mode == Mode::COMMIT {
                publication.mkdir(&destination)?;
            } else {
                publication.mkdir(destination.parent().expect("tracked path has a parent"))?;
                publication.move_file(&staged.join(&path), &destination)?;
            }
        }
        if link.is_some() {
            publication.move_file(&gitfile, &work_dir.join(".git"))?;
        }
        for (source, destination, saved) in [
            (&next_index, index_path, metadata.path().join("old-index")),
            (&next_head, repo_git_dir.join("HEAD"), metadata.path().join("old-HEAD")),
        ] {
            if destination.try_exists().or_error()? {
                publication.move_file(&destination, &saved)?;
            }
            publication.move_file(source, &destination)?;
        }
        if clone_dir.is_some() {
            publication.move_file(&repo_git_dir, link.expect("a new clone has a gitfile"))?;
        }
        Ok(())
    })();
    if let Err(err) = result {
        if let Err(rollback) = publication.rollback() {
            let checkout_recovery = scratch.keep();
            let metadata_recovery = metadata.keep();
            let clone_recovery = clone_dir.map(tempfile::TempDir::keep);
            return Err(err.and_raise(rollback).and_raise(message!(
                "Submodule rollback failed. Saved files at {checkout_recovery:?}, {metadata_recovery:?}, {clone_recovery:?}"
            )));
        }
        return Err(err);
    }
    Ok(())
}

fn stage_checkout(repo: &Repository, commit_id: ObjectId, staged: &Path, scratch: &Path) -> Result<gix::index::File> {
    let mut index = repo.index_from_tree(&repo.find_commit(commit_id)?.tree_id()?)?;
    let mut opts = repo.checkout_options(Source::IdMapping)?;
    opts.destination_is_initially_empty = true;
    let mut attributes = repo.attributes_only(&index, Source::IdMapping)?;
    authoring::refuse_external_filters(
        repo,
        &mut attributes,
        index.entries().iter().map(|entry| entry.path(&index)),
    )?;
    let budget = Budget::new(scratch).or_error()?;
    let outcome = gix::worktree::state::checkout::bounded::checkout(
        &mut index,
        staged,
        &repo.objects,
        opts,
        &budget,
        &IS_INTERRUPTED,
    )?;
    if let Some(error) = outcome.errors.into_iter().next() {
        return Err(error.error).or_raise(|| message!("Submodule checkout failed at {:?}", error.path));
    }
    if let Some(collision) = outcome.collisions.into_iter().next() {
        bail!("Submodule checkout collided at {:?}", collision.path);
    }
    if !outcome.delayed_paths_unprocessed.is_empty() || !outcome.delayed_paths_unknown.is_empty() {
        bail!("A submodule checkout filter left files unfinished");
    }
    gix::utils::stream::check(&IS_INTERRUPTED).or_error()?;
    Ok(index)
}

fn scratch_directory(parent: &Path) -> Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix(".brit-submodule-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(parent).or_error()
}

fn checked_parent(root: &Path, relative: &Path) -> Result<PathBuf> {
    let parent = relative.parent().expect("tracked paths have a parent");
    if !parent.as_os_str().is_empty() {
        gix::fs::symlink::validate_relative_path(root, parent)?;
    }
    Ok(root.join(relative))
}

fn git_path(path: &Path) -> Result<BString> {
    let bytes = gix::path::into_bstr(path)?;
    let bytes = gix::path::to_unix_separators_on_windows(bytes).into_owned();
    if bytes.contains(&b'\n') || bytes.contains(&b'\r') {
        bail!("Submodule gitfiles cannot contain paths with a line break".validation());
    }
    Ok(bytes)
}

/// Record successful filesystem changes so a failed publication can restore the old checkout.
#[derive(Default)]
struct Publication(Vec<Undo>);

enum Undo {
    Rename { from: PathBuf, to: PathBuf },
    RemoveDir(PathBuf),
    CreateDir(PathBuf, fs::Permissions),
}

impl Publication {
    fn back_up_checkout(&mut self, work_dir: &Path, backup: &Path, index: &gix::index::File) -> Result<()> {
        for entry in index.entries().iter().filter(|entry| entry.mode != Mode::COMMIT) {
            let path = gix::path::from_bstr(entry.path(index))?;
            let source = checked_parent(work_dir, &path)?;
            let saved = backup.join(&path);
            fs::create_dir_all(saved.parent().expect("backup path has a parent")).or_error()?;
            self.move_file(&source, &saved)?;
        }
        for entry in index.entries() {
            let relative_path = gix::path::from_bstr(entry.path(index))?;
            let path = checked_parent(work_dir, &relative_path)?;
            // Retain checked-out nested submodules, and remove only empty obsolete directories.
            let mut dir = if entry.mode == Mode::COMMIT {
                Some(path.as_path())
            } else {
                path.parent()
            };
            while let Some(path) = dir.filter(|path| *path != work_dir) {
                if !self.remove_empty_dir(path)? {
                    break;
                }
                dir = path.parent();
            }
        }
        Ok(())
    }

    fn move_file(&mut self, from: &Path, to: &Path) -> Result<()> {
        match fs::symlink_metadata(to) {
            Ok(_) => bail!("Submodule checkout would overwrite {}", to.display()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).or_error(),
        }
        fs::rename(from, to).or_raise(|| message!("Could not move {from:?} to {to:?}"))?;
        self.0.push(Undo::Rename {
            from: to.into(),
            to: from.into(),
        });
        Ok(())
    }

    fn mkdir(&mut self, path: &Path) -> Result<()> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => return Ok(()),
            Ok(_) => bail!("Submodule checkout needs a directory at {}", path.display()),
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err).or_error(),
        }
        self.mkdir(path.parent().expect("a missing directory has a parent"))?;
        fs::create_dir(path).or_error()?;
        self.0.push(Undo::RemoveDir(path.into()));
        Ok(())
    }

    fn remove_empty_dir(&mut self, path: &Path) -> Result<bool> {
        let permissions = match fs::symlink_metadata(path) {
            Ok(meta) if meta.is_dir() => meta.permissions(),
            Ok(_) => return Ok(false),
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(true),
            Err(err) => return Err(err).or_error(),
        };
        match fs::remove_dir(path) {
            Ok(()) => self.0.push(Undo::CreateDir(path.into(), permissions)),
            Err(err) if err.kind() == ErrorKind::DirectoryNotEmpty => return Ok(false),
            Err(err) => return Err(err).or_error(),
        }
        Ok(true)
    }

    fn rollback(self) -> io::Result<()> {
        for action in self.0.into_iter().rev() {
            match action {
                Undo::Rename { from, to } => fs::rename(from, to)?,
                Undo::RemoveDir(path) => fs::remove_dir(path)?,
                Undo::CreateDir(path, permissions) => {
                    fs::create_dir(&path)?;
                    fs::set_permissions(path, permissions)?;
                }
            }
        }
        Ok(())
    }
}
