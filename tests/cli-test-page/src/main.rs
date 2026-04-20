//! brit-test-page — runs the brit CLI test suite, produces baseline.md.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

mod coverage;
mod diff;
mod discover;
mod format;
mod normalize;

#[derive(Parser)]
#[command(name = "brit-test-page", version, about = "Run the brit CLI test suite and produce a markdown test page")]
struct Cli {
    /// Path to the brit workspace root (default: parent of this binary's location)
    #[arg(long)]
    workspace: Option<PathBuf>,

    /// Mode: check (default), update, or candidate
    #[command(flatten)]
    mode: Mode,
}

#[derive(clap::Args)]
#[group(multiple = false)]
struct Mode {
    /// Default mode: diff candidate vs baseline; exit 1 on mismatch
    #[arg(long, conflicts_with_all = ["update", "candidate"])]
    check: bool,
    /// Copy candidate over baseline.md (after human review of diff)
    #[arg(long, conflicts_with_all = ["check", "candidate"])]
    update: bool,
    /// Write candidate to arbitrary path (for the desired-then-iterate TDD loop)
    #[arg(long, value_name = "PATH", conflicts_with_all = ["check", "update"])]
    candidate: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    eprintln!("brit-test-page: scaffold — modes parsed but not yet implemented");
    eprintln!("  workspace: {:?}", cli.workspace);
    eprintln!("  check: {}, update: {}, candidate: {:?}",
        cli.mode.check, cli.mode.update, cli.mode.candidate);
    ExitCode::SUCCESS
}
