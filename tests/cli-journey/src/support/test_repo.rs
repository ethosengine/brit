//! TestRepo — temp git repo with deterministic commit history.
//!
//! Uses the static-git-environment env vars (matching gitoxide's
//! `helpers.sh::set-static-git-environment`) so commits made within the
//! test produce stable SHA values across runs and machines.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, Context, Result};
use tempfile::TempDir;

/// Static git environment matching gitoxide's helpers.sh.
/// Set on every git invocation made by TestRepo.
const STATIC_ENV: &[(&str, &str)] = &[
    ("GIT_AUTHOR_DATE", "2020-09-09 09:06:03 +0800"),
    ("GIT_COMMITTER_DATE", "2020-09-09 09:06:03 +0800"),
    ("GIT_AUTHOR_NAME", "Sebastian Thiel"),
    ("GIT_COMMITTER_NAME", "Sebastian Thiel"),
    ("GIT_AUTHOR_EMAIL", "git@example.com"),
    ("GIT_COMMITTER_EMAIL", "git@example.com"),
];

/// A temp git repo with a deterministic commit history.
///
/// Lifetime tied to the held TempDir — drops the dir when dropped.
pub struct TestRepo {
    _temp: TempDir,
    path: PathBuf,
}

impl TestRepo {
    /// Create a new temp repo with a single empty commit on `main`.
    /// `label` is included in the temp dir name for debugging.
    pub fn new(label: &str) -> Result<Self> {
        let temp = tempfile::Builder::new()
            .prefix(&format!("brit-test-{label}-"))
            .tempdir()
            .context("mktemp")?;
        let path = temp.path().to_path_buf();

        Self::git(&path, &["init", "-q", "--initial-branch=main"])?;
        Self::git(&path, &["commit", "--allow-empty", "-q", "-m", "init"])?;

        Ok(Self { _temp: temp, path })
    }

    /// The repo's working directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Write a file and commit it. Returns the new commit SHA-1 hex.
    pub fn commit_file(&self, rel: &str, contents: &str) -> Result<String> {
        let abs = self.path.join(rel);
        if let Some(parent) = abs.parent() {
            std::fs::create_dir_all(parent).context("mkdir")?;
        }
        std::fs::write(&abs, contents).context("write file")?;
        Self::git(&self.path, &["add", rel])?;
        Self::git(&self.path, &["commit", "-q", "-m", &format!("add {rel}")])?;
        self.head_id()
    }

    /// Get the HEAD commit SHA-1 hex (40 chars).
    pub fn head_id(&self) -> Result<String> {
        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.path)
            .envs(STATIC_ENV.iter().copied())
            .output()
            .context("git rev-parse")?;
        if !out.status.success() {
            return Err(anyhow!(
                "git rev-parse failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(String::from_utf8(out.stdout)?.trim().to_string())
    }

    fn git(path: &Path, args: &[&str]) -> Result<()> {
        let out = Command::new("git")
            .args(args)
            .current_dir(path)
            .envs(STATIC_ENV.iter().copied())
            .output()
            .with_context(|| format!("git {args:?}"))?;
        if !out.status.success() {
            return Err(anyhow!(
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        Ok(())
    }
}
