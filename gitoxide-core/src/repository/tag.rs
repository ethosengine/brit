use std::{collections::HashSet, ffi::OsString};

use gix::{
    bstr::{BString, ByteSlice},
    prelude::ObjectIdExt,
};

use crate::OutputFormat;

/// Options driving `tag::list`. Mirrors the filter-and-format subset
/// of git-tag's listing mode from vendor/git/builtin/tag.c cmd_tag.
#[derive(Debug, Default)]
pub struct ListOptions {
    /// Shell-glob patterns (fnmatch(3), OR'd). Empty = match everything.
    pub patterns: Vec<OsString>,
    /// When true, pattern-match and sort are case-insensitive
    /// (`-i`/`--ignore-case`).
    pub ignore_case: bool,
    /// When `Some`, only list tags that point (after peeling tag
    /// chains) at the resolved object. Git's `--points-at <object>`;
    /// `None` means "no filter".
    pub points_at: Option<OsString>,
    /// When `Some`, only keep tags whose tagged commit is reachable
    /// from the resolved commit. Git's `--merged <commit>`.
    pub merged: Option<OsString>,
    /// When `Some`, only keep tags whose tagged commit is NOT
    /// reachable from the resolved commit. Git's `--no-merged <commit>`.
    pub no_merged: Option<OsString>,
}

fn ancestors_of(repo: &gix::Repository, rev: &OsString) -> anyhow::Result<HashSet<gix::ObjectId>> {
    let rev_bstr = gix::path::os_str_into_bstr(rev)?;
    let oid = repo.rev_parse_single(rev_bstr)?.detach();
    let mut set = HashSet::new();
    set.insert(oid);
    for res in oid.attach(repo).ancestors().all()? {
        let info = res?;
        set.insert(info.id);
    }
    Ok(set)
}

/// git-compat `tag` listing: one shortened refname per line, sorted
/// lexicographically by refname. Matches git's default format
/// `%(refname:strip=2)` from `git tag` / `git tag -l` with no
/// `--format` override (see vendor/git/builtin/tag.c list_tags and
/// vendor/git/Documentation/git-tag.adoc OPTIONS/`--format`).
///
/// `opts.patterns` are fnmatch(3)-style shell globs; a ref is shown
/// if its shortened name matches any pattern (OR), or unconditionally
/// when the list is empty. Matches the `[<pattern>...]` positional of
/// `git tag -l`.
pub fn list(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    format: OutputFormat,
    opts: ListOptions,
) -> anyhow::Result<()> {
    if format != OutputFormat::Human {
        anyhow::bail!("JSON output isn't supported");
    }

    let platform = repo.references()?;

    // Resolve --points-at once up front to an ObjectId. The filter
    // then keeps tags whose peeled target equals this oid.
    let points_at_oid: Option<gix::ObjectId> = match opts.points_at.as_deref() {
        Some(rev) => {
            let rev_bstr = gix::path::os_str_into_bstr(rev)?;
            Some(repo.rev_parse_single(rev_bstr)?.detach())
        }
        None => None,
    };

    // Precompute ancestor sets for --merged / --no-merged. Each is the
    // closure-of-ancestors of the resolved commit, including the commit
    // itself. A tag is kept iff its peeled commit is/is not in the set.
    let merged_set = opts.merged.as_ref().map(|rev| ancestors_of(&repo, rev)).transpose()?;
    let no_merged_set = opts
        .no_merged
        .as_ref()
        .map(|rev| ancestors_of(&repo, rev))
        .transpose()?;

    let mut names: Vec<BString> = platform
        .tags()?
        .flatten()
        .filter_map(|mut reference| {
            let peeled = if points_at_oid.is_some() || merged_set.is_some() || no_merged_set.is_some() {
                Some(reference.peel_to_id().ok()?.detach())
            } else {
                None
            };

            if let (Some(target_oid), Some(peeled)) = (points_at_oid, peeled) {
                if peeled != target_oid {
                    return None;
                }
            }
            if let (Some(set), Some(peeled)) = (merged_set.as_ref(), peeled) {
                if !set.contains(&peeled) {
                    return None;
                }
            }
            if let (Some(set), Some(peeled)) = (no_merged_set.as_ref(), peeled) {
                if set.contains(&peeled) {
                    return None;
                }
            }

            Some(reference.name().shorten().to_owned())
        })
        .collect();

    if opts.ignore_case {
        names.sort_by_key(|a| a.to_ascii_lowercase());
    } else {
        names.sort();
    }

    let patterns: Vec<BString> = opts
        .patterns
        .iter()
        .map(|p| gix::path::os_str_into_bstr(p).map(BString::from))
        .map(|res| {
            res.map(|bs| {
                if opts.ignore_case {
                    bs.to_ascii_lowercase().into()
                } else {
                    bs
                }
            })
        })
        .collect::<Result<_, _>>()?;

    let wildmatch_mode = if opts.ignore_case {
        gix::glob::wildmatch::Mode::IGNORE_CASE
    } else {
        gix::glob::wildmatch::Mode::empty()
    };

    for name in &names {
        if !patterns.is_empty()
            && !patterns
                .iter()
                .any(|pat| gix::glob::wildmatch(pat.as_ref(), name.as_ref(), wildmatch_mode))
        {
            continue;
        }
        writeln!(out, "{name}", name = name.to_str_lossy())?;
    }

    Ok(())
}
