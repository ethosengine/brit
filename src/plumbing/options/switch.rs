//! `gix switch` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/checkout.c::cmd_switch`
//! (entry point at `vendor/git/builtin/checkout.c:2089`) plus the two
//! shared option helpers `add_common_options`
//! (`vendor/git/builtin/checkout.c:1712..1730`) and
//! `add_common_switch_branch_options`
//! (`vendor/git/builtin/checkout.c:1732..1754`), and the synopsis at
//! `vendor/git/Documentation/git-switch.adoc:8..14`.
//!
//! Synopsis:
//!
//! * `git switch [<options>] [--no-guess] <branch>`
//! * `git switch [<options>] --detach [<start-point>]`
//! * `git switch [<options>] (-c|-C) <new-branch> [<start-point>]`
//! * `git switch [<options>] --orphan <new-branch>`
//!
//! Today the porcelain stub at `gitoxide_core::repository::switch::porcelain`
//! is a placeholder that wires the verbatim "fatal: missing branch or commit
//! argument" die for the no-positional case (mirrors checkout.c's argument-
//! count gate when `accept_pathspec=0` and no `-c`/`-C`/`--orphan`/`--detach`
//! is set), and a happy-path stub note for everything else. Per-row entries
//! in `tests/journey/parity/switch.sh` close each happy-path flag with
//! `compat_effect "deferred until switch driver lands"` until the real driver
//! lands.

use gix::bstr::BString;

#[derive(Debug, clap::Parser)]
#[command(about = "Switch branches")]
pub struct Platform {
    /// Create and switch to a new branch named <branch>. Mirrors
    /// `vendor/git/builtin/checkout.c:2097..2098`
    /// `OPT_STRING('c', "create", &opts.new_branch, ...)`.
    #[clap(short = 'c', long = "create", value_name = "branch")]
    pub create: Option<String>,

    /// Create or reset <branch> and switch to it. Mirrors
    /// `vendor/git/builtin/checkout.c:2099..2100`
    /// `OPT_STRING('C', "force-create", &opts.new_branch_force, ...)`.
    #[clap(short = 'C', long = "force-create", value_name = "branch")]
    pub force_create: Option<String>,

    /// Second-guess `git switch <no-such-branch>` by checking remotes.
    /// Mirrors `vendor/git/builtin/checkout.c:2101..2102`
    /// `OPT_BOOL(0, "guess", &opts.dwim_new_local_branch, ...)`. Default
    /// is on (`opts.dwim_new_local_branch = 1` at `:2108`); pass
    /// `--no-guess` to disable.
    #[clap(long, overrides_with = "no_guess")]
    pub guess: bool,

    /// Disable the `--guess` second-guessing.
    #[clap(long = "no-guess", overrides_with = "guess")]
    pub no_guess: bool,

    /// Throw away local modifications. Mirrors
    /// `vendor/git/builtin/checkout.c:2103..2104`
    /// `OPT_BOOL(0, "discard-changes", &opts.discard_changes, ...)`.
    #[clap(long = "discard-changes")]
    pub discard_changes: bool,

    // ----- add_common_options (vendor/git/builtin/checkout.c:1712..1730) -----
    /// Suppress progress reporting. Mirrors
    /// `vendor/git/builtin/checkout.c:1716` `OPT__QUIET`.
    #[clap(short = 'q', long)]
    pub quiet: bool,

    /// Control recursive updating of submodules. Mirrors
    /// `vendor/git/builtin/checkout.c:1717..1719`
    /// `OPT_CALLBACK_F(0, "recurse-submodules", ..., PARSE_OPT_OPTARG, ...)`.
    /// Optional value, e.g. `--recurse-submodules=on-demand`.
    #[clap(
        long = "recurse-submodules",
        value_name = "checkout",
        num_args = 0..=1,
        default_missing_value = "",
        require_equals = true,
    )]
    pub recurse_submodules: Option<String>,

    /// Disable submodule recursion.
    #[clap(long = "no-recurse-submodules")]
    pub no_recurse_submodules: bool,

    /// Force progress reporting. Mirrors
    /// `vendor/git/builtin/checkout.c:1720`
    /// `OPT_BOOL(0, "progress", &opts->show_progress, ...)`.
    #[clap(long, overrides_with = "no_progress")]
    pub progress: bool,

    /// Disable progress reporting (overrides `--progress`).
    #[clap(long = "no-progress", overrides_with = "progress")]
    pub no_progress: bool,

    /// Perform a 3-way merge with the new branch. Mirrors
    /// `vendor/git/builtin/checkout.c:1721`
    /// `OPT_BOOL('m', "merge", &opts->merge, ...)`.
    #[clap(short = 'm', long)]
    pub merge: bool,

    /// Conflict style (merge, diff3, or zdiff3). Mirrors
    /// `vendor/git/builtin/checkout.c:1722..1724`
    /// `OPT_CALLBACK(0, "conflict", ...)`.
    #[clap(long, value_name = "style")]
    pub conflict: Option<String>,

    // ----- add_common_switch_branch_options (vendor/git/builtin/checkout.c:1732..1754) -----
    /// Detach HEAD at the named commit. Mirrors
    /// `vendor/git/builtin/checkout.c:1736`
    /// `OPT_BOOL('d', "detach", &opts->force_detach, ...)`.
    #[clap(short = 'd', long)]
    pub detach: bool,

    /// Set branch tracking configuration. Mirrors
    /// `vendor/git/builtin/checkout.c:1737..1740`
    /// `OPT_CALLBACK_F('t', "track", ..., PARSE_OPT_OPTARG, ...)`.
    /// Optional value of `direct` or `inherit`.
    #[clap(
        short = 't',
        long = "track",
        value_name = "direct|inherit",
        num_args = 0..=1,
        default_missing_value = "",
        require_equals = true,
    )]
    pub track: Option<String>,

    /// Disable branch tracking configuration.
    #[clap(long = "no-track")]
    pub no_track: bool,

    /// Force checkout (throw away local modifications). Mirrors
    /// `vendor/git/builtin/checkout.c:1741..1742` `OPT__FORCE`. Per
    /// `vendor/git/Documentation/git-switch.adoc:113..115`, `-f` /
    /// `--force` is documented as an alias for `--discard-changes`.
    #[clap(short = 'f', long)]
    pub force: bool,

    /// Create a new orphan branch. Mirrors
    /// `vendor/git/builtin/checkout.c:1743`
    /// `OPT_STRING(0, "orphan", &opts->new_orphan_branch, ...)`.
    #[clap(long, value_name = "new-branch")]
    pub orphan: Option<String>,

    /// Update ignored files (default). Mirrors
    /// `vendor/git/builtin/checkout.c:1744..1746`
    /// `OPT_BOOL_F(0, "overwrite-ignore", ...)`.
    #[clap(long = "overwrite-ignore", overrides_with = "no_overwrite_ignore")]
    pub overwrite_ignore: bool,

    /// Do not update ignored files.
    #[clap(long = "no-overwrite-ignore", overrides_with = "overwrite_ignore")]
    pub no_overwrite_ignore: bool,

    /// Do not check if another worktree is using this branch. Mirrors
    /// `vendor/git/builtin/checkout.c:1747..1748`
    /// `OPT_BOOL(0, "ignore-other-worktrees", ...)`.
    #[clap(long = "ignore-other-worktrees")]
    pub ignore_other_worktrees: bool,

    /// Optional `<branch>` (or `<start-point>` for `--detach`). Per
    /// `vendor/git/Documentation/git-switch.adoc:8..14`, the four
    /// synopsis forms accept zero-or-more positionals.
    #[clap(value_parser = crate::shared::AsBString)]
    pub args: Vec<BString>,
}
