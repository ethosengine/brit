//! `brit log` prints a commit's message in UTF-8 whichever `encoding` the commit declares, as
//! `git log` does, with `git log --oneline` as the oracle.
#![cfg(unix)]

use std::{io::Write, path::Path, process::Stdio};

use anyhow::{Result, ensure};
use cli_journey::support::parity::{brit, git, git_ok};

/// Write a raw commit object with `encoding` declared (when given) over `tree` and `parent`.
fn commit(root: &Path, tree: &str, parent: Option<&str>, encoding: &str, message: &[u8]) -> Result<String> {
    let mut data = format!("tree {tree}\n").into_bytes();
    if let Some(parent) = parent {
        data.extend_from_slice(format!("parent {parent}\n").as_bytes());
    }
    data.extend_from_slice(b"author A <a@example.com> 1700000000 +0100\n");
    data.extend_from_slice(b"committer C <c@example.com> 1700000000 +0100\n");
    data.extend_from_slice(format!("encoding {encoding}\n\n").as_bytes());
    data.extend_from_slice(message);
    let mut child = gix_testtools::git_command(root)
        .args(["hash-object", "-t", "commit", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    child.stdin.take().expect("piped").write_all(&data)?;
    let out = child.wait_with_output()?;
    ensure!(out.status.success(), "hash-object");
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

#[test]
fn log_decodes_declared_commit_encodings_like_git() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let root = dir.path();
    git_ok(root, &["init", "-q", "--initial-branch=main"])?;
    let tree = git_ok(root, &["write-tree"])?;
    let latin1 = commit(root, &tree, None, "ISO-8859-1", b"caf\xe9 cr\xe8me\n\nbody \xe9\n")?;
    // "日本語" in Shift_JIS (windows-31j).
    let sjis = commit(root, &tree, Some(&latin1), "windows-31j", b"\x93\xfa\x96\x7b\x8c\xea\n")?;
    git_ok(root, &["update-ref", "refs/heads/main", &sjis])?;

    let actual = brit(root, &["log", "--oneline"])?;
    ensure!(actual.code == Some(0), "brit log: {}", actual.stderr);
    ensure!(
        actual.stdout.contains("café crème"),
        "latin-1 title decoded: {}",
        actual.stdout
    );
    ensure!(
        actual.stdout.contains("日本語"),
        "windows-31j title decoded: {}",
        actual.stdout
    );

    // Some Git builds (the macOS Intel runner's) print these encodings differently; compare with
    // Git only where it decoded the messages too.
    let expected = git(root, &["log", "--oneline"])?;
    if expected.stdout.contains("café crème") && expected.stdout.contains("日本語") {
        assert_eq!(actual.stdout, expected.stdout);
    } else {
        eprintln!("skipping the Git parity: this Git does not decode these encodings as glibc's does");
    }
    Ok(())
}
