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
    /// Raw `--signed` argument as given on the CLI (`yes`/`no`/`true`/`false`/
    /// `on`/`off`/`1`/`0`/`if-asked`, case-insensitive). Parsed at dispatch
    /// with git-compatible error semantics; use `signed()` to decode.
    pub signed_arg: Option<String>,
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

impl Signed {
    /// Parse `--signed=<arg>` with the same value set `git_parse_maybe_bool`
    /// + the `if-asked` literal accept (see `option_parse_push_signed` in
    /// vendor/git/send-pack.c). All matching is case-insensitive.
    ///
    /// Returns `Err(arg)` for any value git would reject — the caller is
    /// expected to print `fatal: bad signed argument: <arg>` and exit 128.
    pub fn parse(arg: &str) -> Result<Self, &str> {
        // git_parse_maybe_bool true-values. Empty string matches too (git
        // treats it as `1` when an equals-form option is supplied with no
        // value, e.g. `--signed=`), matching PARSE_OPT_OPTARG semantics.
        match arg.to_ascii_lowercase().as_str() {
            "yes" | "on" | "true" | "1" | "" => Ok(Signed::Yes),
            "no" | "off" | "false" | "0" => Ok(Signed::No),
            "if-asked" => Ok(Signed::IfAsked),
            _ => Err(arg),
        }
    }
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
    use std::io::Write as _;

    use super::Options;

    /// Mirror git's `die_for_incompatible_opt4` in `vendor/git/parse-options.c`:
    /// given up to four `(predicate, name)` pairs, if two or more predicates are
    /// set, print the list of conflicting names in the exact git format and exit
    /// 128. Returns only when at most one predicate is set.
    ///
    /// Keeping this function generic (rather than hard-coding the push flag set)
    /// matches how git reuses the same primitive across builtins; it also lets
    /// future callers (e.g., `git pull`, `git merge`) adopt the same exit-code
    /// contract without forking the message format.
    fn die_for_incompatible_opts(pairs: &[(bool, &str)]) -> std::io::Result<()> {
        let set: Vec<&str> = pairs.iter().filter(|(b, _)| *b).map(|(_, n)| *n).collect();
        if set.len() < 2 {
            return Ok(());
        }
        let mut stderr = std::io::stderr().lock();
        // git's wording, reproduced exactly for 2/3/4-way conflicts.
        match set.as_slice() {
            [a, b] => writeln!(stderr, "fatal: options '{a}' and '{b}' cannot be used together")?,
            [a, b, c] => writeln!(stderr, "fatal: options '{a}', '{b}', and '{c}' cannot be used together")?,
            [a, b, c, d] => writeln!(
                stderr,
                "fatal: options '{a}', '{b}', '{c}', and '{d}' cannot be used together"
            )?,
            _ => unreachable!("die_for_incompatible_opts only called with ≤4 pairs"),
        }
        drop(stderr);
        std::process::exit(128);
    }

    pub fn push<P>(
        repo: gix::Repository,
        _progress: P,
        _out: impl std::io::Write,
        _err: impl std::io::Write,
        opts: Options,
    ) -> anyhow::Result<()>
    where
        P: gix::NestedProgress,
        P::SubProgress: 'static,
    {
        // Parse --signed early, mirroring option_parse_push_signed in
        // vendor/git/send-pack.c. Invalid values die 128 with the exact
        // "fatal: bad signed argument: %s" text.
        let _signed = match opts.signed_arg.as_deref() {
            Some(arg) => match super::Signed::parse(arg) {
                Ok(s) => Some(s),
                Err(bad) => {
                    let mut stderr = std::io::stderr().lock();
                    writeln!(stderr, "fatal: bad signed argument: {bad}")?;
                    drop(stderr);
                    std::process::exit(128);
                }
            },
            None => None,
        };

        // Mirrors the `die_for_incompatible_opt4` call at the top of cmd_push
        // in vendor/git/builtin/push.c (after the repo_config + parse_options
        // block, right before the `tags → refs/tags/*` refspec append).
        // Exit 128 with git-exact message text on any pair/triple/quadruple
        // conflict between {--delete, --tags, --all/--branches, --mirror}.
        die_for_incompatible_opts(&[
            (opts.delete, "--delete"),
            (opts.tags, "--tags"),
            (opts.all, "--all/--branches"),
            (opts.mirror, "--mirror"),
        ])?;

        // Mirrors `if (deleterefs && argc < 2) die()` at vendor/git/builtin/push.c
        // line ~559. `argc < 2` in git counts the repo itself, so the check is
        // "delete given but no refspecs provided" — exit 128 before resolving
        // the remote.
        if opts.delete && opts.ref_specs.is_empty() {
            let mut stderr = std::io::stderr().lock();
            writeln!(stderr, "fatal: --delete doesn't make sense without any refs")?;
            drop(stderr);
            std::process::exit(128);
        }

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
            //
            // Write directly to the process stderr rather than the passed-in `err`
            // handle: depending on `--verbose`/`--progress` mode, `err` can be a
            // `Vec<u8>` flushed only after `run()` returns. `process::exit` skips
            // destructors, so a buffered write would be lost. Unix `io::stderr()`
            // is unbuffered, so the message reaches the terminal before exit.
            let mut stderr = std::io::stderr().lock();
            if let Some(name) = explicit {
                writeln!(stderr, "fatal: bad repository '{name}'")?;
            } else {
                writeln!(stderr, "fatal: No configured push destination.")?;
                writeln!(
                    stderr,
                    "Either specify the URL from the command-line or configure a remote repository using"
                )?;
                writeln!(stderr)?;
                writeln!(stderr, "    git remote add <name> <url>")?;
                writeln!(stderr)?;
                writeln!(stderr, "and then push using the remote name")?;
                writeln!(stderr)?;
                writeln!(stderr, "    git push <name>")?;
            }
            drop(stderr);
            // git's `die()` exits 128. No pending cleanup at this point — we're
            // early in the function and the stderr handle has been dropped above.
            std::process::exit(128);
        }

        // Mirrors the post-resolve die-checks in vendor/git/builtin/push.c
        // after `set_refspecs(argv + 1, argc - 1, remote)`:
        //     if (flags & TRANSPORT_PUSH_ALL)    { if (argc>=2) die(...) }
        //     if (flags & TRANSPORT_PUSH_MIRROR) { if (argc>=2) die(...) }
        // argc>=2 means "at least one refspec was given past the repo
        // positional" — in our CLI, `opts.ref_specs` already holds exactly
        // those refspecs.
        if opts.all && !opts.ref_specs.is_empty() {
            let mut stderr = std::io::stderr().lock();
            writeln!(stderr, "fatal: --all can't be combined with refspecs")?;
            drop(stderr);
            std::process::exit(128);
        }
        if opts.mirror && !opts.ref_specs.is_empty() {
            let mut stderr = std::io::stderr().lock();
            writeln!(stderr, "fatal: --mirror can't be combined with refspecs")?;
            drop(stderr);
            std::process::exit(128);
        }

        anyhow::bail!(
            "gix push is not yet implemented — parity rows are being closed flag-by-flag; \
             see tests/journey/parity/push.sh for the current surface"
        )
    }
}
