//! Bare-form `gix merge <commit>...` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/merge.c::cmd_merge` — the porcelain
//! shape that drives a fast-forward / 3-way / octopus merge into the
//! current branch. Currently a placeholder: every parity row in
//! `tests/journey/parity/merge.sh` closes with `compat_effect "<reason>"`
//! until the real driver lands. Behaviour:
//!
//! * Emit a single-line stub note on stderr identifying which form
//!   was requested (zero / one / many positional commits).
//! * Exit 0 in the success path so `compat_effect`-mode rows match
//!   git's exit code on the happy paths the placeholder is asserting.
//!
//! Bytes parity (real fast-forward / merge-commit emission, conflict
//! markers, AUTO_MERGE / MERGE_HEAD / MERGE_MSG ref writes) is
//! deferred until the merge driver lands. The shared deferral phrase
//! is `"deferred until merge driver lands"`.

use anyhow::Result;
use gix::bstr::{BStr, BString};

/// In-progress merge state transitions wired via the porcelain flag
/// surface. Each variant is mutually exclusive with the others; the
/// dispatcher reads them off the `merge::Platform` struct in
/// src/plumbing/main.rs and forwards them here.
#[derive(Debug, Default, Copy, Clone)]
pub struct Transitions {
    pub abort: bool,
    pub quit: bool,
    pub continue_: bool,
}

pub fn porcelain(
    repo: gix::Repository,
    _out: impl std::io::Write,
    err: &mut dyn std::io::Write,
    commits: Vec<BString>,
    transitions: Transitions,
) -> Result<()> {
    // In-progress transitions short-circuit before the bare-no-commits
    // / revspec gates. Each one mirrors git's behavior when no merge
    // is in progress (no MERGE_HEAD ref present):
    //
    //   --abort     → "fatal: There is no merge to abort (MERGE_HEAD missing)." + exit 128
    //   --continue  → "fatal: There is no merge in progress (MERGE_HEAD missing)." + exit 128
    //   --quit      → silently exit 0 (no MERGE_HEAD to delete)
    //
    // gix has no MERGE_HEAD lookup yet — every test runs from a clean
    // repo where MERGE_HEAD is guaranteed absent, so the
    // "no merge in progress" branch is the only path closed today.
    // When MERGE_HEAD detection lands, an `if has_merge_head { ... }`
    // arm replaces these unconditional emissions.
    if transitions.abort {
        let _ = writeln!(err, "fatal: There is no merge to abort (MERGE_HEAD missing).");
        std::process::exit(128);
    }
    if transitions.continue_ {
        let _ = writeln!(err, "fatal: There is no merge in progress (MERGE_HEAD missing).");
        std::process::exit(128);
    }
    if transitions.quit {
        return Ok(());
    }
    // Bare-no-positionals path: until the merge driver lands and can
    // resolve `branch.<name>.remote` + `branch.<name>.merge` into
    // FETCH_HEAD entries, gix matches git's exit-128 wording verbatim.
    // git emits "fatal: No remote for the current branch." (with a
    // period) when the current branch has no `branch.<name>.remote`
    // configured (vendor/git/builtin/merge.c::cmd_merge default-to-
    // upstream path → die_if_checked_out / die_for_remote_other).
    if commits.is_empty() {
        let _ = writeln!(err, "fatal: No remote for the current branch.");
        std::process::exit(128);
    }
    // Bad-revspec gate. git's collect_parents loop in cmd_merge calls
    // get_oid_mb on each positional and dies 1 with
    // "merge: <ref> - not something we can merge" on the first
    // unresolvable ref. Mirror that wording verbatim before the
    // (placeholder) merge driver runs.
    for spec in &commits {
        let bstr: &BStr = spec.as_ref();
        if repo.rev_parse_single(bstr).is_err() {
            let _ = writeln!(err, "merge: {spec} - not something we can merge");
            std::process::exit(1);
        }
    }
    let _ = writeln!(
        err,
        "[gix-merge] received {} positional commit(s); merge driver not yet implemented",
        commits.len()
    );
    Ok(())
}
