//! `gix rm` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/rm.c` (entry point
//! `cmd_rm` at `vendor/git/builtin/rm.c:266`) and
//! `vendor/git/Documentation/git-rm.adoc`. Parity coverage lives in
//! `tests/journey/parity/rm.sh`.
//!
//! Synopsis (`vendor/git/builtin/rm.c:29`):
//!
//! * `git rm [-f | --force] [-n] [-r] [--cached] [--ignore-unmatch] [--quiet] [--pathspec-from-file=<file> [--pathspec-file-nul]] [--] [<pathspec>...]`
//!
//! Today the porcelain stub at `gitoxide_core::repository::rm::porcelain`
//! is a placeholder that emits a stub note + exits 0 (except for the
//! outside-of-repo gate handled by the shared `repository(Mode::
//! Lenient)` glue, which matches git's "fatal: not a git repository" +
//! exit 128, and the precondition matrix mirroring
//! `vendor/git/builtin/rm.c:286..299` for byte-exact error stanzas).
//! Per-row entries in `tests/journey/parity/rm.sh` close each flag with
//! `compat_effect "deferred until rm driver lands"` until the real
//! driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Remove files from the working tree and from the index")]
pub struct Platform {
    // ── verbosity / dry-run ─────────────────────────────────────────
    /// Don't actually remove any file(s). Just show if they exist in
    /// the index and would otherwise be removed by the command.
    /// Mirrors `vendor/git/builtin/rm.c:249` `OPT__DRY_RUN`.
    #[clap(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// `git rm` normally outputs one line (in the form of an `rm`
    /// command) for each file removed. This option suppresses that
    /// output. Mirrors `vendor/git/builtin/rm.c:250` `OPT__QUIET`.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    // ── modes ───────────────────────────────────────────────────────
    /// Use this option to unstage and remove paths only from the
    /// index. Working tree files, whether modified or not, will be
    /// left alone. Mirrors `vendor/git/builtin/rm.c:251` `OPT_BOOL(0,
    /// "cached", ...)`.
    #[clap(long)]
    pub cached: bool,

    /// Override the up-to-date check. Mirrors
    /// `vendor/git/builtin/rm.c:252` `OPT__FORCE`.
    #[clap(short = 'f', long)]
    pub force: bool,

    /// Allow recursive removal when a leading directory name is given.
    /// Mirrors `vendor/git/builtin/rm.c:253` `OPT_BOOL('r', NULL,
    /// ...)`. git wires this as a short-only option (no long form).
    #[clap(short = 'r')]
    pub recursive: bool,

    /// Exit with a zero status even if no files matched. Mirrors
    /// `vendor/git/builtin/rm.c:254` `OPT_BOOL(0, "ignore-unmatch",
    /// ...)`.
    #[clap(long = "ignore-unmatch")]
    pub ignore_unmatch: bool,

    /// Allow updating index entries outside of the sparse-checkout
    /// cone. Mirrors `vendor/git/builtin/rm.c:256` `OPT_BOOL(0,
    /// "sparse", ...)`.
    #[clap(long)]
    pub sparse: bool,

    // ── pathspec sources ────────────────────────────────────────────
    /// Pathspec is passed in `<file>` instead of commandline args.
    /// Mirrors `vendor/git/builtin/rm.c:257` `OPT_PATHSPEC_FROM_FILE`.
    #[clap(
        long = "pathspec-from-file",
        value_parser = crate::shared::AsBString,
        value_name = "file"
    )]
    pub pathspec_from_file: Option<BString>,

    /// Pathspec elements in `--pathspec-from-file` are NUL-separated.
    /// Mirrors `vendor/git/builtin/rm.c:258` `OPT_PATHSPEC_FILE_NUL`.
    #[clap(long = "pathspec-file-nul")]
    pub pathspec_file_nul: bool,

    // ── positionals ─────────────────────────────────────────────────
    /// `<pathspec>...` — files to remove. Mirrors the trailing
    /// `[<pathspec>...]` of the synopsis at
    /// `vendor/git/builtin/rm.c:30`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub args: Vec<BString>,

    /// Pathspec after the `--` separator.
    #[clap(last = true, value_parser = crate::shared::AsBString)]
    pub paths: Vec<BString>,
}
