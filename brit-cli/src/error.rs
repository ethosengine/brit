//! Error types and exit code mapping for the brit CLI.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("epr context exited with status {0}")]
    ContextExit(i32),
    #[error(transparent)]
    Git(#[from] anyhow::Error),
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

/// Format an error without exposing arbitrary source diagnostics.
///
/// Configuration parsing errors can contain input bytes, so only the outer
/// message is retained. Slot exhaustion is the one targeted exception because
/// its typed fields are safe and actionable.
pub(crate) fn format_error(error: &(dyn std::error::Error + 'static)) -> String {
    let message = error.to_string();
    if let Some((current, additional)) = slot_exhaustion(error) {
        return format!(
            "{message}: object database slots exhausted (current: {current}, needed: {})",
            current + additional
        );
    }
    message
}

fn slot_exhaustion(error: &(dyn std::error::Error + 'static)) -> Option<(usize, usize)> {
    let mut source = Some(error);
    while let Some(cause) = source {
        if let Some(gix::odb::store::load_index::Error::InsufficientSlots { current, needed }) =
            cause.downcast_ref::<gix::odb::store::load_index::Error>()
        {
            return Some((*current, *needed));
        }
        source = cause.source();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("outer failure")]
    struct Outer {
        #[source]
        source: Inner,
    }

    #[derive(Debug, thiserror::Error)]
    #[error("inner cause")]
    struct Inner;

    #[test]
    fn keeps_only_the_outer_error() {
        assert_eq!(
            format_error(&Outer { source: Inner }),
            "outer failure",
            "string-backed CLI diagnostics do not expose arbitrary source text"
        );
    }

    #[test]
    fn adds_only_slot_counts_for_slot_exhaustion() {
        #[derive(Debug, thiserror::Error)]
        #[error("object lookup failed")]
        struct Lookup {
            #[source]
            source: gix::odb::store::load_index::Error,
        }

        assert_eq!(
            format_error(&Lookup {
                source: gix::odb::store::load_index::Error::InsufficientSlots { current: 32, needed: 1 },
            }),
            "object lookup failed: object database slots exhausted (current: 32, needed: 33)",
            "the targeted diagnostic reports capacity without rendering its source"
        );
    }
}
