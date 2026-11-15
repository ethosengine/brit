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

/// Find the object store's slot-capacity diagnostic anywhere in the cause chain.
///
/// gix-odb raises it as a [`gix::error::Message`] carrying the unsigned
/// metadata `current` (slot count) and `needed` (additional slots). Only those
/// two numbers are read; the message's other values are never rendered.
fn slot_exhaustion(error: &(dyn std::error::Error + 'static)) -> Option<(u64, u64)> {
    let mut source = Some(error);
    while let Some(cause) = source {
        let capacity = if let Some(error) = cause.downcast_ref::<gix::Error>() {
            error.metadata().find_map(slot_capacity)
        } else {
            cause
                .downcast_ref::<gix::error::Message>()
                .and_then(|message| slot_capacity(&message.values))
        };
        if capacity.is_some() {
            return capacity;
        }
        source = cause.source();
    }
    None
}

fn slot_capacity(values: &gix::error::Metadata) -> Option<(u64, u64)> {
    match (values.get("current")?, values.get("needed")?) {
        (gix::error::MetadataValue::U64(current), gix::error::MetadataValue::U64(needed)) => Some((*current, *needed)),
        _ => None,
    }
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
            source: gix::Error,
        }

        let exhausted = gix::Error::from_error(
            gix::error::Message::new("The object database has too few index slots")
                .with("current", 32_usize)
                .with("needed", 1_usize)
                .with("path", "SYNTHETIC_SECRET_PATH"),
        );
        assert_eq!(
            format_error(&Lookup { source: exhausted }),
            "object lookup failed: object database slots exhausted (current: 32, needed: 33)",
            "the targeted diagnostic reports capacity without rendering its source"
        );
    }
}
