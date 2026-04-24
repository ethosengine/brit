use gix::prelude::ObjectIdExt;

use crate::OutputFormat;

/// Return `true` iff `haystack_commit` has `needle` anywhere in its
/// ancestry (walking parents). Inclusive — a commit contains itself.
/// Mirrors git's `commit_contains` semantic used for branch/tag
/// `--contains` filtering.
fn commit_contains(
    repo: &gix::Repository,
    haystack_commit: gix::ObjectId,
    needle: gix::ObjectId,
) -> anyhow::Result<bool> {
    if haystack_commit == needle {
        return Ok(true);
    }
    for res in haystack_commit.attach(repo).ancestors().all()? {
        if res?.id == needle {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `git branch --show-current`: print the current branch name alone,
/// or nothing in detached `HEAD` state. Matches builtin/branch.c
/// show_current behavior — no asterisk, no indent, no newline on
/// detached. Mirrors repo.head_name(): `None` when detached / unborn
/// → no output; `Some` → shortened name followed by `\n`.
pub fn show_current(repo: gix::Repository, out: &mut dyn std::io::Write) -> anyhow::Result<()> {
    if let Some(name) = repo.head_name()? {
        writeln!(out, "{}", name.shorten())?;
    }
    Ok(())
}

pub mod list {
    use std::ffi::OsString;

    use gix::bstr::BString;

    pub enum Kind {
        Local,
        Remote,
        All,
    }

    pub struct Options {
        pub kind: Kind,
        /// Shell-glob patterns (fnmatch(3), OR'd). Empty = match
        /// everything. Matches `git branch [<pattern>...]`.
        pub patterns: Vec<BString>,
        /// If set, only list branches whose tip has `<commit>` in its
        /// ancestry (inclusive). Git's `--contains`.
        pub contains: Option<OsString>,
    }
}

pub fn list(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    format: OutputFormat,
    options: list::Options,
) -> anyhow::Result<()> {
    if format != OutputFormat::Human {
        anyhow::bail!("JSON output isn't supported");
    }

    let platform = repo.references()?;

    let (show_local, show_remotes) = match options.kind {
        list::Kind::Local => (true, false),
        list::Kind::Remote => (false, true),
        list::Kind::All => (true, true),
    };

    // Current-branch marker. None when HEAD is detached or unborn; in
    // both cases no local row is decorated with `*`. Matches git's
    // print_ref_list() which compares refs against head_ref.
    let head_short = repo.head_name()?.map(|name| name.shorten().to_string());

    // Pattern filter: fnmatch(3)-style shell globs OR'd; empty =
    // match everything. Matches `for_each_fullref_in_pattern` in
    // vendor/git/refs.c called from builtin/branch.c's filter_refs.
    let patterns = &options.patterns;
    let match_name = |name: &str| -> bool {
        patterns.is_empty()
            || patterns
                .iter()
                .any(|pat| gix::glob::wildmatch(pat.as_ref(), name.into(), gix::glob::wildmatch::Mode::empty()))
    };

    // --contains filter: resolve the needle once up-front so each
    // branch's ancestor walk compares against the same ObjectId.
    let contains_oid: Option<gix::ObjectId> = match options.contains.as_deref() {
        Some(rev) => {
            let rev_bstr = gix::path::os_str_into_bstr(rev)?;
            Some(repo.rev_parse_single(rev_bstr)?.detach())
        }
        None => None,
    };

    let tip_of = |reference: &gix::Reference<'_>| -> anyhow::Result<gix::ObjectId> {
        Ok(reference.clone().into_fully_peeled_id()?.detach())
    };

    let contains_keep = |tip: gix::ObjectId| -> anyhow::Result<bool> {
        contains_oid.map_or(Ok(true), |needle| commit_contains(&repo, tip, needle))
    };

    if show_local {
        let mut kept: Vec<String> = Vec::new();
        for reference in platform.local_branches()?.flatten() {
            let name = reference.name().shorten().to_string();
            if !match_name(&name) {
                continue;
            }
            if contains_oid.is_some() {
                let tip = tip_of(&reference)?;
                if !contains_keep(tip)? {
                    continue;
                }
            }
            kept.push(name);
        }
        kept.sort();
        for branch_name in kept {
            let marker = if Some(&branch_name) == head_short.as_ref() {
                "* "
            } else {
                "  "
            };
            writeln!(out, "{marker}{branch_name}")?;
        }
    }

    if show_remotes {
        // When --all pairs locals + remotes, git disambiguates the
        // remote rows with a `remotes/` prefix (see
        // vendor/git/builtin/branch.c: REF_REMOTE_BRANCH vs
        // REF_LOCAL_BRANCH filter.kind → ref_array_item's refname is
        // used verbatim after `refs/` strip, so `refs/remotes/x` =>
        // `remotes/x`). With --remotes alone git instead prints the
        // shortened form `origin/main` because there is no ambiguity
        // against locals.
        let include_prefix = show_local;
        let mut kept: Vec<String> = Vec::new();
        for reference in platform.remote_branches()?.flatten() {
            let name = reference.name().shorten().to_string();
            if !match_name(&name) {
                continue;
            }
            if contains_oid.is_some() {
                let tip = tip_of(&reference)?;
                if !contains_keep(tip)? {
                    continue;
                }
            }
            kept.push(name);
        }
        kept.sort();
        for branch_name in kept {
            if include_prefix {
                writeln!(out, "  remotes/{branch_name}")?;
            } else {
                writeln!(out, "  {branch_name}")?;
            }
        }
    }

    Ok(())
}
