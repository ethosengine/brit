use gix::{
    Result,
    bstr::{BString, ByteSlice},
    error::{ResultExt, bail, unsupported},
};

pub fn log(repo: gix::Repository, out: &mut dyn std::io::Write, path: Option<BString>) -> Result<()> {
    log_limited(repo, out, path, None)
}

pub fn log_limited(
    mut repo: gix::Repository,
    out: &mut dyn std::io::Write,
    path: Option<BString>,
    max_count: Option<usize>,
) -> Result<()> {
    repo.object_cache_size_if_unset(repo.compute_object_cache_size_for_tree_diffs(&**repo.index_or_empty()?));

    if let Some(path) = path {
        log_file(repo, out, path)
    } else {
        log_all(repo, out, max_count)
    }
}

fn log_all(repo: gix::Repository, out: &mut dyn std::io::Write, max_count: Option<usize>) -> Result<()> {
    if max_count == Some(0) {
        return Ok(());
    }
    let head = repo.head()?.peel_to_commit()?;
    let topo = gix::traverse::commit::topo::Builder::from_iters(&repo.objects, [head.id], None::<Vec<gix::ObjectId>>)
        .build()?;

    for info in topo.take(max_count.unwrap_or(usize::MAX)) {
        let info = info?;

        write_info(&repo, &mut *out, &info).or_error()?;
    }

    Ok(())
}

fn log_file(_repo: gix::Repository, _out: &mut dyn std::io::Write, _path: BString) -> Result<()> {
    bail!(unsupported(
        "File-based lookup isn't yet implemented in a way that is competitively fast"
    ));
}

fn write_info(
    repo: &gix::Repository,
    mut out: impl std::io::Write,
    info: &gix::traverse::commit::Info,
) -> std::io::Result<()> {
    let commit = repo
        .find_commit(info.id)
        .map_err(|error| std::io::Error::other(error.to_string()))?;

    let message = commit.message_raw_sloppy();
    let title = message.lines().next();

    writeln!(
        out,
        "{} {}",
        info.id.to_hex_with_len(8),
        title.map_or_else(|| "<no message>".into(), BString::from)
    )?;

    Ok(())
}
