//! Bare-form `gix reset` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/reset.c::cmd_reset`
//! (`vendor/git/builtin/reset.c:336`) — the porcelain shape that
//! moves HEAD and optionally rewrites the index / working tree to a
//! given commit, tree, or pathspec subset. Currently a placeholder:
//! every flag-bearing parity row in `tests/journey/parity/reset.sh`
//! closes with `compat_effect "deferred until reset driver lands"`
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
//! * Otherwise: emit a single-line stub note on stdout and exit 0 so
//!   `compat_effect`-mode rows match git's exit code while the real
//!   reset driver is unimplemented.
//!
//! Bytes parity (real ORIG_HEAD bookkeeping, three-way unpack of the
//! target tree under MIXED / HARD / MERGE / KEEP, the
//! `print_new_head_line` "HEAD is now at <abbrev> <subject>" emission
//! at `vendor/git/builtin/reset.c:137`, the deprecated-`--mixed`+paths
//! warning at `vendor/git/builtin/reset.c:457`, the
//! `--mixed`-in-bare-repo `die` at `vendor/git/builtin/reset.c:473`,
//! the `-N`-without-`--mixed` `die` at `vendor/git/builtin/reset.c:477`,
//! and the `--patch` interactive add-p replay at
//! `vendor/git/builtin/reset.c:436`) is deferred until the reset
//! driver lands. The shared deferral phrase is `"deferred until
//! reset driver lands"`.

use anyhow::Result;
use gix::bstr::{BStr, BString, ByteSlice};

/// Subset of `reset::Platform` flags consumed by the porcelain stub.
///
/// Today the stub only consults the positionals + mode flags + a
/// handful of precondition-gate inputs to emit a recognizable note
/// and mirror git's pre-reset validation matrix; once the real reset
/// driver lands, refresh / recurse-submodules / patch-context
/// threading lives here too.
#[derive(Debug, Default)]
pub struct Options {
    pub soft: bool,
    pub mixed: bool,
    pub hard: bool,
    pub merge: bool,
    pub keep: bool,
    pub patch: bool,
    pub intent_to_add: bool,
    pub pathspec_from_file: Option<gix::bstr::BString>,
    pub pathspec_file_nul: bool,
}

impl Options {
    /// Mirror `vendor/git/builtin/reset.c:341` `int reset_type`'s
    /// last-wins semantics across the five mode flags. SOFT > KEEP >
    /// MERGE > HARD > MIXED selection order is irrelevant to git
    /// (parse-options visits flags in argv order and the last
    /// `OPT_SET_INT_F` wins). Today gix's Clap surface exposes one
    /// bool per mode and cannot reproduce argv-order, so the gates
    /// below treat any non-mixed bool as the effective mode for
    /// validation purposes — matching git when only one mode flag is
    /// present and erring on the safe side when several are.
    fn explicit_non_mixed_mode(&self) -> Option<&'static str> {
        if self.soft {
            Some("soft")
        } else if self.hard {
            Some("hard")
        } else if self.merge {
            Some("merge")
        } else if self.keep {
            Some("keep")
        } else {
            None
        }
    }

    fn any_mode(&self) -> bool {
        self.mixed || self.soft || self.hard || self.merge || self.keep
    }
}

pub fn porcelain(
    repo: gix::Repository,
    out: &mut dyn std::io::Write,
    err: &mut dyn std::io::Write,
    args: Vec<BString>,
    paths: Vec<BString>,
    opts: Options,
) -> Result<()> {
    // Precondition gates run in the same order as
    // `vendor/git/builtin/reset.c::cmd_reset`.

    // 1. `--pathspec-file-nul` requires `--pathspec-from-file`
    //    (vendor/git/builtin/reset.c:401..403).
    if opts.pathspec_file_nul && opts.pathspec_from_file.is_none() {
        let _ = writeln!(
            err,
            "fatal: the option '--pathspec-file-nul' requires '--pathspec-from-file'"
        );
        std::process::exit(128);
    }

    // 2. `--pathspec-from-file` and `--patch` are mutually exclusive
    //    (vendor/git/builtin/reset.c:392..393).
    if opts.pathspec_from_file.is_some() && opts.patch {
        let _ = writeln!(
            err,
            "fatal: options '--pathspec-from-file' and '--patch' cannot be used together"
        );
        std::process::exit(128);
    }

    // 3. `--pathspec-from-file` and pathspec arguments are mutually
    //    exclusive (vendor/git/builtin/reset.c:395..396).
    if opts.pathspec_from_file.is_some() && !paths.is_empty() {
        let _ = writeln!(
            err,
            "fatal: '--pathspec-from-file' and pathspec arguments cannot be used together"
        );
        std::process::exit(128);
    }

    // 4. Bad-revspec gate: when there's a positional and it does not
    //    resolve, git's `parse_args` (vendor/git/builtin/reset.c:247)
    //    calls `verify_filename` which dies 128 with the "ambiguous
    //    argument" stanza emitted by
    //    vendor/git/setup.c::verify_filename. Skip the gate when HEAD
    //    is unborn (git treats that as the empty-tree target per
    //    vendor/git/builtin/reset.c:407..409) and when the first
    //    positional is followed by additional args / paths (path-mode
    //    dispatch, gated by the eventual reset driver).
    let head_is_unborn = matches!(repo.head().map(|h| h.kind), Ok(gix::head::Kind::Unborn(_)));
    if !head_is_unborn && args.len() == 1 && paths.is_empty() {
        let spec: &BStr = args[0].as_ref();
        if repo.rev_parse_single(spec).is_err() {
            let _ = writeln!(
                err,
                "fatal: ambiguous argument '{}': unknown revision or path not in the working tree.\n\
                 Use '--' to separate paths from revisions, like this:\n\
                 'git <command> [<revision>...] -- [<file>...]'",
                spec.to_str_lossy()
            );
            std::process::exit(128);
        }
    }

    // 5. `--patch` + `--{hard,mixed,soft}` are mutually exclusive
    //    (vendor/git/builtin/reset.c:437..438).
    if opts.patch && opts.any_mode() {
        let _ = writeln!(
            err,
            "fatal: options '--patch' and '--{{hard,mixed,soft}}' cannot be used together"
        );
        std::process::exit(128);
    }

    // 6. Non-mixed mode + paths errors at
    //    vendor/git/builtin/reset.c:458..460.
    let pathspec_present = !paths.is_empty() || opts.pathspec_from_file.is_some();
    if pathspec_present {
        if let Some(mode_name) = opts.explicit_non_mixed_mode() {
            let _ = writeln!(err, "fatal: Cannot do {mode_name} reset with paths.");
            std::process::exit(128);
        }
    }

    // 7. `--mixed` (the default when no mode flag is set, per
    //    vendor/git/builtin/reset.c:462..463) in a bare repo dies 128
    //    at vendor/git/builtin/reset.c:473..475. Path-mode skips the
    //    gate because path-mode never reaches the bare-repo check
    //    (the check guards the unpack-trees call only).
    let effective_mode_is_mixed = opts.mixed || (!opts.any_mode() && !opts.patch);
    if effective_mode_is_mixed && !pathspec_present && repo.is_bare() {
        let _ = writeln!(err, "fatal: mixed reset is not allowed in a bare repository");
        std::process::exit(128);
    }

    // 8. `-N` requires `--mixed` (vendor/git/builtin/reset.c:477..478).
    //    The default-mixed case (no mode flag) is permitted.
    if opts.intent_to_add && opts.explicit_non_mixed_mode().is_some() {
        let _ = writeln!(err, "fatal: the option '-N' requires '--mixed'");
        std::process::exit(128);
    }

    // Happy path placeholder: emit a stub note so the shape of stdout
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real reset driver is
    // unimplemented.
    let arg_names: Vec<String> = args.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let path_names: Vec<String> = paths.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let _ = writeln!(
        out,
        "[gix-reset] git_dir={} args={arg_names:?} paths={path_names:?}; reset driver not yet implemented",
        repo.git_dir().display(),
    );
    Ok(())
}
