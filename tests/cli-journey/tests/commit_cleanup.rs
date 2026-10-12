//! Compare committed message bytes with stock Git, including hook edits.
use std::{fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn message(repo: &Path) -> Result<Vec<u8>> {
    let object = git(repo, &["cat-file", "commit", "HEAD"])?;
    let offset = object
        .windows(2)
        .position(|bytes| bytes == b"\n\n")
        .expect("commit header separator");
    Ok(object[offset + 2..].to_vec())
}

fn compare(body: &str, options: &[&str], config: &[(&str, &str)], hook_body: Option<&str>, file: bool) -> Result<()> {
    let reference = TestRepo::new("cleanup-git")?;
    let candidate = TestRepo::new("cleanup-brit")?;
    for repo in [&reference, &candidate] {
        for (key, value) in config {
            git(repo.path(), &["config", key, value])?;
        }
        fs::write(repo.path().join("message.txt"), body)?;
        if let Some(body) = hook_body {
            fs::write(repo.path().join("hook-message"), body)?;
            let hook = repo.path().join(".git/hooks/commit-msg");
            fs::write(&hook, "#!/bin/sh\ncat hook-message > \"$1\"\n")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(hook, fs::Permissions::from_mode(0o755))?;
            }
        }
    }
    let mut args = vec!["commit", "--allow-empty"];
    args.extend_from_slice(options);
    if file {
        args.extend(["-F", "message.txt"]);
    } else {
        args.extend(["-m", body]);
    }
    let original_head = candidate.head_id()?;
    let expected = gix_testtools::git_command(reference.path()).args(&args).output()?;
    let actual = BritInvocation::new(brit_bin().expect("required binary"))
        .args(args.iter().copied())
        .current_dir(candidate.path())
        .env("GIT_AUTHOR_NAME", "Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.com")
        .run()?;
    assert_eq!(
        actual.status.success(),
        expected.status.success(),
        "{args:?}: brit={} git={}",
        actual.stderr,
        String::from_utf8_lossy(&expected.stderr)
    );
    if !actual.status.success() {
        assert_eq!(candidate.head_id()?, original_head, "rejected message preserves HEAD");
    }
    assert_eq!(
        message(candidate.path())?,
        message(reference.path())?,
        "message bytes for {args:?}"
    );
    Ok(())
}

#[test]
fn explicit_cleanup_modes_match_git_for_file_and_inline_messages() -> Result<()> {
    let body = "\n \t\n  Subject \t\r\u{000b}\u{000c}\n\n\n# comment\n  # indented comment \t\nbody\u{00a0}\t\n\n# ------------------------ >8 ------------------------\nkeep without editor\n\n";
    for mode in ["strip", "whitespace", "default", "scissors", "verbatim"] {
        for file in [false, true] {
            compare(body, &[&format!("--cleanup={mode}")], &[], None, file)?;
            compare(
                "message without newline",
                &[&format!("--cleanup={mode}")],
                &[],
                None,
                file,
            )?;
        }
    }
    Ok(())
}

#[test]
fn configured_cleanup_and_comment_prefix_match_git() -> Result<()> {
    compare(
        "Subject \t\n; comment\n# retained\n",
        &[],
        &[("commit.cleanup", "strip"), ("core.commentChar", ";")],
        None,
        true,
    )?;
    compare(
        "Subject \t\n; retained\n",
        &["--cleanup=whitespace"],
        &[("commit.cleanup", "strip"), ("core.commentChar", ";")],
        None,
        true,
    )?;
    compare(
        "Subject\n// comment\n# retained\n",
        &["--cleanup=strip"],
        &[("core.commentString", "//")],
        None,
        true,
    )?;
    Ok(())
}

#[test]
#[cfg(unix)]
fn hook_edited_messages_receive_final_cleanup() -> Result<()> {
    for mode in ["strip", "whitespace", "scissors", "verbatim"] {
        compare(
            "initial",
            &[&format!("--cleanup={mode}")],
            &[],
            Some("\nSubject \t\n\n\n# hook comment\nBody  \n\n"),
            true,
        )?;
    }
    Ok(())
}

#[test]
fn empty_and_signoff_only_messages_follow_git_acceptance() -> Result<()> {
    for body in [
        "",
        " \t\n",
        "# comment\n",
        "Signed-off-by: Author <author@example.com>\n",
    ] {
        for mode in ["strip", "whitespace", "verbatim"] {
            compare(body, &[&format!("--cleanup={mode}")], &[], None, true)?;
            compare(
                body,
                &[&format!("--cleanup={mode}"), "--allow-empty-message"],
                &[],
                None,
                true,
            )?;
        }
    }
    Ok(())
}

#[test]
fn comment_aliases_follow_configuration_order() -> Result<()> {
    for config in [
        [("core.commentString", "//"), ("core.commentChar", ";")],
        [("core.commentChar", ";"), ("core.commentString", "//")],
    ] {
        compare(
            "Subject\n// first\n; second\n",
            &["--cleanup=strip"],
            &config,
            None,
            true,
        )?;
    }
    Ok(())
}

#[test]
fn repeated_message_paragraphs_match_git() -> Result<()> {
    for mode in ["strip", "verbatim", "whitespace"] {
        compare(
            "last\n",
            &[&format!("--cleanup={mode}"), "-m", "first\n", "-m", ""],
            &[],
            None,
            false,
        )?;
    }
    Ok(())
}

#[test]
fn automatic_comment_prefix_refuses_without_advancing_head() -> Result<()> {
    let repo = TestRepo::new("cleanup-auto")?;
    git(repo.path(), &["config", "core.commentChar", "auto"])?;
    let head = repo.head_id()?;
    let result = BritInvocation::new(brit_bin().expect("required binary"))
        .args(["commit", "--allow-empty", "--cleanup=strip", "-m", "# retain me"])
        .current_dir(repo.path())
        .run()?;
    ensure!(!result.status.success(), "automatic prefix must refuse");
    ensure!(
        result.stderr.contains("use git commit"),
        "actionable bridge: {}",
        result.stderr
    );
    assert_eq!(repo.head_id()?, head, "unsupported cleanup preserves HEAD");
    Ok(())
}
