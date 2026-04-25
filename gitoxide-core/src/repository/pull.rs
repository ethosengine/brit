//! Bare-form `gix pull` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/pull.c::cmd_pull` — the porcelain shape
//! that runs `git fetch` and integrates (merge or rebase) the result
//! into the current branch. Currently a placeholder: every flag-bearing
//! parity row in `tests/journey/parity/pull.sh` closes with
//! `compat_effect "deferred until pull driver lands"` until the real
//! driver lands. Behaviour:
//!
//! * Emit a single-line stub note on stderr identifying what was
//!   requested (zero / one / many positional args) and exit 0 so
//!   `compat_effect`-mode rows match git's exit code.
//!
//! Bytes parity (real fetch + merge/rebase composition, FETCH_HEAD
//! integration, conflict-marker emission, MERGE_HEAD ref writes, plus
//! the bare-no-upstream "There is no tracking information ..." 128
//! stanza) is deferred until the pull driver lands. The shared deferral
//! phrase is `"deferred until pull driver lands"`.

use anyhow::Result;
use gix::bstr::BString;

pub fn porcelain(
    _repo: gix::Repository,
    _out: impl std::io::Write,
    err: &mut dyn std::io::Write,
    repository: Option<String>,
    refspec: Vec<BString>,
) -> Result<()> {
    let _ = writeln!(
        err,
        "[gix-pull] received repository={:?} refspec-count={}; pull driver not yet implemented",
        repository.as_deref().unwrap_or("<upstream>"),
        refspec.len()
    );
    Ok(())
}
