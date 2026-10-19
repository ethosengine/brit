//! Literal selection and native traversal, with one locked index publication.
use std::{borrow::Cow, collections::BTreeSet, path::Path};

use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    dir::{entry::Kind, walk::EmissionMode},
    error::{ResultExt, bail},
    index::entry::{Flags, Mode, Stat},
};

use super::authoring::{component_options, read_index, worktree_entry};

/// Stage literal files and directories. Empty paths select the whole repository.
/// With `update`, only existing index paths are selected.
pub fn stage_paths(repo: gix::Repository, paths: &[BString], update: bool) -> Result<()> {
    let root = repo
        .workdir()
        .ok_or_else(|| gix::error::message("A worktree is required").validation_error())?;
    let mut lock =
        gix::lock::File::acquire_to_update_resource(repo.index_path(), gix::lock::acquire::Fail::Immediately, None, 0)
            .or_error()?;
    let index = read_index(&repo)?;
    let scopes = selection(&repo, root, paths)?;
    let selected = |path: &BStr| scopes.iter().any(|scope| contains(scope.as_bstr(), path));
    let mut candidates = BTreeSet::<BString>::new();
    for entry in index.entries() {
        let path = entry.path(&index);
        if selected(path) {
            if !matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE | Mode::SYMLINK) {
                bail!("Selected submodules and unsupported index modes require Git: {path}".unsupported());
            }
            candidates.insert(path.into());
        }
    }
    let mut ignores = repo.excludes(&index, None, Default::default())?;
    for scope in &scopes {
        if scope.is_empty() {
            continue;
        }
        let absolute = root.join(gix::path::from_bstr(Cow::Borrowed(scope.as_bstr()))?.as_ref());
        let metadata = match std::fs::symlink_metadata(absolute) {
            Ok(meta) => Some(meta),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => return Err(err).or_error(),
        };
        let tracked = index.entry_by_path(scope.as_bstr()).is_some() || index.path_is_directory(scope.as_bstr());
        if update && !tracked {
            bail!("Path does not match any tracked files: {scope}".validation());
        }
        if metadata.is_none() && !tracked {
            bail!("Path is absent from both worktree and index: {scope}".validation());
        }
        if let Some(meta) = metadata {
            let mode = if meta.is_dir() {
                Mode::DIR
            } else if meta.file_type().is_symlink() {
                Mode::SYMLINK
            } else {
                Mode::FILE
            };
            if !tracked
                && ignores
                    .at_entry(scope.as_bstr(), Some(mode))?
                    .matching_exclude_pattern()
                    .is_some_and(|m| !m.pattern.is_negative())
            {
                bail!("Ignored untracked path requires an explicit Git override: {scope}".validation());
            }
            if !meta.is_dir() && !update {
                candidates.insert(scope.clone());
            }
        }
    }

    // Native traversal owns ignore precedence, symlink handling and repository detection.
    // Literal matching is performed by the delegate, never by Git pathspec parsing.
    let mut walk = SelectionWalk {
        root,
        scopes: &scopes,
        entries: Vec::new(),
        nested: None,
    };
    repo.dirwalk(
        &index,
        Vec::<BString>::new(),
        &gix::interrupt::IS_INTERRUPTED,
        repo.dirwalk_options()?
            .empty_patterns_match_prefix(false)
            .emit_tracked(true)
            .emit_untracked(EmissionMode::Matching)
            .recurse_repositories(false)
            .classify_untracked_bare_repositories(true),
        &mut walk,
    )?;
    if let Some(path) = walk.nested {
        bail!("Selected nested repositories and submodules require Git: {path}".unsupported());
    }
    for entry in walk.entries {
        let path = entry.rela_path;
        if !selected(path.as_bstr())
            || matches!(
                entry.status,
                gix::dir::entry::Status::Ignored(_) | gix::dir::entry::Status::Pruned
            )
        {
            continue;
        }
        if entry.disk_kind == Some(Kind::Repository) || entry.index_kind == Some(Kind::Repository) {
            if entry.index_kind.is_some() || matches!(entry.status, gix::dir::entry::Status::Tracked) {
                bail!("Selected nested repositories and submodules require Git: {path}".unsupported());
            }
            continue;
        }
        if !update
            && !matches!(entry.disk_kind, Some(Kind::Directory) | None)
            && !matches!(
                entry.status,
                gix::dir::entry::Status::Ignored(_) | gix::dir::entry::Status::Pruned
            )
        {
            candidates.insert(path);
        }
    }
    check_cancelled()?;
    let ignore_case = repo.config_snapshot().try_boolean("core.ignorecase")? == Some(true);
    let precompose = repo.config_snapshot().try_boolean("core.precomposeunicode")? == Some(true);
    let mut replacements = Vec::new();
    for path in &candidates {
        check_cancelled()?;
        if precompose && !path.is_ascii() {
            bail!("Unicode precomposition requires Git for non-ASCII paths".unsupported());
        }
        if ignore_case {
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
        // A tracked file replaced by a directory, or hidden beneath a replacement
        // file/symlink, is a deletion. Never follow that symlink to read its target.
        let replacement = if is_file(root, path.as_bstr())? {
            worktree_entry(&repo, &index, path)?
                .map(|(mode, bytes)| repo.write_blob(bytes).map(|id| (mode, id.detach())))
                .transpose()?
        } else {
            None
        };
        replacements.push((path, replacement));
    }
    // Rebuilding invalidates TREE/FSMN/UNTR/REUC while preserving unselected entries.
    let mut state = gix::index::State::new(repo.object_hash());
    for entry in index.entries() {
        let path = entry.path(&index);
        if !candidates.contains(path) {
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
            // Replacing an ancestor outside the selection would damage an unrelated
            // staged entry. A broader directory selection must name that transition.
            if state.entries().iter().any(|entry| {
                let other = entry.path(&state);
                contains(other, path.as_bstr()) || contains(path.as_bstr(), other)
            }) {
                bail!(
                    "Replacing an unselected indexed ancestor or directory requires a broader selection: {path}"
                        .validation()
                );
            }
            state.dangerously_push_entry(Stat::default(), blob_id, Flags::empty(), mode, path.as_bstr());
        }
    }
    state.sort_entries();
    state.verify_entries()?;
    let output = gix::index::File::from_state(state, repo.index_path());
    check_cancelled()?;
    output.write_to(&mut lock, Default::default())?;
    check_cancelled()?;
    lock.commit().or_error()?;
    // Publication is final: a later interrupt must not be reported as a rollback.
    Ok(())
}

fn contains(scope: &BStr, path: &BStr) -> bool {
    scope.is_empty() || scope == path || (path.starts_with(scope.as_bytes()) && path.get(scope.len()) == Some(&b'/'))
}

fn check_cancelled() -> Result<()> {
    if gix::interrupt::is_triggered() {
        bail!("Staging interrupted before index publication".validation());
    }
    Ok(())
}

fn selection(repo: &gix::Repository, root: &Path, paths: &[BString]) -> Result<Vec<BString>> {
    if paths.is_empty() {
        return Ok(vec![BString::default()]);
    }
    let mut scopes = BTreeSet::new();
    for input in paths {
        let raw = input.as_bytes();
        if raw.is_empty() || raw[0] == b':' || raw.iter().any(|b| b"*?[]\\\0".contains(b)) {
            bail!("Only literal file and directory paths are supported; use Git for pathspecs".unsupported());
        }
        let input_path = gix::path::from_bstr(Cow::Borrowed(input.as_bstr()))?;
        if input_path.is_absolute()
            || input_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            bail!("Use relative paths without parent traversal".unsupported());
        }
        // Normalization removes terminal `/.`, but that spelling still requires
        // a physical directory and must never turn `link/.` into symlink staging.
        let requires_directory = raw.last() == Some(&b'/') || raw.ends_with(b"/.");
        let mut path = repo.normalize_path(input)?.into_owned();
        while path.last() == Some(&b'/') {
            path.pop();
        }
        if path == b".".as_bstr() {
            path.clear();
        }
        let components: Vec<_> = path.split(|b| *b == b'/').filter(|c| !c.is_empty()).collect();
        let mut current = root.to_owned();
        for (position, component) in components.iter().enumerate() {
            gix::validate::path::component(component.as_bstr(), None, component_options()).or_error()?;
            current.push(gix::path::from_bstr(Cow::Borrowed(component.as_bstr()))?.as_ref());
            match std::fs::symlink_metadata(&current) {
                Ok(meta) => {
                    if position + 1 == components.len() && requires_directory && !meta.is_dir() {
                        bail!(
                            "A directory spelling requires a directory, never a symlink or regular file".validation()
                        );
                    }
                    if position + 1 < components.len() && !meta.is_dir() {
                        bail!("Paths must not traverse symlink or non-directory ancestors".validation());
                    }
                    if meta.is_dir()
                        && (current.join(".git").symlink_metadata().is_ok() || gix::discover::is_git(&current).is_ok())
                    {
                        bail!("Selected nested repositories and submodules require Git".unsupported());
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => return Err(err).or_error(),
            }
        }
        scopes.insert(path);
    }
    Ok(scopes.into_iter().collect())
}

/// Whether a path still names a file without following any directory symlink.
fn is_file(root: &Path, path: &BStr) -> Result<bool> {
    let mut current = root.to_owned();
    let components: Vec<_> = path.split(|b| *b == b'/').collect();
    for (position, component) in components.iter().enumerate() {
        gix::validate::path::component(component.as_bstr(), None, component_options()).or_error()?;
        current.push(gix::path::from_bstr(Cow::Borrowed(component.as_bstr()))?.as_ref());
        match std::fs::symlink_metadata(&current) {
            Ok(meta)
                if meta.is_dir()
                    && (current.join(".git").symlink_metadata().is_ok() || gix::discover::is_git(&current).is_ok()) =>
            {
                bail!("Selected nested repositories and submodules require Git: {path}".unsupported());
            }
            Ok(meta) if position + 1 == components.len() => return Ok(!meta.is_dir()),
            Ok(meta) if !meta.is_dir() => return Ok(false),
            Ok(_) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(err) => return Err(err).or_error(),
        }
    }
    Ok(false)
}

struct SelectionWalk<'a> {
    root: &'a Path,
    nested: Option<BString>,
    scopes: &'a [BString],
    entries: Vec<gix::dir::Entry>,
}

impl gix::dir::walk::Delegate for SelectionWalk<'_> {
    fn emit(&mut self, entry: gix::dir::EntryRef<'_>, _: Option<gix::dir::entry::Status>) -> gix::dir::walk::Action {
        self.entries.push(entry.to_owned());
        std::ops::ControlFlow::Continue(())
    }

    fn can_recurse(
        &mut self,
        entry: gix::dir::EntryRef<'_>,
        for_deletion: Option<gix::dir::walk::ForDeletionMode>,
        root_is_repository: bool,
    ) -> bool {
        let relevant = self.scopes.iter().any(|scope| {
            contains(scope.as_bstr(), entry.rela_path.as_ref()) || contains(entry.rela_path.as_ref(), scope.as_bstr())
        });
        if !relevant
            || !entry
                .status
                .can_recurse(entry.disk_kind, entry.pathspec_match, for_deletion, root_is_repository)
        {
            return false;
        }
        if entry.rela_path.is_empty() {
            return true;
        }
        if let Ok(relative) = gix::path::from_bstr(Cow::Borrowed(entry.rela_path.as_ref())) {
            let absolute = self.root.join(relative.as_ref());
            if absolute.join(".git").symlink_metadata().is_ok() || gix::discover::is_git(&absolute).is_ok() {
                if entry.index_kind.is_some() || matches!(entry.status, gix::dir::entry::Status::Tracked) {
                    self.nested = Some(entry.rela_path.clone().into_owned());
                }
                return false;
            }
        }
        true
    }
}
