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
//!   pull.c routes through advise_die using git's standard exit-1
//!   default unlike the parse-options 128 path.
//!
//!   Two stanza variants come from `die_no_merge_candidates`
//!   (vendor/git/builtin/pull.c:315..366):
//!     - merge variant ("merge with", "<remote>/<branch>") for the
//!       merge integration path;
//!     - rebase variant ("rebase against", "origin/<branch>") for the
//!       rebase integration path. The "origin" hardcode reflects a
//!       git-internal side effect: under `opt_rebase`, pull.c calls
//!       get_rebase_fork_point → remote_for_branch which falls back
//!       to "origin" when no remote is configured (remote.c:668).
//!
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

/// Subset of `pull::Platform` flags consumed by the porcelain stub.
///
/// Bundles the flags that drive observable behavior in the
/// placeholder driver — rebase variant choice for the
/// bare-no-upstream stanza, --dry-run short-circuit. Once the real
/// pull driver lands, the rest of the flag surface gets threaded in
/// here and forwarded to the fetch + merge sub-invocations.
#[derive(Debug, Default)]
pub struct Options {
    pub rebase: Option<String>,
    pub no_rebase: bool,
    pub dry_run: bool,
}

/// Whether the current invocation integrates via rebase or merge.
///
/// Mirrors `vendor/git/builtin/pull.c::parse_opt_rebase` enum values
/// (REBASE_FALSE / REBASE_TRUE / REBASE_MERGES / REBASE_INTERACTIVE).
/// For the bare-no-upstream stanza we only need the binary
/// "rebase-or-not" decision.
fn rebase_active(rebase: Option<&str>, no_rebase: bool) -> bool {
    if no_rebase {
        return false;
    }
    match rebase {
        None => false,
        Some(v) => !matches!(v, "false" | "no" | "off" | "0"),
    }
}

pub fn porcelain(
    repo: gix::Repository,
    _out: impl std::io::Write,
    err: &mut dyn std::io::Write,
    repository: Option<String>,
    refspec: Vec<BString>,
    opts: Options,
) -> Result<()> {
    // cmd_pull --dry-run short-circuits before the merge-candidates
    // check (vendor/git/builtin/pull.c:1086 `if (opt_dry_run) return 0`),
    // so `git pull --dry-run` in a no-upstream repo silently exits 0
    // rather than dying with the no-tracking stanza. Mirror that
    // shape so flag-only --dry-run rows close at exit-code parity
    // even before the real fetch step is wired in.
    if opts.dry_run {
        return Ok(());
    }
    // cmd_pull bare-no-args path: when no positional <repository> is
    // given AND the current branch has no `branch.<name>.merge`
    // configured, git emits the canonical 8-line "no tracking
    // information" stanza + exits 1. Stanza variant follows opt_rebase.
    if repository.is_none() && refspec.is_empty() {
        let head_ref = repo.head_ref()?;
        let upstream = head_ref.as_ref().and_then(|r| r.remote_ref_name(Direction::Fetch));
        let upstream_present = matches!(upstream, Some(Ok(_)));
        if !upstream_present {
            let head_short: BString = match repo.head_name()? {
                Some(name) => name.shorten().to_owned(),
                None => BString::from("<branch>"),
            };
            let head_short_str = head_short.to_str_lossy();
            let is_rebase = rebase_active(opts.rebase.as_deref(), opts.no_rebase);
            // Under rebase mode git's get_rebase_fork_point side-effect
            // pre-loads remote_state, so `remote_for_branch` resolves
            // its "origin" fallback (vendor/git/remote.c:668). Under
            // merge mode no such pre-load runs and `for_each_remote`
            // in die_no_merge_candidates falls back to "<remote>".
            let remote_placeholder = if is_rebase { "origin" } else { "<remote>" };
            let action = if is_rebase { "rebase against" } else { "merge with" };
            let _ = writeln!(err, "There is no tracking information for the current branch.");
            let _ = writeln!(err, "Please specify which branch you want to {action}.");
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
                "    git branch --set-upstream-to={remote_placeholder}/<branch> {head_short_str}"
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
