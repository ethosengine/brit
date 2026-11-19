//! `diff` between revisions: two trees, `A..B`, `A...B`, `--merge-base`, or one revision against
//! the index or the working files. Every form is defined as Git's with `--no-renames`.
//!
//! Measured limits, each a refusal rather than different output unless said otherwise:
//!
//! * A path whose `diff` attribute names a driver (`diff=<driver>`, even a built-in one with
//!   nothing configured) refuses the patch, `--stat` and `--numstat` forms; `-diff`/`binary` print
//!   a binary change, and `--name-only`/`--name-status` never read attributes.
//! * Intent-to-add, skip-worktree and assume-unchanged entries, sparse and split indexes and a
//!   conflicted index refuse every form that reads the index; a submodule refuses comparisons with
//!   the working files, whose side of it is the submodule's own checkout.
//! * Line diffs use imara-diff's Myers with its own speed heuristics and Brit's slider (indent)
//!   heuristic; Git uses xdiff's Myers with its cost-limit heuristic and `diff.indentHeuristic`.
//!   Neither is minimal, and on very large, heavily changed files they can pick different
//!   alignments: on the monorepo's `origin/main...origin/dev`, 3 of 26,673 files differ by two to
//!   five lines on each side in `--numstat` (`elohim/Cargo.lock` 2725/84 against Git's 2723/82),
//!   the patch text of those files differs in hunk placement, and `--name-status` is identical.

use std::collections::BTreeMap;

use gix::{
    ObjectId, Result,
    bstr::{BStr, BString, ByteSlice},
    error::{ResultExt, bail, message},
    object::bounded::Source,
    utils::stream::Budget,
};

use super::{
    FileStat, GITLINK, binary_attribute, check_external_diff, check_interrupted, collect_stat, load_side, quote_path,
    render_numstat, render_patch, render_stats, working_output, write_diff_output, write_name,
};
use crate::repository::log::{Literal, merge_bases, worktree_path};

/// How changes are printed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    /// A unified patch with three lines of context, like `git diff`.
    #[default]
    Patch,
    /// An 80-column summary, like `git diff --stat=80`.
    Stat,
    /// Changed paths, like `git diff --name-only`.
    NameOnly,
    /// Changed paths with their status letter, like `git diff --name-status`.
    NameStatus,
    /// Added and removed lines per path, like `git diff --numstat`.
    NumStat,
    /// Nothing; only whether anything differs, like `git diff --quiet`.
    Quiet,
}

impl Format {
    /// Whether printing needs the compared blobs, not just their identities.
    pub(super) fn needs_content(self) -> bool {
        matches!(self, Format::Patch | Format::Stat | Format::NumStat)
    }
}

/// What `brit diff` compares and how it prints the result.
#[derive(Debug, Default, Clone)]
pub struct Request {
    /// Up to two revisions, or one range (`A..B`, `A...B`). A leading name that is no revision but
    /// an existing worktree path starts the paths, as in Git.
    pub revisions: Vec<BString>,
    /// Literal paths given after `--`.
    pub paths: Vec<BString>,
    /// Compare with the index instead of the working files.
    pub cached: bool,
    /// Compare the merge base of the revisions (or of one revision and `HEAD`).
    pub merge_base: bool,
    /// How to print changes.
    pub format: Format,
}

/// Returned by callers that turn "differences found" into exit code 1, like `git diff --exit-code`.
#[derive(Debug)]
pub struct Differences;

impl std::fmt::Display for Differences {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the compared versions differ")
    }
}

impl std::error::Error for Differences {}

/// Compare what `request` names, print it to `out`, and return whether anything differs.
/// Warnings, such as the merge base chosen among several, go to `err`.
pub fn diff(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    request: Request,
) -> Result<bool> {
    let Request {
        revisions,
        mut paths,
        cached,
        merge_base,
        format,
    } = request;
    let (revisions, positional_paths) = split_positionals(&repo, &revisions, !paths.is_empty())?;
    paths.splice(0..0, positional_paths);
    match revisions.as_slice() {
        [] => {
            if merge_base {
                bail!(message("--merge-base needs at least one commit").validation());
            }
            working_output(&repo, out, None, cached, &paths, format)
        }
        [range] if is_range(range) => {
            if cached || merge_base {
                bail!(message!("--cached and --merge-base take revisions, not the range `{range}`").validation());
            }
            let (old, new) = endpoints(&repo, err, range.as_bstr(), None, false)?;
            compare_trees(&repo, out, Some(old), Some(new), &paths, format)
        }
        [base] => {
            let base = if merge_base {
                endpoints(&repo, err, base.as_bstr(), Some("HEAD".into()), true)?.0
            } else {
                rev_parse(&repo, base.as_bstr())?
            };
            let tree = peel_tree(&repo, base)?;
            working_output(&repo, out, Some(tree), cached, &paths, format)
        }
        [old, new] => {
            if let Some(range) = [old, new].into_iter().find(|rev| is_range(rev)) {
                bail!(message!("a range like `{range}` must be the only revision").validation());
            }
            if cached {
                bail!(message("--cached compares at most one revision with the index").validation());
            }
            let (old, new) = endpoints(&repo, err, old.as_bstr(), Some(new.as_bstr()), merge_base)?;
            compare_trees(&repo, out, Some(old), Some(new), &paths, format)
        }
        _ => bail!(message("diff compares at most two revisions; use git diff for combined diffs").validation()),
    }
}

/// Compare two tree-ish revisions for `diff tree`.
pub(super) fn trees(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    old: &BStr,
    new: &BStr,
    format: Format,
) -> Result<bool> {
    let (old, new) = (rev_parse(repo, old)?, rev_parse(repo, new)?);
    compare_trees(repo, out, Some(old), Some(new), &[], format)
}

fn is_range(revision: &[u8]) -> bool {
    revision.contains_str("..")
}

fn rev_parse(repo: &gix::Repository, revision: &BStr) -> Result<ObjectId> {
    Ok(repo.rev_parse_single(revision)?.detach())
}

/// Split positionals into leading revisions and the paths that follow them, the way Git does
/// without `--`: a name that is no revision but an existing worktree path, and everything after
/// it, are paths. A name that is both is refused as ambiguous.
fn split_positionals(
    repo: &gix::Repository,
    positionals: &[BString],
    after_separator: bool,
) -> Result<(Vec<BString>, Vec<BString>)> {
    let (mut revisions, mut paths) = (Vec::new(), Vec::new());
    for name in positionals {
        let is_path = !after_separator && worktree_path(repo, name.as_bstr()).is_some();
        if !paths.is_empty() {
            if !is_path {
                bail!(message!("{}", unknown(name.as_bstr())).validation());
            }
            paths.push(name.clone());
            continue;
        }
        if is_range(name) {
            revisions.push(name.clone());
            continue;
        }
        match repo.rev_parse(name.as_bstr()) {
            Ok(_) if is_path => bail!(
                message!(
                    "ambiguous argument '{name}': both revision and filename; use '--' to separate paths from revisions"
                )
                .validation()
            ),
            Ok(_) => revisions.push(name.clone()),
            Err(_) if is_path => paths.push(name.clone()),
            // Git's wording: without `--` a name may still be meant as a path.
            Err(_) if !after_separator => bail!(message!("{}", unknown(name.as_bstr())).validation()),
            Err(_) => bail!(message!("bad revision '{name}'").validation()),
        }
    }
    Ok((revisions, paths))
}

fn unknown(name: &BStr) -> String {
    format!(
        "ambiguous argument '{name}': unknown revision or path not in the working tree; use '--' to separate paths from revisions"
    )
}

/// The objects to compare for `OLD NEW`, `--merge-base OLD NEW` or a lone `A..B` or `A...B`.
///
/// Merge bases follow Git in shallow repositories, where a boundary commit has no parents. With
/// several merge bases, `--merge-base` refuses and names them, and `A...B` uses the first and
/// warns on `err`.
pub(super) fn endpoints(
    repo: &gix::Repository,
    err: &mut dyn std::io::Write,
    old: &BStr,
    new: Option<&BStr>,
    merge_base: bool,
) -> Result<(ObjectId, ObjectId)> {
    use gix::revision::plumbing::Spec;
    let (ours, theirs, symmetric) = match new {
        Some(new) if !merge_base => return Ok((rev_parse(repo, old)?, rev_parse(repo, new)?)),
        Some(new) => (rev_parse(repo, old)?, rev_parse(repo, new)?, false),
        None => match repo.rev_parse(old)?.detach() {
            Spec::Range { from, to } => return Ok((from, to)),
            Spec::Merge { theirs, ours } => (theirs, ours, true),
            _ => bail!(message!("`{old}` needs a second tree, or must be a range like `A..B` or `A...B`").validation()),
        },
    };
    let source = Source::deferred(repo);
    let commit = |id| {
        source
            .peel(id, gix::objs::Kind::Commit)
            .or_raise(|| message!("merge-base comparison needs commits, but {id} is not one").validation())
    };
    let (ours, theirs) = (commit(ours)?, commit(theirs)?);
    let bases = merge_bases(repo, &source, ours, theirs)?;
    match bases.as_slice() {
        [] if symmetric => bail!(message!("{old}: no merge base").validation()),
        [] => bail!(message("no merge base found").validation()),
        [base] => Ok((*base, theirs)),
        [first, ..] if symmetric => {
            writeln!(err, "warning: {old}: multiple merge bases, using {first}").or_error()?;
            Ok((*first, theirs))
        }
        bases => {
            let names: Vec<_> = bases.iter().map(ToString::to_string).collect();
            bail!(message!("multiple merge bases found: {}", names.join(" ")).validation())
        }
    }
}

fn peel_tree(repo: &gix::Repository, id: ObjectId) -> Result<ObjectId> {
    Source::deferred(repo)
        .peel(id, gix::objs::Kind::Tree)
        .or_raise(|| message!("{id} does not name a tree").validation())
}

/// Literal repository paths that restrict a comparison: a file, or a directory and everything
/// below it. A trailing slash matches only a directory or a submodule.
pub(crate) struct PathFilter(Vec<(BString, bool)>);

impl PathFilter {
    pub(crate) fn new(repo: &gix::Repository, paths: &[BString]) -> Result<Self> {
        let mut literals = Vec::new();
        for path in paths {
            if path.starts_with(b":") || path.iter().any(|byte| b"*?[".contains(byte)) {
                bail!(
                    message!("only literal paths are supported, not the pathspec `{path}`; use git diff").unsupported()
                );
            }
            let literal = Literal::new(repo, path.as_bstr())?;
            literals.push((literal.components.join(&b'/').into(), literal.directory));
        }
        Ok(PathFilter(literals))
    }

    fn matches(&self, path: &BStr, gitlink: bool) -> bool {
        self.0.is_empty()
            || self.0.iter().any(|(literal, directory)| {
                literal.is_empty()
                    || (path.starts_with(literal)
                        && match path.get(literal.len()) {
                            None => !directory || gitlink,
                            Some(byte) => *byte == b'/',
                        })
            })
    }
}

/// One side of a changed path: its mode and object.
type Side = (u32, ObjectId);

/// One changed path between two trees, by mode and object; a tree is never a side.
pub(crate) struct Change {
    pub(crate) path: BString,
    pub(crate) old: Option<Side>,
    pub(crate) new: Option<Side>,
}

/// The changed paths between two trees (an absent tree is empty) that `filter` selects, in Git's
/// output order. Only differing subtrees are read.
pub(crate) fn changes(
    repo: &gix::Repository,
    old: Option<ObjectId>,
    new: Option<ObjectId>,
    filter: &PathFilter,
) -> Result<Vec<Change>> {
    use gix::{
        diff::tree::recorder::Change as TreeChange,
        objs::find::{Buffer, Source as _},
    };
    // A plain tree diff reads only differing subtrees and never the working index or attributes.
    // Rename tracking is off: a rename is a deletion and an addition, like `--no-renames`.
    let source = Source::deferred(repo);
    let empty = ObjectId::empty_tree(repo.object_hash());
    let (mut old_buffer, mut new_buffer) = (Buffer::default(), Buffer::default());
    let old_tree = source.find_tree(&old.unwrap_or(empty), &mut old_buffer)?;
    let new_tree = source.find_tree(&new.unwrap_or(empty), &mut new_buffer)?;
    let mut recorder = gix::diff::tree::Recorder::default();
    gix::diff::tree(
        old_tree,
        new_tree,
        gix::diff::tree::State::default(),
        source,
        &mut recorder,
    )
    .or_error()?;
    let side =
        |mode: gix::objs::tree::EntryMode, id: ObjectId| (!mode.is_tree()).then(|| (u32::from(mode.value()), id));
    // Full paths sort in Git's tree order, which compares a directory as its name and a slash.
    let mut by_path: BTreeMap<BString, (Option<Side>, Option<Side>)> = BTreeMap::new();
    for change in recorder.records {
        check_interrupted()?;
        let (location, old, new) = match change {
            TreeChange::Addition {
                path, entry_mode, oid, ..
            } => (path, None, side(entry_mode, oid)),
            TreeChange::Deletion {
                path, entry_mode, oid, ..
            } => (path, side(entry_mode, oid), None),
            TreeChange::Modification {
                path,
                previous_entry_mode,
                previous_oid,
                entry_mode,
                oid,
            } => (path, side(previous_entry_mode, previous_oid), side(entry_mode, oid)),
        };
        if old.is_none() && new.is_none() {
            continue;
        }
        let entry = by_path.entry(location).or_default();
        entry.0 = entry.0.or(old);
        entry.1 = entry.1.or(new);
    }
    Ok(by_path
        .into_iter()
        .filter(|(path, (old, new))| {
            let gitlink = old.iter().chain(new).any(|(mode, _)| *mode == GITLINK);
            filter.matches(path.as_bstr(), gitlink)
        })
        .map(|(path, (old, new))| Change { path, old, new })
        .collect())
}

/// Compare two trees, buffering the output so a refusal never leaves partial output.
fn compare_trees(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    old: Option<ObjectId>,
    new: Option<ObjectId>,
    paths: &[BString],
    format: Format,
) -> Result<bool> {
    let old = old.map(|id| peel_tree(repo, id)).transpose()?;
    let new = new.map(|id| peel_tree(repo, id)).transpose()?;
    let filter = PathFilter::new(repo, paths)?;
    let changes = changes(repo, old, new, &filter)?;
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let mut rendered = std::io::BufWriter::new(budget.spool().or_error()?);
    render(repo, &mut rendered, &changes, (old, new), format)?;
    write_diff_output(out, &mut rendered)?;
    Ok(!changes.is_empty())
}

/// Print `changes` between the trees `old` and `new` in `format`.
///
/// Attributes come from both trees, never from the working index, and are checked conservatively:
/// `-diff` and `binary` print a binary change, and any diff driver or text conversion refuses.
pub(crate) fn render(
    repo: &gix::Repository,
    out: &mut dyn std::io::Write,
    changes: &[Change],
    (old, new): (Option<ObjectId>, Option<ObjectId>),
    format: Format,
) -> Result<()> {
    let quote_high = quote_path(repo)?;
    match format {
        Format::Quiet => return Ok(()),
        Format::NameOnly | Format::NameStatus => {
            for change in changes {
                check_interrupted()?;
                write_name(out, format, &change.path, change.old, change.new, quote_high)?;
            }
            return Ok(());
        }
        Format::Patch | Format::Stat | Format::NumStat => {}
    }
    if changes.is_empty() {
        return Ok(());
    }
    check_external_diff(repo)?;
    let index = |tree: Option<ObjectId>| -> Result<gix::index::State> {
        Ok(match tree {
            Some(tree) => repo.index_from_tree(&tree)?.into(),
            None => gix::index::State::new(repo.object_hash()),
        })
    };
    let (old_index, new_index) = (index(old)?, index(new)?);
    let source = gix::worktree::stack::state::attributes::Source::IdMapping;
    let mut old_attrs = repo.attributes_only(&old_index, source)?;
    let mut new_attrs = repo.attributes_only(&new_index, source)?;
    let budget = Budget::new(&std::env::temp_dir()).or_error()?;
    let limit = repo.big_file_threshold()?;
    let mut stats: Vec<FileStat> = Vec::new();
    for change in changes {
        check_interrupted()?;
        let mut binary = false;
        for attrs in [&mut old_attrs, &mut new_attrs] {
            let mut matches = attrs.attribute_matches();
            attrs
                .at_entry(change.path.as_bstr(), None)?
                .matching_attributes(&mut matches);
            binary |= binary_attribute(&matches, &change.path)?;
        }
        let old = change
            .old
            .map(|side| load_side(repo, side, &budget, limit))
            .transpose()?;
        let new = change
            .new
            .map(|side| load_side(repo, side, &budget, limit))
            .transpose()?;
        if format == Format::Patch {
            render_patch(repo, out, &change.path, old.as_ref(), new.as_ref(), binary)?;
        } else {
            collect_stat(repo, &mut stats, &change.path, old.as_ref(), new.as_ref(), binary)?;
        }
    }
    match format {
        Format::Stat => render_stats(&stats, out),
        Format::NumStat => render_numstat(&stats, out),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn ranges_are_recognised_by_their_dots() {
        assert!(super::is_range(b"a..b"));
        assert!(super::is_range(b"a...b"));
        assert!(!super::is_range(b"a.b"));
    }
}
