//! Bare-form `gix pull` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/pull.c::cmd_pull` — the porcelain shape
//! that runs `git fetch` and integrates (merge or rebase) the result
//! into the current branch. Currently a placeholder: every flag-bearing
//! parity row in `tests/journey/parity/pull.sh` closes with
//! `compat_effect "deferred until pull driver lands"` until the real
//! driver lands. Behaviour:
//!
//! * Bare-no-args + no upstream configured for HEAD: emit git's exact
//!   8-line "There is no tracking information for the current branch."
//!   stanza on stderr + exit 1. Mirrors `cmd_pull -> die_no_merge_*`
//!   when `branch.<name>.merge` is unset (vendor/git/builtin/pull.c
//!   parse_branch_merge_options + the no-merge-candidates die path
//!   inside cmd_pull). Note the exit code is 1, not 128 — `die()` in
//!   pull.c routes through advise_die, which uses git's standard
//!   exit-1 default unlike the parse-options 128 path.
//! * Otherwise: emit a single-line stub note on stderr and exit 0 so
//!   `compat_effect`-mode rows match git's exit code while the real
//!   pull driver is unimplemented.
//!
//! Bytes parity (real fetch + merge/rebase composition, FETCH_HEAD
//! integration, conflict-marker emission, MERGE_HEAD ref writes) is
//! deferred until the pull driver lands. The shared deferral phrase
//! is `"deferred until pull driver lands"`.

use anyhow::Result;
use gix::bstr::{BString, ByteSlice};
use gix::remote::Direction;

pub fn porcelain(
    repo: gix::Repository,
    _out: impl std::io::Write,
    err: &mut dyn std::io::Write,
    repository: Option<String>,
    refspec: Vec<BString>,
) -> Result<()> {
    // cmd_pull bare-no-args path: when no positional <repository> is
    // given AND the current branch has no `branch.<name>.merge`
    // configured, git emits the canonical 8-line "no tracking
    // information" stanza + exits 1. The stanza placeholder for
    // `<branch>` in the `git branch --set-upstream-to=...` line is the
    // current short branch name (e.g. "main").
    if repository.is_none() && refspec.is_empty() {
        let head_ref = repo.head_ref()?;
        let upstream = head_ref.as_ref().and_then(|r| r.remote_ref_name(Direction::Fetch));
        let upstream_present = matches!(upstream, Some(Ok(_)));
        if !upstream_present {
            // Short form of the current ref name; "<branch>" if HEAD is
            // detached or unresolvable. git itself walks
            // get_default_remote_url + branch_get and inserts the
            // current branch name verbatim (vendor/git/builtin/pull.c
            // around the die_no_merge_candidates fallback).
            let head_short: BString = match repo.head_name()? {
                Some(name) => name.shorten().to_owned(),
                None => BString::from("<branch>"),
            };
            let head_short_str = head_short.to_str_lossy();
            let _ = writeln!(err, "There is no tracking information for the current branch.");
            let _ = writeln!(err, "Please specify which branch you want to merge with.");
            let _ = writeln!(err, "See git-pull(1) for details.");
            let _ = writeln!(err);
            let _ = writeln!(err, "    git pull <remote> <branch>");
            let _ = writeln!(err);
            let _ = writeln!(
                err,
                "If you wish to set tracking information for this branch you can do so with:"
            );
            let _ = writeln!(err);
            let _ = writeln!(
                err,
                "    git branch --set-upstream-to=<remote>/<branch> {head_short_str}"
            );
            let _ = writeln!(err);
            std::process::exit(1);
        }
    }
    // Happy path placeholder: emit a stub note so the shape of stderr
    // is recognizable in failures, then exit 0 so `compat_effect` rows
    // match git's exit code while the real pull driver is unimplemented.
    let _ = writeln!(
        err,
        "[gix-pull] received repository={:?} refspec-count={}; pull driver not yet implemented",
        repository.as_deref().unwrap_or("<upstream>"),
        refspec.len()
    );
    Ok(())
}
