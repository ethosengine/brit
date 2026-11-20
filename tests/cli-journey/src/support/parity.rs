//! Git-as-oracle helpers: run the same arguments through `git` and `brit` in the same directory.

use std::{path::Path, process::Command};

use anyhow::{ensure, Result};

use crate::support::runner::brit_bin;

/// What one invocation printed and how it exited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub stdout: String,
    pub stderr: String,
    pub code: Option<i32>,
}

fn outcome(mut command: Command) -> Result<Outcome> {
    let out = command.output()?;
    Ok(Outcome {
        stdout: String::from_utf8(out.stdout)?,
        stderr: String::from_utf8(out.stderr)?,
        code: out.status.code(),
    })
}

/// Run isolated `git <args>` in `cwd`.
pub fn git(cwd: &Path, args: &[&str]) -> Result<Outcome> {
    outcome({
        let mut command = gix_testtools::git_command(cwd);
        command.args(args);
        command
    })
}

/// Run isolated `git <args>` in `cwd`, requiring success, and return trimmed stdout.
pub fn git_ok(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = git(cwd, args)?;
    ensure!(out.code == Some(0), "git {args:?} in {}: {}", cwd.display(), out.stderr);
    Ok(out.stdout.trim().to_owned())
}

/// Run isolated `brit <args>` in `cwd`, with the same environment `git` gets.
pub fn brit(cwd: &Path, args: &[&str]) -> Result<Outcome> {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, cwd);
    // gix treats the null device as an unreadable Git config, unlike Git: name an absent file instead.
    command.env(
        "GIT_CONFIG_GLOBAL",
        std::env::temp_dir().join("brit-journey-absent-global.gitconfig"),
    );
    command.current_dir(cwd).args(args);
    outcome(command)
}

/// Assert that `brit <args>` prints the same stdout and exits like `git <args>` in `cwd`.
///
/// Stderr is shown on failure but not compared; use [`assert_same_streams()`] where a script
/// would notice whether anything was written to it.
pub fn assert_same(cwd: &Path, args: &[&str]) -> Result<Outcome> {
    let expected = git(cwd, args)?;
    let actual = brit(cwd, args)?;
    assert_eq!(
        (&actual.stdout, actual.code),
        (&expected.stdout, expected.code),
        "brit {args:?} in {} differs from git\nbrit stderr: {}\ngit stderr: {}",
        cwd.display(),
        actual.stderr,
        expected.stderr
    );
    Ok(actual)
}

/// Like [`assert_same()`], and also require that stderr is empty exactly when Git's is.
///
/// Diagnostics are not compared byte for byte, but a script that treats any stderr as a warning,
/// or that expects Git's `From <url>` line, sees the same thing from both.
pub fn assert_same_streams(cwd: &Path, args: &[&str]) -> Result<Outcome> {
    let expected = git(cwd, args)?;
    let actual = brit(cwd, args)?;
    assert_eq!(
        (&actual.stdout, actual.code, actual.stderr.is_empty()),
        (&expected.stdout, expected.code, expected.stderr.is_empty()),
        "brit {args:?} in {} differs from git (stdout, status, empty stderr)\nbrit stderr: {}\ngit stderr: {}",
        cwd.display(),
        actual.stderr,
        expected.stderr
    );
    Ok(actual)
}

/// Assert that `brit <spelling> --help` names Brit's own verb.
pub fn assert_help_names(spelling: &str, native: &str) -> Result<()> {
    let help = brit(&std::env::temp_dir(), &[spelling, "--help"])?;
    ensure!(help.code == Some(0), "brit {spelling} --help: {}", help.stderr);
    ensure!(
        help.stdout.contains(native),
        "brit {spelling} --help must name `{native}`:\n{}",
        help.stdout
    );
    Ok(())
}
