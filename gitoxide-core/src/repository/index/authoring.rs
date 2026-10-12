//! Bounded literal-file authoring. All index mutations hold the lock from read through publication.
use std::{borrow::Cow, io::Read};

use gix::{
    Result,
    bstr::{BString, ByteSlice},
    error::{ResultExt, bail},
    index::entry::{Flags, Mode, Stat},
};

/// Resolve explicit literal file paths without following directory symlinks.
pub fn literal_paths(repo: &gix::Repository, paths: &[BString]) -> Result<Vec<BString>> {
    let root = repo
        .workdir()
        .ok_or_else(|| gix::error::message("A worktree is required").validation_error())?;
    if paths.is_empty() {
        bail!("At least one explicit file path is required".validation());
    }
    let mut out = Vec::new();
    for input in paths {
        let raw = input.as_bytes();
        if raw.is_empty() || raw.last() == Some(&b'/') || raw[0] == b':' || raw.iter().any(|b| b"*?[]\\\0".contains(b))
        {
            bail!("Only literal file paths are supported; use Git for pathspecs".unsupported());
        }
        let input_path = gix::path::from_bstr(Cow::Borrowed(input.as_bstr()))?;
        if input_path.is_absolute()
            || input_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            bail!("Use relative file paths without parent traversal".unsupported());
        }
        let path = repo.normalize_path(input)?.into_owned();
        if path.is_empty()
            || path == b".".as_bstr()
            || path.split(|b| *b == b'/').any(|c| c.eq_ignore_ascii_case(b".git"))
        {
            bail!("Repository root and .git paths cannot be authored".validation());
        }
        validate_relative_path(root, path.as_bstr())?;
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}

pub(crate) fn component_options() -> gix::validate::path::component::Options {
    gix::validate::path::component::Options {
        protect_windows: cfg!(windows),
        // Protect Git metadata aliases on every platform without imposing Windows filename rules.
        protect_hfs: true,
        protect_ntfs: true,
    }
}

pub(crate) fn validate_relative_path(root: &std::path::Path, path: &gix::bstr::BStr) -> Result<()> {
    let mut current = root.to_owned();
    let components: Vec<_> = path.split(|b| *b == b'/').collect();
    for (position, component) in components.iter().enumerate() {
        gix::validate::path::component(component.as_bstr(), None, component_options()).or_error()?;
        current.push(gix::path::from_bstr(Cow::Borrowed(component.as_bstr()))?.as_ref());
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if position + 1 < components.len() && !meta.is_dir() => {
                bail!("File paths must not traverse symlink or non-directory ancestors".validation());
            }
            Ok(meta) if position + 1 == components.len() && meta.is_dir() => {
                bail!("Directory staging is not supported; name explicit files".unsupported());
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err).or_error(),
        }
    }
    Ok(())
}

/// Refuse index forms whose special semantics are not supported by this bounded command.
pub fn validate_index(index: &gix::index::State) -> Result<()> {
    index.verify_entries()?;
    if index.is_sparse() || index.link().is_some() {
        bail!("Sparse and split indexes require Git".unsupported());
    }
    for entry in index.entries() {
        if entry.stage_raw() != 0
            || entry
                .flags
                .intersects(Flags::ASSUME_VALID | Flags::SKIP_WORKTREE | Flags::INTENT_TO_ADD)
        {
            bail!("Conflicted, assume-unchanged, skip-worktree, and intent-to-add indexes require Git".unsupported());
        }
    }
    Ok(())
}

/// Read a validated worktree file in Git's canonical representation. Never executes external filters.
pub fn worktree_entry(
    repo: &gix::Repository,
    index: &gix::index::State,
    path: &BString,
) -> Result<Option<(Mode, Vec<u8>)>> {
    let relative = gix::path::from_bstr(Cow::Borrowed(path.as_bstr()))?;
    let root = repo
        .workdir()
        .ok_or_else(|| gix::error::message("A worktree is required").validation_error())?;
    validate_relative_path(root, path.as_bstr())?;
    let absolute = root.join(relative.as_ref());
    let meta = match std::fs::symlink_metadata(&absolute) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).or_error(),
    };
    if meta.file_type().is_symlink() {
        gix::validate::path::component(
            path.rsplit(|b| *b == b'/').next().unwrap_or_default().as_bstr(),
            Some(gix::validate::path::component::Mode::Symlink),
            component_options(),
        )
        .or_error()?;
        let target = std::fs::read_link(absolute).or_error()?;
        return Ok(Some((Mode::SYMLINK, gix::path::into_bstr(target)?.into_owned().into())));
    }
    if !meta.is_file() {
        bail!("Only regular files and symlinks can be staged".unsupported());
    }
    if repo.config_snapshot().try_boolean("core.symlinks")? == Some(false)
        && index
            .entry_by_path(path.as_bstr())
            .is_some_and(|entry| entry.mode == Mode::SYMLINK)
    {
        bail!("Emulated symlinks require Git".unsupported());
    }
    if !gix::filter::Pipeline::options(repo)?.drivers.is_empty() {
        bail!("Configured external clean filters require Git".unsupported());
    }
    let attributes = repo.attributes_only(
        index,
        gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
    )?;
    let mut pipeline = gix::filter::Pipeline::new(repo, attributes.detach())?;
    let file = std::fs::File::open(absolute).or_error()?;
    let mut filtered = pipeline.convert_to_git(file, relative.as_ref(), index)?;
    let mut bytes = Vec::new();
    filtered.read_to_end(&mut bytes).or_error()?;
    let mut mode = executable_mode(&meta);
    if repo.config_snapshot().try_boolean("core.filemode")? == Some(false) {
        mode = index
            .entry_by_path(path.as_bstr())
            .filter(|entry| matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE))
            .map_or(Mode::FILE, |entry| entry.mode);
    }
    Ok(Some((mode, bytes)))
}

#[cfg(unix)]
fn executable_mode(meta: &std::fs::Metadata) -> Mode {
    use std::os::unix::fs::PermissionsExt;
    if meta.permissions().mode() & 0o100 != 0 {
        Mode::FILE_EXECUTABLE
    } else {
        Mode::FILE
    }
}
#[cfg(not(unix))]
fn executable_mode(_: &std::fs::Metadata) -> Mode {
    Mode::FILE
}

/// Stage exactly the named files, including deletions, under one index transaction.
pub fn add_paths(repo: gix::Repository, paths: &[BString]) -> Result<()> {
    mutate(repo, paths, false)
}

/// Restore only named index entries from HEAD, without writing worktree files.
pub fn restore_staged(repo: gix::Repository, paths: &[BString]) -> Result<()> {
    mutate(repo, paths, true)
}

/// Read the original index form, refusing split-index expansion and special files.
pub(crate) fn read_index(repo: &gix::Repository) -> Result<gix::index::File> {
    let index = match std::fs::symlink_metadata(repo.index_path()) {
        Ok(meta) if meta.is_file() => {
            let index = repo.open_index()?;
            // File::at transparently dissolves split indexes. Inspect the original state as well.
            let bytes = std::fs::read(repo.index_path()).or_error()?;
            let (raw, _) =
                gix::index::State::from_bytes(&bytes, index.timestamp(), repo.object_hash(), Default::default())?;
            if raw.link().is_some() {
                bail!("Split indexes require Git".unsupported());
            }
            index
        }
        Ok(_) => bail!("The index must be a regular file".validation()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            gix::index::File::from_state(gix::index::State::new(repo.object_hash()), repo.index_path())
        }
        Err(err) => return Err(err).or_error(),
    };
    validate_index(&index)?;
    Ok(index)
}

fn mutate(repo: gix::Repository, paths: &[BString], restore: bool) -> Result<()> {
    let mut lock =
        gix::lock::File::acquire_to_update_resource(repo.index_path(), gix::lock::acquire::Fail::Immediately, None, 0)
            .or_error()?;
    // Do not consult the repository's cached index: the lock must precede the read.
    let index = read_index(&repo)?;
    let paths = literal_paths(&repo, paths)?;
    if repo.config_snapshot().try_boolean("core.ignorecase")? == Some(true) {
        for path in &paths {
            for entry in index.entries() {
                for (selected, tracked) in path.split(|b| *b == b'/').zip(entry.path(&index).split(|b| *b == b'/')) {
                    if !selected.eq_ignore_ascii_case(tracked) {
                        break;
                    }
                    if selected != tracked {
                        bail!("Case aliases of indexed paths require Git".unsupported());
                    }
                }
            }
        }
    }
    if repo.config_snapshot().try_boolean("core.precomposeunicode")? == Some(true)
        && paths.iter().any(|path| !path.is_ascii())
    {
        bail!("Unicode precomposition requires Git for non-ASCII paths".unsupported());
    }

    let head = if restore {
        Some(repo.index_from_tree(&repo.head_commit()?.tree_id()?)?)
    } else {
        None
    };
    let mut replacements = Vec::new();
    let mut ignores = repo.excludes(&index, None, Default::default())?;
    for path in &paths {
        let existing = index.entry_by_path(path.as_bstr());
        if existing.is_some_and(|entry| entry.mode == Mode::COMMIT) || index.path_is_directory(path.as_bstr()) {
            bail!("Submodules and indexed directories require Git".unsupported());
        }
        if index.entries().iter().any(|entry| {
            let other = entry.path(&index);
            path.starts_with(other.as_bytes()) && path.get(other.len()) == Some(&b'/')
        }) {
            bail!("Replacing an indexed ancestor requires Git".unsupported());
        }
        let replacement = if let Some(head) = &head {
            let source = head.entry_by_path(path.as_bstr());
            if head.path_is_directory(path.as_bstr()) || source.is_some_and(|entry| entry.mode == Mode::COMMIT) {
                bail!("Restoring directories and submodules requires Git".unsupported());
            }
            if source.is_none() && existing.is_none() {
                bail!("Path is absent from both HEAD and the index: {path}".validation());
            }
            source.map(|entry| (entry.mode, entry.id))
        } else {
            match worktree_entry(&repo, &index, path)? {
                None if existing.is_none() => bail!("Path is absent from both worktree and index: {path}".validation()),
                None => None,
                Some((mode, bytes)) => {
                    if existing.is_none()
                        && ignores
                            .at_entry(path.as_bstr(), Some(mode))?
                            .matching_exclude_pattern()
                            .is_some_and(|m| !m.pattern.is_negative())
                    {
                        bail!("Ignored untracked path requires an explicit Git override: {path}".validation());
                    }
                    Some((mode, repo.write_blob(bytes)?.detach()))
                }
            }
        };
        replacements.push((path, replacement));
    }
    // Rebuild to drop TREE, FSMN, UNTR, REUC and other caches invalidated by entry changes.
    let mut state = gix::index::State::new(repo.object_hash());
    for entry in index.entries() {
        let path = entry.path(&index);
        if !paths.iter().any(|selected| selected.as_bstr() == path) {
            state.dangerously_push_entry(
                entry.stat,
                entry.id,
                entry.flags - Flags::FSMONITOR_VALID,
                entry.mode,
                path,
            );
        }
    }
    for (path, replacement) in replacements {
        if let Some((mode, blob_id)) = replacement {
            state.dangerously_push_entry(Stat::default(), blob_id, Flags::empty(), mode, path.as_bstr());
        }
    }
    state.sort_entries();
    let output = gix::index::File::from_state(state, repo.index_path());
    output.write_to(&mut lock, Default::default())?;
    lock.commit().or_error()?;
    Ok(())
}
