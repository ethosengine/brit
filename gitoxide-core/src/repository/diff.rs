use gix::{
    ObjectId, Result,
    bstr::{BStr, BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message, unsupported},
    objs::FindStream,
    objs::tree::EntryMode,
    odb::store::RefreshMode,
    prelude::ObjectIdExt,
    utils::stream::{self, Budget, Mapped, Spool},
};
use std::{
    io::{Seek, Write},
    sync::Arc,
};

pub(crate) mod revisions;
pub use revisions::{Differences, Format, Request, diff};

pub fn tree(
    mut repo: gix::Repository,
    out: &mut dyn std::io::Write,
    old_treeish: BString,
    new_treeish: BString,
) -> Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));
    repo.objects.refresh = RefreshMode::Never;

    let old_tree_id = repo.rev_parse_single(old_treeish.as_bstr())?;
    let new_tree_id = repo.rev_parse_single(new_treeish.as_bstr())?;

    let source = gix::object::bounded::Source::deferred(&repo);
    let old_tree = source.peel(old_tree_id.detach(), gix::objs::Kind::Tree)?;
    let new_tree = source.peel(new_tree_id.detach(), gix::objs::Kind::Tree)?;

    let changes = repo.diff_tree_to_tree_ids(Some(old_tree), Some(new_tree), None)?;
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);

    writeln!(
        rendered,
        "Diffing trees `{old_treeish}` ({old_tree_id}) -> `{new_treeish}` ({new_tree_id})\n"
    )
    .or_error()?;
    write_changes(&repo, &mut rendered, changes).or_error()?;
    write_diff_output(out, &mut rendered)
}

fn write_changes(
    repo: &gix::Repository,
    mut out: impl std::io::Write,
    changes: Vec<gix::diff::tree_with_rewrites::Change>,
) -> std::io::Result<()> {
    for change in changes {
        match change {
            gix::diff::tree_with_rewrites::Change::Addition {
                location,
                id,
                entry_mode,
                ..
            } => {
                writeln!(out, "A: {}", typed_location(location, entry_mode))?;
                writeln!(out, "  {}", id.attach(repo).shorten_or_id())?;
                writeln!(out, "  -> {entry_mode:o}")?;
            }
            gix::diff::tree_with_rewrites::Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            } => {
                writeln!(out, "D: {}", typed_location(location, entry_mode))?;
                writeln!(out, "  {}", id.attach(repo).shorten_or_id())?;
                writeln!(out, "  {entry_mode:o} ->")?;
            }
            gix::diff::tree_with_rewrites::Change::Modification {
                location,
                previous_id,
                id,
                previous_entry_mode,
                entry_mode,
            } => {
                writeln!(out, "M: {}", typed_location(location, entry_mode))?;
                writeln!(
                    out,
                    "  {previous_id} -> {id}",
                    previous_id = previous_id.attach(repo).shorten_or_id(),
                    id = id.attach(repo).shorten_or_id()
                )?;
                if previous_entry_mode != entry_mode {
                    writeln!(out, "  {previous_entry_mode:o} -> {entry_mode:o}")?;
                }
            }
            gix::diff::tree_with_rewrites::Change::Rewrite {
                source_location,
                source_id,
                id,
                location,
                source_entry_mode,
                entry_mode,
                ..
            } => {
                writeln!(
                    out,
                    "R: {source} -> {dest}",
                    source = typed_location(source_location, source_entry_mode),
                    dest = typed_location(location, entry_mode)
                )?;
                writeln!(
                    out,
                    "  {source_id} -> {id}",
                    source_id = source_id.attach(repo).shorten_or_id(),
                    id = id.attach(repo).shorten_or_id()
                )?;
                if source_entry_mode != entry_mode {
                    writeln!(out, "  {source_entry_mode:o} -> {entry_mode:o}")?;
                }
            }
        }
    }

    Ok(())
}

fn typed_location(mut location: BString, mode: EntryMode) -> BString {
    if mode.is_tree() {
        location.push(b'/');
    }
    location
}

fn resolve_revspec(
    repo: &gix::Repository,
    revspec: BString,
) -> Result<(ObjectId, Option<std::path::PathBuf>, BString)> {
    let result = repo.rev_parse(revspec.as_bstr());

    match result {
        Err(err) => {
            // `is_not_found()` also matches missing objects, which must not become filesystem paths.
            // Extract the missing reference's name, which may differ from the revspec after following symbolic refs.
            let not_found = err.downcast_any_ref::<gix::refs::file::find::NotFound>();
            if let Some(gix::refs::file::find::NotFound { name }) = not_found {
                let root = repo.workdir().map(ToOwned::to_owned);
                Ok((ObjectId::null(gix::hash::Kind::Sha1), root, name.clone()))
            } else {
                Err(err)
            }
        }
        Ok(resolved_revspec) => {
            let blob_id = resolved_revspec
                .single()
                .ok_or_raise(|| message!("rev-spec '{revspec}' must resolve to a single object").validation())?;

            let (path, _) = resolved_revspec
                .path_and_mode()
                .ok_or_raise(|| message!("rev-spec '{revspec}' must contain a path").validation())?;

            Ok((blob_id.into(), None, path.into()))
        }
    }
}

pub fn file(
    mut repo: gix::Repository,
    out: &mut dyn std::io::Write,
    old_revspec: BString,
    new_revspec: BString,
) -> Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));
    repo.objects.refresh = RefreshMode::Never;

    let (old_blob_id, old_root, old_path) = resolve_revspec(&repo, old_revspec)?;
    let (new_blob_id, new_root, new_path) = resolve_revspec(&repo, new_revspec)?;

    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let buffer_limit = repo
        .allocation_limit()
        .unwrap_or(usize::try_from(repo.big_file_threshold()?).unwrap_or(usize::MAX));
    let worktree_roots = gix::diff::blob::pipeline::WorktreeRoots { old_root, new_root };

    let mut resource_cache = repo.diff_resource_cache(
        gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
        worktree_roots,
    )?;

    resource_cache.set_resource_bounded(
        old_blob_id,
        gix::object::tree::EntryKind::Blob,
        old_path.as_ref(),
        gix::diff::blob::ResourceKind::OldOrSource,
        &repo.objects,
        &budget,
        buffer_limit,
        &gix::interrupt::IS_INTERRUPTED,
    )?;
    resource_cache.set_resource_bounded(
        new_blob_id,
        gix::object::tree::EntryKind::Blob,
        new_path.as_ref(),
        gix::diff::blob::ResourceKind::NewOrDestination,
        &repo.objects,
        &budget,
        buffer_limit,
        &gix::interrupt::IS_INTERRUPTED,
    )?;

    let outcome = resource_cache.prepare_diff()?;

    use gix::diff::blob::platform::prepare_diff::Operation;

    let (algorithm, change) = match outcome.operation {
        Operation::InternalDiff { algorithm, change } => (algorithm, change),
        Operation::ExternalCommand { .. } => {
            unreachable!("We disabled that")
        }
        Operation::SourceOrDestinationIsBinary => {
            bail!(unsupported("Source or destination is binary and we can't diff that"))
        }
        Operation::NeitherExists => {
            bail!(message("Neither source nor destination exists; choose at least one existing file").validation())
        }
    };

    let interner = change.patch_input();

    let diff = gix::diff::blob::diff_with_slider_heuristics(algorithm, &interner);
    let mut rendered = budget.spool().or_error()?;
    gix::diff::blob::UnifiedDiff::new(
        &diff,
        &interner,
        FileHunks(&mut rendered),
        gix::diff::blob::unified_diff::ContextSize::symmetrical(3),
    )
    .consume()
    .or_error()?;
    drop(interner);
    drop(resource_cache);
    rendered.rewind().or_error()?;
    stream::copy(&mut rendered, out, &gix::interrupt::IS_INTERRUPTED).or_error()?;

    Ok(())
}

struct FileHunks<'a>(&'a mut dyn std::io::Write);
impl gix::diff::blob::unified_diff::ConsumeHunk for FileHunks<'_> {
    type Out = ();
    fn consume_hunk(
        &mut self,
        header: gix::diff::blob::unified_diff::HunkHeader,
        lines: &[(gix::diff::blob::unified_diff::DiffLineKind, &[u8])],
    ) -> std::io::Result<()> {
        stream::check(&gix::interrupt::IS_INTERRUPTED)?;
        writeln!(self.0, "{header}")?;
        for &(kind, bytes) in lines {
            stream::check(&gix::interrupt::IS_INTERRUPTED)?;
            self.0.write_all(&[kind.to_prefix() as u8])?;
            self.0.write_all(bytes)?;
            if !bytes.ends_with(b"\n") {
                self.0.write_all(b"\n")?;
            }
        }
        Ok(())
    }
    fn finish(self) {}
}

/// Show HEAD-to-index or index-to-worktree patches without refreshing or writing the index.
/// Literal file selection deliberately does not interpret Git pathspec magic.
pub fn working(repo: gix::Repository, out: &mut dyn std::io::Write, cached: bool, paths: &[BString]) -> Result<()> {
    working_output(&repo, out, None, cached, paths, Format::Patch).map(drop)
}

/// Show HEAD-to-index or index-to-worktree statistics without writing repository state.
pub fn working_stat(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    cached: bool,
    paths: &[BString],
) -> Result<()> {
    working_output(&repo, out, None, cached, paths, Format::Stat).map(drop)
}

/// Compare `base` (a tree, or HEAD when `None` and `cached`, or the index otherwise) with the
/// index (`cached`) or the working files, and return whether anything differs.
fn working_output(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    base: Option<ObjectId>,
    cached: bool,
    paths: &[BString],
    format: Format,
) -> Result<bool> {
    use super::index::authoring::{literal_paths, read_index, validate_index};
    use std::collections::{BTreeMap, BTreeSet};

    if format.needs_content() {
        check_external_diff(repo)?;
    }
    let index = read_index(repo)?;
    let selected = if paths.is_empty() {
        Vec::new()
    } else {
        literal_paths(repo, paths)?
    };
    let mut before = BTreeMap::new();
    let mut after = BTreeMap::new();
    let insert = |state: &gix::index::State, map: &mut BTreeMap<BString, (u32, ObjectId)>| -> Result<()> {
        for entry in state.entries() {
            map.insert(entry.path(state).into(), (entry.mode.bits(), entry.id));
        }
        Ok(())
    };
    insert(&index, &mut after)?;
    let base = match base {
        Some(tree) => Some(tree),
        None if cached && !repo.head()?.is_unborn() => Some(repo.head_tree_id()?.detach()),
        None => None,
    };
    if let Some(tree) = base {
        let base = repo.index_from_tree(&tree)?;
        validate_index(&base)?;
        insert(&base, &mut before)?;
    } else if !cached {
        before.clone_from(&after);
    }
    // A literal path selects itself or, as a directory, every tracked path below it.
    let is_selected = |path: &BString, selected: &BString| {
        path == selected || (path.starts_with(selected) && path.get(selected.len()) == Some(&b'/'))
    };
    // Like Git, a path that selects nothing selects nothing; it is not an error.
    let all: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    let paths: Vec<_> = all
        .into_iter()
        .filter(|path| selected.is_empty() || selected.iter().any(|selected| is_selected(path, selected)))
        .collect();
    let mut attrs = repo.attributes_only(
        &index,
        gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
    )?;
    let mut matches = attrs.attribute_matches();
    let limit = repo.big_file_threshold()?;
    let quote_high = quote_path(repo)?;
    // Buffer output so a refused path never leaves a misleading partial patch.
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    let mut stats = Vec::new();
    let mut changed = false;
    let mut submodules_checked = false;
    for path in paths {
        check_interrupted()?;
        // HEAD and index identities prove no staged change, even for paths whose
        // patches we cannot render. Worktree comparisons still need to read disk.
        if cached && before.get(&path) == after.get(&path) {
            continue;
        }
        let gitlink = |side: Option<&(u32, ObjectId)>| side.is_some_and(|(mode, _)| *mode == GITLINK);
        if !submodules_checked && (gitlink(before.get(&path)) || gitlink(after.get(&path))) {
            check_submodule_settings(repo, format)?;
            submodules_checked = true;
        }
        let new = if cached {
            after
                .get(&path)
                .map(|&side| load_side(repo, side, &budget, limit))
                .transpose()?
        } else if let Some(&(GITLINK, id)) = after.get(&path) {
            worktree_gitlink(repo, &index, &path, id, &budget)?
        } else if after.contains_key(&path) {
            worktree_patch_blob(repo, &index, &path, &budget)?
        } else {
            // Like Git, a path the index does not track is absent from the working side.
            None
        };
        // Canonical identity and effective mode prove a worktree entry unchanged
        // without loading the index blob. The authoring read above still validates
        // paths and conversions and retains external-filter refusals.
        if !cached
            && before
                .get(&path)
                .zip(new.as_ref())
                .is_some_and(|(&(mode, blob_id), new)| mode == new.mode && blob_id == new.blob_id && !new.dirty)
        {
            continue;
        }
        let old_side = before.get(&path).copied();
        if old_side.is_none() && new.is_none() {
            continue;
        }
        changed = true;
        let new_side = new.as_ref().map(|new| (new.mode, new.blob_id));
        match format {
            Format::Quiet => break,
            Format::NameOnly | Format::NameStatus => {
                write_name(&mut rendered, format, &path, old_side, new_side, quote_high)?;
                continue;
            }
            Format::Patch | Format::Stat | Format::NumStat => {}
        }
        let old = old_side.map(|side| load_side(repo, side, &budget, limit)).transpose()?;
        attrs.at_entry(&path, None)?.matching_attributes(&mut matches);
        let binary = binary_attribute(&matches, &path)?;
        if format == Format::Patch {
            render_patch(repo, &mut rendered, &path, old.as_ref(), new.as_ref(), binary)?;
        } else {
            collect_stat(repo, &mut stats, &path, old.as_ref(), new.as_ref(), binary)?;
        }
    }
    match format {
        Format::Stat => render_stats(&stats, &mut rendered)?,
        Format::NumStat => render_numstat(&stats, &mut rendered)?,
        _ => {}
    }
    write_diff_output(out, &mut rendered)?;
    Ok(changed)
}

/// Whether `core.quotePath` asks for bytes above 0x7f to be quoted (Git's default).
fn quote_path(repo: &gix::Repository) -> Result<bool> {
    Ok(repo.config_snapshot().try_boolean("core.quotePath")?.unwrap_or(true))
}

/// Write one `--name-only` or `--name-status` line, quoted like Git.
fn write_name(
    out: &mut dyn std::io::Write,
    format: Format,
    path: &[u8],
    old: Option<(u32, ObjectId)>,
    new: Option<(u32, ObjectId)>,
    quote_high: bool,
) -> Result<()> {
    if format == Format::NameStatus {
        let status = match (old, new) {
            (None, _) => b'A',
            (_, None) => b'D',
            (Some((old, _)), Some((new, _))) if old & 0o170000 != new & 0o170000 => b'T',
            _ => b'M',
        };
        out.write_all(&[status, b'\t']).or_error()?;
    }
    out.write_all(&quote_patch_path(path, quote_high)).or_error()?;
    out.write_all(b"\n").or_error()?;
    Ok(())
}

/// Git's `--numstat`: added and removed lines per path, or `-` twice for binary files.
fn render_numstat(stats: &[FileStat], out: &mut dyn std::io::Write) -> Result<()> {
    for stat in stats {
        if stat.binary {
            out.write_all(b"-\t-\t").or_error()?;
        } else {
            write!(out, "{}\t{}\t", stat.added, stat.removed).or_error()?;
        }
        out.write_all(&stat.name).or_error()?;
        out.write_all(b"\n").or_error()?;
    }
    Ok(())
}

struct PatchBlob {
    mode: u32,
    blob_id: ObjectId,
    data: PatchData,
    from_worktree: bool,
    /// A checked-out submodule whose files differ from its `HEAD`, which Git marks `-dirty`.
    dirty: bool,
}

enum PatchData {
    Buffered(Vec<u8>),
    Mapped(Mapped),
    /// A blob above `core.bigFileThreshold`, which Git reports as binary without reading it.
    TooLarge(u64),
}
impl PatchData {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Buffered(bytes) => bytes,
            Self::Mapped(bytes) => bytes.as_slice(),
            Self::TooLarge(_) => &[],
        }
    }
    fn len(&self) -> u64 {
        match self {
            Self::TooLarge(size) => *size,
            data => data.as_slice().len() as u64,
        }
    }
    fn is_too_large(&self) -> bool {
        matches!(self, Self::TooLarge(_))
    }
    fn from_spool(repo: &gix::Repository, mut spool: Spool) -> Result<Self> {
        use std::io::Read;
        let limit = repo
            .allocation_limit()
            .map(|n| n as u64)
            .unwrap_or(repo.big_file_threshold()?);
        if spool.len() > limit {
            return Ok(Self::Mapped(spool.into_mapped().or_error()?));
        }
        let size = usize::try_from(spool.len()).or_error()?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(size)
            .or_raise(|| message!("Diff could not allocate {size} bytes within limit {limit}").allocation_failure())?;
        spool.rewind().or_error()?;
        spool.read_to_end(&mut bytes).or_error()?;
        Ok(Self::Buffered(bytes))
    }
}

fn load_patch_blob(repo: &gix::Repository, mode: u32, blob_id: ObjectId, budget: &Arc<Budget>) -> Result<PatchBlob> {
    let (kind, data) = repo
        .objects
        .find_stream(&blob_id, budget, &gix::interrupt::IS_INTERRUPTED)?;
    if kind != gix::objs::Kind::Blob {
        bail!(gix::error::corruption(format!("Expected blob {blob_id}, found {kind}")));
    }
    Ok(PatchBlob {
        mode,
        blob_id,
        data: PatchData::from_spool(repo, data)?,
        from_worktree: false,
        dirty: false,
    })
}

/// The mode of a submodule (gitlink) entry.
const GITLINK: u32 = 0o160000;

/// Load one side of a comparison whose object identity is known: a gitlink reads as Git's
/// `Subproject commit` line, and a blob above `limit` (`core.bigFileThreshold`) is only sized.
fn load_side(
    repo: &gix::Repository,
    (mode, id): (u32, ObjectId),
    budget: &Arc<Budget>,
    limit: u64,
) -> Result<PatchBlob> {
    if mode == GITLINK {
        return Ok(PatchBlob {
            mode,
            blob_id: id,
            data: PatchData::Buffered(format!("Subproject commit {id}\n").into_bytes()),
            from_worktree: false,
            dirty: false,
        });
    }
    let size = repo.find_header(id)?.size();
    if size > limit {
        return Ok(PatchBlob {
            mode,
            blob_id: id,
            data: PatchData::TooLarge(size),
            from_worktree: false,
            dirty: false,
        });
    }
    load_patch_blob(repo, mode, id, budget)
}

fn worktree_patch_blob(
    repo: &gix::Repository,
    index: &gix::index::State,
    path: &BString,
    budget: &Arc<Budget>,
) -> Result<Option<PatchBlob>> {
    let Some((mode, mut data)) = super::index::authoring::worktree_entry_spooled(repo, index, path, budget)? else {
        return Ok(None);
    };
    if data.len() > repo.big_file_threshold()? {
        // Hash the spooled file in place; like Git, its content is never compared.
        let size = data.len();
        data.rewind().or_error()?;
        let blob_id = gix::objs::compute_stream_hash(
            repo.object_hash(),
            gix::objs::Kind::Blob,
            &mut data,
            size,
            &mut gix::progress::Discard,
            &gix::interrupt::IS_INTERRUPTED,
        )?;
        return Ok(Some(PatchBlob {
            mode: mode.bits(),
            blob_id,
            data: PatchData::TooLarge(size),
            from_worktree: true,
            dirty: false,
        }));
    }
    let data = PatchData::from_spool(repo, data)?;
    let blob_id = gix::objs::compute_stream_hash(
        repo.object_hash(),
        gix::objs::Kind::Blob,
        &mut data.as_slice(),
        data.as_slice().len() as u64,
        &mut gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
    )?;
    Ok(Some(PatchBlob {
        mode: mode.bits(),
        blob_id,
        data,
        from_worktree: true,
        dirty: false,
    }))
}

/// The working side of the gitlink at `path`, recorded in the index as `index_id`, read like Git's
/// default (`diff.submodule=short`): the commit checked out in the submodule, marked `-dirty` when
/// its tracked files or index differ from that commit (untracked files never do). A missing path is
/// a deletion, a file or symlink there is read as one, and a directory without a repository of its
/// own, or whose `HEAD` is unborn, is an unpopulated submodule and so unchanged.
fn worktree_gitlink(
    repo: &gix::Repository,
    index: &gix::index::State,
    path: &BString,
    index_id: ObjectId,
    budget: &Arc<Budget>,
) -> Result<Option<PatchBlob>> {
    let root = repo
        .workdir()
        .ok_or_raise(|| message("A worktree is required").validation())?;
    let absolute = root.join(gix::path::from_bstr(path.as_bstr())?);
    let meta = match std::fs::symlink_metadata(&absolute) {
        Ok(meta) => meta,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).or_error(),
    };
    if !meta.is_dir() {
        return worktree_patch_blob(repo, index, path, budget);
    }
    let side = |id: ObjectId, dirty: bool| PatchBlob {
        mode: GITLINK,
        blob_id: id,
        data: PatchData::Buffered(
            format!("Subproject commit {id}{}\n", if dirty { "-dirty" } else { "" }).into_bytes(),
        ),
        from_worktree: true,
        dirty,
    };
    if std::fs::symlink_metadata(absolute.join(gix::discover::DOT_GIT_DIR)).is_err() {
        return Ok(Some(side(index_id, false)));
    }
    let submodule = gix::open(&absolute).or_raise(|| message!("Could not open the submodule at '{path}'"))?;
    let Ok(head) = submodule.head_id() else {
        return Ok(Some(side(index_id, false)));
    };
    let dirty = submodule
        .status(gix::progress::Discard)?
        .untracked_files(gix::status::UntrackedFiles::None)
        .should_interrupt_shared(&gix::interrupt::IS_INTERRUPTED)
        .into_iter(None)?
        .next()
        .transpose()?
        .is_some();
    Ok(Some(side(head.detach(), dirty)))
}

/// Refuse the submodule settings that make Git print gitlinks other than in its default short
/// form (`diff.submodule=log|diff`, which changes patches only) or hide some of their changes
/// (`diff.ignoreSubmodules` or `submodule.<name>.ignore` set to `dirty` or `all`).
fn check_submodule_settings(repo: &gix::Repository, format: Format) -> Result<()> {
    let config = repo.config_snapshot();
    if format == Format::Patch
        && let Some(value) = config.string("diff.submodule")
        && value.as_slice() != b"short"
    {
        bail!(unsupported(format!(
            "diff.submodule={value} is not supported; Brit prints submodules in Git's short form; use git diff"
        )));
    }
    if let Some(value) = config.string("diff.ignoreSubmodules")
        && !matches!(value.as_slice(), b"none" | b"untracked")
    {
        bail!(unsupported(format!(
            "diff.ignoreSubmodules={value} is not supported; use git diff"
        )));
    }
    for submodule in repo.submodules()?.into_iter().flatten() {
        use gix::submodule::config::Ignore;
        let value = match submodule.ignore()? {
            Some(Ignore::Dirty) => "dirty",
            Some(Ignore::All) => "all",
            Some(Ignore::None | Ignore::Untracked) | None => continue,
        };
        bail!(unsupported(format!(
            "submodule.{}.ignore={value} is not supported; use git diff",
            submodule.name()
        )));
    }
    Ok(())
}

fn binary_attribute(matches: &gix::attrs::search::Outcome, path: &BString) -> Result<bool> {
    match matches
        .iter()
        .find(|m| m.assignment.name.as_str() == "diff")
        .map(|m| m.assignment.state)
    {
        None | Some(gix::attrs::StateRef::Unspecified) => Ok(false),
        Some(gix::attrs::StateRef::Unset) => Ok(true),
        _ => bail!(unsupported(format!(
            "diff attributes and text conversion are not supported for '{path}'; use git diff"
        ))),
    }
}

fn render_patch(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    path: &BString,
    old: Option<&PatchBlob>,
    new: Option<&PatchBlob>,
    binary: bool,
) -> Result<()> {
    let dirty = new.is_some_and(|new| new.dirty);
    if !dirty
        && old
            .zip(new)
            .is_some_and(|(old, new)| old.mode == new.mode && old.blob_id == new.blob_id)
    {
        return Ok(());
    }
    if old
        .zip(new)
        .is_some_and(|(old, new)| old.mode & 0o170000 != new.mode & 0o170000)
    {
        render_patch(repo, out, path, old, None, binary)?;
        return render_patch(repo, out, path, None, new, binary);
    }
    let quote_high = repo.config_snapshot().try_boolean("core.quotePath")?.unwrap_or(true);
    let mut a_path = BString::from("a/");
    a_path.extend_from_slice(path);
    let a_path = quote_patch_path(&a_path, quote_high);
    let mut b_path = BString::from("b/");
    b_path.extend_from_slice(path);
    let b_path = quote_patch_path(&b_path, quote_high);
    out.write_all(b"diff --git ").or_error()?;
    out.write_all(&a_path).or_error()?;
    out.write_all(b" ").or_error()?;
    out.write_all(&b_path).or_error()?;
    out.write_all(b"\n").or_error()?;
    match (old, new) {
        (None, Some(new)) => writeln!(out, "new file mode {:06o}", new.mode).or_error()?,
        (Some(old), None) => writeln!(out, "deleted file mode {:06o}", old.mode).or_error()?,
        (Some(old), Some(new)) if old.mode != new.mode => {
            writeln!(out, "old mode {:06o}\nnew mode {:06o}", old.mode, new.mode).or_error()?;
        }
        _ => {}
    }
    let null = ObjectId::null(repo.object_hash());
    let old_id = old.map_or(null, |b| b.blob_id);
    let new_id = new.map_or(null, |b| b.blob_id);
    // Like Git, a dirty submodule at its recorded commit has a patch but no `index` line.
    if old_id == new_id && !dirty {
        return Ok(());
    }
    if old_id != new_id {
        write_index_line(repo, out, old, new, old_id, new_id)?;
    }
    let a_label = if old.is_some() {
        a_path.as_bstr()
    } else {
        "/dev/null".as_bytes().as_bstr()
    };
    let b_label = if new.is_some() {
        b_path.as_bstr()
    } else {
        "/dev/null".as_bytes().as_bstr()
    };
    render_patch_body(out, path, old, new, binary, a_label, b_label)
}

/// Git's `index <old>..<new>[ <mode>]` line, abbreviated as long as Git's.
fn write_index_line(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    old: Option<&PatchBlob>,
    new: Option<&PatchBlob>,
    old_id: ObjectId,
    new_id: ObjectId,
) -> Result<()> {
    let configured = repo
        .config_snapshot()
        .string("core.abbrev")
        .map(|value| gix::config::tree::Core::ABBREV.try_into_abbreviation(value.as_bstr(), repo.object_hash()))
        .transpose()?
        .flatten();
    let hex_len = match configured {
        Some(len) => len,
        None => ((64 - repo.objects.packed_object_count()?.leading_zeros()).div_ceil(2) as usize).max(7),
    };
    let old_len = old_id.attach(repo).shorten().map_or(hex_len, |prefix| prefix.hex_len());
    let new_len = new_id.attach(repo).shorten().map_or(hex_len, |prefix| prefix.hex_len());
    write!(
        out,
        "index {}..{}",
        old_id.to_hex_with_len(old_len),
        new_id.to_hex_with_len(new_len)
    )
    .or_error()?;
    if let Some((old, new)) = old.zip(new)
        && old.mode == new.mode
    {
        write!(out, " {:06o}", old.mode).or_error()?;
    }
    writeln!(out).or_error()?;
    Ok(())
}

/// The `Binary files` line, or the `---`/`+++` labels and hunks, of a patch.
fn render_patch_body(
    out: &mut dyn std::io::Write,
    path: &BString,
    old: Option<&PatchBlob>,
    new: Option<&PatchBlob>,
    binary: bool,
    a_label: &BStr,
    b_label: &BStr,
) -> Result<()> {
    let old_data = old.map_or(&[][..], |b| b.data.as_slice());
    let new_data = new.map_or(&[][..], |b| b.data.as_slice());
    let too_large = old.into_iter().chain(new).any(|b| b.data.is_too_large());
    if binary || too_large || is_binary(old_data, new_data) {
        out.write_all(b"Binary files ").or_error()?;
        out.write_all(a_label).or_error()?;
        out.write_all(b" and ").or_error()?;
        out.write_all(b_label).or_error()?;
        out.write_all(b" differ\n").or_error()?;
        return Ok(());
    }
    if old_data == new_data {
        return Ok(());
    }
    // Like Git, a label ends with a tab when the path has a space, quoted or not.
    let tab = if path.contains(&b' ') { "\t" } else { "" };
    let a_tab = if old.is_some() { tab } else { "" };
    let b_tab = if new.is_some() { tab } else { "" };
    out.write_all(b"--- ").or_error()?;
    out.write_all(a_label).or_error()?;
    out.write_all(a_tab.as_bytes()).or_error()?;
    out.write_all(b"\n+++ ").or_error()?;
    out.write_all(b_label).or_error()?;
    out.write_all(b_tab.as_bytes()).or_error()?;
    out.write_all(b"\n").or_error()?;
    let input = gix::diff::blob::InternedInput::new(
        gix::diff::blob::sources::byte_lines(old_data),
        gix::diff::blob::sources::byte_lines(new_data),
    );
    let diff = compare_lines(&input);
    gix::diff::blob::UnifiedDiff::new(
        &diff,
        &input,
        PatchHunks {
            out,
            old: old_data,
            scanned: (0, 0),
            function: None,
        },
        gix::diff::blob::unified_diff::ContextSize::symmetrical(3),
    )
    .consume()
    .or_error()?;
    Ok(())
}

/// Writes hunks like Git, each header followed by its function context: the last line before the
/// hunk in the old file that starts with a letter, `_` or `$` (Git's default `funcname`), cut to 80
/// bytes and without trailing whitespace.
struct PatchHunks<'a> {
    out: &'a mut dyn std::io::Write,
    old: &'a [u8],
    /// The number of old lines searched so far and the byte offset where the next one starts.
    scanned: (u32, usize),
    function: Option<&'a [u8]>,
}

impl<'a> PatchHunks<'a> {
    /// Search the old lines before line `end` (zero-based) not searched for an earlier hunk.
    fn function_before(&mut self, end: u32) -> Option<&'a [u8]> {
        let old = self.old;
        let (mut line, mut offset) = self.scanned;
        while line < end && offset < old.len() {
            let rest = &old[offset..];
            let len = rest.find_byte(b'\n').map_or(rest.len(), |pos| pos + 1);
            let record = &rest[..len];
            if record
                .first()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || *byte == b'_' || *byte == b'$')
            {
                let mut record = &record[..record.len().min(80)];
                while let Some((last, head)) = record.split_last() {
                    if !(last.is_ascii_whitespace() || *last == 0x0b) {
                        break;
                    }
                    record = head;
                }
                self.function = Some(record);
            }
            line += 1;
            offset += len;
        }
        self.scanned = (line, offset);
        self.function
    }
}

impl gix::diff::blob::unified_diff::ConsumeHunk for PatchHunks<'_> {
    type Out = ();
    fn consume_hunk(
        &mut self,
        header: gix::diff::blob::unified_diff::HunkHeader,
        lines: &[(gix::diff::blob::unified_diff::DiffLineKind, &[u8])],
    ) -> std::io::Result<()> {
        fn range(start: u32, len: u32) -> String {
            match len {
                0 => format!("{},0", start.saturating_sub(1)),
                1 => start.to_string(),
                _ => format!("{start},{len}"),
            }
        }
        write!(
            self.out,
            "@@ -{} +{} @@",
            range(header.before_hunk_start, header.before_hunk_len),
            range(header.after_hunk_start, header.after_hunk_len)
        )?;
        // An empty old side has no line before the hunk.
        if header.before_hunk_len > 0
            && let Some(function) = self.function_before(header.before_hunk_start.saturating_sub(1))
        {
            self.out.write_all(b" ")?;
            self.out.write_all(function)?;
        }
        self.out.write_all(b"\n")?;
        for &(kind, line) in lines {
            self.out.write_all(&[kind.to_prefix() as u8])?;
            self.out.write_all(line)?;
            if !line.ends_with(b"\n") {
                self.out.write_all(b"\n\\ No newline at end of file\n")?;
            }
        }
        Ok(())
    }
    fn finish(self) {}
}

fn quote_patch_path(path: &[u8], quote_high: bool) -> BString {
    if !path
        .iter()
        .any(|b| *b < 32 || *b == 127 || b"\"\\".contains(b) || (quote_high && *b >= 128))
    {
        return path.into();
    }
    let mut quoted = Vec::with_capacity(path.len() + 2);
    quoted.push(b'"');
    for &byte in path {
        match byte {
            b'"' | b'\\' => quoted.extend_from_slice(&[b'\\', byte]),
            7 => quoted.extend_from_slice(b"\\a"),
            8 => quoted.extend_from_slice(b"\\b"),
            b'\t' => quoted.extend_from_slice(b"\\t"),
            b'\n' => quoted.extend_from_slice(b"\\n"),
            11 => quoted.extend_from_slice(b"\\v"),
            12 => quoted.extend_from_slice(b"\\f"),
            b'\r' => quoted.extend_from_slice(b"\\r"),
            byte if byte < 32 || byte == 127 || (quote_high && byte >= 128) => {
                quoted.extend_from_slice(&[b'\\', b'0' + (byte >> 6), b'0' + ((byte >> 3) & 7), b'0' + (byte & 7)]);
            }
            byte => quoted.push(byte),
        }
    }
    quoted.push(b'"');
    quoted.into()
}

/// Render a pinned commit's parent-to-commit patch without consulting mutable refs,
/// worktree attributes, external drivers or the public index.
pub fn pinned_commit(repo: &gix::Repository, out: &mut dyn std::io::Write, commit: ObjectId) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet};
    let (parent, tree) = {
        let source = gix::object::bounded::Source::deferred(repo);
        let mut buffer = gix::objs::find::Buffer::default();
        let data = source.load(&commit, &mut buffer)?;
        if data.kind != gix::objs::Kind::Commit {
            bail!(message!("Expected commit {commit}"));
        }
        let commit = gix::objs::CommitRef::from_bytes(data.data, repo.object_hash()).or_error()?;
        let parent = commit
            .parents()
            .next()
            .ok_or_else(|| message("root patch is unsupported"))?;
        (parent, commit.tree())
    };
    let before = repo.index_from_tree(&repo.tree_id_of_commit(parent)?)?;
    let after = repo.index_from_tree(&tree)?;
    let entries = |index: &gix::index::State| -> BTreeMap<BString, (u32, ObjectId)> {
        index
            .entries()
            .iter()
            .map(|entry| (entry.path(index).to_owned(), (entry.mode.bits(), entry.id)))
            .collect()
    };
    let before = entries(&before);
    let after = entries(&after);
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    for path in paths {
        if before.get(&path) == after.get(&path) {
            continue;
        }
        if before
            .get(&path)
            .into_iter()
            .chain(after.get(&path))
            .any(|(mode, _)| *mode == 0o160000)
        {
            bail!(unsupported("submodule patches are unsupported"));
        }
        let old = before
            .get(&path)
            .map(|&(mode, id)| load_patch_blob(repo, mode, id, &budget))
            .transpose()?;
        let new = after
            .get(&path)
            .map(|&(mode, id)| load_patch_blob(repo, mode, id, &budget))
            .transpose()?;
        render_patch(repo, &mut rendered, &path, old.as_ref(), new.as_ref(), false)?;
    }
    write_diff_output(out, &mut rendered)
}

fn check_external_diff(repo: &gix::Repository) -> Result<()> {
    if repo.config_snapshot().string("diff.external").is_some() || std::env::var_os("GIT_EXTERNAL_DIFF").is_some() {
        bail!(unsupported("external diff commands are not supported; use git diff"));
    }
    Ok(())
}

fn check_interrupted() -> Result<()> {
    if gix::interrupt::is_triggered() {
        bail!(message("diff interrupted"));
    }
    Ok(())
}

fn is_binary(old: &[u8], new: &[u8]) -> bool {
    old.iter().take(8000).chain(new.iter().take(8000)).any(|b| *b == 0)
}

fn compare_lines(input: &gix::diff::blob::InternedInput<&[u8]>) -> gix::diff::blob::Diff {
    gix::diff::blob::diff_with_slider_heuristics(gix::diff::blob::Algorithm::Myers, input)
}

/// Compare exactly the requested trees, independently of the working index and files.
/// Both trees' attributes are checked conservatively; no custom diff driver is run.
pub fn tree_stat(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    old_treeish: BString,
    new_treeish: BString,
) -> Result<()> {
    revisions::trees(&repo, out, old_treeish.as_bstr(), new_treeish.as_bstr(), Format::Stat).map(drop)
}

/// Resolve what `diff tree` compares: `OLD NEW` unchanged, `--merge-base OLD NEW` as the merge base
/// of both commits and `NEW`, or a lone `A..B` (`A` and `B`) or `A...B` (merge base of both and `B`).
///
/// Merge bases are computed like Git's, with shallow boundary commits read as roots. With several
/// merge bases, `--merge-base` refuses and names them, while `A...B` uses the first and says so on
/// `err`, as Git does.
pub fn tree_endpoints(
    repo: &gix::Repository,
    err: &mut dyn std::io::Write,
    old_treeish: BString,
    new_treeish: Option<BString>,
    merge_base: bool,
) -> Result<(BString, BString)> {
    if let Some(new_treeish) = new_treeish.as_ref().filter(|_| !merge_base) {
        return Ok((old_treeish, new_treeish.clone()));
    }
    let (old, new) = revisions::endpoints(
        repo,
        err,
        old_treeish.as_bstr(),
        new_treeish.as_ref().map(AsRef::as_ref),
        merge_base,
    )?;
    Ok((old.to_string().into(), new.to_string().into()))
}

/// Print each path whose mode or object differs between two trees, in index order and
/// without rename detection, quoted like Git's `diff --name-only --no-renames`.
pub fn tree_name_only(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    old_treeish: BString,
    new_treeish: BString,
) -> Result<()> {
    revisions::trees(
        &repo,
        out,
        old_treeish.as_bstr(),
        new_treeish.as_bstr(),
        Format::NameOnly,
    )
    .map(drop)
}

struct FileStat {
    name: BString,
    added: usize,
    removed: usize,
    binary: bool,
}

fn collect_stat(
    repo: &gix::Repository,
    stats: &mut Vec<FileStat>,
    path: &BString,
    old: Option<&PatchBlob>,
    new: Option<&PatchBlob>,
    binary: bool,
) -> Result<()> {
    if !new.is_some_and(|b| b.dirty)
        && old
            .zip(new)
            .is_some_and(|(a, b)| a.mode == b.mode && a.blob_id == b.blob_id)
    {
        return Ok(());
    }
    let a = old.map_or(&[][..], |b| b.data.as_slice());
    let b = new.map_or(&[][..], |b| b.data.as_slice());
    let binary = binary || old.into_iter().chain(new).any(|b| b.data.is_too_large()) || is_binary(a, b);
    let type_changed = old
        .zip(new)
        .is_some_and(|(a, b)| a.mode & 0o170000 != b.mode & 0o170000);
    let (added, removed) = if binary {
        // Git omits byte sizes when both object identities are already known
        // and equal. A worktree file has no cached object identity in Git.
        if old
            .zip(new)
            .is_some_and(|(a, b)| !b.from_worktree && a.blob_id == b.blob_id)
        {
            (0, 0)
        } else {
            (
                new.map_or(0, |b| b.data.len()) as usize,
                old.map_or(0, |b| b.data.len()) as usize,
            )
        }
    } else if old
        .zip(new)
        .is_some_and(|(a, b)| a.mode == GITLINK && b.mode == GITLINK)
    {
        // Git counts a moved submodule as one line out and one in, and a dirty one as none.
        let moved = usize::from(old.zip(new).is_some_and(|(a, b)| a.blob_id != b.blob_id));
        (moved, moved)
    } else if type_changed {
        (
            gix::diff::blob::sources::byte_lines(b).count(),
            gix::diff::blob::sources::byte_lines(a).count(),
        )
    } else if a == b {
        (0, 0)
    } else {
        let input = gix::diff::blob::InternedInput::new(
            gix::diff::blob::sources::byte_lines(a),
            gix::diff::blob::sources::byte_lines(b),
        );
        let diff = compare_lines(&input);
        (diff.count_additions() as usize, diff.count_removals() as usize)
    };
    let quote_high = repo.config_snapshot().try_boolean("core.quotePath")?.unwrap_or(true);
    stats.push(FileStat {
        name: quote_patch_path(path, quote_high),
        added,
        removed,
        binary,
    });
    Ok(())
}

fn stat_name_width(name: &[u8]) -> usize {
    use unicode_width::UnicodeWidthChar;
    // Git falls back to byte width for an invalid UTF-8 name.
    std::str::from_utf8(name).map_or(name.len(), |s| s.chars().map(|c| c.width().unwrap_or(0)).sum())
}

fn stat_name_suffix(mut name: &[u8], available: usize) -> &[u8] {
    use unicode_width::UnicodeWidthChar;
    let mut width = stat_name_width(name);
    while width > available && !name.is_empty() {
        let (character, size) = gix::bstr::decode_utf8(name);
        let (columns, bytes) = character.map_or((1, 1), |c| (c.width().unwrap_or(0), size));
        width = width.saturating_sub(columns);
        name = &name[bytes..];
    }
    name
}

fn render_stats(stats: &[FileStat], out: &mut dyn std::io::Write) -> Result<()> {
    if stats.is_empty() {
        return Ok(());
    }
    let digits = |n: usize| n.to_string().len();
    let max_change = stats
        .iter()
        .filter(|s| !s.binary)
        .map(|s| s.added + s.removed)
        .max()
        .unwrap_or(0);
    let binary_width = stats
        .iter()
        .filter(|s| s.binary)
        .map(|s| 14 + digits(s.added) + digits(s.removed))
        .max()
        .unwrap_or(0);
    let number_width = digits(max_change).max(if binary_width > 0 { 3 } else { 0 });
    let mut name_width = stats.iter().map(|s| stat_name_width(&s.name)).max().unwrap_or(0);
    let mut graph_width = max_change.max(binary_width.saturating_sub(4));
    // Fixed Git default: leave one blank column at the right, assigning 5/8 of
    // the available width to names when both the name and graph need clipping.
    if name_width + number_width + 6 + graph_width > 80 {
        graph_width = graph_width.min((30 - number_width - 6).max(6));
        if name_width > 80 - number_width - 6 - graph_width {
            name_width = 80 - number_width - 6 - graph_width;
        } else {
            graph_width = 80 - number_width - 6 - name_width;
        }
    }
    let (mut additions, mut removals) = (0, 0);
    for stat in stats {
        check_interrupted()?;
        let mut name = stat.name.as_slice();
        let mut available = name_width;
        out.write_all(b" ").or_error()?;
        if stat_name_width(name) > available {
            out.write_all(b"...").or_error()?;
            available -= 3;
            // Git 2.56 clips complete characters by display width, advancing
            // one byte for malformed UTF-8 instead of splitting valid text.
            name = stat_name_suffix(name, available);
            if let Some(slash) = name.find_byte(b'/') {
                name = &name[slash..];
            }
        }
        out.write_all(name).or_error()?;
        for byte in std::iter::repeat_n(b' ', available.saturating_sub(stat_name_width(name))) {
            out.write_all(&[byte]).or_error()?;
        }
        if stat.binary {
            write!(out, " | {:>number_width$}", "Bin").or_error()?;
            if stat.added != 0 || stat.removed != 0 {
                write!(out, " {} -> {} bytes", stat.removed, stat.added).or_error()?;
            }
        } else {
            additions += stat.added;
            removals += stat.removed;
            let total = stat.added + stat.removed;
            write!(out, " | {total:>number_width$}").or_error()?;
            if total > 0 {
                out.write_all(b" ").or_error()?;
            }
            let (mut add, mut del) = (stat.added, stat.removed);
            if graph_width <= max_change && max_change > 0 {
                let scale = |n| {
                    if n == 0 {
                        0
                    } else {
                        1 + n * (graph_width - 1) / max_change
                    }
                };
                let total = scale(total).max(if add > 0 && del > 0 { 2 } else { 0 });
                if add < del {
                    add = scale(add);
                    del = total - add;
                } else {
                    del = scale(del);
                    add = total - del;
                }
            }
            for byte in std::iter::repeat_n(b'+', add) {
                out.write_all(&[byte]).or_error()?;
            }
            for byte in std::iter::repeat_n(b'-', del) {
                out.write_all(&[byte]).or_error()?;
            }
        }
        out.write_all(b"\n").or_error()?;
    }
    write!(
        out,
        " {} file{} changed",
        stats.len(),
        if stats.len() == 1 { "" } else { "s" }
    )
    .or_error()?;
    if additions > 0 || removals == 0 {
        write!(
            out,
            ", {additions} insertion{}(+)",
            if additions == 1 { "" } else { "s" }
        )
        .or_error()?;
    }
    if removals > 0 || additions == 0 {
        write!(out, ", {removals} deletion{}(-)", if removals == 1 { "" } else { "s" }).or_error()?;
    }
    writeln!(out).or_error()?;
    Ok(())
}

pub(crate) fn write_diff_output(
    mut out: &mut dyn std::io::Write,
    rendered: &mut std::io::BufWriter<Spool>,
) -> Result<()> {
    #[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
    super::switch::checkpoint("diff-before-output").map_err(|err| message!("{err:#}"))?;
    check_interrupted()?;
    rendered.flush().or_error()?;
    let rendered = rendered.get_mut();
    rendered.rewind().or_error()?;
    stream::copy(rendered, &mut out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn stat_suffix_preserves_character_boundaries_and_git_width_semantics() {
        for (name, available, expected) in [
            ("ééé".as_bytes(), 2, "éé".as_bytes()),
            ("界界界".as_bytes(), 3, "界".as_bytes()),
            ("e\u{301}e\u{301}".as_bytes(), 1, "\u{301}e\u{301}".as_bytes()),
            (b"\xff\xfeabc".as_slice(), 3, b"abc".as_slice()),
            (b"\xe2\x98abc".as_slice(), 3, b"abc".as_slice()),
            (b"\x01abc".as_slice(), 2, b"bc".as_slice()),
            (b"abc".as_slice(), 0, b"".as_slice()),
        ] {
            assert_eq!(super::stat_name_suffix(name, available), expected, "name={name:?}");
        }
    }
}
