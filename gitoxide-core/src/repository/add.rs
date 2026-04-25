//! Bare-form `gix add` porcelain entry point.
//!
//! Mirrors `vendor/git/builtin/add.c::cmd_add`
//! (`vendor/git/builtin/add.c:381`) — the porcelain shape that stages
//! file contents into the index. Currently a placeholder: every
//! flag-bearing parity row in `tests/journey/parity/add.sh` closes
//! with `compat_effect "deferred until add driver lands"` until the
//! real driver lands.
//!
//! Behaviour:
//!
//! * Outside a repository: handled by the shared `repository(Mode::
//!   Lenient)` glue in `src/plumbing/main.rs`, which remaps
//!   `gix_discover::upwards::Error::NoGitRepository*` to git's exact
//!   "fatal: not a git repository (or any of the parent directories):
//!   .git" + exit 128.
//!
//! * Otherwise: walk through git's pre-pathspec validation matrix
//!   (`vendor/git/builtin/add.c:405..474`) and emit the verbatim
//!   wording for each precondition gate. On the happy path emit a
//!   single-line stub note on stdout and exit 0 so
//!   `compat_effect`-mode rows match git's exit code while the real
//!   add driver is unimplemented.
//!
//! Bytes parity (real index update with `add_files_to_cache`,
//! `--chmod` toggle, `--pathspec` matching, `--dry-run` ignore
//! reporting, `--refresh` stat-only update, `--renormalize` clean-
//! filter replay, `--intent-to-add` empty-blob entries, embedded-repo
//! advice, sparse-checkout warnings) is deferred until the add driver
//! lands. The shared deferral phrase is `"deferred until add driver
//! lands"`.

use anyhow::Result;
use gix::bstr::{BString, ByteSlice};

/// Subset of `add::Platform` flags consumed by the porcelain stub.
#[derive(Debug, Default)]
pub struct Options {
    pub dry_run: bool,
    pub interactive: bool,
    pub patch: bool,
    pub auto_advance: bool,
    pub no_auto_advance: bool,
    pub unified: Option<i32>,
    pub inter_hunk_context: Option<i32>,
    pub edit: bool,
    pub force: bool,
    pub update: bool,
    pub renormalize: bool,
    pub intent_to_add: bool,
    pub all: bool,
    pub ignore_removal: bool,
    pub refresh: bool,
    pub ignore_errors: bool,
    pub ignore_missing: bool,
    pub sparse: bool,
    pub chmod: Option<String>,
    pub pathspec_from_file: Option<BString>,
    pub pathspec_file_nul: bool,
}

impl Options {
    fn interactive_or_patch(&self) -> bool {
        self.interactive || self.patch
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
    // `vendor/git/builtin/add.c::cmd_add` (lines 405..474).

    // 1. `--unified` / `--inter-hunk-context` cannot be negative
    //    (vendor/git/builtin/add.c:405..408). Clap accepts negatives;
    //    the C-mirror gate emits `die("'%s' cannot be negative", ...)`.
    if let Some(n) = opts.unified {
        if n < -1 {
            let _ = writeln!(err, "fatal: '--unified' cannot be negative");
            std::process::exit(128);
        }
    }
    if let Some(n) = opts.inter_hunk_context {
        if n < -1 {
            let _ = writeln!(err, "fatal: '--inter-hunk-context' cannot be negative");
            std::process::exit(128);
        }
    }

    // 2. `--interactive` / `--patch` mutually exclusive with
    //    `--dry-run` and `--pathspec-from-file`
    //    (vendor/git/builtin/add.c:412..417).
    if opts.interactive_or_patch() {
        if opts.dry_run {
            let _ = writeln!(
                err,
                "fatal: options '--dry-run' and '--interactive/--patch' cannot be used together"
            );
            std::process::exit(128);
        }
        if opts.pathspec_from_file.is_some() {
            let _ = writeln!(
                err,
                "fatal: options '--pathspec-from-file' and '--interactive/--patch' cannot be used together"
            );
            std::process::exit(128);
        }
    } else {
        // 3. `-U` / `--inter-hunk-context` / `--no-auto-advance`
        //    require `--interactive` / `--patch`
        //    (vendor/git/builtin/add.c:419..425).
        if opts.unified.is_some() {
            let _ = writeln!(err, "fatal: the option '--unified' requires '--interactive/--patch'");
            std::process::exit(128);
        }
        if opts.inter_hunk_context.is_some() {
            let _ = writeln!(
                err,
                "fatal: the option '--inter-hunk-context' requires '--interactive/--patch'"
            );
            std::process::exit(128);
        }
        if opts.no_auto_advance {
            let _ = writeln!(
                err,
                "fatal: the option '--no-auto-advance' requires '--interactive/--patch'"
            );
            std::process::exit(128);
        }
    }

    // 4. `--edit` mutually exclusive with `--pathspec-from-file`
    //    (vendor/git/builtin/add.c:427..430).
    if opts.edit && opts.pathspec_from_file.is_some() {
        let _ = writeln!(
            err,
            "fatal: options '--pathspec-from-file' and '--edit' cannot be used together"
        );
        std::process::exit(128);
    }

    // 5. `-A` and `-u` mutually exclusive
    //    (vendor/git/builtin/add.c:440..441).
    if opts.all && opts.update {
        let _ = writeln!(err, "fatal: options '-A' and '-u' cannot be used together");
        std::process::exit(128);
    }

    // 6. `--ignore-missing` requires `--dry-run`
    //    (vendor/git/builtin/add.c:443..444).
    if opts.ignore_missing && !opts.dry_run {
        let _ = writeln!(err, "fatal: the option '--ignore-missing' requires '--dry-run'");
        std::process::exit(128);
    }

    // 7. `--chmod` parameter must be exactly `+x` or `-x`
    //    (vendor/git/builtin/add.c:446..448).
    if let Some(arg) = opts.chmod.as_deref() {
        if arg != "+x" && arg != "-x" {
            let _ = writeln!(err, "fatal: --chmod param '{arg}' must be either -x or +x");
            std::process::exit(128);
        }
    }

    // 8. `--pathspec-from-file` and pathspec arguments are mutually
    //    exclusive (vendor/git/builtin/add.c:464..466).
    let pathspec_present = !args.is_empty() || !paths.is_empty();
    if opts.pathspec_from_file.is_some() && pathspec_present {
        let _ = writeln!(
            err,
            "fatal: '--pathspec-from-file' and pathspec arguments cannot be used together"
        );
        std::process::exit(128);
    }

    // 9. `--pathspec-file-nul` requires `--pathspec-from-file`
    //    (vendor/git/builtin/add.c:472..474).
    if opts.pathspec_file_nul && opts.pathspec_from_file.is_none() {
        let _ = writeln!(
            err,
            "fatal: the option '--pathspec-file-nul' requires '--pathspec-from-file'"
        );
        std::process::exit(128);
    }

    // 10. Empty pathspec without `-u` / `-A` / `--pathspec-from-file`
    //     emits "Nothing specified, nothing added." on stderr and
    //     returns 0 (vendor/git/builtin/add.c:476..481). The advice
    //     line ("Maybe you wanted to say 'git add .'?") is gated by
    //     `advice.addEmptyPathspec`; default-on in git, hence the test
    //     fixture always sees both lines.
    let require_pathspec = !opts.update && !opts.all;
    if require_pathspec && !pathspec_present && opts.pathspec_from_file.is_none() {
        let _ = writeln!(err, "Nothing specified, nothing added.");
        let _ = writeln!(err, "hint: Maybe you wanted to say 'git add .'?");
        let _ = writeln!(
            err,
            "hint: Disable this message with \"git config advice.addEmptyPathspec false\""
        );
        return Ok(());
    }

    // 11. For each non-magic positional pathspec, error if it does not
    //     exist in the worktree (vendor/git/builtin/add.c:559..568).
    //     `.` and `--` are special-cased (PATHSPEC_FROMTOP / dash-dash
    //     terminator). Skip the gate under `--ignore-missing` per
    //     vendor/git/builtin/add.c:561..565. Magic-char pathspecs
    //     (`*`/`?`/`[`) and the `:` magic-prefix forms always reach the
    //     full pathspec walker, so we let them through to the deferred
    //     driver instead of guessing here.
    if !opts.ignore_missing {
        let workdir = repo.workdir();
        let combined_paths: Vec<&BString> = args.iter().chain(paths.iter()).collect();
        for spec in combined_paths {
            let bytes: &[u8] = spec.as_ref();
            if bytes.is_empty() || bytes == b"." {
                continue;
            }
            // Skip magic / glob forms — those need the real pathspec
            // walker, which is part of the deferred add driver.
            if bytes.iter().any(|&b| b == b'*' || b == b'?' || b == b'[' || b == b':') {
                continue;
            }
            let Some(workdir) = workdir else { continue };
            let rel = match bytes.to_str() {
                Ok(s) => s,
                Err(_) => continue,
            };
            if !workdir.join(rel).exists() {
                let _ = writeln!(err, "fatal: pathspec '{}' did not match any files", spec.to_str_lossy());
                std::process::exit(128);
            }
        }
    }

    // Happy path placeholder: emit a stub note so the shape of stdout
    // is recognizable in failures, then exit 0 so `compat_effect`-mode
    // rows match git's exit code while the real add driver is
    // unimplemented.
    let arg_names: Vec<String> = args.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let path_names: Vec<String> = paths.iter().map(|b| b.to_str_lossy().into_owned()).collect();
    let _ = writeln!(
        out,
        "[gix-add] git_dir={} args={arg_names:?} paths={path_names:?}; add driver not yet implemented",
        repo.git_dir().display(),
    );
    Ok(())
}
