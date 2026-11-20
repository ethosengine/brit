use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap, HashSet},
    path::{Component, Path},
};

use gix::{
    Result,
    bstr::{BStr, BString, ByteSlice},
    error::{OptionExt, ResultExt, bail, message, unsupported},
    hash::ObjectId,
    object::bounded::Source,
    objs::{CommitRefIter, Kind, find::Buffer, tree::EntryKind},
    revision::walk::Sorting,
    traverse::commit::Parents,
};

/// Configure the starting commits, hidden ancestry, order, and printing of a log traversal.
#[derive(Clone)]
pub struct Options {
    /// Revision specs to list instead of `HEAD`: `A`, `^A`, `A..B`, `A...B`, `A^@` or `A^!`.
    pub revisions: Vec<BString>,
    /// The order in which to return commits, defaulting to [`Sorting::DateOrder`].
    pub sorting: Sorting,
    /// Which parents to follow when walking visible commits.
    pub parents: Parents,
    /// Stop after this many commits; `Some(0)` prints nothing, and `None` is unlimited.
    pub max_count: Option<usize>,
    /// Abbreviate hashes like `git log --oneline` instead of printing eight hex digits.
    pub oneline: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            revisions: Vec::new(),
            sorting: Sorting::DateOrder,
            parents: Parents::default(),
            max_count: None,
            oneline: false,
        }
    }
}

/// List the commits selected by `options.revisions` (or reachable from `HEAD`), optionally only
/// those changing `pathspecs`, literal paths relative to the current directory.
///
/// Without paths, in the default order (date order over all parents), the set is exactly the
/// commits reachable from the included specs and not from the excluded ones, ordered newest first
/// with ancestors after their descendants, like `git rev-list --date-order`. That equals Git's set
/// except where clock skew makes Git's limiter stop early; the order of commits with identical
/// commit times is unspecified. Another `options.sorting`, or a first-parent traversal, walks
/// through [`gix::Repository::rev_walk()`].
///
/// With paths, commits are listed like `git log <specs> -- <paths>`, following Git's limiter and
/// its default history simplification: a commit changes a path when its entry there (kind and
/// object) differs from its parent's, a commit equal to a parent at the paths is not listed and
/// only that parent is followed, and root and shallow-boundary commits change every path they
/// contain. Ties in commit time keep Git's queue order. A trailing slash matches only directories
/// (and submodules). Path-limited history supports only the default order over all parents.
///
/// Every form stops at a shallow boundary, reading its commits as roots.
pub fn log(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    pathspecs: Vec<BString>,
    options: Options,
) -> Result<()> {
    let format = if options.oneline {
        Format::Oneline
    } else {
        Format::Default
    };
    let remaining = options.max_count.unwrap_or(usize::MAX);
    if remaining == 0 {
        return Ok(());
    }
    let source = Source::new(
        &repo,
        gix::utils::stream::Budget::new(&std::env::temp_dir()).or_error()?,
    )?;
    let mut paths = pathspecs
        .iter()
        .map(|path| Literal::new(&repo, path.as_bstr()))
        .collect::<Result<Vec<_>>>()?;
    let starts = match options.revisions.as_slice() {
        [] => vec![Start::include(head_commit(&repo, &source)?)],
        // Like Git, a lone argument that names no revision but an existing worktree path is a path.
        [single] if paths.is_empty() && !single.contains_str("..") && !single.starts_with(b"^") => {
            match starts(&repo, &source, &options.revisions) {
                Ok(starts) => starts,
                Err(error) => match worktree_path(&repo, single.as_bstr()) {
                    Some(path) if repo.rev_parse(single.as_bstr()).is_err() => {
                        paths.push(path);
                        vec![Start::include(head_commit(&repo, &source)?)]
                    }
                    _ => return Err(error),
                },
            }
        }
        revisions => starts(&repo, &source, revisions)?,
    };

    let default_order = matches!(options.sorting, Sorting::DateOrder) && matches!(options.parents, Parents::All);
    if !paths.is_empty() {
        if !default_order {
            bail!(unsupported(
                "Path-limited history supports only the default date order over all parents; omit --sort and --first-parent or use `git log`"
            ));
        }
        let walk = SimplifiedWalk::new(&repo, &source, &paths)?;
        return walk.run(&repo, out, &starts, remaining, format);
    }
    if !default_order {
        return log_sorted(&repo, &source, out, starts, &options, remaining, format);
    }
    // Every form honours the shallow boundary, as Git does, even where the boundary's parents
    // happen to be present.
    let shallow = repo.shallow_commits()?;
    let (mut tips, mut ends) = (Vec::new(), Vec::new());
    for start in starts {
        if start.excluded {
            ends.push(start.id);
        } else {
            tips.push(start.id);
        }
    }
    // The topological walk queues every tip before it applies the ends, so a tip that is itself
    // excluded (`HEAD..HEAD`) would still be printed.
    tips.retain(|tip| !ends.contains(tip));
    if tips.is_empty() {
        return Ok(());
    }
    let commits = Grafted {
        source: &source,
        shallow,
    };
    // The topological walk limits hidden ancestry by commit date, as Git does, so clock skew can
    // leave excluded commits in its output. Paint the excluded ancestry completely instead, and
    // prune the walk where it reaches a painted commit: every ancestor of one is painted too.
    let hidden = hidden_ancestry(&commits, &ends)?;
    let topo = gix::traverse::commit::topo::Builder::from_iters(commits, tips, None::<Vec<ObjectId>>)
        .with_predicate(move |id: &gix::hash::oid| !hidden.contains(id))
        .build()?;
    let mut buffer = Buffer::default();
    for info in topo.take(remaining) {
        write_info(&repo, &source, &mut buffer, &mut *out, info?.id, format)?;
    }
    Ok(())
}

/// Walk `starts` through [`gix::Repository::rev_walk()`] in `options.sorting`, following only
/// the first parent of visible commits if `options.parents` asks for it.
fn log_sorted(
    repo: &gix::Repository,
    source: &Source<'_>,
    out: &mut dyn std::io::Write,
    starts: Vec<Start>,
    options: &Options,
    remaining: usize,
    format: Format,
) -> Result<()> {
    let (mut tips, mut hidden) = (Vec::new(), Vec::new());
    for start in starts {
        if start.excluded {
            hidden.push(start.id);
        } else {
            tips.push(start.id);
        }
    }
    let _ = repo.config_snapshot().try_boolean("core.commitGraph")?;
    let mut walk = repo.rev_walk(tips).with_hidden(hidden).sorting(options.sorting);
    if matches!(options.parents, Parents::First) {
        walk = walk.first_parent_only();
    }
    let mut buffer = Buffer::default();
    for info in walk.all()?.take(remaining) {
        write_info(repo, source, &mut buffer, &mut *out, info?.id, format)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Format {
    /// Eight hex digits and the title.
    Default,
    /// Git's abbreviation (honouring `core.abbrev`) and the title.
    Oneline,
}

/// A commit a walk starts from, or (when `excluded`) whose ancestry it hides.
#[derive(Clone, Copy)]
struct Start {
    id: ObjectId,
    excluded: bool,
}

impl Start {
    fn include(id: ObjectId) -> Self {
        Start { id, excluded: false }
    }
    fn exclude(id: ObjectId) -> Self {
        Start { id, excluded: true }
    }
}

/// Resolve `revisions` into starting and excluded commits, in the order Git queues them.
fn starts(repo: &gix::Repository, source: &Source<'_>, revisions: &[BString]) -> Result<Vec<Start>> {
    use gix::revision::plumbing::Spec;
    let parents = |commit_id: ObjectId| -> Result<Vec<ObjectId>> {
        let mut buffer = Buffer::default();
        let commit = source
            .load(&commit_id, &mut buffer)?
            .try_into_commit_iter()
            .ok_or_raise(|| message!("Expected commit {commit_id}").validation())?;
        Ok(commit.parent_ids().collect())
    };
    let mut starts = Vec::new();
    for revision in revisions {
        let name = revision.as_bstr();
        let commit = |id| peel_to_commit(repo, source, id, name);
        match repo.rev_parse(name)?.detach() {
            Spec::Include(id) => starts.push(Start::include(commit(id)?)),
            Spec::Exclude(id) => starts.push(Start::exclude(commit(id)?)),
            Spec::Range { from, to } => {
                starts.push(Start::exclude(commit(from)?));
                starts.push(Start::include(commit(to)?));
            }
            Spec::Merge { theirs, ours } => {
                let (theirs, ours) = (commit(theirs)?, commit(ours)?);
                let bases = merge_bases(repo, source, theirs, ours)?;
                starts.extend(bases.into_iter().map(Start::exclude));
                starts.extend([Start::include(theirs), Start::include(ours)]);
            }
            Spec::IncludeOnlyParents(id) => starts.extend(parents(commit(id)?)?.into_iter().map(Start::include)),
            Spec::ExcludeParents(id) => {
                let id = commit(id)?;
                starts.extend(parents(id)?.into_iter().map(Start::exclude));
                starts.push(Start::include(id));
            }
        }
    }
    Ok(starts)
}

/// Commits for the topological walk, with shallow boundary commits read as roots.
struct Grafted<'a, 'repo> {
    source: &'a Source<'repo>,
    shallow: Option<gix::shallow::Commits>,
}

impl Grafted<'_, '_> {
    fn is_boundary(&self, id: &gix::hash::oid) -> bool {
        self.shallow
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&id.to_owned()).is_ok())
    }
}

impl gix::traverse::commit::topo::source::Source for Grafted<'_, '_> {
    fn find_commit<'a>(&self, id: &gix::hash::oid, buffer: &'a mut Buffer) -> Result<CommitRefIter<'a>> {
        if !self.is_boundary(id) {
            return gix::traverse::commit::topo::source::Source::find_commit(self.source, id, buffer);
        }
        let commit = without_parents(self.source.load(id, buffer)?.data);
        let owned = buffer.owned();
        *owned = commit;
        let commit: &'a Vec<u8> = owned;
        Ok(CommitRefIter::from_bytes(commit, id.kind()))
    }
}

/// Merge bases read through the same grafts, so they never reach past a shallow boundary.
impl gix::objs::find::Source for Grafted<'_, '_> {
    fn try_find<'a>(&self, id: &gix::hash::oid, buffer: &'a mut Buffer) -> Result<Option<gix::objs::Data<'a>>> {
        if !self.is_boundary(id) {
            return gix::objs::find::Source::try_find(self.source, id, buffer);
        }
        let Some(data) = gix::objs::find::Source::try_find(self.source, id, buffer)? else {
            return Ok(None);
        };
        let (kind, commit) = (data.kind, without_parents(data.data));
        let owned = buffer.owned();
        *owned = commit;
        let commit: &'a Vec<u8> = owned;
        Ok(Some(gix::objs::Data::new(commit, kind, id.kind())))
    }
}

/// Every commit reachable from `ends`, `ends` included, read through the shallow grafts.
fn hidden_ancestry(commits: &Grafted<'_, '_>, ends: &[ObjectId]) -> Result<HashSet<ObjectId>> {
    use gix::traverse::commit::topo::source::Source as _;
    let mut hidden = HashSet::new();
    let mut queue = ends.to_vec();
    let mut buffer = Buffer::default();
    while let Some(id) = queue.pop() {
        if !hidden.insert(id) {
            continue;
        }
        let parents: Vec<ObjectId> = commits.find_commit(&id, &mut buffer)?.parent_ids().collect();
        queue.extend(parents.into_iter().filter(|parent| !hidden.contains(parent)));
    }
    Ok(hidden)
}

/// The merge bases of `one` and `other`, as Git computes them in a shallow repository.
pub(crate) fn merge_bases(
    repo: &gix::Repository,
    source: &Source<'_>,
    one: ObjectId,
    other: ObjectId,
) -> Result<Vec<ObjectId>> {
    let commits = Grafted {
        source,
        shallow: repo.shallow_commits()?,
    };
    // The commit-graph records the parents a shallow boundary hides, so it is not consulted.
    let mut graph = gix::revwalk::Graph::with_source(commits, None);
    Ok(gix::revision::plumbing::merge_base(one, &[other], &mut graph)?
        .map(|bases| bases.into_iter().collect())
        .unwrap_or_default())
}

/// Return the commit bytes `data` without their `parent` header lines.
fn without_parents(data: &[u8]) -> Vec<u8> {
    let header_end = data.find(b"\n\n").map_or(data.len(), |pos| pos + 1);
    let mut commit = Vec::with_capacity(data.len());
    for line in data[..header_end].split_inclusive(|byte| *byte == b'\n') {
        if !line.starts_with(b"parent ") {
            commit.extend_from_slice(line);
        }
    }
    commit.extend_from_slice(&data[header_end..]);
    commit
}

/// A commit read once: when it was committed, what it follows and what it holds at the paths.
struct Node {
    time: gix::date::SecondsSinceUnixEpoch,
    parents: Vec<ObjectId>,
    entries: Vec<Option<Located>>,
}

const SEEN: u8 = 1;
const UNINTERESTING: u8 = 2;
/// An excluded commit named by the caller, still relevant when simplifying its children.
const BOTTOM: u8 = 4;
const TREESAME: u8 = 8;
/// How many uninteresting commits Git reads past the last listed one to tolerate clock skew.
const SLOP: u8 = 5;

/// Git's revision walk with default history simplification for `paths`.
///
/// Commits are processed newest first by commit time, ties in queue order. A commit equal to a
/// relevant parent at the paths is not listed and only that parent is followed; a commit equal to
/// none is listed and all of its parents are followed. With excluded revisions the walk is limited
/// like Git's: excluded ancestry is painted uninteresting, and the walk ends a few commits after
/// only uninteresting commits remain queued. Without exclusions commits stream and the walk stops
/// once enough are listed.
struct SimplifiedWalk<'a, 'repo> {
    source: &'a Source<'repo>,
    paths: &'a [Literal],
    shallow: Option<gix::shallow::Commits>,
    buffer: Buffer,
    flags: HashMap<ObjectId, u8>,
    nodes: HashMap<ObjectId, Node>,
    queue: BinaryHeap<(gix::date::SecondsSinceUnixEpoch, Reverse<u64>, ObjectId)>,
    order: u64,
}

impl<'a, 'repo> SimplifiedWalk<'a, 'repo> {
    fn new(repo: &gix::Repository, source: &'a Source<'repo>, paths: &'a [Literal]) -> Result<Self> {
        Ok(SimplifiedWalk {
            source,
            paths,
            // Shallow boundary commits have no parents here, so they count as roots, like in Git.
            shallow: repo.shallow_commits()?,
            buffer: Buffer::default(),
            flags: HashMap::new(),
            nodes: HashMap::new(),
            queue: BinaryHeap::new(),
            order: 0,
        })
    }

    fn run(
        mut self,
        repo: &gix::Repository,
        out: &mut dyn std::io::Write,
        starts: &[Start],
        mut remaining: usize,
        format: Format,
    ) -> Result<()> {
        for start in starts {
            self.parse(start.id, false)?;
            if start.excluded {
                self.set(start.id, UNINTERESTING | BOTTOM);
                self.mark_parents_uninteresting(start.id);
            }
        }
        for start in starts {
            if self.flags(start.id) & SEEN == 0 {
                self.set(start.id, SEEN);
                self.push(start.id);
            }
        }
        let source = self.source;
        let mut buffer = Buffer::default();
        let mut print = |id: ObjectId| -> Result<bool> {
            write_info(repo, source, &mut buffer, &mut *out, id, format)?;
            remaining -= 1;
            Ok(remaining == 0)
        };
        if !starts.iter().any(|start| start.excluded) {
            while let Some((_, _, id)) = self.queue.pop() {
                self.process_parents(id)?;
                if self.flags(id) & (UNINTERESTING | TREESAME) == 0 && print(id)? {
                    break;
                }
            }
            return Ok(());
        }
        let mut listed = Vec::new();
        let mut date = gix::date::SecondsSinceUnixEpoch::MAX;
        let mut slop = SLOP;
        while let Some((_, _, id)) = self.queue.pop() {
            self.process_parents(id)?;
            if self.flags(id) & UNINTERESTING != 0 {
                self.mark_parents_uninteresting(id);
                slop = self.still_interesting(date, slop);
                if slop == 0 {
                    break;
                }
                continue;
            }
            date = self.nodes[&id].time;
            listed.push(id);
        }
        for id in listed {
            if self.flags(id) & (UNINTERESTING | TREESAME) == 0 && print(id)? {
                break;
            }
        }
        Ok(())
    }

    fn flags(&self, id: ObjectId) -> u8 {
        self.flags.get(&id).copied().unwrap_or(0)
    }

    fn set(&mut self, id: ObjectId, flags: u8) {
        *self.flags.entry(id).or_default() |= flags;
    }

    fn parents(&self, id: ObjectId) -> Vec<ObjectId> {
        self.nodes.get(&id).map(|node| node.parents.clone()).unwrap_or_default()
    }

    /// Read `id` once. Return `false` if it is absent and `gently` allows that.
    fn parse(&mut self, id: ObjectId, gently: bool) -> Result<bool> {
        if self.nodes.contains_key(&id) {
            return Ok(true);
        }
        if gently && !gix::objs::Exists::exists(self.source, &id) {
            return Ok(false);
        }
        let boundary = self
            .shallow
            .as_ref()
            .is_some_and(|commits| commits.binary_search(&id).is_ok());
        let node = read_node(self.source, &mut self.buffer, id, self.paths, boundary)?;
        self.nodes.insert(id, node);
        Ok(true)
    }

    fn push(&mut self, id: ObjectId) {
        self.order += 1;
        self.queue.push((self.nodes[&id].time, Reverse(self.order), id));
    }

    /// Git's `process_parents()`: paint an uninteresting commit's parents, or simplify an
    /// interesting one, then queue the parents not seen yet.
    fn process_parents(&mut self, id: ObjectId) -> Result<()> {
        if self.flags(id) & UNINTERESTING != 0 {
            for parent in self.parents(id) {
                self.set(parent, UNINTERESTING);
                if !self.parse(parent, true)? {
                    continue;
                }
                self.mark_parents_uninteresting(parent);
                if self.flags(parent) & SEEN == 0 {
                    self.set(parent, SEEN);
                    self.push(parent);
                }
            }
            return Ok(());
        }
        self.simplify(id)?;
        for parent in self.parents(id) {
            self.parse(parent, false)?;
            if self.flags(parent) & SEEN == 0 {
                self.set(parent, SEEN);
                self.push(parent);
            }
        }
        Ok(())
    }

    /// Git's `try_to_simplify_commit()` for the paths.
    fn simplify(&mut self, id: ObjectId) -> Result<()> {
        let parents = self.parents(id);
        if parents.is_empty() {
            if self.nodes[&id].entries.iter().all(Option::is_none) {
                self.set(id, TREESAME);
            }
            return Ok(());
        }
        let (mut relevant_parents, mut relevant_change, mut irrelevant_change) = (0, false, false);
        for parent in parents {
            let relevant = self.flags(parent) & (UNINTERESTING | BOTTOM) != UNINTERESTING;
            if relevant {
                relevant_parents += 1;
            }
            self.parse(parent, false)?;
            if self.nodes[&parent].entries == self.nodes[&id].entries {
                // An uninteresting side that brought the whole change keeps the other parents.
                if !relevant {
                    continue;
                }
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.parents = vec![parent];
                }
                self.set(id, TREESAME);
                return Ok(());
            }
            if relevant {
                relevant_change = true;
            } else {
                irrelevant_change = true;
            }
        }
        let treesame = if relevant_parents > 0 {
            !relevant_change
        } else {
            !irrelevant_change
        };
        if treesame {
            self.set(id, TREESAME);
        }
        Ok(())
    }

    /// Paint the known ancestry of `id` uninteresting, following parents already read.
    fn mark_parents_uninteresting(&mut self, id: ObjectId) {
        let mut pending = self.parents(id);
        while let Some(parent) = pending.pop() {
            if self.flags(parent) & UNINTERESTING != 0 {
                continue;
            }
            self.set(parent, UNINTERESTING);
            pending.extend(self.parents(parent));
        }
    }

    /// Git's `still_interesting()`: keep walking while interesting commits are queued or the queue
    /// still holds commits newer than the last listed one, then for `SLOP` more commits.
    fn still_interesting(&self, date: gix::date::SecondsSinceUnixEpoch, slop: u8) -> u8 {
        let Some(&(newest, _, _)) = self.queue.peek() else {
            return 0;
        };
        if date <= newest || self.queue.iter().any(|(_, _, id)| self.flags(*id) & UNINTERESTING == 0) {
            return SLOP;
        }
        slop - 1
    }
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

/// The kind and object of a path's entry, which together say whether the path changed.
type Located = (EntryKind, ObjectId);

/// A literal path filter in repository-relative components.
pub(crate) struct Literal {
    pub(crate) components: Vec<BString>,
    /// Like Git, a trailing slash matches only a tree or a submodule at the path.
    pub(crate) directory: bool,
}

impl Literal {
    pub(crate) fn new(repo: &gix::Repository, path: &BStr) -> Result<Self> {
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
pub(crate) fn worktree_path(repo: &gix::Repository, name: &BStr) -> Option<Literal> {
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
