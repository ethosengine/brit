//! One executable for Git plumbing and build operations.

use std::{path::PathBuf, process::ExitCode};

use clap::parser::ValueSource;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use gitoxide::plumbing;

mod commands;
mod error;
mod output;

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
    /// Build graph, affected steps, plans, fingerprints and baselines.
    #[command(subcommand)]
    Build(Command),
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

    let command = BuildNamespace::augment_subcommands(plumbing::Args::command())
        .version(env!("CARGO_PKG_VERSION"))
        .long_version(VERSION_DETAIL);
    let matches = command.clone().get_matches_from(args);
    if matches.subcommand_name() == Some("build") {
        if let Some(option) = matches
            .ids()
            .find(|id| matches.value_source(id.as_str()) == Some(ValueSource::CommandLine))
        {
            return Err(error::CliError::Args(format!(
                "global Git option `{option}` does not apply to `build`; use the build command's `--repo` option"
            )));
        }
        let build = BuildNamespace::from_arg_matches(&matches).map_err(|e| error::CliError::Args(e.to_string()))?;
        let BuildNamespace::Build(command) = build;
        run_build(command)
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
        BuildNamespace::augment_subcommands(plumbing::Args::command()).debug_assert();
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
