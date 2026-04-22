//! `gix push` plumbing CLI.
//!
//! Flag surface mirrors `vendor/git/builtin/push.c::cmd_push` options[] and
//! `vendor/git/Documentation/git-push.adoc`. Parity coverage lives in
//! `tests/journey/parity/push.sh`.

use std::ffi::OsString;

/// How to recursively push submodules (mirrors git's `--recurse-submodules`).
#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum RecurseSubmodules {
    /// Do not recurse into submodules.
    No,
    /// Abort the push if any submodule has unpushed commits.
    Check,
    /// Push submodules that have been checked out locally.
    OnDemand,
    /// Push only submodules, not the superproject.
    Only,
}

/// How to GPG-sign the push (mirrors git's `--signed`).
#[derive(Debug, Copy, Clone, PartialEq, Eq, clap::ValueEnum)]
pub enum Signed {
    /// Do not sign the push.
    No,
    /// Sign only if the server requested it.
    IfAsked,
    /// Always sign the push.
    Yes,
}

#[derive(Debug, clap::Parser)]
pub struct Platform {
    /// Push all branches (equivalent to refspec `refs/heads/*`).
    #[clap(long, visible_alias = "branches", conflicts_with_all = ["mirror", "tags", "delete"])]
    pub all: bool,

    /// Mirror all refs to the remote (implies `--force`).
    #[clap(long, conflicts_with_all = ["all", "tags", "delete"])]
    pub mirror: bool,

    /// Delete the given refs from the remote.
    #[clap(long, short = 'd', conflicts_with_all = ["all", "mirror", "tags"])]
    pub delete: bool,

    /// Push all refs under `refs/tags`.
    #[clap(long, conflicts_with_all = ["all", "mirror", "delete"])]
    pub tags: bool,

    /// Push all missing but reachable tags after normal refs.
    #[clap(long)]
    pub follow_tags: bool,

    /// Show what would be pushed without actually pushing.
    #[clap(long, short = 'n')]
    pub dry_run: bool,

    /// Produce machine-readable output (`<flag> <from>:<to> <summary> (<reason>)`).
    #[clap(long)]
    pub porcelain: bool,

    /// Force-update refs even when the update is not a fast-forward.
    #[clap(long, short = 'f')]
    pub force: bool,

    /// Require the remote ref's current value to match before updating.
    ///
    /// Accepts an optional `[<refname>[:<expect>]]`. Passing just the flag
    /// compares against the locally-recorded remote-tracking branch.
    #[clap(long, value_name = "REFNAME[:EXPECT]", num_args = 0..=1, default_missing_value = "")]
    pub force_with_lease: Option<String>,

    /// Require remote refs to include our locally-known commits before force-update.
    #[clap(long)]
    pub force_if_includes: bool,

    /// Request an atomic transaction on the remote side (all-or-nothing updates).
    #[clap(long)]
    pub atomic: bool,

    /// Remove remote-tracking refs that no longer exist on the remote.
    #[clap(long)]
    pub prune: bool,

    /// Set upstream (tracking) reference for the pushed branch.
    #[clap(long, short = 'u')]
    pub set_upstream: bool,

    /// Force progress reporting.
    #[clap(long, overrides_with = "no_progress")]
    pub progress: bool,

    /// Disable progress reporting.
    #[clap(long, overrides_with = "progress")]
    pub no_progress: bool,

    /// Produce a thin pack (the default).
    #[clap(long, overrides_with = "no_thin")]
    pub thin: bool,

    /// Disable thin-pack generation.
    #[clap(long, overrides_with = "thin")]
    pub no_thin: bool,

    /// Bypass the `pre-push` hook.
    #[clap(long)]
    pub no_verify: bool,

    /// Path (or name on PATH) of the receive-pack program to invoke remotely.
    #[clap(long, visible_alias = "exec", value_name = "PROGRAM")]
    pub receive_pack: Option<OsString>,

    /// GPG-sign the push (`no`, `if-asked`, or `yes`).
    #[clap(long, value_name = "MODE", num_args = 0..=1, default_missing_value = "yes", value_enum)]
    pub signed: Option<Signed>,

    /// Transmit the given option to the receive-pack on the other side.
    #[clap(long, short = 'o', value_name = "OPTION")]
    pub push_option: Vec<String>,

    /// Recursion strategy for submodules.
    #[clap(long, value_name = "MODE", value_enum)]
    pub recurse_submodules: Option<RecurseSubmodules>,

    /// Force IPv4 connections to the remote.
    #[clap(short = '4', long, conflicts_with = "ipv6")]
    pub ipv4: bool,

    /// Force IPv6 connections to the remote.
    #[clap(short = '6', long, conflicts_with = "ipv4")]
    pub ipv6: bool,

    /// Repository override (equivalent to the first positional `<repository>`).
    #[clap(long, value_name = "REPOSITORY")]
    pub repo: Option<String>,

    /// The remote to push to; either a named remote or a URL.
    ///
    /// If unset, the upstream of the current branch is used.
    pub repository: Option<String>,

    /// Refspecs to push (e.g. `main`, `main:main`, `+main:upstream`).
    #[clap(value_parser = crate::shared::AsBString)]
    pub refspec: Vec<gix::bstr::BString>,
}
