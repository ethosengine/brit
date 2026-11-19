//! Every command admitted to brit-cli's `READ_ONLY_COMMANDS` after `8e5072ff` ends quietly with 141
//! when its reader closed early, keeps its own status when it genuinely fails, and prints the same
//! thing to a live reader. Linux only: the closed reader is a pipe whose reading end is closed with
//! `libc` before Brit starts, like `broken_pipe.rs`.
#![cfg(target_os = "linux")]

use std::{
    os::fd::FromRawFd,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use anyhow::{ensure, Result};
use cli_journey::support::runner::brit_bin;

fn git(cwd: &Path, args: &[&str]) -> Result<()> {
    let status = gix_testtools::git_command(cwd).args(args).status()?;
    ensure!(status.success(), "git {args:?} in {}", cwd.display());
    Ok(())
}

fn brit(cwd: &Path) -> Command {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, cwd);
    command.current_dir(cwd).stdin(Stdio::null());
    command
}

/// Run `brit <args>` with stdout connected to a pipe whose reading end is already closed.
fn closed_stdout(cwd: &Path, args: &[&str]) -> Result<Output> {
    let mut fds = [0; 2];
    // SAFETY: `fds` has room for the two descriptors `pipe` writes.
    ensure!(unsafe { libc::pipe(fds.as_mut_ptr()) } == 0, "pipe(2) failed");
    // SAFETY: both descriptors were just created by `pipe` and are owned here exactly once.
    let (reader, writer) = unsafe { (std::fs::File::from_raw_fd(fds[0]), std::fs::File::from_raw_fd(fds[1])) };
    drop(reader);
    Ok(brit(cwd).args(args).stdout(writer).stderr(Stdio::piped()).output()?)
}

/// A repository with a tag, a mailmap, attributes, an ignore file, a submodule, a commit-graph, a
/// `file://` remote, an untracked file and a build manifest, so that every admitted command prints.
fn fixture(root: &Path) -> Result<PathBuf> {
    git(root, &["init", "-q", "--bare", "remote.git"])?;
    git(root, &["init", "-q", "--initial-branch=main", "sub"])?;
    git(&root.join("sub"), &["commit", "-q", "--allow-empty", "-m", "sub"])?;
    git(root, &["init", "-q", "--initial-branch=main", "repo"])?;
    let repo = root.join("repo");
    std::fs::write(repo.join(".mailmap"), "Canonical <c@x> author <author@example.com>\n")?;
    std::fs::write(repo.join(".gitattributes"), "*.txt text\n")?;
    std::fs::write(repo.join(".gitignore"), "ignored.log\n")?;
    std::fs::write(
        repo.join("build-manifest.json"),
        r#"{"manifestVersion":"1.0","pipeline":"probe","description":"probe","steps":{"lint":{"description":"lint","inputs":{"sources":["*.txt"],"buildProcess":[]},"outputs":{"artifacts":[],"verify":null},"depends":[],"executor":{"stage":"lint","function":null}}}}"#,
    )?;
    std::fs::write(repo.join("a.txt"), "a\n")?;
    git(&repo, &["add", "-A"])?;
    git(&repo, &["commit", "-q", "-m", "one"])?;
    std::fs::write(repo.join("a.txt"), "a\nb\n")?;
    git(&repo, &["commit", "-q", "-am", "two"])?;
    git(&repo, &["tag", "-a", "v1", "-m", "tagged", "HEAD~1"])?;
    git(&repo, &["checkout", "-q", "-b", "side"])?;
    git(&repo, &["checkout", "-q", "main"])?;
    let sub = root.join("sub");
    git(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            sub.to_str().expect("UTF-8 path"),
            "sub",
        ],
    )?;
    git(&repo, &["commit", "-q", "-m", "submodule"])?;
    git(&repo, &["commit-graph", "write", "--reachable"])?;
    let remote = format!("file://{}", root.join("remote.git").display());
    git(&repo, &["remote", "add", "origin", &remote])?;
    git(&repo, &["push", "-q", "origin", "main"])?;
    std::fs::write(repo.join("untracked.txt"), "u\n")?;
    Ok(repo)
}

/// The admitted commands, each in a form that prints to stdout and nothing to stderr.
const PRINTING: &[&[&str]] = &[
    &["--no-verbose", "show", "HEAD"],
    &["--no-verbose", "tag", "list"],
    &["--no-verbose", "remote", "-n", "origin", "url"],
    &["--no-verbose", "remote", "-n", "origin", "refs"],
    &["--no-verbose", "remote", "-n", "origin", "ref-map"],
    &["--no-verbose", "commit", "describe"],
    &["--no-verbose", "revision", "previous-branches"],
    &["--no-verbose", "verify", "--statistics"],
    &["--no-verbose", "tree", "entries"],
    &["--no-verbose", "-f", "json", "tree", "info"],
    &["--no-verbose", "commit-graph", "list"],
    &["--no-verbose", "commit-graph", "verify", "--statistics"],
    &["--no-verbose", "-f", "json", "mailmap", "entries"],
    &["--no-verbose", "mailmap", "check", "author <author@example.com>"],
    &["--no-verbose", "attributes", "query", "a.txt"],
    &["--no-verbose", "exclude", "query", "ignored.log"],
    &["--no-verbose", "submodule"],
    &["--no-verbose", "submodule", "list"],
    &["--no-verbose", "is-clean"],
    &["--no-verbose", "config"],
    &["--no-verbose", "config", "show"],
    &["--no-verbose", "config", "list"],
    &["--no-verbose", "index", "entries"],
    &["--no-verbose", "dirwalk"],
    &["build", "graph", "discover"],
    &["build", "graph", "show"],
    &["build", "graph", "show", "--format", "dot"],
    &["build", "affected", "--files", "a.txt"],
    &["build", "plan", "--files", "a.txt"],
    &["build", "fingerprint", "build-manifest.json"],
    &["build", "baseline", "read", "probe"],
];

/// Admitted commands that genuinely fail here: a closed reader must not change their status.
const FAILING: &[&[&str]] = &[
    &["--no-verbose", "show", "nope"],
    &["--no-verbose", "tree", "entries", "nope"],
    &["--no-verbose", "commit", "describe", "nope"],
    &["--no-verbose", "remote", "-n", "nope", "refs"],
    &["--no-verbose", "is-changed"],
    &["build", "fingerprint", "missing.json"],
];

#[test]
fn admitted_read_only_commands_end_quietly_when_their_reader_closes() -> Result<()> {
    let root = tempfile::tempdir()?;
    let repo = fixture(root.path())?;
    for args in PRINTING {
        let live = brit(&repo).args(*args).output()?;
        assert_eq!(live.status.code(), Some(0), "brit {args:?}: {live:?}");
        assert!(!live.stdout.is_empty(), "brit {args:?} prints to a live reader");
        let closed = closed_stdout(&repo, args)?;
        assert_eq!(
            (
                closed.status.code(),
                String::from_utf8_lossy(&closed.stderr).into_owned()
            ),
            (Some(141), String::new()),
            "brit {args:?} with its reader closed"
        );
    }
    for args in FAILING {
        let live = brit(&repo).args(*args).output()?;
        assert_ne!(live.status.code(), Some(0), "brit {args:?} fails here: {live:?}");
        let closed = closed_stdout(&repo, args)?;
        assert_eq!(
            closed.status.code(),
            live.status.code(),
            "brit {args:?} keeps its own status with its reader closed: {closed:?}"
        );
        assert!(!closed.stderr.is_empty(), "the failure is still reported: {closed:?}");
    }

    // `is-changed` prints only once there is a change; `is-clean` then fails and keeps its status.
    std::fs::write(repo.join("a.txt"), "changed\n")?;
    let args = ["--no-verbose", "is-changed"];
    let live = brit(&repo).args(args).output()?;
    assert_eq!(live.status.code(), Some(0), "brit {args:?}: {live:?}");
    let closed = closed_stdout(&repo, &args)?;
    assert_eq!(
        (
            closed.status.code(),
            String::from_utf8_lossy(&closed.stderr).into_owned()
        ),
        (Some(141), String::new()),
        "brit {args:?} with its reader closed"
    );
    let closed = closed_stdout(&repo, &["--no-verbose", "is-clean"])?;
    assert_eq!(closed.status.code(), Some(1), "is-clean fails on a change: {closed:?}");
    Ok(())
}

/// `fsck` prints only what is missing, so it gets a repository with a deleted blob.
#[test]
fn fsck_reporting_a_missing_object_ends_quietly_when_its_reader_closes() -> Result<()> {
    let root = tempfile::tempdir()?;
    let repo = root.path();
    git(repo, &["init", "-q", "--initial-branch=main"])?;
    std::fs::write(repo.join("f"), "x\n")?;
    git(repo, &["add", "f"])?;
    git(repo, &["commit", "-q", "-m", "one"])?;
    let blob = String::from_utf8(
        gix_testtools::git_command(repo)
            .args(["rev-parse", "HEAD:f"])
            .output()?
            .stdout,
    )?;
    let blob = blob.trim();
    std::fs::remove_file(repo.join(".git/objects").join(&blob[..2]).join(&blob[2..]))?;

    let args = ["--no-verbose", "fsck", "HEAD"];
    let live = brit(repo).args(args).output()?;
    assert!(
        String::from_utf8_lossy(&live.stdout).contains(blob),
        "fsck names the missing blob: {live:?}"
    );
    let closed = closed_stdout(repo, &args)?;
    assert_eq!(
        (
            closed.status.code(),
            String::from_utf8_lossy(&closed.stderr).into_owned()
        ),
        (Some(141), String::new()),
        "brit {args:?} with its reader closed"
    );
    Ok(())
}
