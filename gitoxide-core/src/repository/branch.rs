use crate::OutputFormat;

pub mod list {
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

    if show_local {
        let mut branch_names: Vec<String> = platform
            .local_branches()?
            .flatten()
            .map(|branch| branch.name().shorten().to_string())
            .filter(|name| match_name(name))
            .collect();

        branch_names.sort();

        for branch_name in branch_names {
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
        let mut branch_names: Vec<String> = platform
            .remote_branches()?
            .flatten()
            .map(|branch| branch.name().shorten().to_string())
            .filter(|name| match_name(name))
            .collect();

        branch_names.sort();

        for branch_name in branch_names {
            if include_prefix {
                writeln!(out, "  remotes/{branch_name}")?;
            } else {
                writeln!(out, "  {branch_name}")?;
            }
        }
    }

    Ok(())
}
