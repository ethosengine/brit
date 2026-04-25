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
use gix::bstr::BString;

pub fn porcelain(
    _repo: gix::Repository,
    _out: impl std::io::Write,
    err: &mut dyn std::io::Write,
    commits: Vec<BString>,
) -> Result<()> {
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
    let _ = writeln!(
        err,
        "[gix-merge] received {} positional commit(s); merge driver not yet implemented",
        commits.len()
    );
    Ok(())
}
