//! `brit-build-ref` — CLI for build/deploy/validate/reach attestation refs.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod build_cmd;
mod deploy_cmd;
mod reach_cmd;
mod validate_cmd;

#[derive(Parser)]
#[command(name = "brit-build-ref", about = "Manage build/deploy/validate/reach attestation refs")]
struct Cli {
    /// Path to the git repository (default: current directory).
    #[arg(long, default_value = ".")]
    repo: PathBuf,

    #[command(subcommand)]
    command: TopCommand,
}

#[derive(Subcommand)]
enum TopCommand {
    /// Build attestation refs.
    Build {
        #[command(subcommand)]
        cmd: BuildCmd,
    },
    /// Deploy attestation refs.
    Deploy {
        #[command(subcommand)]
        cmd: DeployCmd,
    },
    /// Validation attestation refs.
    Validate {
        #[command(subcommand)]
        cmd: ValidateCmd,
    },
    /// Reach level computation.
    Reach {
        #[command(subcommand)]
        cmd: ReachCmd,
    },
}

// ─── Build subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum BuildCmd {
    /// Record a build attestation.
    Put {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
        /// CID of the build manifest.
        #[arg(long)]
        manifest: String,
        /// CID of the artifact produced.
        #[arg(long)]
        output: String,
        /// Content hash of all declared inputs at build time.
        #[arg(long)]
        inputs_hash: String,
        /// Whether the build succeeded.
        #[arg(long, default_value_t = true)]
        success: bool,
        /// Hardware profile JSON.
        #[arg(long, default_value = "{}")]
        hardware: String,
        /// Build duration in milliseconds.
        #[arg(long, default_value_t = 0)]
        duration_ms: u64,
        /// Git commit revision.
        #[arg(long, default_value = "HEAD")]
        commit: String,
    },
    /// Retrieve a build attestation.
    Get {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
        /// Git commit revision.
        #[arg(long, default_value = "HEAD")]
        commit: String,
    },
    /// List all build attestation step names.
    List,
}

// ─── Deploy subcommands ───────────────────────────────────────────────────────

#[derive(Subcommand)]
enum DeployCmd {
    /// Record a deploy attestation.
    Put {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
        /// Deployment environment label.
        #[arg(long)]
        env: String,
        /// CID of the artifact deployed.
        #[arg(long)]
        artifact: String,
        /// Base URL of the deployed service.
        #[arg(long)]
        endpoint: String,
        /// EPR reference for the liveness health check.
        #[arg(long)]
        health_check_epr: String,
        /// Health status (healthy|degraded|unreachable).
        #[arg(long, default_value = "healthy")]
        health: String,
        /// Liveness TTL in seconds.
        #[arg(long, default_value_t = 300)]
        ttl: u64,
    },
    /// Retrieve a deploy attestation.
    Get {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
        /// Deployment environment label.
        #[arg(long)]
        env: String,
    },
    /// List all deploy attestation names.
    List,
}

// ─── Validate subcommands ─────────────────────────────────────────────────────

#[derive(Subcommand)]
enum ValidateCmd {
    /// Record a validation attestation.
    Put {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
        /// Check name (e.g. lint@v1).
        #[arg(long)]
        check: String,
        /// CID of the artifact validated.
        #[arg(long)]
        artifact: String,
        /// Validation result (pass|fail|warn|skip).
        #[arg(long)]
        result: String,
        /// Human-readable result summary.
        #[arg(long, default_value = "")]
        summary: String,
        /// Version of the validator tool.
        #[arg(long, default_value = "")]
        validator_version: String,
    },
    /// Retrieve a validation attestation.
    Get {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
        /// Check name.
        #[arg(long)]
        check: String,
    },
    /// List all validation attestation names.
    List,
}

// ─── Reach subcommands ────────────────────────────────────────────────────────

#[derive(Subcommand)]
enum ReachCmd {
    /// Compute and store reach level for a step.
    Compute {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
    },
    /// Retrieve the stored reach level.
    Get {
        /// Pipeline step name.
        #[arg(long)]
        step: String,
    },
}

// ─── Entry point ──────────────────────────────────────────────────────────────

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let repo = cli.repo.canonicalize()
        .unwrap_or_else(|_| cli.repo.clone());

    match cli.command {
        TopCommand::Build { cmd } => match cmd {
            BuildCmd::Put { step, manifest, output, inputs_hash, success, hardware, duration_ms, commit } => {
                build_cmd::put(&repo, &step, &manifest, &output, &inputs_hash, success, &hardware, duration_ms, &commit)
            }
            BuildCmd::Get { step, commit } => build_cmd::get(&repo, &step, &commit),
            BuildCmd::List => build_cmd::list(&repo),
        },

        TopCommand::Deploy { cmd } => match cmd {
            DeployCmd::Put { step, env, artifact, endpoint, health_check_epr, health, ttl } => {
                use brit_epr::elohim::attestation::deploy::HealthStatus;
                let health_status = match health.as_str() {
                    "healthy" => HealthStatus::Healthy,
                    "degraded" => HealthStatus::Degraded,
                    "unreachable" => HealthStatus::Unreachable,
                    other => anyhow::bail!("unknown --health value: {other} (expected healthy|degraded|unreachable)"),
                };
                deploy_cmd::put(&repo, &step, &env, &artifact, &endpoint, &health_check_epr, health_status, ttl)
            }
            DeployCmd::Get { step, env } => deploy_cmd::get(&repo, &step, &env),
            DeployCmd::List => deploy_cmd::list(&repo),
        },

        TopCommand::Validate { cmd } => match cmd {
            ValidateCmd::Put { step, check, artifact, result, summary, validator_version } => {
                use brit_epr::elohim::attestation::validation::ValidationResult;
                let vr = match result.as_str() {
                    "pass" => ValidationResult::Pass,
                    "fail" => ValidationResult::Fail,
                    "warn" => ValidationResult::Warn,
                    "skip" => ValidationResult::Skip,
                    other => anyhow::bail!("unknown --result value: {other} (expected pass|fail|warn|skip)"),
                };
                validate_cmd::put(&repo, &step, &check, &artifact, vr, &summary, &validator_version)
            }
            ValidateCmd::Get { step, check } => validate_cmd::get(&repo, &step, &check),
            ValidateCmd::List => validate_cmd::list(&repo),
        },

        TopCommand::Reach { cmd } => match cmd {
            ReachCmd::Compute { step } => reach_cmd::compute(&repo, &step),
            ReachCmd::Get { step } => reach_cmd::get(&repo, &step),
        },
    }
}
