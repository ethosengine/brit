use std::ffi::OsStr;

use gix::{
    Result,
    blame::Start,
    bstr::BStr,
    config::tree,
    error::{ResultExt, message},
    utils::AsBStr,
};

pub fn blame_file(
    mut repo: gix::Repository,
    file: &OsStr,
    options: gix::blame::Options,
    out: impl std::io::Write,
    err: Option<&mut dyn std::io::Write>,
) -> Result<()> {
    {
        let mut config = repo.config_snapshot_mut();
        if config.string(tree::Core::DELTA_BASE_CACHE_LIMIT).is_none() {
            config.set_value(&tree::Core::DELTA_BASE_CACHE_LIMIT, "100m")?;
        }
    }
    let index = repo.index_or_empty()?;
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&index));

    let file = gix::path::os_str_into_bstr(file)?;
    let file = repo.normalize_path(file)?;

    let cache: Option<gix::commitgraph::Graph> = repo.commit_graph_if_enabled()?;
    let mut resource_cache = repo.diff_resource_cache(
        // TODO(blame): Git uses something akin to `ToGitUnlessBinaryToTextIsPresent` here, but with a specialty: textconv output is converted
        //              to Git, which isn't happening for normal diffing. In theory, this shouldn't be a problem as it's always apples to apples,
        //              at least in theory.
        gix::diff::blob::pipeline::Mode::ToGit,
        gix::diff::blob::pipeline::WorktreeRoots {
            old_root: repo.workdir().map(ToOwned::to_owned),
            new_root: None,
        },
    )?;
    let storage = gix::blame::Bounded {
        budget: gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
        buffer_limit: repo
            .allocation_limit()
            .unwrap_or(usize::try_from(repo.big_file_threshold()?).unwrap_or(usize::MAX)),
        interrupt: &gix::interrupt::IS_INTERRUPTED,
    };
    // The worktree root is only for constructing `start`; historical `OldOrSource` resources must be loaded by
    // object ID, and we make sure that worktree contents can't possibly be used.
    let mut history_resources = resource_cache.clone();
    history_resources.filter.roots = Default::default();
    let start = start_for_blame(&repo, file.as_bstr(), &mut resource_cache, &storage)?;
    let outcome = gix::blame::file_bounded(
        &repo.objects,
        start,
        cache,
        &mut history_resources,
        file.as_ref(),
        options,
        &storage,
    )?;
    drop(resource_cache);
    drop(history_resources);
    let statistics = outcome.statistics;
    show_blame_entries(out, outcome, file.as_ref()).or_error()?;

    if let Some(err) = err {
        writeln!(err, "{statistics:#?}").or_error()?;
    }
    Ok(())
}

/// Start at `HEAD`, overlaying diffable worktree contents so uncommitted changes are included.
/// Missing or binary worktree files fall back to blaming `HEAD` directly.
fn start_for_blame<'a>(
    repo: &'a gix::Repository,
    file: &'a gix::bstr::BStr,
    resources: &'a mut gix::diff::blob::Platform,
    storage: &gix::blame::Bounded<'_>,
) -> Result<gix::blame::Start<'a>> {
    let first_suspect: gix::ObjectId = repo.head()?.into_peeled_id()?.into();
    let Some(workdir) = repo.workdir() else {
        return Ok(Start::Commit(first_suspect));
    };
    let path = workdir.join(gix::path::from_bstr(file)?);
    let metadata = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) if gix::fs::io_err::is_not_found(err.kind(), err.raw_os_error()) => {
            return Ok(Start::Commit(first_suspect));
        }
        Err(err) => return Err(err).or_raise(|| message!("Could not read metadata of \"{}\"", path.display())),
    };
    // State the correct type here so that the resource cache and its possibly converted bytes match the actual type, i.e.
    // - read the file for blobs
    // - read the symlink bytes themselves, the target path for symlinks.
    let entry_kind = if metadata.file_type().is_symlink() {
        gix::objs::tree::EntryKind::Link
    } else {
        // executable bits don't matter.
        gix::objs::tree::EntryKind::Blob
    };
    resources.set_resource_bounded(
        repo.object_hash().null(),
        entry_kind,
        file,
        gix::diff::blob::ResourceKind::OldOrSource,
        &repo.objects,
        &storage.budget,
        storage.buffer_limit,
        storage.interrupt,
    )?;
    let contents = resources
        .resource(gix::diff::blob::ResourceKind::OldOrSource)
        .and_then(|resource| match resource.data {
            gix::diff::blob::platform::resource::Data::Buffer { buf, .. } => Some(buf),
            gix::diff::blob::platform::resource::Data::Binary { .. }
            | gix::diff::blob::platform::resource::Data::Missing => None,
        });

    Ok(contents
        .map(|contents| Start::Contents {
            first_suspect,
            contents: contents.into(),
        })
        .unwrap_or(Start::Commit(first_suspect)))
}

fn show_blame_entries(
    mut out: impl std::io::Write,
    outcome: gix::blame::Outcome<impl AsRef<[u8]>>,
    source_file_name: &BStr,
) -> std::io::Result<()> {
    let num_digits_for_line_number = {
        let largest_line_number = outcome
            .entries
            .last()
            .map_or(0, |entry| entry.range_in_blamed_file().end);
        (largest_line_number.checked_ilog10().unwrap_or(0) + 1) as usize
    };

    let mut lines = gix::diff::blob::sources::byte_lines(outcome.blob.as_ref());
    let mut next_line = 0;
    for entry in &outcome.entries {
        let range = entry.range_in_blamed_file();
        let lines_in_hunk = lines.by_ref().skip(range.start - next_line).take(range.len());
        next_line = range.end;
        for ((actual_lno, source_lno), line) in entry
            .range_in_blamed_file()
            .zip(entry.range_in_source_file())
            .zip(lines_in_hunk)
        {
            gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED)?;
            write!(
                out,
                "{short_id} {line_no:>num_digits_for_line_number$} ",
                short_id = entry.commit_id.to_hex_with_len(8),
                line_no = actual_lno + 1,
            )?;

            let source_file_name = entry.source_file_name.as_ref().map_or(source_file_name, BStr::new);
            write!(out, "{source_file_name} ")?;

            write!(
                out,
                "{src_line_no:>num_digits_for_line_number$} {line}",
                src_line_no = source_lno + 1,
                line = BStr::new(line)
            )?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn rendering_streams_disjoint_ranges_and_preserves_byte_display() -> std::io::Result<()> {
        let id = gix::ObjectId::null(gix::hash::Kind::default());
        let outcome = gix::blame::Outcome {
            entries: vec![
                gix::blame::BlameEntry::new(1..2, 9..10, id, None),
                gix::blame::BlameEntry::new(3..4, 19..20, id, Some("old".into())),
            ],
            blob: b"skip\nalpha\r\nskip\n\xfflast".to_vec(),
            statistics: Default::default(),
            blame_path: None,
        };
        let mut output = Vec::new();
        super::show_blame_entries(&mut output, outcome, gix::bstr::BStr::new("path"))?;
        assert_eq!(output, "00000000 2 path 10 alpha\r\n00000000 4 old 20 �last".as_bytes());
        Ok(())
    }
}
