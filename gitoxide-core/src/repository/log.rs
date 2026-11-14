use gix::{
    Result,
    bstr::{BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message, unsupported},
    objs::{FindExt, FindStream, Kind},
    traverse::commit::topo::source::{Buffer, Source},
};

#[derive(Clone)]
struct CommitSource<'a> {
    repo: &'a gix::Repository,
    budget: std::sync::Arc<gix::utils::stream::Budget>,
    buffer_limit: u64,
}

impl CommitSource<'_> {
    fn load<'a>(&self, id: &gix::hash::oid, buffer: &'a mut Buffer) -> Result<gix::objs::Data<'a>> {
        gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED).or_error()?;
        if self.repo.find_header(id)?.size() <= self.buffer_limit {
            return self
                .repo
                .objects
                .find(id, buffer.owned())
                .or_raise(|| message!("Could not read history object {id}"));
        }
        buffer.clear();
        let (kind, spool) = self
            .repo
            .objects
            .find_stream(id, &self.budget, &gix::interrupt::IS_INTERRUPTED)
            .or_raise(|| message!("Could not stream history object {id}"))?;
        Ok(gix::objs::Data::new(
            buffer.set_external(
                spool
                    .into_mapped()
                    .or_raise(|| message!("Could not map history object {id}"))?,
            ),
            kind,
            id.kind(),
        ))
    }
}

impl Source for CommitSource<'_> {
    fn find_commit<'a>(&self, id: &gix::hash::oid, buffer: &'a mut Buffer) -> Result<gix::objs::CommitRefIter<'a>> {
        self.load(id, buffer)?
            .try_into_commit_iter()
            .ok_or_raise(|| message!("Expected commit {id}").validation())
    }
}

pub fn log(repo: gix::Repository, out: &mut dyn std::io::Write, path: Option<BString>) -> Result<()> {
    log_limited(repo, out, path, None)
}

pub fn log_limited(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    path: Option<BString>,
    max_count: Option<usize>,
) -> Result<()> {
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
    let source = CommitSource {
        repo: &repo,
        budget: gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
        buffer_limit: repo
            .allocation_limit()
            .map_or(repo.big_file_threshold()?, |limit| limit as u64),
    };
    // Follow symbolic references without loading their terminal commit into a Vec.
    let mut head = repo.find_reference("HEAD")?.follow_to_object()?.detach();
    let mut seen = std::collections::HashSet::new();
    let mut buffer = Buffer::default();
    loop {
        match repo.find_header(head)?.kind() {
            Kind::Commit => break,
            Kind::Tag => {}
            _ => bail!(message!("HEAD object {head} does not point to a commit").validation()),
        }
        if !seen.insert(head) {
            bail!(gix::error::corruption("Tag cycle while resolving log HEAD"));
        }
        let object = source.load(&head, &mut buffer)?;
        let tag = object
            .try_into_tag_iter()
            .ok_or_raise(|| message("HEAD does not point to a commit").validation())?;
        head = tag.target_id()?;
    }
    buffer.clear();
    let topo =
        gix::traverse::commit::topo::Builder::from_iters(source.clone(), [head], None::<Vec<gix::ObjectId>>).build()?;

    for info in topo.take(max_count.unwrap_or(usize::MAX)) {
        let info = info?;

        write_info(&source, &mut buffer, &mut *out, &info)?;
    }

    Ok(())
}

fn log_file(_repo: gix::Repository, _out: &mut dyn std::io::Write, _path: BString) -> Result<()> {
    bail!(unsupported(
        "File-based lookup isn't yet implemented in a way that is competitively fast"
    ));
}

fn write_info(
    source: &CommitSource<'_>,
    buffer: &mut Buffer,
    mut out: impl std::io::Write,
    info: &gix::traverse::commit::Info,
) -> Result<()> {
    let commit = source.load(&info.id, buffer)?;
    if commit.kind != Kind::Commit {
        bail!(message!("Expected commit {}", info.id).validation());
    }
    // Keep message_raw_sloppy's existing byte selection without copying the title.
    let message = commit
        .data
        .find(b"\n\n")
        .map(|pos| &commit.data[pos + 2..])
        .unwrap_or_default();
    let title = message.lines().next();
    write!(out, "{} ", info.id.to_hex_with_len(8)).or_error()?;
    let mut title = title.unwrap_or(b"<no message>");
    gix::utils::stream::copy(&mut title, &mut out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
    writeln!(out).or_error()?;
    buffer.clear();

    Ok(())
}
