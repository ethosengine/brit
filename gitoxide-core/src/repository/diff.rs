use gix::{
    ObjectId, Result,
    bstr::{BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message, unsupported},
    objs::tree::EntryMode,
    odb::store::RefreshMode,
    prelude::ObjectIdExt,
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

    let worktree_roots = gix::diff::blob::pipeline::WorktreeRoots { old_root, new_root };

    let mut resource_cache = repo.diff_resource_cache(
        gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
        worktree_roots,
    )?;

    resource_cache.set_resource(
        old_blob_id,
        gix::object::tree::EntryKind::Blob,
        old_path.as_ref(),
        gix::diff::blob::ResourceKind::OldOrSource,
        &repo.objects,
    )?;
    resource_cache.set_resource(
        new_blob_id,
        gix::object::tree::EntryKind::Blob,
        new_path.as_ref(),
        gix::diff::blob::ResourceKind::NewOrDestination,
        &repo.objects,
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
    let rendered = gix::diff::blob::UnifiedDiff::new(
        &diff,
        &interner,
        gix::diff::blob::unified_diff::ConsumeBinaryHunk::new(BString::default(), "\n"),
        gix::diff::blob::unified_diff::ContextSize::symmetrical(3),
    )
    .consume()
    .or_error()?;
    write!(out, "{rendered}").or_error()?;

    Ok(())
}

pub(crate) fn tokens_for_diffing(data: &[u8]) -> gix::diff::blob::platform::resource::ByteLinesWithoutTerminator<'_> {
    gix::diff::blob::platform::resource::ByteLinesWithoutTerminator::new(data)
}

/// Show HEAD-to-index or index-to-worktree patches without refreshing or writing the index.
/// Literal file selection deliberately does not interpret Git pathspec magic.
pub fn working(repo: gix::Repository, out: &mut dyn std::io::Write, cached: bool, paths: &[BString]) -> Result<()> {
    use super::index::authoring::{literal_paths, validate_index};
    use std::collections::{BTreeMap, BTreeSet};

    if repo.config_snapshot().string("diff.external").is_some() || std::env::var_os("GIT_EXTERNAL_DIFF").is_some() {
        bail!(unsupported("external diff commands are not supported; use git diff"));
    }
    let index = repo.index_or_empty()?;
    validate_index(&index)?;
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
    let mut rendered = Vec::new();
    for path in paths {
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
        attrs.at_entry(&path, None)?.matching_attributes(&mut matches);
        if matches.iter().any(|m| m.assignment.name.as_str() == "diff") {
            bail!(unsupported(
                "diff attributes and text conversion are not supported; use git diff"
            ));
        }
        let old = before
            .get(&path)
            .map(|&(mode, blob_id)| load_patch_blob(&repo, mode, blob_id))
            .transpose()?;
        let new = if cached {
            after
                .get(&path)
                .map(|&(mode, blob_id)| load_patch_blob(&repo, mode, blob_id))
                .transpose()?
        } else {
            worktree_patch_blob(&repo, &index, &path)?
        };
        render_patch(&repo, &mut rendered, &path, old.as_ref(), new.as_ref())?;
    }
    out.write_all(&rendered).or_error()?;
    Ok(())
}

struct PatchBlob {
    mode: u32,
    blob_id: ObjectId,
    data: Vec<u8>,
}

fn load_patch_blob(repo: &gix::Repository, mode: u32, blob_id: ObjectId) -> Result<PatchBlob> {
    let blob = repo.find_blob(blob_id)?;
    Ok(PatchBlob {
        mode,
        blob_id,
        data: blob.data.clone(),
    })
}

fn worktree_patch_blob(repo: &gix::Repository, index: &gix::index::State, path: &BString) -> Result<Option<PatchBlob>> {
    let Some((mode, data)) = super::index::authoring::worktree_entry(repo, index, path)? else {
        return Ok(None);
    };
    let blob_id = gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, &data)?;
    Ok(Some(PatchBlob {
        mode: mode.bits(),
        blob_id,
        data,
    }))
}

fn render_patch(
    repo: &gix::Repository,
    out: &mut Vec<u8>,
    path: &BString,
    old: Option<&PatchBlob>,
    new: Option<&PatchBlob>,
) -> Result<()> {
    use std::io::Write;
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
    out.extend_from_slice(b"diff --git ");
    out.extend_from_slice(&a_path);
    out.push(b' ');
    out.extend_from_slice(&b_path);
    out.push(b'\n');
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
    if old_data
        .iter()
        .take(8000)
        .chain(new_data.iter().take(8000))
        .any(|b| *b == 0)
    {
        out.extend_from_slice(b"Binary files ");
        out.extend_from_slice(a_label);
        out.extend_from_slice(b" and ");
        out.extend_from_slice(b_label);
        out.extend_from_slice(b" differ\n");
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
    out.extend_from_slice(b"--- ");
    out.extend_from_slice(a_label);
    out.extend_from_slice(a_tab.as_bytes());
    out.extend_from_slice(b"\n+++ ");
    out.extend_from_slice(b_label);
    out.extend_from_slice(b_tab.as_bytes());
    out.push(b'\n');
    let input = gix::diff::blob::InternedInput::new(
        gix::diff::blob::sources::byte_lines(old_data),
        gix::diff::blob::sources::byte_lines(new_data),
    );
    let diff = gix::diff::blob::diff_with_slider_heuristics(gix::diff::blob::Algorithm::Myers, &input);
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

struct PatchHunks<'a>(&'a mut Vec<u8>);

impl gix::diff::blob::unified_diff::ConsumeHunk for PatchHunks<'_> {
    type Out = ();
    fn consume_hunk(
        &mut self,
        header: gix::diff::blob::unified_diff::HunkHeader,
        lines: &[(gix::diff::blob::unified_diff::DiffLineKind, &[u8])],
    ) -> std::io::Result<()> {
        use std::io::Write;
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
            self.0.push(kind.to_prefix() as u8);
            self.0.extend_from_slice(line);
            if !line.ends_with(b"\n") {
                self.0.extend_from_slice(b"\n\\ No newline at end of file\n");
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
    let mut rendered = Vec::new();
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
            .map(|&(mode, id)| load_patch_blob(repo, mode, id))
            .transpose()?;
        let new = after
            .get(&path)
            .map(|&(mode, id)| load_patch_blob(repo, mode, id))
            .transpose()?;
        render_patch(repo, &mut rendered, &path, old.as_ref(), new.as_ref())?;
    }
    out.write_all(&rendered).or_error()?;
    Ok(())
}
