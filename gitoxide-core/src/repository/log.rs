use std::path::{Component, Path};

use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message},
    hash::ObjectId,
    object::bounded::Source,
    objs::{Kind, find::Buffer, tree::EntryKind},
    revision::walk::Sorting,
    traverse::commit::simple::CommitTimeOrder,
};

/// What [`log_with()`] lists, and how each commit is printed.
#[derive(Debug, Default, Clone)]
pub struct Options {
    /// The revision to start from, `HEAD` if unset.
    pub rev: Option<BString>,
    /// Only list commits that change one of these literal paths, relative to the current directory.
    pub paths: Vec<BString>,
    /// Stop after this many commits.
    pub max_count: Option<usize>,
    /// Abbreviate hashes like `git log --oneline` instead of printing eight hex digits.
    pub oneline: bool,
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
    log_with(
        repo,
        out,
        Options {
            paths: path.into_iter().collect(),
            max_count,
            ..Default::default()
        },
    )
}

/// List commits reachable from `options.rev` (or `HEAD`), optionally only those changing `options.paths`.
///
/// Without a revision or paths, the historical topological listing is kept. Otherwise commits stream
/// newest first by commit time, like `git log`, and the walk ends at a shallow boundary.
///
/// With paths, Git's default history simplification applies: a commit changes a path when its
/// entry there (kind and object) differs from its parent's, a merge equal to a parent at the paths
/// is not listed and only that parent is followed, and root and shallow-boundary commits change
/// every path they contain. A trailing slash matches only directories (and submodules).
pub fn log_with(repo: gix::Repository, out: &mut dyn std::io::Write, options: Options) -> Result<()> {
    let format = if options.oneline {
        Format::Oneline
    } else {
        Format::Default
    };
    // The topological walk cannot cross a shallow boundary whose parents are absent; the
    // streaming walk stops there instead.
    if options.rev.is_none() && options.paths.is_empty() && !shallow_parents_missing(&repo)? {
        log_all(repo, out, options.max_count, format)
    } else {
        log_history(repo, out, options, format)
    }
}

#[derive(Clone, Copy)]
enum Format {
    /// Eight hex digits and the title.
    Default,
    /// Git's abbreviation (honouring `core.abbrev`) and the title.
    Oneline,
}

fn log_all(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    max_count: Option<usize>,
    format: Format,
) -> Result<()> {
    if max_count == Some(0) {
        return Ok(());
    }
    let source = Source::new(
        &repo,
        gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
    )?;
    let head = head_commit(&repo, &source)?;
    let mut buffer = Buffer::default();
    let topo =
        gix::traverse::commit::topo::Builder::from_iters(source.clone(), [head], None::<Vec<gix::ObjectId>>).build()?;

    for info in topo.take(max_count.unwrap_or(usize::MAX)) {
        let info = info?;

        write_info(&repo, &source, &mut buffer, &mut *out, info.id, format)?;
    }

    Ok(())
}

fn log_history(repo: gix::Repository, out: &mut dyn std::io::Write, options: Options, format: Format) -> Result<()> {
    let remaining = options.max_count.unwrap_or(usize::MAX);
    if remaining == 0 {
        return Ok(());
    }
    let source = Source::new(
        &repo,
        gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
    )?;
    let mut paths = options
        .paths
        .iter()
        .map(|path| Literal::new(&repo, path.as_bstr()))
        .collect::<Result<Vec<_>>>()?;
    let tip = match options.rev.as_ref() {
        None => head_commit(&repo, &source)?,
        Some(rev) => match repo.rev_parse_single(rev.as_bstr()) {
            Ok(id) => peel_to_commit(&repo, &source, id.detach(), rev.as_bstr())?,
            // Like Git, an argument that names no revision but an existing worktree path is a path.
            Err(error) => match paths.is_empty().then(|| worktree_path(&repo, rev.as_bstr())).flatten() {
                Some(path) => {
                    paths.push(path);
                    head_commit(&repo, &source)?
                }
                None => return Err(error),
            },
        },
    };

    if !paths.is_empty() {
        return simplified_walk(&repo, &source, out, tip, &paths, remaining, format);
    }
    let walk = repo
        .rev_walk([tip])
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .all()?;
    let mut buffer = Buffer::default();
    for info in walk.take(remaining) {
        write_info(&repo, &source, &mut buffer, &mut *out, info?.id, format)?;
    }
    Ok(())
}

/// A queued commit, read once: when it was committed, what it follows and what it holds at the paths.
struct Node {
    time: gix::date::SecondsSinceUnixEpoch,
    parents: Vec<ObjectId>,
    entries: Vec<Option<Located>>,
}

/// Walk from `tip` newest first by commit time with Git's default history simplification.
///
/// A commit that equals a parent at `paths` is not listed, and only the first such parent is
/// followed. A commit that equals none of its parents is listed and all of them are followed.
/// Root and shallow-boundary commits are listed when they hold a path. Only queued commits and
/// the ids already seen are kept, and the walk ends once `remaining` commits are listed.
fn simplified_walk(
    repo: &gix::Repository,
    source: &Source<'_>,
    out: &mut dyn std::io::Write,
    tip: ObjectId,
    paths: &[Literal],
    mut remaining: usize,
    format: Format,
) -> Result<()> {
    use std::{
        cmp::Reverse,
        collections::{BinaryHeap, HashMap, HashSet},
    };

    // Shallow boundary commits have no parents here, so they count as roots, like in Git.
    let shallow = repo.shallow_commits()?;
    let mut read_buffer = Buffer::default();
    let mut read = |id: ObjectId| {
        let boundary = shallow
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&id).is_ok());
        read_node(source, &mut read_buffer, id, paths, boundary)
    };
    let mut seen = HashSet::from([tip]);
    let mut pending = HashMap::new();
    // Ties in commit time keep insertion order, like Git's date-ordered queue.
    let mut queue = BinaryHeap::new();
    let mut order = 0_u64;
    let node = read(tip)?;
    queue.push((node.time, Reverse(order), tip));
    pending.insert(tip, node);
    let mut buffer = Buffer::default();
    while let Some((_, _, id)) = queue.pop() {
        let Some(node) = pending.remove(&id) else {
            continue;
        };
        let mut follow = Vec::with_capacity(node.parents.len());
        let mut treesame = false;
        for &parent in &node.parents {
            // A parent that is already queued was read when it was queued.
            let read_parent = match pending.get(&parent) {
                Some(queued) => {
                    treesame = queued.entries == node.entries;
                    None
                }
                None => {
                    let parent_node = read(parent)?;
                    treesame = parent_node.entries == node.entries;
                    Some(parent_node)
                }
            };
            follow.push((parent, read_parent));
            if treesame {
                let last = follow.len() - 1;
                follow.drain(..last);
                break;
            }
        }
        for (parent, read_parent) in follow {
            if let Some(parent_node) = read_parent
                && seen.insert(parent)
            {
                order += 1;
                queue.push((parent_node.time, Reverse(order), parent));
                pending.insert(parent, parent_node);
            }
        }
        let listed = if node.parents.is_empty() {
            node.entries.iter().any(Option::is_some)
        } else {
            !treesame
        };
        if listed {
            write_info(repo, source, &mut buffer, &mut *out, id, format)?;
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
    }
    Ok(())
}

fn read_node(
    source: &Source<'_>,
    buffer: &mut Buffer,
    id: ObjectId,
    paths: &[Literal],
    boundary: bool,
) -> Result<Node> {
    use gix::objs::commit::ref_iter::Token;

    let mut tree = None;
    let mut parents = Vec::new();
    let mut time = 0;
    let commit = source
        .load(&id, buffer)?
        .try_into_commit_iter()
        .ok_or_raise(|| message!("Expected commit {id}").validation())?;
    for token in commit {
        match token.or_raise(|| message!("Could not decode commit {id}").corrupted())? {
            Token::Tree { id: tree_id } => tree = Some(tree_id),
            Token::Parent { id: parent } => parents.push(parent),
            Token::Author { .. } => {}
            Token::Committer { signature } => {
                time = signature.seconds();
                break;
            }
            _ => break,
        }
    }
    buffer.clear();
    let tree = tree.ok_or_raise(|| message!("Commit {id} names no tree").corrupted())?;
    if boundary {
        parents.clear();
    }
    let entries = paths
        .iter()
        .map(|path| lookup(source, buffer, tree, path))
        .collect::<Result<_>>()?;
    Ok(Node { time, parents, entries })
}

/// Return `true` if a shallow boundary commit names a parent this repository lacks.
fn shallow_parents_missing(repo: &gix::Repository) -> Result<bool> {
    let Some(commits) = repo.shallow_commits()? else {
        return Ok(false);
    };
    let source = Source::deferred(repo);
    let mut buffer = Buffer::default();
    for id in commits.iter() {
        if !repo.has_object(id) {
            continue;
        }
        let commit = source
            .load(id, &mut buffer)?
            .try_into_commit_iter()
            .ok_or_raise(|| message!("Expected shallow commit {id}").validation())?;
        if commit.parent_ids().any(|parent| !repo.has_object(parent)) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The kind and object of a path's entry, which together say whether the path changed.
type Located = (EntryKind, ObjectId);

/// A literal path filter in repository-relative components.
struct Literal {
    components: Vec<BString>,
    /// Like Git, a trailing slash matches only a tree or a submodule at the path.
    directory: bool,
}

impl Literal {
    fn new(repo: &gix::Repository, path: &BStr) -> Result<Self> {
        Ok(Literal {
            components: repo_relative(repo, path)?,
            directory: path.ends_with(b"/") || (cfg!(windows) && path.ends_with(b"\\")),
        })
    }
}

/// Find `path` below the tree `root`, reading only the trees along it.
fn lookup(source: &Source<'_>, buffer: &mut Buffer, root: ObjectId, path: &Literal) -> Result<Option<Located>> {
    let mut current = (EntryKind::Tree, root);
    for component in &path.components {
        if current.0 != EntryKind::Tree {
            // A blob, link or submodule has nothing below it in this repository.
            return Ok(None);
        }
        let tree = source
            .load(&current.1, buffer)?
            .try_into_tree_iter()
            .ok_or_raise(|| message!("Expected tree {}", current.1).validation())?;
        let mut found = None;
        for entry in tree {
            let entry = entry?;
            if entry.filename == component.as_bstr() {
                found = Some((entry.mode.kind(), entry.oid.to_owned()));
                break;
            }
        }
        buffer.clear();
        match found {
            Some(entry) => current = entry,
            None => return Ok(None),
        }
    }
    Ok(Some(current).filter(|(kind, _)| !path.directory || matches!(kind, EntryKind::Tree | EntryKind::Commit)))
}

/// Turn a path relative to the current directory into repository-relative components.
fn repo_relative(repo: &gix::Repository, path: &BStr) -> Result<Vec<BString>> {
    let path = gix::path::from_bstr(path)?;
    let mut components = Vec::new();
    let relative: &Path = if path.is_absolute() {
        let workdir = repo
            .workdir()
            .ok_or_raise(|| message!("Absolute path {} needs a worktree", path.display()).validation())?;
        let workdir = std::path::absolute(workdir).or_error()?;
        match path.strip_prefix(&workdir) {
            Ok(relative) => relative,
            Err(_) => {
                let canonical = gix::path::realpath(&workdir).or_error()?;
                path.strip_prefix(&canonical)
                    .or_raise(|| message!("Path {} is outside the worktree", path.display()).validation())?
            }
        }
    } else {
        if let Some(prefix) = repo.prefix()? {
            push_components(&mut components, prefix, path.as_ref())?;
        }
        path.as_ref()
    };
    push_components(&mut components, relative, path.as_ref())?;
    Ok(components)
}

fn push_components(components: &mut Vec<BString>, path: &Path, original: &Path) -> Result<()> {
    for component in path.components() {
        match component {
            Component::Normal(name) => components.push(gix::path::os_str_into_bstr(name)?.to_owned()),
            Component::CurDir | Component::RootDir | Component::Prefix(_) => {}
            Component::ParentDir => {
                if components.pop().is_none() {
                    bail!(message!("Path {} is outside the worktree", original.display()).validation());
                }
            }
        }
    }
    Ok(())
}

/// Return `name` as a path filter if it names something in the worktree.
fn worktree_path(repo: &gix::Repository, name: &BStr) -> Option<Literal> {
    let literal = Literal::new(repo, name).ok()?;
    let mut path = repo.workdir()?.to_owned();
    for component in &literal.components {
        path.push(gix::path::from_bstr(component.as_bstr()).ok()?);
    }
    path.symlink_metadata().ok().map(|_| literal)
}

/// Resolve `HEAD`, following symbolic references without loading the terminal commit.
fn head_commit(repo: &gix::Repository, source: &Source<'_>) -> Result<ObjectId> {
    let head = repo.find_reference("HEAD")?.follow_to_object()?.detach();
    peel_to_commit(repo, source, head, "HEAD".into())
}

/// Follow annotated tags from `id` to a commit, reading only the header of the terminal object.
fn peel_to_commit(repo: &gix::Repository, source: &Source<'_>, mut id: ObjectId, name: &BStr) -> Result<ObjectId> {
    let mut seen = std::collections::HashSet::new();
    let mut buffer = Buffer::default();
    loop {
        match repo.find_header(id)?.kind() {
            Kind::Commit => return Ok(id),
            Kind::Tag => {}
            _ => bail!(message!("{name} object {id} does not point to a commit").validation()),
        }
        if !seen.insert(id) {
            bail!(gix::error::corruption(format!("Tag cycle while resolving log {name}")));
        }
        let object = source.load(&id, &mut buffer)?;
        let tag = object
            .try_into_tag_iter()
            .ok_or_raise(|| message!("{name} does not point to a commit").validation())?;
        id = tag.target_id()?;
    }
}

fn write_info(
    repo: &gix::Repository,
    source: &Source<'_>,
    buffer: &mut Buffer,
    mut out: impl std::io::Write,
    id: ObjectId,
    format: Format,
) -> Result<()> {
    let commit = source.load(&id, buffer)?;
    if commit.kind != Kind::Commit {
        bail!(message!("Expected commit {}", id).validation());
    }
    // Keep message_raw_sloppy's existing byte selection without copying the title.
    let message = commit
        .data
        .find(b"\n\n")
        .map(|pos| &commit.data[pos + 2..])
        .unwrap_or_default();
    let title = message.lines().next();
    match format {
        Format::Default => write!(out, "{} ", id.to_hex_with_len(8)).or_error()?,
        Format::Oneline => {
            use gix::prelude::ObjectIdExt;
            write!(out, "{} ", id.attach(repo).shorten_or_id()).or_error()?;
        }
    }
    let mut title = title.unwrap_or(b"<no message>");
    gix::utils::stream::copy(&mut title, &mut out, &gix::interrupt::IS_INTERRUPTED).or_error()?;
    writeln!(out).or_error()?;
    buffer.clear();

    Ok(())
}
