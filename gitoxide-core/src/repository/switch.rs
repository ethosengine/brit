//! Bare-form `gix switch` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/checkout.c::cmd_switch`
//! (`vendor/git/builtin/checkout.c:2089`) — the porcelain shape that
//! switches the working tree and the index to the given branch.
//! Currently a placeholder: the happy-path rows in
//! `tests/journey/parity/switch.sh` close with
//! `compat_effect "deferred until switch driver lands"` until the real
//! driver lands.
//!
//! Behaviour:
//!
//! * Outside a repository: handled by the shared `repository(Mode::
//!   Lenient)` glue in `src/plumbing/main.rs`, which remaps
//!   `gix_discover::upwards::Error::NoGitRepository*` to git's exact
//!   "fatal: not a git repository (or any of the parent directories):
//!   .git" + exit 128.
//!
//! * Otherwise: walk the cmd_switch entry-point preconditions that
//!   don't require the checkout machinery:
//!     - No positional and no `-c`/`-C`/`--orphan`/`--detach`: emit
//!       `fatal: missing branch or commit argument` + exit 128
//!       (mirrors checkout_main's argument-count gate when
//!       `accept_ref=1` / `accept_pathspec=0` — see
//!       `vendor/git/builtin/checkout.c:1783..` `checkout_main` /
//!       `parse_branchname_arg`).
//!
//! On the happy path emit a single-line stub note on stdout and exit
//! 0 so `compat_effect`-mode rows match git's exit code while the
//! real switch driver is unimplemented.
//!
//! Bytes parity on the happy path (real ref resolution, three-way
//! merge under `--merge`, branch creation under `-c`/`-C`/`--orphan`,
//! detach under `--detach`, tracking-config under `-t`/`--track`,
//! progress reporting, recurse-submodules) is deferred until the
//! switch driver lands. The shared deferral phrase is
//! `"deferred until switch driver lands"`.

use anyhow::Result;
use gix::bstr::BString;

/// Subset of `switch::Platform` flags consumed by the porcelain stub.
#[derive(Debug, Default)]
pub struct Options {
    pub create: Option<String>,
    pub force_create: Option<String>,
    pub orphan: Option<String>,
    pub detach: bool,
    pub discard_changes: bool,
    pub force: bool,
    pub merge: bool,
    pub quiet: bool,
    pub progress: bool,
    pub no_progress: bool,
    pub track: Option<String>,
    pub no_track: bool,
    pub guess: bool,
    pub no_guess: bool,
    pub conflict: Option<String>,
    pub recurse_submodules: Option<String>,
    pub no_recurse_submodules: bool,
    pub overwrite_ignore: bool,
    pub no_overwrite_ignore: bool,
    pub ignore_other_worktrees: bool,
}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    args: Vec<BString>,
    opts: Options,
) -> Result<()> {
    // No positional + no -c/-C/--orphan/--detach: git dies 128 with
    // "fatal: missing branch or commit argument" — mirrors
    // checkout_main's argument-count gate at the top of
    // vendor/git/builtin/checkout.c:1783 when accept_ref=1 and no
    // create/orphan/detach is set.
    let has_target = opts.create.is_some()
        || opts.force_create.is_some()
        || opts.orphan.is_some()
        || opts.detach
        || !args.is_empty();
    if !has_target {
        let _ = writeln!(err, "fatal: missing branch or commit argument");
        std::process::exit(128);
    }

    // Happy path placeholder: emit a stub note so the shape of stdout
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real switch driver is
    // unimplemented.
    let _ = writeln!(
        out,
        "[gix-switch] git_dir={} args={args:?} create={:?} force_create={:?} orphan={:?} detach={} discard_changes={} merge={}; switch driver not yet implemented",
        repo.git_dir().display(),
        opts.create,
        opts.force_create,
        opts.orphan,
        opts.detach,
        opts.discard_changes,
        opts.merge,
    );
    Ok(())
}
