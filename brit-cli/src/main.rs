//! One executable for Git plumbing and build operations.

use std::{io::Write, path::PathBuf, process::ExitCode};

use clap::{parser::ValueSource, CommandFactory, FromArgMatches, Parser, Subcommand};
use gitoxide::plumbing;

mod bridge;
mod commands;
mod context;
mod error;
mod feedback;
mod native_task;
mod output;
mod repository;
mod surface;
mod tree;

use error::Result;

const VERSION_DETAIL: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nsource HEAD: ",
    env!("BRIT_GIT_SHA"),
    "\nsource state: ",
    env!("BRIT_SOURCE_STATE"),
    "\nfrontend presets: ",
    env!("BRIT_FEATURES"),
);

#[derive(Parser)]
#[command(
    name = "rakia",
    version,
    about = "Legacy build commands (deprecated; use brit build)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Execute one exact local source candidate through the installed Ark task supervisor.
    Run(native_task::RunArgs),
    /// Graph operations on the build constellation
    #[command(subcommand)]
    Graph(GraphCmd),
    /// Show which steps are affected by changes
    Affected(AffectedArgs),
    /// Compute a topologically-grouped build plan
    Plan(PlanArgs),
    /// Compute the content fingerprint of a step's inputs
    ///
    /// Keyed: each committed blob a step glob matches, by content; each
    /// gitlink a glob matches or can match beneath, by the commit it pins
    /// (the submodule's files are not read); each symlink a glob matches or
    /// can match beneath (a glob written through a directory link), by its
    /// own object id, i.e. its target path. Not keyed: the file a symlink
    /// points to unless a glob matches that file, and anything uncommitted.
    Fingerprint(FingerprintArgs),
    /// Manage rakia baseline refs
    #[command(subcommand)]
    Baseline(BaselineCmd),
}

#[derive(Subcommand)]
enum BuildNamespace {
    /// Integrate one locally approved acceptance through installed epr; never pushes.
    Integrate {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// Reconcile feature work through the installed epr; accepts epr flow context arguments.
    Context {
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<std::ffi::OsString>,
    },
    /// Build graph, affected steps, plans, fingerprints and baselines.
    #[command(subcommand)]
    Build(Command),
    /// Seal, verify and restore immutable source trees (no publication or head election).
    #[command(subcommand)]
    Snapshot(tree::TreeCommand),
}

#[derive(Subcommand)]
enum GraphCmd {
    /// Discover and list all build manifests
    Discover {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Show the full constellation graph
    Show {
        #[arg(long, default_value = ".")]
        repo: PathBuf,
        #[arg(long, default_value = "json", value_parser = ["json", "dot"])]
        format: String,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum InputsArg {
    Sources,
    All,
}

impl From<InputsArg> for rakia_core::inputs::InputSelection {
    fn from(value: InputsArg) -> Self {
        match value {
            InputsArg::Sources => Self::Sources,
            InputsArg::All => Self::All,
        }
    }
}

#[derive(clap::Args)]
struct AffectedArgs {
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    /// Comma-separated list of changed files (workspace-relative)
    #[arg(long, conflicts_with = "since", required_unless_present = "since")]
    files: Option<String>,
    /// Compute affected from changes since the given git ref (e.g. baseline)
    #[arg(long)]
    since: Option<String>,
    /// Select source inputs alone or sources plus build-process inputs
    #[arg(long, value_enum)]
    inputs: Option<InputsArg>,
}

#[derive(clap::Args)]
struct PlanArgs {
    #[arg(long, default_value = ".")]
    repo: PathBuf,
    #[arg(long, conflicts_with = "since", required_unless_present = "since")]
    files: Option<String>,
    #[arg(long)]
    since: Option<String>,
    /// Pipeline name (used to locate baseline ref when --since is auto)
    #[arg(long)]
    pipeline: Option<String>,
}

#[derive(clap::Args)]
struct FingerprintArgs {
    /// Path to a build-manifest.json
    manifest: PathBuf,
    /// Specific step name (default: all steps in the manifest)
    #[arg(long)]
    step: Option<String>,
    /// Git ref or SHA to fingerprint against (default: HEAD)
    #[arg(long, default_value = "HEAD")]
    commit: String,
    /// Select source inputs alone or sources plus build-process inputs; omitted, the
    /// legacy union and today's CID
    #[arg(long, value_enum)]
    inputs: Option<InputsArg>,
}

#[derive(Subcommand)]
enum BaselineCmd {
    /// Read the current baseline ref for a pipeline
    Read {
        pipeline: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// Write a baseline ref for a pipeline
    Write {
        pipeline: String,
        commit: String,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
    /// One-shot migration from Jenkins pipeline-baselines.json
    Migrate {
        json_path: PathBuf,
        #[arg(long, default_value = ".")]
        repo: PathBuf,
    },
}

fn run_build(command: Command) -> Result<()> {
    match command {
        Command::Run(args) => native_task::run(args),
        Command::Graph(GraphCmd::Discover { repo }) => commands::graph_discover::run(&repo),
        Command::Graph(GraphCmd::Show { repo, format }) => commands::graph_show::run(&repo, &format),
        Command::Affected(args) => commands::affected::run(
            &args.repo,
            args.files.as_deref(),
            args.since.as_deref(),
            args.inputs.map(Into::into),
        ),
        Command::Plan(args) => commands::plan::run(
            &args.repo,
            args.files.as_deref(),
            args.since.as_deref(),
            args.pipeline.as_deref(),
        ),
        Command::Fingerprint(args) => commands::fingerprint::run(
            &args.manifest,
            args.step.as_deref(),
            &args.commit,
            args.inputs.map(Into::into),
        ),
        Command::Baseline(BaselineCmd::Read { pipeline, repo }) => commands::baseline::read(&repo, &pipeline),
        Command::Baseline(BaselineCmd::Write { pipeline, commit, repo }) => {
            commands::baseline::write(&repo, &pipeline, &commit)
        }
        Command::Baseline(BaselineCmd::Migrate { json_path, repo }) => commands::baseline::migrate(&repo, &json_path),
    }
}

#[derive(Subcommand)]
enum FeedbackNamespace {
    #[command(hide = true, subcommand)]
    Feedback(feedback::CommandLine),
}

fn public_command() -> clap::Command {
    FeedbackNamespace::augment_subcommands(BuildNamespace::augment_subcommands(plumbing::Args::command()))
        .version(env!("CARGO_PKG_VERSION"))
        .long_version(VERSION_DETAIL)
        .arg(
            clap::Arg::new("capture-quiet")
                .long("quiet")
                .help("Silence feedback capture gap notices (also BRIT_QUIET=1)")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("cli-surface-json")
                .long("cli-surface-json")
                .help("Print the compiled parser surface as JSON; no behavior or acceptance claim")
                .action(clap::ArgAction::SetTrue)
                .exclusive(true),
        )
}

fn command_prefix(args: &[std::ffi::OsString], command: &clap::Command) -> Option<(usize, String)> {
    let partial = command.clone().ignore_errors(true).try_get_matches_from(args).ok()?;
    let name = partial.subcommand_name()?;
    args.iter().enumerate().skip(1).find_map(|(index, arg)| {
        let subcommand = command.find_subcommand(arg.to_str()?)?;
        if subcommand.get_name() != name {
            return None;
        }
        command
            .clone()
            .ignore_errors(true)
            .try_get_matches_from(&args[..=index])
            .ok()
            .filter(|matches| matches.subcommand_name() == Some(name))
            .map(|_| (index, name.to_owned()))
    })
}

/// The global `-C` directories given before the subcommand, read from the arguments up to it so that
/// they are known even when clap stops at `--help` or a usage error after it.
fn change_dirs_before_command(args: &[std::ffi::OsString], command: &clap::Command) -> Vec<std::ffi::OsString> {
    args.iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, arg)| {
            command.find_subcommand(arg.to_str()?)?;
            let partial = command
                .clone()
                .ignore_errors(true)
                .try_get_matches_from(&args[..=index])
                .ok()?;
            partial.subcommand_name()?;
            Some(
                partial
                    .get_many::<std::ffi::OsString>("change_dir")
                    .map(|dirs| dirs.cloned().collect())
                    .unwrap_or_default(),
            )
        })
        .unwrap_or_default()
}

/// The spelling of the global option with clap id `id` as a user types it: the flag found in `args`,
/// else its long name, else its short one, and a short flag that takes a value shows the value too
/// (`-C <DIR>`), never the clap id.
fn typed_spelling(command: &clap::Command, id: &str, args: &[std::ffi::OsString]) -> String {
    let Some(arg) = command.get_arguments().find(|arg| arg.get_id() == id) else {
        return id.to_owned();
    };
    let long = arg.get_long().map(|long| format!("--{long}"));
    let short = arg.get_short().map(|short| format!("-{short}"));
    let typed = |spelling: &String| {
        args.iter().skip(1).any(|candidate| {
            let candidate = candidate.to_string_lossy();
            candidate == spelling.as_str() || candidate.starts_with(&format!("{spelling}="))
        })
    };
    let flag = [&long, &short]
        .into_iter()
        .flatten()
        .find(|spelling| typed(spelling))
        .or(long.as_ref())
        .or(short.as_ref());
    match flag {
        Some(flag) if !flag.starts_with("--") && arg.get_action().takes_values() => {
            let value = arg
                .get_value_names()
                .and_then(|names| names.first())
                .map_or_else(|| "VALUE".to_owned(), |name| name.to_string());
            format!("{flag} <{value}>")
        }
        Some(flag) => flag.clone(),
        None => id.to_owned(),
    }
}

/// Print the long help of the subcommand `matches` selected, for a spelling whose own parser met
/// `--help` among arguments it takes verbatim (like `rev-parse HEAD --help`), and exit with clap's status.
fn subcommand_help(command: &clap::Command, matches: &clap::ArgMatches) -> error::CliError {
    let mut argv = vec!["brit".to_owned()];
    let mut current = matches;
    while let Some((name, sub)) = current.subcommand() {
        argv.push(name.to_owned());
        current = sub;
    }
    argv.push("--help".to_owned());
    match command.clone().try_get_matches_from(argv) {
        Err(help) => {
            let _ = help.print();
            error::CliError::ContextExit(help.exit_code())
        }
        Ok(_) => error::CliError::ContextExit(0),
    }
}

fn normalize_log_counts(args: &mut [std::ffi::OsString], index: usize) {
    // Familiar bounded history spelling, scoped to log and before `--`. Never reinterpret
    // a path after the option terminator or a numeric argument to another command. Let
    // the real parser identify log after root options; a repository named "log" is not it.
    let mut takes_value = false;
    for arg in args.iter_mut().skip(index + 1) {
        if std::mem::take(&mut takes_value) {
            continue;
        }
        if arg == "--" {
            break;
        }
        takes_value = matches!(arg.to_str(), Some("-S" | "-n" | "--max-count" | "--sort"));
        if let Some(count) = arg
            .to_str()
            .and_then(|s| s.strip_prefix('-'))
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        {
            *arg = format!("--max-count={count}").into();
        }
    }
}

/// Commands whose only effect is what they print, each by its exact subcommand path. A reader that
/// closes their output early may end them quietly; every other command's own outcome is reported.
/// A path is admitted only when listed itself: a child of a listed command is not read-only until it
/// is listed too.
///
/// This list is not a census of read-only commands. A path is admitted only after it was checked
/// that the command changes nothing and that everything it prints goes through the watched writer.
/// A read-only command that is not listed was not checked and keeps the previous behaviour on a
/// closed reader.
const READ_ONLY_COMMANDS: &[&[&str]] = &[
    &["rev-parse"],
    &["for-each-ref"],
    &["ls-remote"],
    &["check-ref-format"],
    &["merge-base"],
    &["rev-list"],
    &["log"],
    &["grep"],
    &["cat"],
    &["cat-file"],
    &["ls-tree"],
    &["notes", "show"],
    &["blame"],
    &["diff"],
    &["diff", "tree"],
    &["diff", "file"],
    &["branch", "list"],
    &["revision", "resolve"],
    &["revision", "explain"],
    &["worktree", "list"],
    &["config-tree"],
    &["env"],
    // Admitted after reading each path: no write, and every line through the watched writer.
    &["show"],
    &["tag", "list"],
    &["remote", "url"],
    // `remote refs`, `remote ref-map` and `ls-remote` are one function under three spellings and are
    // treated alike. This list exists so that a command with an effect its caller must learn about never
    // has its outcome replaced by 141. Their only effect is the credential helper's: approve after a
    // successful authentication, erase after a failed one. After an erase the command has failed, and
    // its own failure wins. After an approve the command otherwise succeeded; the stored credential is
    // not something a caller reads from the exit status, and Git dies of SIGPIPE in the same pipeline
    // having done the same thing. Gap: when authentication makes a helper or a prompt write, that output
    // does not go through the watched writer, so on that path a closed reader is not guaranteed to be
    // seen, and the command behaves as an unlisted one.
    &["remote", "refs"],
    &["remote", "ref-map"],
    &["revision", "previous-branches"],
    &["verify"],
    &["fsck"],
    &["tree", "entries"],
    &["tree", "info"],
    &["commit-graph", "list"],
    &["commit-graph", "verify"],
    &["mailmap", "entries"],
    &["mailmap", "check"],
    &["attributes", "query"],
    &["exclude", "query"],
    // Bare `submodule` passes no `--dirty-suffix`, so it never runs the status engine.
    &["submodule"],
    &["config"],
    &["config", "show"],
    &["config", "list"],
    &["index", "entries"],
    &["dirwalk"],
    // Their JSON and DOT output goes through `output::print_line`, the watched writer.
    &["build", "graph", "discover"],
    &["build", "graph", "show"],
    &["build", "affected"],
    &["build", "plan"],
    &["build", "fingerprint"],
    &["build", "baseline", "read"],
    // Left out:
    // - `is-clean`, `is-changed`: predicates whose answer is their exit status, so 141 would replace the
    //   answer a script reads; their status engine can also run a configured clean filter, an external
    //   program with inherited stderr.
    // - `commit describe`, `submodule list`: `--dirty-suffix` on the same path runs that status engine.
    // - bare `tag`: `tag <name>` creates a tag on the same path.
    // - `revision list`: `--svg` writes and opens a file on the same path.
    // - `commit verify`: runs the configured verifier and writes its output to an unwatched stderr.
    // - `attributes validate-baseline`: its children's output is piped, but it reports a failed helper
    //   or a mismatch with a fallible write before returning its own failure, so a closed reader would
    //   replace that failure.
    // - `build run`, `build baseline write|migrate`, `config fmt`, `index from-tree`, `tag delete`,
    //   `commit sign`: they write.
    // Known limits, not fixed here: `--trace` and the verbose progress renderer write to raw stderr
    // outside the watched writer (shared plumbing); and a streaming query ends at its first failed write
    // even if a later input would have failed, as Git killed by SIGPIPE does.
];

fn read_only_path(path: &[&str]) -> bool {
    READ_ONLY_COMMANDS.contains(&path)
}

fn read_only_command(matches: &clap::ArgMatches) -> bool {
    if let Some(sub) = matches.subcommand_matches("maintenance") {
        return sub.get_flag("dry_run");
    }
    let mut path = Vec::new();
    let mut current = matches;
    while let Some((name, sub)) = current.subcommand() {
        path.push(name);
        current = sub;
    }
    read_only_path(&path)
}

/// Whether `error` is a write that met a closed reader, by the typed `ErrorKind::BrokenPipe` of an I/O
/// error in its chain. Message text is never consulted.
fn is_closed_reader_write(error: &error::CliError) -> bool {
    let broken = |kind: Option<std::io::ErrorKind>| kind == Some(std::io::ErrorKind::BrokenPipe);
    match error {
        error::CliError::Io(io) => broken(Some(io.kind())),
        error::CliError::Json(json) => broken(json.io_error_kind()),
        error::CliError::Git(cause) => cause.chain().any(|cause| {
            broken(cause.downcast_ref::<std::io::Error>().map(std::io::Error::kind))
                || broken(
                    cause
                        .downcast_ref::<gix::Error>()
                        .and_then(|error| error.downcast_any_ref::<std::io::Error>())
                        .map(std::io::Error::kind),
                )
                || broken(
                    cause
                        .downcast_ref::<serde_json::Error>()
                        .and_then(serde_json::Error::io_error_kind),
                )
                // JSON written by a plumbing command, like `tree info`, arrives wrapped in a `gix::Error`.
                || broken(
                    cause
                        .downcast_ref::<gix::Error>()
                        .and_then(|error| error.downcast_any_ref::<serde_json::Error>())
                        .and_then(serde_json::Error::io_error_kind),
                )
        }),
        _ => false,
    }
}

/// How a finished run ends: its exit status, and whether its error still has to be reported.
///
/// This is the one place the status is decided. The command's own result is judged first: an error,
/// or a Git-shaped non-zero exit, keeps its own status whatever happened to the reader. Only a command
/// that otherwise succeeded (`Ok`, a zero exit, or an error that is nothing but its write meeting the
/// closed reader) ends with 141, the status of Git killed by SIGPIPE, and only when it is read-only
/// (see [`READ_ONLY_COMMANDS`]) and `reader_closed` says a watched write to stdout or stderr failed
/// with `ErrorKind::BrokenPipe`.
fn exit_status(outcome: &Result<()>, read_only: bool, reader_closed: bool) -> (i32, bool) {
    let quiet_pipe = read_only && reader_closed;
    match outcome {
        Ok(()) | Err(error::CliError::ContextExit(0)) if quiet_pipe => (141, false),
        Ok(()) => (0, false),
        Err(error::CliError::ContextExit(code)) => (*code, false),
        // A Git spelling that answers through its exit code already wrote its diagnostic.
        Err(error::CliError::Git(cause)) if cause.chain().any(|cause| plumbing::exit_status(cause).is_some()) => {
            match cause.chain().find_map(plumbing::exit_status).unwrap_or(1) {
                0 if quiet_pipe => (141, false),
                code => (code, false),
            }
        }
        Err(e) if quiet_pipe && is_closed_reader_write(e) => (141, false),
        Err(e) => (e.exit_code(), true),
    }
}

fn print_parse_error(error: &clap::Error, capture: &mut feedback::Capture) {
    if error.kind() != clap::error::ErrorKind::InvalidSubcommand {
        let _ = error.print();
        return;
    }
    let claim = feedback::claims::UNKNOWN_CAPABILITY;
    capture.claim = claim;
    let rendered = error.render().ansi().to_string();
    let (line, rest) = rendered.split_once('\n').unwrap_or((&rendered, ""));
    let mut stderr = anstream::AutoStream::auto(std::io::stderr().lock());
    let _ = write!(stderr, "{line} [fb {}]\n{rest}", claim.id());
}

fn run(mut args: Vec<std::ffi::OsString>, capture: &mut feedback::Capture, read_only: &mut bool) -> Result<()> {
    let invoked_as_rakia = args
        .first()
        .and_then(|name| std::path::PathBuf::from(name).file_stem().map(|stem| stem == "rakia"))
        .unwrap_or(false);
    if invoked_as_rakia {
        brit_epr::diagnostic::emit("warning: rakia is deprecated; use `brit build` with the same arguments");
        capture.operation("build");
        plumbing::configure_mutation_guards(None, &public_command()).map_err(|err| error::CliError::Git(err.into()))?;
        let cli = Cli::try_parse_from(args).map_err(|error| {
            let code = error.exit_code();
            capture.outcome = Some(if code == 0 { "passed" } else { "refused" });
            print_parse_error(&error, capture);
            error::CliError::ContextExit(code)
        })?;
        return run_build(cli.command);
    }

    // Locate the command after global options before checking whether it needs a bridge.
    let command = public_command();
    // Read the root flag before clap can stop at a later help, version or usage exit.
    capture.quiet = args.iter().position(|arg| arg == "--quiet").is_some_and(|index| {
        command
            .clone()
            .ignore_errors(true)
            .try_get_matches_from(&args[..=index])
            .is_ok_and(|matches| matches.get_flag("capture-quiet"))
    });
    let prefix = command_prefix(&args, &command);
    let command_index = prefix.as_ref().map_or_else(
        || 1 + usize::from(args.get(1).is_some_and(|arg| arg == "--quiet")),
        |(index, _)| *index,
    );
    let native_submodule_update = command
        .find_subcommand("submodule")
        .and_then(|submodule| submodule.find_subcommand("update"))
        .is_some();
    if let Some((line, claim)) = bridge::line(&args, command_index, native_submodule_update) {
        brit_epr::diagnostic::emit_with_claim(line, claim.id());
        capture.claim = claim;
        capture.operation = "bridge";
        capture.outcome = Some("bridge-declared");
        return Err(error::CliError::ContextExit(bridge::EXIT_CODE));
    }

    let mut separator = false;
    if let Some((index, name)) = prefix {
        capture.operation(&name);
        if name == "log" {
            normalize_log_counts(&mut args, index);
        }
        // `diff` and `show` take only flags, so the first `--` after them separates paths.
        separator = matches!(name.as_str(), "diff" | "show") && args.iter().skip(index + 1).any(|arg| arg == "--");
    }
    // Introspection is deliberately standalone. Never dispatch a Git/native command
    // when the caller combines a census request with work-performing arguments.
    if args.get(1).is_some_and(|arg| arg == "--cli-surface-json") {
        capture.operation("surface");
        if args.len() != 2 {
            return Err(error::CliError::Args("--cli-surface-json must be used alone".into()));
        }
        *read_only = true;
        let mut stdout = gitoxide::shared::closed_reader::Watch(std::io::stdout().lock());
        serde_json::to_writer_pretty(&mut stdout, &surface::introspect(command)).map_err(anyhow::Error::from)?;
        writeln!(stdout)?;
        return Ok(());
    }
    let matches = command.clone().try_get_matches_from(&args).map_err(|error| {
        // Git enters every `-C` directory before it reads the command: a missing one wins over help and usage.
        if plumbing::change_dirs(&change_dirs_before_command(&args, &command)).is_err() {
            capture.outcome = Some("refused");
            return error::CliError::ContextExit(128);
        }
        let code = error.exit_code();
        capture.outcome = Some(if code == 0 { "passed" } else { "refused" });
        print_parse_error(&error, capture);
        error::CliError::ContextExit(code)
    })?;
    if matches.subcommand_name() == Some("feedback") {
        capture.skip = true;
        *read_only = true;
        let dirs = change_dirs_before_command(&args, &command);
        if plumbing::change_dirs(&dirs).is_err() {
            return Err(error::CliError::ContextExit(128));
        }
        let FeedbackNamespace::Feedback(feedback::CommandLine::Claims { json }) =
            FeedbackNamespace::from_arg_matches(&matches).map_err(|e| error::CliError::Args(e.to_string()))?;
        return feedback::write_claims(gitoxide::shared::closed_reader::Watch(std::io::stdout().lock()), json);
    }
    if let Some(name) = matches.subcommand_name() {
        capture.operation(name);
    }
    *read_only = read_only_command(&matches);
    capture.skip = *read_only && matches.subcommand_name() == Some("maintenance");
    if matches.get_flag("cli-surface-json") {
        return Err(error::CliError::Args("--cli-surface-json must be used alone".into()));
    }
    if matches!(
        matches.subcommand_name(),
        Some("build" | "snapshot" | "context" | "integrate")
    ) {
        let namespace = matches.subcommand_name().unwrap_or("native");
        // Native writes use the same guards. Only a `--wait` before the first passed-through
        // argument is Brit's; later tokens belong to the namespace (to epr for `context` and `integrate`).
        let wait = matches.get_one::<plumbing::MutationWait>("wait").copied();
        plumbing::configure_mutation_guards(wait, &command).map_err(|err| error::CliError::Git(err.into()))?;
        if let Some(option) = matches
            .ids()
            .filter(|id| {
                !matches!(id.as_str(), "wait" | "capture-quiet")
                    && command.get_arguments().any(|arg| arg.get_id() == *id)
            })
            .find(|id| matches.value_source(id.as_str()) == Some(ValueSource::CommandLine))
        {
            let root_option = if matches!(namespace, "context" | "integrate") {
                "--root"
            } else {
                "--repo"
            };
            return Err(error::CliError::Args(format!(
                "global Git option `{}` does not apply to `{namespace}`; use the {namespace} command's `{root_option}` option",
                typed_spelling(&command, option.as_str(), &args)
            )));
        }
        let build = BuildNamespace::from_arg_matches(&matches).map_err(|e| error::CliError::Args(e.to_string()))?;
        match build {
            BuildNamespace::Integrate { args } => context::integrate(args),
            BuildNamespace::Context { args } => context::run(args),
            BuildNamespace::Build(command) => run_build(command),
            BuildNamespace::Snapshot(command) => tree::run(command).map_err(error::CliError::Git),
        }
    } else {
        let mut args = plumbing::Args::from_arg_matches(&matches).map_err(|e| {
            if e.kind() == clap::error::ErrorKind::DisplayHelp {
                return subcommand_help(&command, &matches);
            }
            error::CliError::Args(e.to_string())
        })?;
        if separator {
            args.note_path_separator();
        }
        plumbing::run_with_command(args, command).map_err(|err| {
            // `diff --exit-code` and `--quiet` report differences by status alone, like Git.
            if err.downcast_any_ref::<plumbing::Differences>().is_some() {
                error::CliError::ContextExit(1)
            } else {
                error::CliError::Git(err.into())
            }
        })?;
        capture.note_successful_form(&matches);
        Ok(())
    }
}

fn diagnostic(error: &error::CliError) -> String {
    match error {
        error::CliError::Git(cause) => match error::slot_exhaustion_report(cause.as_ref()) {
            Some(report) => format!("Error: {report}"),
            // The shared printer: the cause chain, last, and a backtrace only on `BRIT_BACKTRACE`.
            None => brit_epr::diagnostic::report(cause.as_ref(), Some(cause.backtrace())),
        },
        _ => format!("error: {error}"),
    }
}

/// The stack every command runs on. Windows gives the main thread 1 MiB, and a debug build's
/// argument parsing and dispatch frames outgrew it; an explicit size is the same on every host.
const COMMAND_STACK_BYTES: usize = 16 * 1024 * 1024;

fn main() -> ExitCode {
    // Named `main` so panic and backtrace text reads as it did on the process's own thread.
    match std::thread::Builder::new()
        .name("main".into())
        .stack_size(COMMAND_STACK_BYTES)
        .spawn(run_command)
    {
        // A panic was already reported by the hook; exit as a panicking main thread does.
        Ok(worker) => worker.join().unwrap_or(ExitCode::from(101)),
        Err(_) => run_command(),
    }
}

fn run_command() -> ExitCode {
    brit_epr::diagnostic::install_panic_hook("brit", env!("CARGO_PKG_VERSION"));
    gix::utils::stream::Budget::share_process_quota();
    let mut capture = feedback::Capture::new();
    let mut read_only = false;
    let outcome = run(plumbing::args_os().collect(), &mut capture, &mut read_only);
    if read_only {
        // Bytes still in stdout's line buffer (a short blob or `[]` without a newline) would otherwise
        // meet a closed reader only at process exit, after the status was chosen.
        let _ = gitoxide::shared::closed_reader::Watch(std::io::stdout()).flush();
    }
    // The status is decided first and never by the report: the report is attempted once with its
    // result ignored (a reader of stderr may have closed, and `eprintln!` would then panic and exit
    // 101), and the run is recorded after it whatever the write did.
    let (code, report) = exit_status(&outcome, read_only, gitoxide::shared::closed_reader::seen());
    let refusal = if let Err(error::CliError::Git(cause)) = &outcome {
        cause
            .chain()
            .find_map(|error| {
                error.downcast_ref::<plumbing::FeedbackRefusal>().or_else(|| {
                    error
                        .downcast_ref::<gix::Error>()?
                        .downcast_any_ref::<plumbing::FeedbackRefusal>()
                })
            })
            .map(|refusal| feedback::claims::refusal(refusal.code))
    } else {
        None
    };
    if let Some(claim) = refusal {
        capture.claim = claim;
    }
    if let (true, Err(e)) = (report, &outcome) {
        if matches!(e, error::CliError::Args(_)) {
            capture.outcome = Some("refused");
        }
        if let Some(claim) = refusal {
            brit_epr::diagnostic::emit_with_claim(diagnostic(e), claim.id());
        } else {
            brit_epr::diagnostic::emit(diagnostic(e));
        }
    }
    capture.finish(code);
    ExitCode::from(code as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_clap_tree_is_valid() {
        public_command().debug_assert();
    }

    #[test]
    fn no_bridge_row_shadows_an_owned_verb() {
        let command = public_command();
        let owned: Vec<&str> = command
            .get_subcommands()
            .flat_map(|sub| std::iter::once(sub.get_name()).chain(sub.get_all_aliases()))
            .collect();
        for name in bridge::BRIDGED {
            assert!(!owned.contains(name), "`{name}` is bridged but brit owns it");
        }
        let submodule = command
            .find_subcommand("submodule")
            .expect("brit owns `submodule list`");
        for verb in bridge::BRIDGED_SUBMODULE {
            assert!(
                submodule.find_subcommand(verb).is_none(),
                "`submodule {verb}` is bridged but brit owns it"
            );
        }
        assert!(
            submodule.find_subcommand("list").is_some(),
            "`submodule list` stays owned by brit"
        );
    }

    #[test]
    fn bridge_refusals_exit_with_the_documented_usage_code() {
        assert_eq!(bridge::EXIT_CODE, error::CliError::Args(String::new()).exit_code());
    }

    #[test]
    fn switch_exposes_only_existing_branch_and_explicit_recovery() {
        for args in [
            vec!["brit", "switch", "main"],
            vec!["brit", "switch", "refs/heads/topic"],
            vec!["brit", "switch", "--status"],
            vec!["brit", "switch", "--continue"],
            vec!["brit", "switch", "--abort"],
        ] {
            assert!(
                public_command().try_get_matches_from(&args).is_ok(),
                "supported switch: {args:?}"
            );
        }
        for args in [
            vec!["brit", "switch"],
            vec!["brit", "switch", "one", "two"],
            vec!["brit", "switch", "main", "--status"],
            vec!["brit", "switch", "--continue", "--abort"],
            vec!["brit", "switch", "--status", "--continue"],
            vec!["brit", "switch", "--status", "--abort"],
            vec!["brit", "switch", "-c", "new"],
            vec!["brit", "switch", "--force", "main"],
            vec!["brit", "switch", "--detach", "main"],
            vec!["brit", "switch", "--track", "main"],
            vec!["brit", "switch", "--guess", "main"],
            vec!["brit", "switch", "--recurse-submodules", "main"],
        ] {
            assert!(
                public_command().try_get_matches_from(&args).is_err(),
                "deferred or ambiguous switch: {args:?}"
            );
        }
    }

    #[cfg(any(feature = "max", feature = "max-pure", feature = "lean"))]
    #[test]
    fn pull_requires_explicit_fast_forward_selection_or_recovery() {
        for args in [
            vec!["brit", "pull", "--ff-only", "origin", "main"],
            vec!["brit", "pull", "--ff-only", "../remote", "refs/heads/main"],
            vec!["brit", "pull", "--ff-only", "https://example.test/repo", "main"],
            vec!["brit", "pull", "--status"],
            vec!["brit", "pull", "--continue"],
            vec!["brit", "pull", "--abort"],
        ] {
            assert!(
                public_command().try_get_matches_from(&args).is_ok(),
                "supported pull: {args:?}"
            );
        }
        for args in [
            vec!["brit", "pull"],
            vec!["brit", "pull", "origin", "main"],
            vec!["brit", "pull", "--ff-only"],
            vec!["brit", "pull", "--ff-only", "origin"],
            vec!["brit", "pull", "--ff-only", "origin", "main", "other"],
            vec!["brit", "pull", "--status", "--continue"],
            vec!["brit", "pull", "--continue", "--abort"],
            vec!["brit", "pull", "--status", "--ff-only", "origin", "main"],
            vec!["brit", "pull", "--rebase", "origin", "main"],
            vec!["brit", "pull", "--ff-only", "--autostash", "origin", "main"],
        ] {
            assert!(
                public_command().try_get_matches_from(&args).is_err(),
                "unsupported pull: {args:?}"
            );
        }
    }

    #[test]
    fn broader_staging_requires_literal_paths_or_a_repository_wide_flag() {
        for args in [
            vec!["brit", "add", "file", "directory/"],
            vec!["brit", "add", "--", "file", "directory/"],
            vec!["brit", "add", "-u"],
            vec!["brit", "add", "-A"],
            vec!["brit", "add", "--update", "--", "directory"],
            vec!["brit", "add", "--all", "--", "-literal"],
        ] {
            assert!(
                public_command().try_get_matches_from(&args).is_ok(),
                "supported staging arguments: {args:?}"
            );
        }
        for args in [
            vec!["brit", "add"],
            vec!["brit", "add", "-u", "-A"],
            vec!["brit", "add", "-f", "--", "file"],
            vec!["brit", "add", "-p"],
            vec!["brit", "restore", "--worktree"],
        ] {
            assert!(
                public_command().try_get_matches_from(&args).is_err(),
                "deferred or ambiguous staging arguments: {args:?}"
            );
        }
    }

    #[test]
    fn git_errors_preserve_anyhow_main_diagnostics() {
        use anyhow::Context;

        let cause = Err::<(), _>(anyhow::anyhow!("root cause"))
            .context("operation failed")
            .expect_err("the synthetic Git failure must remain an error");
        let diagnostic = diagnostic(&error::CliError::Git(cause));
        assert_eq!(
            diagnostic, "Error: operation failed\n\nCaused by:\n    root cause",
            "Git errors must keep the old anyhow::Result main output, including causes"
        );
    }

    #[test]
    fn git_error_causes_keep_numbering_and_multiline_indentation() {
        let cause = anyhow::anyhow!("root\nsecond line")
            .context("middle")
            .context("operation failed");
        assert_eq!(
            diagnostic(&error::CliError::Git(cause)),
            "Error: operation failed\n\nCaused by:\n    0: middle\n    1: root\n       second line"
        );
    }

    #[test]
    fn git_error_diagnostics_ignore_backtrace_environment() {
        for value in ["0", "1", "full"] {
            let output = std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args(["--exact", "tests::git_errors_preserve_anyhow_main_diagnostics"])
                .env("RUST_BACKTRACE", value)
                .env("RUST_LIB_BACKTRACE", value)
                .output()
                .expect("isolated diagnostic test");
            assert!(output.status.success(), "RUST_BACKTRACE={value}: {output:?}");
        }
    }

    #[test]
    fn a_closed_reader_never_replaces_the_commands_own_failure() {
        let git = |text: &str| -> Result<()> { Err(error::CliError::Git(anyhow::anyhow!("{text}"))) };
        let typed = |code: i32| -> Result<()> {
            Err(error::CliError::Git(anyhow::Error::from(gix::Error::from_error(
                plumbing::GitExit(code),
            ))))
        };
        let pipe = || std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        let pipe_write = || -> Result<()> {
            Err(error::CliError::Git(anyhow::Error::from(
                gix::Error::from_error(pipe()),
            )))
        };

        // Failed, and the reader closed too: the failure's own status.
        assert_eq!(
            exit_status(&typed(128), true, true),
            (128, false),
            "rev-parse --verify missing"
        );
        assert_eq!(exit_status(&typed(1), true, true), (1, false));
        assert_eq!(exit_status(&git("bad revision"), true, true), (1, true));
        assert_eq!(
            exit_status(&git("could not read x: Broken pipe (os error 32)"), true, true),
            (1, true),
            "an error that merely contains the text is reported"
        );
        // Succeeded, or failed only at the write that met the closed reader: 141.
        assert_eq!(exit_status(&Ok(()), true, true), (141, false));
        assert_eq!(exit_status(&typed(0), true, true), (141, false));
        assert_eq!(exit_status(&pipe_write(), true, true), (141, false));
        assert_eq!(exit_status(&Err(error::CliError::Io(pipe())), true, true), (141, false));
        // Not read-only, or no watched write saw the closed reader: reported as it is.
        assert_eq!(
            exit_status(&pipe_write(), false, true),
            (1, true),
            "a mutation's own outcome"
        );
        assert_eq!(
            exit_status(&pipe_write(), true, false),
            (1, true),
            "an unwatched broken pipe"
        );
        assert_eq!(exit_status(&Ok(()), false, true), (0, false));
        assert_eq!(exit_status(&Ok(()), true, false), (0, false));
    }

    #[test]
    fn read_only_commands_are_classified_by_their_exact_path() {
        let read_only = |args: &[&str]| {
            read_only_command(
                &public_command()
                    .try_get_matches_from(args)
                    .unwrap_or_else(|e| panic!("{args:?}: {e}")),
            )
        };
        for args in [
            &["brit", "rev-parse", "HEAD"][..],
            &["brit", "for-each-ref"],
            &["brit", "branch", "list"],
            &["brit", "branches", "list"],
            &["brit", "log"],
            &["brit", "diff"],
        ] {
            assert!(read_only(args), "{args:?}");
        }
        for args in [
            &["brit", "push", "origin", "main"][..],
            &["brit", "branch", "create", "x"],
            &["brit", "commit", "-m", "x"],
            &["brit", "add", "x"],
            &["brit", "revision", "list", "HEAD"],
        ] {
            assert!(!read_only(args), "{args:?}");
        }
        assert!(read_only_path(&["diff", "tree"]));
        assert!(
            !read_only_path(&["rev-parse", "child"]),
            "a path that only has a read-only prefix is not read-only"
        );
        assert!(!read_only_path(&["branch"]));
    }

    #[test]
    fn the_extension_admits_read_only_paths_and_leaves_writers_out() {
        let read_only = |args: &[&str]| {
            read_only_command(
                &public_command()
                    .try_get_matches_from(args)
                    .unwrap_or_else(|e| panic!("{args:?}: {e}")),
            )
        };
        for args in [
            &["brit", "show", "HEAD"][..],
            &["brit", "tag", "list"],
            &["brit", "remote", "refs"],
            &["brit", "remote", "ref-map"],
            &["brit", "config"],
            &["brit", "submodule"],
            &["brit", "build", "graph", "discover"],
            &["brit", "build", "fingerprint", "build-manifest.json"],
            &["brit", "build", "baseline", "read", "p"],
        ] {
            assert!(read_only(args), "{args:?}");
        }
        for args in [
            &["brit", "tag", "v1"][..],
            &["brit", "tag"],
            &["brit", "commit", "verify"],
            &["brit", "attributes", "validate-baseline"],
            &["brit", "is-clean"],
            &["brit", "is-changed"],
            &["brit", "commit", "describe"],
            &["brit", "submodule", "list"],
            &["brit", "config", "fmt"],
            &["brit", "config", "set", "a.b", "c"],
            &["brit", "config", "unset", "a.b"],
            &["brit", "build", "baseline", "write", "p", "HEAD"],
            &["brit", "build", "baseline", "migrate", "x.json"],
        ] {
            assert!(!read_only(args), "{args:?}");
        }
    }

    #[test]
    fn json_from_a_plumbing_command_that_met_the_closed_reader_is_recognised() {
        let pipe = std::io::Error::from(std::io::ErrorKind::BrokenPipe);
        let json = serde_json::Error::io(pipe);
        let wrapped = error::CliError::Git(anyhow::Error::from(gix::Error::from_error(json)));
        assert!(
            is_closed_reader_write(&wrapped),
            "`tree info` writes JSON through `.or_error()`"
        );
        let other = serde_json::from_str::<u8>("x").expect_err("not JSON");
        let wrapped = error::CliError::Git(anyhow::Error::from(gix::Error::from_error(other)));
        assert!(
            !is_closed_reader_write(&wrapped),
            "a JSON syntax error is not the closed reader"
        );
    }

    #[test]
    fn build_errors_keep_their_existing_format() {
        assert_eq!(
            diagnostic(&error::CliError::Args("bad option".into())),
            "error: invalid arguments: bad option",
            "build errors must not acquire the legacy Git prefix"
        );
    }
}
