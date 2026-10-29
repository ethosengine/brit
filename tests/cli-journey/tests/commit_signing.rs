//! Required commit signing fails closed before ordinary commit creation has side effects.
use std::{
    fs,
    path::Path,
    process::{Command, Output, Stdio},
};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

const IDENTITY: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Commit Author"),
    ("GIT_AUTHOR_EMAIL", "author@example.com"),
    ("GIT_COMMITTER_NAME", "Commit Author"),
    ("GIT_COMMITTER_EMAIL", "author@example.com"),
];

fn command(repo: &Path, native: bool) -> Command {
    let mut cmd = if native {
        let mut cmd = Command::new(brit_bin().expect("required Brit binary"));
        gix_testtools::configure_git_environment(&mut cmd, repo);
        cmd.current_dir(repo);
        cmd
    } else {
        gix_testtools::git_command(repo)
    };
    cmd.env("GIT_CONFIG_GLOBAL", repo.join("absent-global-config"));
    cmd.envs(IDENTITY);
    cmd
}

fn run(repo: &Path, native: bool, args: &[&str]) -> Result<Output> {
    Ok(command(repo, native)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?)
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = run(repo, false, args)?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn git_path(repo: &Path, path: &str) -> Result<std::path::PathBuf> {
    let path = String::from_utf8(git(repo, &["rev-parse", "--git-path", path])?)?;
    Ok(repo.join(path.trim()))
}

fn fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("selected", "original\n")?;
    fs::write(repo.path().join("selected"), "staged change\n")?;
    git(repo.path(), &["add", "--", "selected"])?;
    Ok(repo)
}

#[cfg(unix)]
fn pre_commit_marker(repo: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let hook_path = repo.join(".git/hooks/pre-commit");
    fs::write(&hook_path, "#!/bin/sh\nset -eu\necho ran > hook-ran\n")?;
    fs::set_permissions(hook_path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn object_files(dir: &Path, found: &mut Vec<std::path::PathBuf>) -> Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            object_files(&path, found)?;
        } else {
            found.push(path);
        }
    }
    Ok(())
}

fn object_store(repo: &Path) -> Result<Vec<std::path::PathBuf>> {
    let mut found = Vec::new();
    object_files(&git_path(repo, "objects")?, &mut found)?;
    found.sort();
    Ok(found)
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[cfg(unix)]
#[test]
fn required_signing_refuses_before_hooks_or_repository_writes() -> Result<()> {
    for (value, via_cli) in ["true", "yes", "on", "1"]
        .into_iter()
        .flat_map(|value| [(value, false), (value, true)])
    {
        let repo = fixture("required-signing")?;
        if !via_cli {
            git(repo.path(), &["config", "commit.gpgSign", value])?;
        }
        pre_commit_marker(repo.path())?;

        let head = repo.head_id()?;
        let index_path = git_path(repo.path(), "index")?;
        let head_log_path = git_path(repo.path(), "logs/HEAD")?;
        let message_path = git_path(repo.path(), "COMMIT_EDITMSG")?;
        let index = fs::read(&index_path)?;
        let head_log = fs::read(&head_log_path)?;
        let message = optional_bytes(&message_path)?;
        let objects = object_store(repo.path())?;
        let setting = format!("commit.gpgSign={value}");
        let args = if via_cli {
            vec!["-c", setting.as_str(), "commit", "-m", "must be signed"]
        } else {
            vec!["commit", "-m", "must be signed"]
        };

        let out = run(repo.path(), true, &args)?;
        ensure!(
            !out.status.success(),
            "commit.gpgSign={value} via {} must refuse",
            if via_cli { "CLI" } else { "repository config" }
        );
        let stderr = String::from_utf8(out.stderr)?;
        ensure!(
            stderr.contains("git commit -S"),
            "refusal must name the signing bridge: {stderr}"
        );
        assert_eq!(repo.head_id()?, head, "refusal must preserve HEAD");
        assert_eq!(fs::read(index_path)?, index, "refusal must preserve index bytes");
        assert_eq!(
            fs::read(head_log_path)?,
            head_log,
            "refusal must preserve logs/HEAD bytes"
        );
        assert_eq!(
            optional_bytes(&message_path)?,
            message,
            "refusal must not write COMMIT_EDITMSG"
        );
        assert_eq!(
            object_store(repo.path())?,
            objects,
            "refusal must not write tree or commit objects"
        );
        ensure!(
            !repo.path().join("hook-ran").exists(),
            "refusal must happen before pre-commit"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn false_cli_override_still_allows_commit() -> Result<()> {
    let repo = fixture("disabled-required-signing")?;
    git(repo.path(), &["config", "commit.gpgSign", "true"])?;
    pre_commit_marker(repo.path())?;
    let head = repo.head_id()?;

    let out = run(
        repo.path(),
        true,
        &["-c", "commit.gpgSign=false", "commit", "-m", "unsigned allowed"],
    )?;
    ensure!(
        out.status.success(),
        "false override must allow commit: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    ensure!(repo.head_id()? != head, "successful commit must advance HEAD");
    ensure!(
        repo.path().join("hook-ran").exists(),
        "successful commit runs pre-commit"
    );
    assert_eq!(git(repo.path(), &["show", "HEAD:selected"])?, b"staged change\n");
    Ok(())
}
