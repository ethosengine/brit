//! Compare revision search and pickaxe history with stock Git.
use std::{fs, path::Path, process::Output};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

fn fixture() -> Result<TestRepo> {
    let repo = TestRepo::new("grep-pickaxe")?;
    repo.commit_file_with_message(
        "hooks/a",
        "Needle and needle
old context
",
        "add needle",
    )?;
    repo.commit_file_with_message(
        "hooks/a",
        "old context
Needle and needle
",
        "move needle within the file",
    )?;
    repo.commit_file_with_message("other", "needle+ needle", "add needle to another file")?;
    repo.commit_file_with_message("hooks/a", "old context\n", "remove needle from hook")?;
    fs::write(repo.path().join("hooks/a"), "uncommitted change\n")?;
    Ok(repo)
}

fn brit(repo: &Path, args: &[&str]) -> Result<Output> {
    let mut command = std::process::Command::new(brit_bin().expect("required binary"));
    gix_testtools::configure_git_environment(&mut command, repo);
    command.env("GIT_CONFIG_GLOBAL", repo.join("missing.gitconfig"));
    test_locale(&mut command);
    Ok(command.current_dir(repo).args(args).output()?)
}

fn test_locale(command: &mut std::process::Command) {
    let locale = if cfg!(all(target_os = "linux", target_env = "gnu")) {
        "C.UTF-8"
    } else {
        "C"
    };
    command.env("LC_ALL", locale);
}

fn git(repo: &TestRepo, args: &[&str]) -> Result<()> {
    let output = gix_testtools::git_command(repo.path()).args(args).output()?;
    ensure!(output.status.success(), "git {args:?}: {output:?}");
    Ok(())
}

fn parity(repo: &Path, git_args: &[&str], brit_args: &[&str], exit: i32) -> Result<Vec<u8>> {
    let mut command = gix_testtools::git_command(repo);
    test_locale(&mut command);
    let expected = command.args(git_args).output()?;
    assert_eq!(expected.status.code(), Some(exit), "git {git_args:?}: {expected:?}");
    let actual = brit(repo, brit_args)?;
    assert_eq!(
        actual.status.code(),
        expected.status.code(),
        "brit {brit_args:?}: {actual:?}"
    );
    assert_eq!(
        actual.stdout, expected.stdout,
        "brit {brit_args:?} matches Git byte for byte"
    );
    Ok(actual.stdout)
}

#[test]
fn grep_revision_matches_git() -> Result<()> {
    let repo = fixture()?;
    for (args, exit) in [
        (&["grep", "needle", "HEAD~1"][..], 0),
        (&["grep", "-n", "needle", "HEAD~1"], 0),
        (&["grep", "-l", "needle", "HEAD~1"], 0),
        (&["grep", "-i", "NEEDLE", "HEAD~1"], 0),
        (&["grep", "needle", "HEAD~1", "--", "hooks/*"], 0),
        (&["grep", "absent", "HEAD~1"], 1),
        (&["grep", "-e", "^Needle", "HEAD~1"], 0),
        (&["grep", "-e", "-needle", "HEAD~1"], 1),
        (&["grep", "-e", "absent", "-e", "^Needle", "HEAD~1"], 0),
        (&["grep", "needle+", "HEAD~1"], 0),
        (&["grep", "-F", "needle+", "HEAD~1"], 0),
        (&["grep", "-E", "needle+", "HEAD~1"], 0),
    ] {
        parity(repo.path(), args, args, exit)?;
    }
    parity(
        repo.path(),
        &["grep", "needle", "HEAD~1"],
        &["grep", "HEAD~1", "--", "needle"],
        0,
    )?;
    Ok(())
}

#[test]
fn log_pickaxe_matches_git() -> Result<()> {
    let repo = fixture()?;
    for args in [
        &["log", "--oneline", "-S", "needle"][..],
        &["log", "--oneline", "-S", "needle", "HEAD", "--", "hooks/*"],
        &["log", "--oneline", "-Sneedle", "-1", "HEAD~1"],
        &["log", "--oneline", "-S", "-1"],
        &["log", "--oneline", "-S", "needle", "HEAD~2..HEAD"],
    ] {
        let stdout = parity(repo.path(), args, args, 0)?;
        ensure!(
            !stdout
                .split(|byte| *byte == b'\n')
                .any(|line| line.ends_with(b" move needle within the file")),
            "moving a string within a file preserves its count"
        );
    }
    Ok(())
}

#[test]
fn grep_preserves_binary_and_relative_output() -> Result<()> {
    let repo = fixture()?;
    repo.commit_file("hooks/café", "needle café\r\n")?;
    repo.commit_file("binary", "before\0needle\n")?;
    for args in [
        &["grep", "-n", "needle", "HEAD"][..],
        &["grep", "-l", "needle", "HEAD"],
        &["grep", "-n", "needle", "HEAD", "--", ":(top)hooks/*"],
    ] {
        parity(repo.path(), args, args, 0)?;
    }
    let args = &["grep", "-n", "needle", "HEAD"];
    parity(&repo.path().join("hooks"), args, args, 0)?;
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    for flag in ["-F", "-E"] {
        let args = ["grep", flag, "-i", "CAFÉ", "HEAD"];
        parity(repo.path(), &args, &args, 0)?;
    }
    Ok(())
}

#[test]
fn pickaxe_pairs_renames_and_keeps_existing_default_format() -> Result<()> {
    let repo = TestRepo::new("pickaxe-rename")?;
    repo.commit_file_with_message("before", "needle\n", "add needle")?;
    git(&repo, &["mv", "before", "after"])?;
    git(&repo, &["commit", "-qm", "rename needle"])?;

    for args in [
        &["log", "--oneline", "-S", "needle"][..],
        &["log", "--oneline", "-S", "needle", "--", "after"],
    ] {
        parity(repo.path(), args, args, 0)?;
    }
    let selected = brit(repo.path(), &["log", "-S", "needle"])?;
    let expected = brit(repo.path(), &["log", "-1", "HEAD^"])?;
    assert!(selected.status.success(), "pickaxe with default output: {selected:?}");
    assert!(
        expected.status.success(),
        "ordinary log with default output: {expected:?}"
    );
    assert_eq!(
        selected.stdout, expected.stdout,
        "pickaxe uses the ordinary log renderer"
    );
    Ok(())
}

#[test]
fn pickaxe_follows_git_merge_history() -> Result<()> {
    let repo = TestRepo::new("pickaxe-merge")?;
    git(&repo, &["branch", "side"])?;
    repo.commit_file("main-only", "needle\n")?;
    git(&repo, &["checkout", "side"])?;
    repo.commit_file("hooks/side-only", "needle\n")?;
    git(&repo, &["checkout", "main"])?;
    git(&repo, &["merge", "--no-ff", "-m", "merge side", "side"])?;

    for args in [
        &["log", "--oneline", "-S", "needle"][..],
        &["log", "--oneline", "-S", "needle", "--", "hooks/*"],
        &["log", "--oneline", "--first-parent", "-S", "needle"],
    ] {
        parity(repo.path(), args, args, 0)?;
    }
    Ok(())
}
