//! One executable for Git plumbing and build operations.

use std::{path::PathBuf, process::ExitCode};

use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use gitoxide::plumbing;

mod commands;
mod context;
mod error;
mod output;
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
    /// Graph operations on the build constellation
    #[command(subcommand)]
    Graph(GraphCmd),
    /// Show which steps are affected by changes
    Affected(AffectedArgs),
    /// Compute a topologically-grouped build plan
    Plan(PlanArgs),
    /// Compute the content fingerprint of a step's inputs
    Fingerprint(FingerprintArgs),
    /// Manage rakia baseline refs
    #[command(subcommand)]
    Baseline(BaselineCmd),
}

#[derive(Subcommand)]
enum BuildNamespace {
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
        Command::Graph(GraphCmd::Discover { repo }) => commands::graph_discover::run(&repo),
        Command::Graph(GraphCmd::Show { repo, format }) => commands::graph_show::run(&repo, &format),
        Command::Affected(args) => commands::affected::run(&args.repo, args.files.as_deref(), args.since.as_deref()),
        Command::Plan(args) => commands::plan::run(
            &args.repo,
            args.files.as_deref(),
            args.since.as_deref(),
            args.pipeline.as_deref(),
        ),
        Command::Fingerprint(args) => commands::fingerprint::run(&args.manifest, args.step.as_deref(), &args.commit),
        Command::Baseline(BaselineCmd::Read { pipeline, repo }) => commands::baseline::read(&repo, &pipeline),
        Command::Baseline(BaselineCmd::Write { pipeline, commit, repo }) => {
            commands::baseline::write(&repo, &pipeline, &commit)
        }
        Command::Baseline(BaselineCmd::Migrate { json_path, repo }) => commands::baseline::migrate(&repo, &json_path),
    }
}

fn public_command() -> clap::Command {
    BuildNamespace::augment_subcommands(plumbing::Args::command())
        .version(env!("CARGO_PKG_VERSION"))
        .long_version(VERSION_DETAIL)
        .arg(
            clap::Arg::new("cli-surface-json")
                .long("cli-surface-json")
                .help("Print the compiled parser surface as JSON; no behavior or acceptance claim")
                .action(clap::ArgAction::SetTrue)
                .exclusive(true),
        )
}

fn run() -> Result<()> {
    let args: Vec<_> = plumbing::args_os().collect();
    let invoked_as_rakia = args
        .first()
        .and_then(|name| std::path::PathBuf::from(name).file_stem().map(|stem| stem == "rakia"))
        .unwrap_or(false);
    if invoked_as_rakia {
        eprintln!("warning: rakia is deprecated; use `brit build` with the same arguments");
        return run_build(Cli::parse_from(args).command);
    }

    let command = public_command();
    // Introspection is deliberately standalone. Never dispatch a Git/native command
    // when the caller combines a census request with work-performing arguments.
    if args.get(1).is_some_and(|arg| arg == "--cli-surface-json") {
        if args.len() != 2 {
            return Err(error::CliError::Args("--cli-surface-json must be used alone".into()));
        }
        let stdout = std::io::stdout();
        serde_json::to_writer_pretty(stdout.lock(), &surface::introspect(command)).map_err(anyhow::Error::from)?;
        println!();
        return Ok(());
    }
    let matches = command.clone().get_matches_from(args);
    if matches.get_flag("cli-surface-json") {
        return Err(error::CliError::Args("--cli-surface-json must be used alone".into()));
    }
    if matches!(matches.subcommand_name(), Some("build" | "snapshot" | "context")) {
        let namespace = matches.subcommand_name().unwrap_or("native");
        if let Some(option) = matches
            .ids()
            .find(|id| matches.value_source(id.as_str()) == Some(ValueSource::CommandLine))
        {
            let root_option = if namespace == "context" { "--root" } else { "--repo" };
            return Err(error::CliError::Args(format!(
                "global Git option `{option}` does not apply to `{namespace}`; use the {namespace} command's `{root_option}` option"
            )));
        }
        let build = BuildNamespace::from_arg_matches(&matches).map_err(|e| error::CliError::Args(e.to_string()))?;
        match build {
            BuildNamespace::Context { args } => context::run(args),
            BuildNamespace::Build(command) => run_build(command),
            BuildNamespace::Snapshot(command) => tree::run(command).map_err(error::CliError::Git),
        }
    } else {
        let args = plumbing::Args::from_arg_matches(&matches).map_err(|e| error::CliError::Args(e.to_string()))?;
        plumbing::run_with_command(args, command).map_err(error::CliError::Git)
    }
}

fn diagnostic(error: &error::CliError) -> String {
    match error {
        // Match the old `fn main() -> anyhow::Result<()>` termination output,
        // including anyhow's multi-cause Debug formatting.
        error::CliError::Git(cause) => format!("Error: {cause:?}"),
        _ => format!("error: {error}"),
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error::CliError::ContextExit(code)) => std::process::exit(code),
        Err(e) => {
            eprintln!("{}", diagnostic(&e));
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_clap_tree_is_valid() {
        public_command().debug_assert();
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
    fn build_errors_keep_their_existing_format() {
        assert_eq!(
            diagnostic(&error::CliError::Args("bad option".into())),
            "error: invalid arguments: bad option",
            "build errors must not acquire the legacy Git prefix"
        );
    }
}
