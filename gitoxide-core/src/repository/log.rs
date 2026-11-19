use gix::{
    Result,
    bstr::{BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message, unsupported},
    object::bounded::Source,
    objs::{Kind, find::Buffer},
    revision::{plumbing::Spec, walk::Sorting},
    traverse::commit::Parents,
};

/// Configure the starting commits, hidden ancestry, and order of a log traversal.
pub struct Options {
    /// Revision expressions to walk, including exclusions and ranges, or `HEAD` if empty.
    pub revisions: Vec<BString>,
    /// The order in which to return commits, defaulting to [`Sorting::DateOrder`].
    pub sorting: Sorting,
    /// Which parents to follow when walking visible commits.
    pub parents: Parents,
    /// Stop after this many commits; `Some(0)` prints nothing, and `None` is unlimited.
    pub max_count: Option<usize>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            revisions: Vec::new(),
            sorting: Sorting::DateOrder,
            parents: Parents::default(),
            max_count: None,
        }
    }
}

pub fn log(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    pathspecs: Vec<BString>,
    options: Options,
) -> Result<()> {
    // No object cache is configured here: commits are read through the bounded source below,
    // which must not retain their bodies.
    if !pathspecs.is_empty() {
        bail!(unsupported(
            "Pathspec-limited history is not implemented yet; use `git log` for path filtering or omit the pathspecs"
        ));
    }
    log_all(repo, out, options)
}

fn log_all(repo: gix::Repository, out: &mut dyn std::io::Write, options: Options) -> Result<()> {
    if options.max_count == Some(0) {
        return Ok(());
    }
    let source = Source::new(
        &repo,
        gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
    )?;
    let mut buffer = Buffer::default();
    let mut tips = Vec::new();
    if options.revisions.is_empty() {
        tips.push(head_commit(&repo, &source, &mut buffer)?);
    }
    let mut hidden = Vec::new();
    for revision in &options.revisions {
        add_revision(&repo, &source, &mut buffer, revision.as_bstr(), &mut tips, &mut hidden)
            .or_raise(|| message!("Could not resolve revision {revision}"))?;
    }
    buffer.clear();
    let _ = repo.config_snapshot().try_boolean("core.commitGraph")?;
    let mut walk = repo.rev_walk(tips).with_hidden(hidden).sorting(options.sorting);
    if matches!(options.parents, Parents::First) {
        walk = walk.first_parent_only();
    }

    for info in walk.all()?.take(options.max_count.unwrap_or(usize::MAX)) {
        let info = info?;

        write_info(&source, &mut buffer, &mut *out, &info)?;
    }

    Ok(())
}

/// Resolve `HEAD` to its commit, reading only tags and the header of the terminal object.
fn head_commit(repo: &gix::Repository, source: &Source<'_>, buffer: &mut Buffer) -> Result<gix::ObjectId> {
    // Follow symbolic references without loading their terminal commit into a Vec.
    let mut head = repo.find_reference("HEAD")?.follow_to_object()?.detach();
    let mut seen = std::collections::HashSet::new();
    loop {
        match repo.find_header(head)?.kind() {
            Kind::Commit => break,
            Kind::Tag => {}
            _ => bail!(message!("HEAD object {head} does not point to a commit").validation()),
        }
        if !seen.insert(head) {
            bail!(gix::error::corruption("Tag cycle while resolving log HEAD"));
        }
        let object = source.load(&head, buffer)?;
        let tag = object
            .try_into_tag_iter()
            .ok_or_raise(|| message("HEAD does not point to a commit").validation())?;
        head = tag.target_id()?;
    }
    buffer.clear();
    Ok(head)
}

fn add_revision(
    repo: &gix::Repository,
    source: &Source<'_>,
    buffer: &mut Buffer,
    revision: &gix::bstr::BStr,
    tips: &mut Vec<gix::ObjectId>,
    hidden: &mut Vec<gix::ObjectId>,
) -> Result<()> {
    match *repo.rev_parse(revision)? {
        Spec::Include(commit_id) => tips.push(commit(source, commit_id)?),
        Spec::Exclude(commit_id) => hidden.push(commit(source, commit_id)?),
        Spec::Range { from, to } => {
            hidden.push(commit(source, from)?);
            tips.push(commit(source, to)?);
        }
        Spec::Merge { theirs, ours } => {
            let theirs = commit(source, theirs)?;
            let ours = commit(source, ours)?;
            hidden.extend(repo.merge_bases_many(theirs, &[ours])?.into_iter().map(gix::Id::detach));
            tips.extend([theirs, ours]);
        }
        Spec::IncludeOnlyParents(commit_id) => {
            tips.extend(parent_ids(source, buffer, commit(source, commit_id)?)?);
        }
        Spec::ExcludeParents(commit_id) => {
            let commit_id = commit(source, commit_id)?;
            tips.push(commit_id);
            hidden.extend(parent_ids(source, buffer, commit_id)?);
        }
    }
    Ok(())
}

/// Peel `object_id` to a commit, reading only the header of the commit itself.
fn commit(source: &Source<'_>, object_id: gix::ObjectId) -> Result<gix::ObjectId> {
    source
        .peel(object_id, Kind::Commit)
        .or_raise(|| message!("Object {object_id} must refer to a commit"))
}

fn parent_ids(source: &Source<'_>, buffer: &mut Buffer, commit_id: gix::ObjectId) -> Result<Vec<gix::ObjectId>> {
    let mut parents = Vec::new();
    for token in gix::objs::find::Source::find_commit(source, &commit_id, buffer)? {
        match token.or_raise(|| message!("Could not decode the parents of commit {commit_id}").corrupted())? {
            gix::objs::commit::ref_iter::Token::Tree { .. } => {}
            gix::objs::commit::ref_iter::Token::Parent { id } => parents.push(id),
            _ => break,
        }
    }
    buffer.clear();
    Ok(parents)
}

fn write_info(
    source: &Source<'_>,
    buffer: &mut Buffer,
    mut out: impl std::io::Write,
    info: &gix::revision::walk::Info<'_>,
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
