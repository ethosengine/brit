//! Fault-only rendezvous. Release returns to the production cancellation poll.
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

pub(super) fn checkpoint(point: &str) -> Result<()> {
    if std::env::var("BRIT_SWITCH_CHECKPOINT").ok().as_deref() != Some(point) {
        return Ok(());
    }
    let directory =
        PathBuf::from(std::env::var_os("BRIT_SWITCH_CHECKPOINT_DIR").context("checkpoint directory required")?);
    fs::write(directory.join(format!("{point}.reached")), b"reached")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    // Deliberately do not observe interruption here. The controller signals and
    // releases us; only selection_interrupt may turn that signal into refusal.
    while !directory.join(format!("{point}.release")).exists() {
        ensure!(Instant::now() < deadline, "cancellation checkpoint release timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
