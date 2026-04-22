//! `git push` equivalent — stub.
//!
//! The plumbing CLI in `src/plumbing/` parses the full flag surface (see
//! `src/plumbing/options/push.rs`), but the implementation is staged
//! flag-by-flag through the parity loop in
//! `tests/journey/parity/push.sh`. Until each row is closed, calling this
//! function panics with a clear message.

use gix::bstr::BString;

use crate::OutputFormat;

/// Mirrors `vendor/git/builtin/push.c::cmd_push` options[] — one field per flag.
#[derive(Debug)]
pub struct Options {
    pub format: OutputFormat,
    pub all: bool,
    pub mirror: bool,
    pub delete: bool,
    pub tags: bool,
    pub follow_tags: bool,
    pub dry_run: bool,
    pub porcelain: bool,
    pub force: bool,
    pub force_with_lease: Option<String>,
    pub force_if_includes: bool,
    pub atomic: bool,
    pub prune: bool,
    pub set_upstream: bool,
    pub progress: Option<bool>,
    pub thin: Option<bool>,
    pub no_verify: bool,
    pub receive_pack: Option<std::ffi::OsString>,
    pub signed: Option<Signed>,
    pub push_options: Vec<String>,
    pub recurse_submodules: Option<RecurseSubmodules>,
    pub ipv4: bool,
    pub ipv6: bool,
    pub repo: Option<String>,
    pub remote: Option<String>,
    pub ref_specs: Vec<BString>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Signed {
    No,
    IfAsked,
    Yes,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecurseSubmodules {
    No,
    Check,
    OnDemand,
    Only,
}

pub const PROGRESS_RANGE: std::ops::RangeInclusive<u8> = 1..=3;

pub(crate) mod function {
    use super::Options;

    pub fn push<P>(
        repo: gix::Repository,
        _progress: P,
        _out: impl std::io::Write,
        mut err: impl std::io::Write,
        opts: Options,
    ) -> anyhow::Result<()>
    where
        P: gix::NestedProgress,
        P::SubProgress: 'static,
    {
        // Resolve the push target. `opts.repo` (`--repo=<repository>`) is the canonical
        // override; `opts.remote` holds the first positional `<repository>` from the CLI.
        // Fall back to the repo's configured default push remote when neither is set
        // (mirrors `pushremote_get(NULL)` in vendor/git/builtin/push.c::cmd_push).
        let explicit = opts.repo.as_deref().or(opts.remote.as_deref());
        let found = match explicit {
            Some(name_or_url) => repo.find_remote(name_or_url).ok(),
            None => repo
                .find_default_remote(gix::remote::Direction::Push)
                .and_then(Result::ok),
        };

        if found.is_none() {
            // Matches the `die()` branch at vendor/git/builtin/push.c around line 631.
            // We preserve git's exit code (128) for parity; the message text is a
            // close render of git's wording but not byte-exact (effect-mode parity).
            if let Some(name) = explicit {
                writeln!(err, "fatal: bad repository '{name}'")?;
            } else {
                writeln!(err, "fatal: No configured push destination.")?;
                writeln!(
                    err,
                    "Either specify the URL from the command-line or configure a remote repository using"
                )?;
                writeln!(err)?;
                writeln!(err, "    git remote add <name> <url>")?;
                writeln!(err)?;
                writeln!(err, "and then push using the remote name")?;
                writeln!(err)?;
                writeln!(err, "    git push <name>")?;
            }
            err.flush()?;
            // git's `die()` exits 128. std::process::exit skips destructors, but we're
            // early on in the function — no pending cleanup.
            std::process::exit(128);
        }

        anyhow::bail!(
            "gix push is not yet implemented — parity rows are being closed flag-by-flag; \
             see tests/journey/parity/push.sh for the current surface"
        )
    }
}
