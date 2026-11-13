//! Bounded worktree restoration: validate every selection before publishing any file.
use std::{
    borrow::Cow,
    io::{Read, Seek},
    path::Path,
    sync::Arc,
};

use gix::{
    Result,
    bstr::{BString, ByteSlice},
    error::{ResultExt, bail},
    index::entry::Mode,
    objs::FindStream,
    utils::stream::{self, Budget, Spool},
};

use super::authoring::{component_options, literal_paths, read_index, refuse_external_filters, validate_relative_path};

/// Restore literal worktree files from the index, or HEAD when `from_head` is set.
/// The index is read only. Publication is atomic per file, not across the selection.
pub fn restore_worktree(repo: gix::Repository, paths: &[BString], from_head: bool) -> Result<()> {
    // Attribute snapshots below are private objects, never additions to the repository.
    let repo = repo.with_object_memory();
    let index = read_index(&repo)?;
    let paths = literal_paths(&repo, paths)?;
    let root = repo.workdir().expect("literal_paths required a worktree");
    if repo.config_snapshot().try_boolean("core.precomposeunicode")? == Some(true)
        && paths.iter().any(|path| !path.is_ascii())
    {
        bail!("Unicode precomposition requires Git for non-ASCII paths".unsupported());
    }
    let head = if from_head {
        Some(repo.index_from_tree(&repo.head_commit()?.tree_id()?)?)
    } else {
        None
    };
    let source: &gix::index::State = head.as_ref().map_or(&index, |head| head);
    let attribute_index = attribute_snapshot(&repo, &index, source, &paths)?;
    let mut attributes = repo.attributes_only(
        &attribute_index,
        gix::worktree::stack::state::attributes::Source::IdMapping,
    )?;
    refuse_external_filters(&repo, &mut attributes, paths.iter().map(|path| path.as_bstr()))?;
    let (mut pipeline, mut attributes) = gix::filter::Pipeline::new(&repo, attributes.detach())?.into_parts();
    let budget = Budget::new(root).or_error()?;
    let emulate_symlinks = repo.config_snapshot().try_boolean("core.symlinks")? == Some(false);
    let mut replacements = Vec::with_capacity(paths.len());
    for path in &paths {
        for state in [&*index, source] {
            if state.path_is_directory(path.as_bstr()) {
                bail!("Restoring indexed directories requires Git: {path}".unsupported());
            }
            for entry in state.entries() {
                let other = entry.path(state);
                if path.starts_with(other.as_bytes()) && path.get(other.len()) == Some(&b'/') {
                    bail!("Replacing an indexed ancestor requires Git: {path}".unsupported());
                }
                if other == path.as_bstr() && !matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE | Mode::SYMLINK)
                {
                    bail!("Restoring submodules and unsupported entry modes requires Git: {path}".unsupported());
                }
                if repo.config_snapshot().try_boolean("core.ignorecase")? == Some(true) {
                    for (selected, tracked) in path.split(|b| *b == b'/').zip(other.split(|b| *b == b'/')) {
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
        let absolute = root.join(gix::path::from_bstr(Cow::Borrowed(path.as_bstr()))?.as_ref());
        match std::fs::symlink_metadata(&absolute) {
            Ok(meta) if !meta.is_file() && !meta.file_type().is_symlink() => {
                bail!("Only regular files and symlinks can be restored: {path}".unsupported());
            }
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err).or_error(),
        }
        let replacement = match source.entry_by_path(path.as_bstr()) {
            None if !from_head || index.entry_by_path(path.as_bstr()).is_none() => {
                bail!("Path is absent from the restore source: {path}".validation());
            }
            None => None,
            Some(entry) => {
                let (kind, mut blob) = repo
                    .objects
                    .find_stream(&entry.id, &budget, &gix::interrupt::IS_INTERRUPTED)?;
                if kind != gix::objs::Kind::Blob {
                    bail!("Restore source is not a blob: {path} ({})", entry.id);
                }
                let bytes = if entry.mode == Mode::SYMLINK {
                    if !cfg!(unix) && !emulate_symlinks {
                        bail!("Native symlink restoration is unsupported on this platform".unsupported());
                    }
                    gix::validate::path::component(
                        path.rsplit(|b| *b == b'/').next().unwrap_or_default().as_bstr(),
                        Some(gix::validate::path::component::Mode::Symlink),
                        component_options(),
                    )
                    .or_error()?;
                    let mut chunk = [0; stream::CHUNK];
                    let mut invalid = blob.is_empty();
                    loop {
                        stream::check(&gix::interrupt::IS_INTERRUPTED).or_error()?;
                        let n = blob.read(&mut chunk).or_error()?;
                        if n == 0 {
                            break;
                        }
                        invalid |= chunk[..n].contains(&0);
                    }
                    blob.rewind().or_error()?;
                    if invalid {
                        bail!("Invalid symlink target: {path}".validation());
                    }
                    blob
                } else {
                    let platform = attributes.at_entry(path.as_bstr(), None, &repo.objects).or_error()?;
                    let converted = pipeline.convert_to_worktree_stream(
                        blob,
                        path.as_bstr(),
                        &mut |_, attrs| {
                            platform.matching_attributes(attrs);
                        },
                        Default::default(),
                        &budget,
                        &gix::interrupt::IS_INTERRUPTED,
                    )?;
                    let gix::filter::plumbing::pipeline::stream::Outcome::Ready(bytes) = converted else {
                        unreachable!("delay is forbidden")
                    };
                    bytes
                };
                let mode = if entry.mode == Mode::SYMLINK && emulate_symlinks {
                    Mode::FILE
                } else {
                    entry.mode
                };
                Some((mode, bytes))
            }
        };
        replacements.push(replacement);
        if gix::interrupt::is_triggered() {
            return stopped(&paths, 0, "cancelled before publication");
        }
    }
    for (position, (path, replacement)) in paths.iter().zip(replacements).enumerate() {
        if gix::interrupt::is_triggered() {
            return stopped(&paths, position, "cancelled before publication of the next file");
        }
        let result = publish(root, path, replacement, &budget);
        if let Err(err) = result {
            return stopped(&paths, position, &err.to_string());
        }
    }
    if gix::interrupt::is_triggered() {
        return stopped(&paths, paths.len(), "cancelled after the final publication");
    }
    Ok(())
}

/// Model the attributes visible after selected attribute files have been restored, without
/// touching the worktree. Unselected files retain worktree precedence and actual-index fallback,
/// including when the content source is HEAD. The ordinary native filter pipeline consumes this.
fn attribute_snapshot(
    repo: &gix::Repository,
    index: &gix::index::State,
    source: &gix::index::State,
    paths: &[BString],
) -> Result<gix::index::State> {
    use gix::index::entry::{Flags, Stat};
    let root = repo.workdir().expect("literal_paths required a worktree");
    let mut attribute_paths = std::collections::BTreeSet::new();
    for path in paths {
        let mut directory = Vec::new();
        for component in path.split(|b| *b == b'/') {
            let mut attribute = directory.clone();
            attribute.extend_from_slice(b".gitattributes");
            attribute_paths.insert(BString::from(attribute));
            directory.extend_from_slice(component);
            directory.push(b'/');
        }
    }
    let mut snapshot = gix::index::State::new(repo.object_hash());
    for path in attribute_paths {
        let selected = paths.contains(&path);
        let regular = |entry: &&gix::index::Entry| matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE);
        let entry = if selected {
            source
                .entry_by_path(path.as_bstr())
                .filter(regular)
                .or_else(|| index.entry_by_path(path.as_bstr()).filter(regular))
        } else {
            index.entry_by_path(path.as_bstr()).filter(regular)
        };
        let worktree_bytes = if selected {
            None
        } else {
            let absolute = root.join(gix::path::from_bstr(Cow::Borrowed(path.as_bstr()))?.as_ref());
            match std::fs::symlink_metadata(&absolute) {
                Ok(meta) if meta.is_file() => Some(std::fs::read(absolute).or_error()?),
                Ok(_) => None,
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                Err(err) => return Err(err).or_error(),
            }
        };
        let id = if let Some(bytes) = worktree_bytes {
            repo.write_blob(bytes)?.detach()
        } else if let Some(entry) = entry {
            entry.id
        } else {
            // An explicit empty mapping records absent attributes without consulting the worktree.
            repo.write_blob([])?.detach()
        };
        snapshot.dangerously_push_entry(Stat::default(), id, Flags::empty(), Mode::FILE, path.as_bstr());
    }
    snapshot.sort_entries();
    Ok(snapshot)
}

fn stopped(paths: &[BString], completed: usize, reason: &str) -> Result<()> {
    bail!(
        "restore stopped: {reason}; completed paths: {:?}; remaining paths: {:?}; no rollback was attempted",
        &paths[..completed],
        &paths[completed..]
    );
}

fn publish(root: &Path, path: &BString, replacement: Option<(Mode, Spool)>, budget: &Arc<Budget>) -> Result<()> {
    // Repeat traversal validation immediately before each publication; never follow a final symlink.
    validate_relative_path(root, path.as_bstr())?;
    let absolute = root.join(gix::path::from_bstr(Cow::Borrowed(path.as_bstr()))?.as_ref());
    let Some((mode, mut bytes)) = replacement else {
        if gix::interrupt::is_triggered() {
            bail!("cancelled before publication");
        }
        return match std::fs::remove_file(&absolute) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err).or_error(),
        };
    };
    let parent = absolute.parent().expect("a literal file has a parent");
    // Missing ancestors are created only during publication. Remove them on failure/cancellation.
    let mut missing = Vec::new();
    let mut ancestor = parent;
    while !ancestor.exists() {
        missing.push(ancestor.to_owned());
        ancestor = ancestor.parent().expect("the worktree root exists");
    }
    let mut created = Vec::new();
    let result = (|| -> Result<()> {
        for directory in missing.iter().rev() {
            std::fs::create_dir(directory).or_error()?;
            created.push(directory.clone());
        }
        if mode == Mode::SYMLINK {
            let temporary = tempfile::Builder::new()
                .prefix(".brit-restore-")
                .tempdir_in(parent)
                .or_error()?;
            let staged = temporary.path().join("link");
            let bytes = bytes.into_mapped().or_error()?;
            create_symlink(bytes.as_slice(), &staged)?;
            if gix::interrupt::is_triggered() {
                bail!("cancelled before publication");
            }
            validate_relative_path(root, path.as_bstr())?;
            std::fs::rename(staged, &absolute).or_error()?;
        } else {
            let mut temporary = temporary_file(parent, mode, budget)?;
            stream::copy(&mut bytes, &mut temporary, &gix::interrupt::IS_INTERRUPTED).or_error()?;
            drop(bytes);
            if gix::interrupt::is_triggered() {
                bail!("cancelled before publication");
            }
            validate_relative_path(root, path.as_bstr())?;
            // Drop a failed persistence temporary before removing newly created parents.
            temporary.publish_replace(&absolute).or_error()?;
        }
        Ok(())
    })();
    if result.is_err() {
        for directory in created.iter().rev() {
            // Only empty directories created for this file can be removed.
            let _ = std::fs::remove_dir(directory);
        }
    }
    result
}

#[cfg(unix)]
fn create_symlink(bytes: &[u8], path: &Path) -> Result<()> {
    use std::os::unix::ffi::OsStrExt;
    std::os::unix::fs::symlink(std::ffi::OsStr::from_bytes(bytes), path).or_error()
}
#[cfg(not(unix))]
fn create_symlink(_: &[u8], _: &Path) -> Result<()> {
    bail!("Native symlink restoration is unsupported on this platform".unsupported());
}

fn temporary_file(parent: &Path, mode: Mode, budget: &Arc<Budget>) -> Result<Spool> {
    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        // Builder passes these to open(2), so the process umask remains authoritative.
        Some(std::fs::Permissions::from_mode(if mode == Mode::FILE_EXECUTABLE {
            0o777
        } else {
            0o666
        }))
    };
    #[cfg(not(unix))]
    let permissions = {
        let _ = mode;
        None
    };
    budget.spool_in(parent, ".brit-restore-", permissions).or_error()
}
