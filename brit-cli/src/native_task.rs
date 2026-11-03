//! Thin CLI delegation. Ark alone validates, admits, supervises and answers the task.

use std::{path::PathBuf, process::Command};

use crate::error::{CliError, Result};

#[derive(clap::Args)]
pub struct RunArgs {
    #[arg(long)]
    root: String,
    #[arg(long)]
    store: PathBuf,
    #[arg(long)]
    request: PathBuf,
    #[arg(long)]
    artifact: PathBuf,
    #[arg(long)]
    run_dir: PathBuf,
}

pub fn run(args: RunArgs) -> Result<()> {
    // These paths are private immutable custody and a fresh run layout. Git discovery and
    // repository writer leases belong to Git mutations, which this command never performs.
    // An explicit runtime selection must never silently execute a different PATH artifact.
    // Preserve byte-oriented paths; an empty or invalid selection fails at exec.
    let ark = std::env::var_os("ARK_BIN").unwrap_or_else(|| "ark".into());
    let mut command = Command::new(ark);
    command.args(["task", "run", "--root", &args.root]);
    command.arg("--store").arg(args.store);
    command.arg("--request").arg(args.request);
    command.arg("--artifact").arg(args.artifact);
    command.arg("--run-dir").arg(args.run_dir);
    // Replace the transport process so signals go directly to Ark, the single supervisor.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(CliError::Git(anyhow::anyhow!(
            "could not execute installed ark: {}",
            command.exec()
        )))
    }
    #[cfg(not(unix))]
    {
        let _ = command;
        Err(CliError::Git(anyhow::anyhow!("native tree tasks require Linux")))
    }
}
