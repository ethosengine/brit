//! `gix mv` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/mv.c` (entry point
//! `cmd_mv` at `vendor/git/builtin/mv.c:208`) and
//! `vendor/git/Documentation/git-mv.adoc`. Parity coverage lives in
//! `tests/journey/parity/mv.sh`.
//!
//! Synopsis (`vendor/git/builtin/mv.c:31..34`):
//!
//! * `git mv [-v] [-f] [-n] [-k] <source> <destination>`
//! * `git mv [-v] [-f] [-n] [-k] <source>... <destination-directory>`
//!
//! Today the porcelain stub at `gitoxide_core::repository::mv::porcelain`
//! is a placeholder that wires the verbatim usage banner for the
//! `argc < 2` / `argc < 1`-after-decrement case (mirrors
//! `vendor/git/builtin/mv.c:247..248` `usage_with_options`), the
//! `bad source` workdir-miss gate, the `can not move directory into
//! itself` self-rename gate, and the `destination 'X' is not a
//! directory` multi-source gate. Per-row entries in
//! `tests/journey/parity/mv.sh` close each happy-path flag with
//! `compat_effect "deferred until mv driver lands"` until the real
//! driver lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Move or rename a file, a directory, or a symlink")]
pub struct Platform {
    /// Be verbose: report the names of files as they are moved.
    /// Mirrors `vendor/git/builtin/mv.c:216` `OPT__VERBOSE`.
    #[clap(short = 'v', long)]
    pub verbose: bool,

    /// Don't actually move any file(s); only show what would happen.
    /// Mirrors `vendor/git/builtin/mv.c:217` `OPT__DRY_RUN` (the C
    /// variable is `show_only`).
    #[clap(short = 'n', long = "dry-run")]
    pub dry_run: bool,

    /// Force renaming or moving of a file even if the destination
    /// exists. Mirrors `vendor/git/builtin/mv.c:218..219` `OPT__FORCE`.
    #[clap(short = 'f', long)]
    pub force: bool,

    /// Skip move/rename actions which would lead to an error
    /// condition. An error happens when a source is neither existing
    /// nor controlled by Git, or when it would overwrite an existing
    /// file unless `-f` is given. Mirrors
    /// `vendor/git/builtin/mv.c:220` `OPT_BOOL('k', NULL, ...)`. git
    /// wires this as a short-only option (no long form).
    #[clap(short = 'k')]
    pub ignore_errors: bool,

    /// Allow updating index entries outside of the sparse-checkout
    /// cone. Mirrors `vendor/git/builtin/mv.c:221` `OPT_BOOL(0,
    /// "sparse", ...)`.
    #[clap(long)]
    pub sparse: bool,

    /// `<source>...` followed by `<destination>` (single-source rename
    /// or multi-source move-into-directory). Mirrors the trailing
    /// `<source>... <destination>` of the synopsis at
    /// `vendor/git/builtin/mv.c:32..33`.
    #[clap(value_parser = crate::shared::AsBString)]
    pub args: Vec<BString>,

    /// Pathspec after the `--` separator. git's parse-options for `mv`
    /// does not set `PARSE_OPT_KEEP_DASHDASH`, so `--` is the standard
    /// option terminator and everything after it is positional.
    #[clap(last = true, value_parser = crate::shared::AsBString)]
    pub paths: Vec<BString>,
}
