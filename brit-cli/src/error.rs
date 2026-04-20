//! Error types and exit code mapping for the brit CLI.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("repo not found at {path}: {source}")]
    RepoNotFound {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("manifest discovery failed: {0}")]
    ManifestDiscovery(String),

    #[error("constellation construction failed: {0}")]
    Constellation(#[from] rakia_core::constellation::ConstellationError),

    #[error("change detection failed: {0}")]
    ChangeDetection(String),

    #[error("baseline operation failed: {0}")]
    Baseline(String),

    #[error("invalid arguments: {0}")]
    Args(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CliError {
    /// Map error variants to exit codes.
    /// 0 — success (not used here)
    /// 1 — generic failure
    /// 2 — argument/usage error
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Args(_) => 2,
            _ => 1,
        }
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
