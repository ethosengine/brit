//! MockRemote — bare git repo + file:// URL for clone/fetch/push tests.
//!
//! Uses local file:// transport; no daemon, no network. Deterministic.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;

/// A bare git repo at a temp path, accessible via file:// URL.
pub struct MockRemote {
    _temp: TempDir,
    path: PathBuf,
}

impl MockRemote {
    /// Create a new bare repo. `label` is included in the temp dir name.
    pub fn new(label: &str) -> Result<Self> {
        let temp = tempfile::Builder::new()
            .prefix(&format!("brit-mockremote-{label}-"))
            .tempdir()
            .context("mktemp")?;
        // The bare repo lives at <temp>/<label>.git
        let path = temp.path().join(format!("{label}.git"));
        std::fs::create_dir_all(&path).context("mkdir bare path")?;
        let out = Command::new("git")
            .args(["init", "-q", "--bare"])
            .current_dir(&path)
            .output()
            .context("git init --bare")?;
        if !out.status.success() {
            return Err(anyhow!(
                "git init --bare failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(Self { _temp: temp, path })
    }

    /// The bare repo's path on disk.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file:// URL for clone/fetch/push.
    pub fn url(&self) -> String {
        format!("file://{}", self.path.display())
    }
}
