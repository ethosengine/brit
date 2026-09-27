//! Bare-form `gix restore` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/checkout.c::cmd_restore`
//! (`vendor/git/builtin/checkout.c:2128..2162`) — the porcelain shape
//! that restores working-tree files from a tree-ish (or the index when
//! `--staged` is set without `--source`). Currently a placeholder:
//! every flag-bearing parity row in `tests/journey/parity/restore.sh`
//! closes with `compat_effect "deferred until restore driver lands"`
//! until the real driver lands.
//!
//! Behaviour:
//!
//! * Outside a repository: handled by the shared `repository(Mode::
//!   Lenient)` glue in `src/plumbing/main.rs`, which remaps
//!   `gix_discover::upwards::Error::NoGitRepository*` to git's exact
//!   "fatal: not a git repository (or any of the parent directories):
//!   .git" + exit 128.
//!
//! * Otherwise: walk the cmd_restore precondition matrix that doesn't
//!   require the checkout machinery. cmd_restore sets
//!   `accept_pathspec=1, empty_pathspec_ok=0` at
//!   `vendor/git/builtin/checkout.c:2148..2155`, so checkout_main's
//!   pathspec gate fires when no positional / `--pathspec-from-file`
//!   source is supplied:
//!     - "fatal: you must specify path(s) to restore" + exit 128
//!       (mirrors checkout.c's `accept_pathspec` empty-pathspec die).
//!
//!   The pathspec-source mutual-exclusion gates from
//!   `vendor/git/parse-options.c::parse_opt_pathspec_from_file` and
//!   `parse_opt_pathspec_file_nul` (`--pathspec-file-nul` requires
//!   `--pathspec-from-file`) are mirrored verbatim.
//!
//! On the happy path emit a single-line stub note on stdout and exit
//! 0 so `compat_effect`-mode rows match git's exit code while the
//! real restore driver is unimplemented.
//!
//! Bytes parity on the happy path (real `--source` tree-ish parsing,
//! `--staged`/`--worktree` index/worktree update split, `--ignore-
//! unmerged` unmerged-skip, `--overlay`/`--no-overlay` tracked-file
//! removal, `--ours`/`--theirs` stage-pick for unmerged paths,
//! `--merge`/`--conflict` three-way merge, `--patch` interactive
//! hunk select, `--ignore-skip-worktree-bits` sparse override,
//! `--pathspec-from-file` parser, and `--recurse-submodules` submodule
//! tree update) is deferred until the restore driver lands. The shared
//! deferral phrase is `"deferred until restore driver lands"`.

use anyhow::Result;
use gix::bstr::BString;

/// Subset of `restore::Platform` flags consumed by the porcelain stub.
#[derive(Debug, Default)]
pub struct Options {
    pub source: Option<String>,
    pub staged: bool,
    pub worktree: bool,
    pub ignore_unmerged: bool,
    pub overlay: bool,
    pub no_overlay: bool,
    pub quiet: bool,
    pub recurse_submodules: Option<String>,
    pub no_recurse_submodules: bool,
    pub progress: bool,
    pub no_progress: bool,
    pub merge: bool,
    pub conflict: Option<String>,
    pub ours: bool,
    pub theirs: bool,
    pub patch: bool,
    pub unified: Option<u32>,
    pub inter_hunk_context: Option<u32>,
    pub ignore_skip_worktree_bits: bool,
    pub pathspec_from_file: Option<BString>,
    pub pathspec_file_nul: bool,
}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    args: Vec<BString>,
    paths: Vec<BString>,
    opts: Options,
) -> Result<()> {
    let pathspec_present = !args.is_empty() || !paths.is_empty();

    // 1. `--pathspec-from-file` and pathspec arguments are mutually
    //    exclusive (mirrors vendor/git/parse-options.c::
    //    parse_opt_pathspec_from_file).
    if opts.pathspec_from_file.is_some() && pathspec_present {
        let _ = writeln!(
            err,
            "fatal: '--pathspec-from-file' and pathspec arguments cannot be used together"
        );
        std::process::exit(128);
    }

    // 2. `--pathspec-file-nul` requires `--pathspec-from-file`
    //    (mirrors vendor/git/parse-options.c::
    //    parse_opt_pathspec_file_nul).
    if opts.pathspec_file_nul && opts.pathspec_from_file.is_none() {
        let _ = writeln!(
            err,
            "fatal: the option '--pathspec-file-nul' requires '--pathspec-from-file'"
        );
        std::process::exit(128);
    }

    // 3. `accept_pathspec=1, empty_pathspec_ok=0` (vendor/git/builtin/
    //    checkout.c:2148..2150) means checkout_main dies when no
    //    pathspec source is supplied. The wording is git's standard
    //    "you must specify path(s) to restore" stanza.
    if !pathspec_present && opts.pathspec_from_file.is_none() {
        let _ = writeln!(err, "fatal: you must specify path(s) to restore");
        std::process::exit(128);
    }

    // Happy path placeholder: emit a stub note so the shape of stdout
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real restore driver is
    // unimplemented.
    let _ = writeln!(
        out,
        "[gix-restore] git_dir={} args={args:?} paths={paths:?} source={:?} staged={} worktree={} patch={}; restore driver not yet implemented",
        repo.git_dir().display(),
        opts.source,
        opts.staged,
        opts.worktree,
        opts.patch,
    );
    Ok(())
}
