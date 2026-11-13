use gix::{
    ObjectId, Result,
    bstr::{BString, ByteSlice},
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

    let old_tree = old_tree_id.object()?.peel_to_tree()?;
    let new_tree = new_tree_id.object()?.peel_to_tree()?;

    let changes = repo.diff_tree_to_tree(&old_tree, &new_tree, None)?;

    writeln!(
        out,
        "Diffing trees `{old_treeish}` ({old_tree_id}) -> `{new_treeish}` ({new_tree_id})\n"
    )
    .or_error()?;
    write_changes(&repo, out, changes).or_error()?;

    Ok(())
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

    let algorithm = match outcome.operation {
        Operation::InternalDiff { algorithm } => algorithm,
        Operation::ExternalCommand { .. } => {
            unreachable!("We disabled that")
        }
        Operation::SourceOrDestinationIsBinary => {
            bail!(unsupported("Source or destination is binary and we can't diff that"))
        }
    };

    let interner = gix::diff::blob::InternedInput::new(
        tokens_for_diffing(outcome.old.data.as_slice().unwrap_or_default()),
        tokens_for_diffing(outcome.new.data.as_slice().unwrap_or_default()),
    );

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

pub(crate) fn tokens_for_diffing(data: &[u8]) -> gix::diff::blob::platform::resource::ByteLinesWithoutTerminator<'_> {
    gix::diff::blob::platform::resource::ByteLinesWithoutTerminator::new(data)
}

/// Show HEAD-to-index or index-to-worktree patches without refreshing or writing the index.
/// Literal file selection deliberately does not interpret Git pathspec magic.
pub fn working(repo: gix::Repository, out: &mut dyn std::io::Write, cached: bool, paths: &[BString]) -> Result<()> {
    working_output(repo, out, cached, paths, false)
}

/// Show HEAD-to-index or index-to-worktree statistics without writing repository state.
pub fn working_stat(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    cached: bool,
    paths: &[BString],
) -> Result<()> {
    working_output(repo, out, cached, paths, true)
}

fn working_output(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    cached: bool,
    paths: &[BString],
    stat: bool,
) -> Result<()> {
    use super::index::authoring::{literal_paths, read_index, validate_index};
    use std::collections::{BTreeMap, BTreeSet};

    check_external_diff(&repo)?;
    let index = read_index(&repo)?;
    let selected = if paths.is_empty() {
        Vec::new()
    } else {
        literal_paths(&repo, paths)?
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
    if cached {
        if !repo.head()?.is_unborn() {
            let head = repo.index_from_tree(&repo.head_tree_id()?)?;
            validate_index(&head)?;
            insert(&head, &mut before)?;
        }
    } else {
        before.clone_from(&after);
    }
    let all: BTreeSet<_> = before.keys().chain(after.keys()).cloned().collect();
    for path in &selected {
        if !all.contains(path) {
            bail!(message!("path '{path}' is not tracked in this comparison"));
        }
    }
    let paths: Vec<_> = all
        .into_iter()
        .filter(|path| selected.is_empty() || selected.contains(path))
        .collect();
    let mut attrs = repo.attributes_only(
        &index,
        gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
    )?;
    let mut matches = attrs.attribute_matches();
    // Buffer output so a refused path never leaves a misleading partial patch.
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    let mut stats = Vec::new();
    for path in paths {
        check_interrupted()?;
        // HEAD and index identities prove no staged change, even for paths whose
        // patches we cannot render. Worktree comparisons still need to read disk.
        if cached && before.get(&path) == after.get(&path) {
            continue;
        }
        if before
            .get(&path)
            .into_iter()
            .chain(after.get(&path))
            .any(|(mode, _)| *mode == 0o160000)
        {
            bail!(unsupported(
                "submodule patches are not supported; select ordinary files or use git diff"
            ));
        }
        let old = before
            .get(&path)
            .map(|&(mode, blob_id)| load_patch_blob(&repo, mode, blob_id, &budget))
            .transpose()?;
        let new = if cached {
            after
                .get(&path)
                .map(|&(mode, blob_id)| load_patch_blob(&repo, mode, blob_id, &budget))
                .transpose()?
        } else {
            worktree_patch_blob(&repo, &index, &path, &budget)?
        };
        // Only canonical content and effective mode prove a worktree entry unchanged.
        // Read through the authoring path first, retaining validation and filter refusals.
        if !cached
            && old
                .as_ref()
                .zip(new.as_ref())
                .is_some_and(|(old, new)| old.mode == new.mode && old.data.as_slice() == new.data.as_slice())
        {
            continue;
        }
        attrs.at_entry(&path, None)?.matching_attributes(&mut matches);
        if matches.iter().any(|m| m.assignment.name.as_str() == "diff") {
            bail!(unsupported(
                "diff attributes and text conversion are not supported; use git diff"
            ));
        }
        if stat {
            collect_stat(&repo, &mut stats, &path, old.as_ref(), new.as_ref())?;
        } else {
            render_patch(&repo, &mut rendered, &path, old.as_ref(), new.as_ref())?;
        }
    }
    if stat {
        render_stats(&stats, &mut rendered)?;
    }
    write_diff_output(out, &mut rendered)
}

struct PatchBlob {
    mode: u32,
    blob_id: ObjectId,
    data: PatchData,
    from_worktree: bool,
}

enum PatchData {
    Buffered(Vec<u8>),
    Mapped(Mapped),
}
impl PatchData {
    fn as_slice(&self) -> &[u8] {
        match self {
            Self::Buffered(bytes) => bytes,
            Self::Mapped(bytes) => bytes.as_slice(),
        }
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
    })
}

fn worktree_patch_blob(
    repo: &gix::Repository,
    index: &gix::index::State,
    path: &BString,
    budget: &Arc<Budget>,
) -> Result<Option<PatchBlob>> {
    let Some((mode, data)) = super::index::authoring::worktree_entry_spooled(repo, index, path, budget)? else {
        return Ok(None);
    };
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
    }))
}

fn render_patch(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    path: &BString,
    old: Option<&PatchBlob>,
    new: Option<&PatchBlob>,
) -> Result<()> {
    if old
        .zip(new)
        .is_some_and(|(old, new)| old.mode == new.mode && old.blob_id == new.blob_id)
    {
        return Ok(());
    }
    if old
        .zip(new)
        .is_some_and(|(old, new)| old.mode & 0o170000 != new.mode & 0o170000)
    {
        render_patch(repo, out, path, old, None)?;
        return render_patch(repo, out, path, None, new);
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
    if old_id == new_id {
        return Ok(());
    }
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
    let old_data = old.map_or(&[][..], |b| b.data.as_slice());
    let new_data = new.map_or(&[][..], |b| b.data.as_slice());
    if is_binary(old_data, new_data) {
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
    let a_tab = if a_label.contains(&b' ') && !a_label.starts_with(b"\"") {
        "\t"
    } else {
        ""
    };
    let b_tab = if b_label.contains(&b' ') && !b_label.starts_with(b"\"") {
        "\t"
    } else {
        ""
    };
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
        PatchHunks(out),
        gix::diff::blob::unified_diff::ContextSize::symmetrical(3),
    )
    .consume()
    .or_error()?;
    Ok(())
}

struct PatchHunks<'a>(&'a mut dyn std::io::Write);

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
        writeln!(
            self.0,
            "@@ -{} +{} @@",
            range(header.before_hunk_start, header.before_hunk_len),
            range(header.after_hunk_start, header.after_hunk_len)
        )?;
        for &(kind, line) in lines {
            self.0.write_all(&[kind.to_prefix() as u8])?;
            self.0.write_all(line)?;
            if !line.ends_with(b"\n") {
                self.0.write_all(b"\n\\ No newline at end of file\n")?;
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
    let commit = repo.find_commit(commit)?;
    let parent = commit
        .parent_ids()
        .next()
        .ok_or_else(|| message("root patch is unsupported"))?;
    let before = repo.index_from_tree(&repo.find_commit(parent)?.tree_id()?)?;
    let after = repo.index_from_tree(&commit.tree_id()?)?;
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
        render_patch(repo, &mut rendered, &path, old.as_ref(), new.as_ref())?;
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
    use std::collections::{BTreeMap, BTreeSet};
    check_external_diff(&repo)?;
    let old_tree = repo.rev_parse_single(old_treeish.as_bstr())?.object()?.peel_to_tree()?;
    let new_tree = repo.rev_parse_single(new_treeish.as_bstr())?.object()?.peel_to_tree()?;
    let before_index = repo.index_from_tree(&old_tree.id)?;
    let after_index = repo.index_from_tree(&new_tree.id)?;
    let entries = |index: &gix::index::State| -> BTreeMap<BString, (u32, ObjectId)> {
        index
            .entries()
            .iter()
            .map(|e| (e.path(index).into(), (e.mode.bits(), e.id)))
            .collect()
    };
    let before = entries(&before_index);
    let after = entries(&after_index);
    let paths: BTreeSet<_> = before.keys().chain(after.keys()).collect();
    let source = gix::worktree::stack::state::attributes::Source::IdMapping;
    let mut old_attrs = repo.attributes_only(&before_index, source)?;
    let mut new_attrs = repo.attributes_only(&after_index, source)?;
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut stats = Vec::new();
    for path in paths {
        check_interrupted()?;
        if before.get(path) == after.get(path) {
            continue;
        }
        if before
            .get(path)
            .into_iter()
            .chain(after.get(path))
            .any(|(mode, _)| *mode == 0o160000)
        {
            bail!(unsupported("submodule patches are not supported; use git diff"));
        }
        for attrs in [&mut old_attrs, &mut new_attrs] {
            let mut matches = attrs.attribute_matches();
            attrs.at_entry(path, None)?.matching_attributes(&mut matches);
            if matches.iter().any(|m| m.assignment.name.as_str() == "diff") {
                bail!(unsupported(
                    "diff attributes and text conversion are not supported; use git diff"
                ));
            }
        }
        let old = before
            .get(path)
            .map(|&(mode, id)| load_patch_blob(&repo, mode, id, &budget))
            .transpose()?;
        let new = after
            .get(path)
            .map(|&(mode, id)| load_patch_blob(&repo, mode, id, &budget))
            .transpose()?;
        collect_stat(&repo, &mut stats, path, old.as_ref(), new.as_ref())?;
    }
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    render_stats(&stats, &mut rendered)?;
    write_diff_output(out, &mut rendered)
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
) -> Result<()> {
    if old
        .zip(new)
        .is_some_and(|(a, b)| a.mode == b.mode && a.blob_id == b.blob_id)
    {
        return Ok(());
    }
    let a = old.map_or(&[][..], |b| b.data.as_slice());
    let b = new.map_or(&[][..], |b| b.data.as_slice());
    let binary = is_binary(a, b);
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
            (b.len(), a.len())
        }
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

fn write_diff_output(mut out: &mut dyn std::io::Write, rendered: &mut std::io::BufWriter<Spool>) -> Result<()> {
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
