//! Bare-form `gix rebase` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/rebase.c::cmd_rebase` — the porcelain
//! shape that transplants a series of commits onto a different
//! starting point. Currently a placeholder: every flag-bearing parity
//! row in `tests/journey/parity/rebase.sh` closes with
//! `compat_effect "deferred until rebase driver lands"` until the
//! real driver lands.
//!
//! Behaviour:
//!
//! * Bare-no-args + no upstream configured for HEAD: emit git's exact
//!   "There is no tracking information for the current branch."
//!   stanza on **stdout** (not stderr — verified empirically and
//!   confirmed by `printf` in `vendor/git/builtin/rebase.c:1058..1066`,
//!   `error_on_missing_default_upstream`) and exit 1.
//!
//!   The stanza is split into two variants by
//!   `error_on_missing_default_upstream` at `builtin/rebase.c:1054`:
//!     - on a branch (`current_branch != NULL`) → "There is no
//!       tracking information for the current branch." + the
//!       trailing `git branch --set-upstream-to=...` hint, with
//!       remote name falling back to "<remote>" when none is set;
//!     - detached HEAD (`current_branch == NULL`) → "You are not
//!       currently on a branch." + no trailing hint.
//!
//! * Otherwise: emit a single-line stub note on stderr and exit 0 so
//!   `compat_effect`-mode rows match git's exit code while the real
//!   rebase driver is unimplemented.
//!
//! Bytes parity (real revision-walk + cherry-pick replay, todo-list
//! emission, --abort/--quit/--continue state transitions, AUTO_MERGE
//! / REBASE_HEAD / `.git/rebase-merge/` / `.git/rebase-apply/`
//! ref+state writes) is deferred until the rebase driver lands. The
//! shared deferral phrase is `"deferred until rebase driver lands"`.

use anyhow::Result;
use gix::bstr::{BString, ByteSlice};
use gix::remote::Direction;

/// Subset of `rebase::Platform` flags consumed by the porcelain stub.
///
/// Today the stub only consults the positionals to decide between the
/// bare-no-upstream stanza and the placeholder happy path. Once the
/// real rebase driver lands the rest of the flag surface gets
/// threaded in here.
#[derive(Debug, Default)]
pub struct Options {
    pub onto: Option<String>,
    pub root: bool,
}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    upstream: Option<BString>,
    branch: Option<BString>,
    opts: Options,
) -> Result<()> {
    // Bare `git rebase` with no <upstream>, no --root, no --onto:
    // `cmd_rebase` falls into the default-to-upstream path; if no
    // `branch.<name>.merge` is configured it calls
    // `error_on_missing_default_upstream` (builtin/rebase.c:1054)
    // which `printf`s the canonical stanza to **stdout** and exits 1.
    if upstream.is_none() && branch.is_none() && opts.onto.is_none() && !opts.root {
        let head_ref = repo.head_ref()?;
        let upstream_ref = head_ref.as_ref().and_then(|r| r.remote_ref_name(Direction::Fetch));
        let upstream_present = matches!(upstream_ref, Some(Ok(_)));
        if !upstream_present {
            // Two variants: on-a-branch (with `--set-upstream-to`
            // hint) vs. detached HEAD (no hint). The hint's remote
            // name falls back to "<remote>" when none is configured.
            let head_short: Option<BString> = repo.head_name()?.map(|name| name.shorten().to_owned());
            match head_short {
                Some(branch_name) => {
                    let branch_str = branch_name.to_str_lossy();
                    let _ = writeln!(out, "There is no tracking information for the current branch.");
                    let _ = writeln!(out, "Please specify which branch you want to rebase against.");
                    let _ = writeln!(out, "See git-rebase(1) for details.");
                    let _ = writeln!(out);
                    let _ = writeln!(out, "    git rebase '<branch>'");
                    let _ = writeln!(out);
                    let _ = writeln!(
                        out,
                        "If you wish to set tracking information for this branch you can do so with:"
                    );
                    let _ = writeln!(out);
                    let _ = writeln!(out, "    git branch --set-upstream-to=<remote>/<branch> {branch_str}");
                    let _ = writeln!(out);
                }
                None => {
                    let _ = writeln!(out, "You are not currently on a branch.");
                    let _ = writeln!(out, "Please specify which branch you want to rebase against.");
                    let _ = writeln!(out, "See git-rebase(1) for details.");
                    let _ = writeln!(out);
                    let _ = writeln!(out, "    git rebase '<branch>'");
                    let _ = writeln!(out);
                }
            }
            std::process::exit(1);
        }
    }
    // Happy path placeholder: emit a stub note so the shape of stderr
    // is recognizable in failures, then exit 0 so `compat_effect`
    // rows match git's exit code while the real rebase driver is
    // unimplemented.
    let _ = writeln!(
        err,
        "[gix-rebase] received upstream={:?} branch={:?} onto={:?} root={}; rebase driver not yet implemented",
        upstream
            .as_deref()
            .map_or_else(|| "<upstream>".into(), |b| b.to_str_lossy().into_owned()),
        branch
            .as_deref()
            .map_or_else(|| "<HEAD>".into(), |b| b.to_str_lossy().into_owned()),
        opts.onto.as_deref().unwrap_or("<unset>"),
        opts.root,
    );
    Ok(())
}
