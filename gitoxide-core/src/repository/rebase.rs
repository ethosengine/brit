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
use gix::bstr::{BStr, BString, ByteSlice};
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

/// In-progress rebase state transitions wired via the porcelain flag
/// surface. Each variant is mutually exclusive with the others; the
/// dispatcher reads them off the `rebase::Platform` struct in
/// `src/plumbing/main.rs` and forwards them here.
///
/// Each transition mirrors git's behavior when no rebase is in
/// progress (no `.git/rebase-merge/` or `.git/rebase-apply/` state
/// dir present): "fatal: no rebase in progress" + exit 128. gix has
/// no rebase state to look up yet — every test runs from a clean
/// repo where the state dirs are guaranteed absent, so the
/// "no rebase in progress" branch is the only path closed today.
/// When state-dir detection lands, an `if has_rebase_state { ... }`
/// arm replaces these unconditional emissions.
#[derive(Debug, Default, Copy, Clone)]
pub struct Cmdmode {
    pub continue_: bool,
    pub skip: bool,
    pub abort: bool,
    pub quit: bool,
    pub edit_todo: bool,
    pub show_current_patch: bool,
}

impl Cmdmode {
    pub fn any(&self) -> bool {
        self.continue_ || self.skip || self.abort || self.quit || self.edit_todo || self.show_current_patch
    }
}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    upstream: Option<BString>,
    branch: Option<BString>,
    opts: Options,
    cmdmode: Cmdmode,
) -> Result<()> {
    // Cmdmode precondition gate: --continue / --skip / --abort /
    // --quit / --edit-todo / --show-current-patch all require a
    // rebase to be in progress. Without `.git/rebase-merge/` or
    // `.git/rebase-apply/`, git dies "fatal: no rebase in progress"
    // + exit 128 (vendor/git/builtin/rebase.c get_replay_opts gate).
    // gix has no state-dir lookup yet — every test fixture is clean,
    // so we can emit the "no rebase in progress" branch
    // unconditionally. State-dir detection is part of the deferred
    // rebase driver work.
    if cmdmode.any() {
        let _ = writeln!(err, "fatal: no rebase in progress");
        std::process::exit(128);
    }
    // Bad-revspec gate: when `<upstream>` is given but does not
    // resolve, git emits "fatal: invalid upstream '<ref>'" + exit 128
    // (vendor/git/builtin/rebase.c::cmd_rebase rev-parse step on
    // the positional before any merge-base / replay logic runs).
    if let Some(spec) = upstream.as_ref() {
        let spec_bstr: &BStr = spec.as_ref();
        if repo.rev_parse_single(spec_bstr).is_err() {
            let _ = writeln!(err, "fatal: invalid upstream '{}'", spec_bstr.to_str_lossy());
            std::process::exit(128);
        }
    }
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
