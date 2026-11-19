//! `brit rev-list --count`: Git's spelling of counting what `brit revision list` walks.
//!
//! `--count <rev>…` prints the number of commits reachable from the included revisions and from none
//! of the excluded ones, where a revision is `A`, `^A`, `A..B` or `A...B` (both sides, without their
//! merge bases). `--left-right --count` prints `<left>\t<right>`: commits reachable from the left side
//! of an `A...B`, and the rest. Listing commits without `--count` is refused and points to Brit's verb.
//!
//! ### Known differences from Git
//!
//! - A revision that names no commit, like `HEAD^{tree}`, fails with 128; Git counts it as 0.

use std::io::Write;

use gix::{Result, bstr::BStr, error::ResultExt};

use super::exit;

/// What `rev-list` was asked for.
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    /// `--count`
    pub count: bool,
    /// `--left-right`
    pub left_right: bool,
}

/// Count commits like `git rev-list --count [--left-right] <rev>…`.
pub fn rev_list(
    repo: gix::Repository,
    revs: Vec<String>,
    mut out: impl Write,
    mut err: impl Write,
    options: Options,
) -> Result<()> {
    if revs.is_empty() {
        writeln!(err, "usage: brit rev-list --count [--left-right] <revision>...").ok();
        return Err(exit(129));
    }
    if !options.count {
        writeln!(
            err,
            "brit rev-list only counts commits (`--count`); list them with `brit revision list <rev>` or `brit log`"
        )
        .ok();
        return Err(exit(128));
    }

    let source = gix::object::bounded::Source::deferred(&repo);
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut hidden = Vec::new();
    for rev in &revs {
        let unknown = |err: &mut dyn Write| -> gix::Error {
            writeln!(
                err,
                "fatal: ambiguous argument '{rev}': unknown revision or path not in the working tree.\nUse '--' to separate paths from revisions, like this:\n'git <command> [<revision>...] -- [<file>...]'"
            )
            .ok();
            exit(128)
        };
        let Ok(spec) = repo
            .rev_parse(BStr::new(rev.as_bytes()))
            .map(gix::revision::Spec::detach)
        else {
            return Err(unknown(&mut err));
        };
        let mut commit = |id: gix::ObjectId| -> Result<gix::ObjectId> {
            source.peel(id, gix::object::Kind::Commit).map_err(|_| {
                writeln!(err, "fatal: '{rev}' does not name a commit").ok();
                exit(128)
            })
        };
        use gix::revision::plumbing::Spec;
        match spec {
            Spec::Include(id) => right.push(commit(id)?),
            Spec::Exclude(id) => hidden.push(commit(id)?),
            Spec::Range { from, to } => {
                hidden.push(commit(from)?);
                right.push(commit(to)?);
            }
            Spec::Merge { theirs, ours } => {
                let (theirs, ours) = (commit(theirs)?, commit(ours)?);
                left.push(theirs);
                right.push(ours);
                hidden.extend(repo.merge_bases_many(theirs, &[ours])?.into_iter().map(gix::Id::detach));
            }
            _ => {
                writeln!(
                    err,
                    "brit rev-list does not implement `{rev}`; resolve it with `brit revision resolve`"
                )
                .ok();
                return Err(exit(128));
            }
        }
    }

    let count = |tips: Vec<gix::ObjectId>| -> Result<usize> {
        if tips.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        for commit in repo
            .rev_walk(tips)
            .sorting(gix::revision::walk::Sorting::ByCommitTime(Default::default()))
            .with_hidden(hidden.iter().copied())
            .all()?
        {
            commit?;
            count += 1;
        }
        Ok(count)
    };
    let left_count = count(left.clone())?;
    let total = count(left.into_iter().chain(right).collect())?;
    if options.left_right {
        writeln!(out, "{left_count}\t{}", total - left_count).or_error()?;
    } else {
        writeln!(out, "{total}").or_error()?;
    }
    Ok(())
}
