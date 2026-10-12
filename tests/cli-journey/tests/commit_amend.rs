//! Noninteractive amendment: Git is the oracle; every repository is disposable.
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

const IDENTITY: [(&str, &str); 6] = [
    ("GIT_AUTHOR_NAME", "New Author Must Not Replace Original"),
    ("GIT_AUTHOR_EMAIL", "new-author@example.com"),
    ("GIT_AUTHOR_DATE", "2025-01-02 03:04:05 +0000"),
    ("GIT_COMMITTER_NAME", "Current Committer"),
    ("GIT_COMMITTER_EMAIL", "committer@example.com"),
    ("GIT_COMMITTER_DATE", "2026-01-02 03:04:05 +0000"),
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

fn run(repo: &Path, native: bool, args: &[&str], stdin: Option<&[u8]>) -> Result<Output> {
    let mut cmd = command(repo, native);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(bytes) = stdin {
        let mut child = cmd.stdin(Stdio::piped()).spawn()?;
        child.stdin.take().expect("piped stdin").write_all(bytes)?;
        Ok(child.wait_with_output()?)
    } else {
        Ok(cmd.output()?)
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = run(repo, false, args, None)?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn success(repo: &Path, args: &[&str]) -> Result<Output> {
    let out = run(repo, true, args, None)?;
    ensure!(
        out.status.success(),
        "brit {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out)
}

fn index(repo: &Path) -> Result<Vec<u8>> {
    let path = String::from_utf8(git(repo, &["rev-parse", "--git-path", "index"])?)?;
    Ok(fs::read(repo.join(path.trim()))?)
}

fn reflog_tip(repo: &Path, name: &str) -> Result<Vec<u8>> {
    let path = String::from_utf8(git(repo, &["rev-parse", "--git-path", name])?)?;
    let bytes = fs::read(repo.join(path.trim()))?;
    Ok(bytes
        .split(|byte| *byte == b'\n')
        .rfind(|line| !line.is_empty())
        .expect("reflog entry")
        .to_vec())
}

fn fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file_with_message("selected", "original\n", "Original subject\n\nOriginal body")?;
    Ok(repo)
}

fn compare(actual: &Path, oracle: &Path, args: &[&str], input: Option<&[u8]>) -> Result<bool> {
    let before_head = git(actual, &["rev-parse", "HEAD"])?;
    let before_index = index(actual)?;
    let expected = run(oracle, false, args, input)?;
    let result = run(actual, true, args, input)?;
    assert_eq!(
        result.status.success(),
        expected.status.success(),
        "{args:?}: brit={} git={}",
        String::from_utf8_lossy(&result.stderr),
        String::from_utf8_lossy(&expected.stderr)
    );
    assert_eq!(
        git(actual, &["cat-file", "commit", "HEAD"])?,
        git(oracle, &["cat-file", "commit", "HEAD"])?,
        "exact tree, complete parents, author, current committer and message for {args:?}"
    );
    assert_eq!(
        index(actual)?,
        before_index,
        "amend never refreshes or rewrites the index"
    );
    if result.status.success() {
        assert_eq!(
            git(actual, &["reflog", "-1", "--format=%gs"])?,
            git(oracle, &["reflog", "-1", "--format=%gs"])?,
            "amend reflog subject"
        );
        assert_eq!(
            reflog_tip(actual, "logs/HEAD")?,
            reflog_tip(oracle, "logs/HEAD")?,
            "raw HEAD reflog includes expected old/new OIDs and committer"
        );
        if let Ok(branch) = git(actual, &["symbolic-ref", "HEAD"]) {
            let branch = String::from_utf8(branch)?;
            let log = format!("logs/{}", branch.trim());
            assert_eq!(
                reflog_tip(actual, &log)?,
                reflog_tip(oracle, &log)?,
                "raw branch reflog mapping"
            );
        }
    } else {
        assert_eq!(
            git(actual, &["rev-parse", "HEAD"])?,
            before_head,
            "refusal preserves HEAD"
        );
    }
    Ok(result.status.success())
}

#[test]
fn root_ordinary_merge_detached_and_staged_amendments_match_git_objects() -> Result<()> {
    for shape in ["root", "ordinary", "merge", "detached"] {
        for staged in [false, true] {
            let actual = TestRepo::new("amend-object")?;
            let oracle = TestRepo::new("amend-oracle")?;
            for repo in [&actual, &oracle] {
                if shape == "root" {
                    fs::write(repo.path().join("selected"), "root content\n")?;
                    git(repo.path(), &["add", "selected"])?;
                    git(repo.path(), &["commit", "--amend", "-m", "root"])?;
                } else {
                    repo.commit_file("selected", "original\n")?;
                }
                if shape == "merge" {
                    git(repo.path(), &["checkout", "-qb", "side", "HEAD~1"])?;
                    repo.commit_file("side", "side content\n")?;
                    git(repo.path(), &["checkout", "main"])?;
                    git(repo.path(), &["merge", "--no-ff", "-m", "merge", "side"])?;
                }
                if shape == "detached" {
                    git(repo.path(), &["checkout", "--detach"])?;
                }
                if staged {
                    fs::write(repo.path().join("selected"), "staged content\n")?;
                    git(repo.path(), &["add", "selected"])?;
                }
                fs::write(repo.path().join("selected"), "unstaged neighbor\n")?;
                fs::write(repo.path().join("private"), "private bytes\n")?;
            }
            ensure!(compare(
                actual.path(),
                oracle.path(),
                &["commit", "--amend", "--no-edit"],
                None
            )?);
            assert_eq!(fs::read(actual.path().join("selected"))?, b"unstaged neighbor\n");
            assert_eq!(fs::read(actual.path().join("private"))?, b"private bytes\n");
            assert_eq!(
                git(actual.path(), &["symbolic-ref", "-q", "HEAD"]).is_ok(),
                shape != "detached"
            );
        }
    }
    Ok(())
}

#[test]
fn message_sources_cleanup_and_explicit_empty_messages_match_git() -> Result<()> {
    for mode in ["strip", "whitespace", "verbatim", "scissors", "default"] {
        for source in ["inline", "file", "stdin"] {
            for body in ["\nNew subject \t\n\n# comment\nBody \t\n\n", ""] {
                for allow_empty in [false, true] {
                    let actual = fixture("amend-message")?;
                    let oracle = fixture("amend-message-oracle")?;
                    for repo in [&actual, &oracle] {
                        fs::write(repo.path().join("message.txt"), body)?;
                    }
                    let cleanup = format!("--cleanup={mode}");
                    let mut args = vec!["commit", "--amend", &cleanup];
                    if allow_empty {
                        args.push("--allow-empty-message");
                    }
                    args.extend(match source {
                        "inline" => ["-m", body],
                        "file" => ["-F", "message.txt"],
                        _ => ["-F", "-"],
                    });
                    compare(
                        actual.path(),
                        oracle.path(),
                        &args,
                        (source == "stdin").then_some(body.as_bytes()),
                    )?;
                }
            }
        }
    }
    Ok(())
}

#[test]
fn no_edit_reuses_verbatim_message_and_empty_commit_rules_match_git() -> Result<()> {
    for cleanup in ["--cleanup=default", "--cleanup=verbatim"] {
        let actual = fixture("amend-verbatim")?;
        let oracle = fixture("amend-verbatim-oracle")?;
        let original_message = "Subject  \n\n# preserved\n\n";
        for repo in [&actual, &oracle] {
            git(
                repo.path(),
                &["commit", "--amend", "--cleanup=verbatim", "-m", original_message],
            )?;
        }
        ensure!(compare(
            actual.path(),
            oracle.path(),
            &["commit", "--amend", "--no-edit", cleanup],
            None
        )?);
        if cleanup == "--cleanup=verbatim" {
            let object = git(actual.path(), &["cat-file", "commit", "HEAD"])?;
            let offset = object
                .windows(2)
                .position(|bytes| bytes == b"\n\n")
                .expect("message separator")
                + 2;
            assert_eq!(
                &object[offset..],
                original_message.as_bytes(),
                "explicit verbatim reuse preserves every original message byte"
            );
        }
    }
    for allow in [false, true] {
        let actual = fixture("amend-empty")?;
        let oracle = fixture("amend-empty-oracle")?;
        for repo in [&actual, &oracle] {
            git(repo.path(), &["rm", "selected"])?;
        }
        let mut args = vec!["commit", "--amend", "--no-edit"];
        if allow {
            args.push("--allow-empty");
        }
        assert_eq!(compare(actual.path(), oracle.path(), &args, None)?, allow);
    }
    Ok(())
}

#[cfg(unix)]
fn hook(repo: &Path, name: &str, body: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let hook_path = repo.join(".git/hooks").join(name);
    fs::write(&hook_path, format!("#!/bin/sh\nset -eu\n{body}\n"))?;
    fs::set_permissions(hook_path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn hooks_receive_amend_context_mapping_and_commit_hook_staged_content() -> Result<()> {
    for source in ["--no-edit", "-m"] {
        let actual = fixture("amend-hooks")?;
        let oracle = fixture("amend-hooks-oracle")?;
        let old = actual.head_id()?;
        for repo in [&actual, &oracle] {
            hook(
                repo.path(),
                "pre-commit",
                "echo pre >> hook-log\nprintf 'hook staged\\n' > from-hook\ngit add from-hook",
            )?;
            hook(
                repo.path(),
                "prepare-commit-msg",
                "printf 'prepare %s %s\\n' \"${2-}\" \"${3-}\" >> hook-log",
            )?;
            hook(
                repo.path(),
                "commit-msg",
                "echo message >> hook-log\nprintf 'Hook subject  \\n\\n' > \"$1\"",
            )?;
            hook(repo.path(), "post-commit", "echo post >> hook-log")?;
            hook(
                repo.path(),
                "post-rewrite",
                "echo rewrite \"$*\" >> hook-log\ncat > rewrite-input",
            )?;
        }
        let args = if source == "--no-edit" {
            vec!["commit", "--amend", "--no-edit"]
        } else {
            vec!["commit", "--amend", "-m", "input"]
        };
        let expected = run(oracle.path(), false, &args, None)?;
        ensure!(expected.status.success());
        success(actual.path(), &args)?;
        assert_eq!(
            git(actual.path(), &["cat-file", "commit", "HEAD"])?,
            git(oracle.path(), &["cat-file", "commit", "HEAD"])?,
            "hook-edited message and hook-staged tree"
        );
        assert_eq!(
            fs::read(actual.path().join("hook-log"))?,
            fs::read(oracle.path().join("hook-log"))?,
            "hook order and arguments"
        );
        assert_eq!(
            fs::read(actual.path().join("rewrite-input"))?,
            format!("{old} {}\n", actual.head_id()?).as_bytes()
        );
        assert_eq!(git(actual.path(), &["show", "HEAD:from-hook"])?, b"hook staged\n");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn rejecting_hooks_preserve_intentional_side_effects_and_bypass_is_scoped() -> Result<()> {
    for rejecting in ["pre-commit", "prepare-commit-msg", "commit-msg"] {
        let repo = fixture("amend-reject-hook")?;
        let old = repo.head_id()?;
        hook(
            repo.path(),
            rejecting,
            "printf 'intentional side effect\\n' > hook-owned\ngit add hook-owned\nexit 1",
        )?;
        let out = run(repo.path(), true, &["commit", "--amend", "--no-edit"], None)?;
        ensure!(!out.status.success());
        assert_eq!(repo.head_id()?, old);
        assert_eq!(
            git(repo.path(), &["show", ":hook-owned"])?,
            b"intentional side effect\n"
        );
        assert_eq!(fs::read(repo.path().join("hook-owned"))?, b"intentional side effect\n");
    }
    let repo = fixture("amend-bypass")?;
    hook(repo.path(), "pre-commit", "exit 1")?;
    hook(repo.path(), "commit-msg", "exit 1")?;
    hook(repo.path(), "prepare-commit-msg", "echo prepare > hook-log")?;
    hook(repo.path(), "post-commit", "echo post >> hook-log\nexit 1")?;
    hook(
        repo.path(),
        "post-rewrite",
        "echo rewrite >> hook-log\ncat > rewrite-input\nexit 1",
    )?;
    let out = success(repo.path(), &["commit", "--amend", "--no-edit", "--no-verify"])?;
    assert_eq!(fs::read(repo.path().join("hook-log"))?, b"prepare\npost\nrewrite\n");
    let warning = String::from_utf8(out.stderr)?;
    ensure!(
        warning.contains("post-commit") && warning.contains("post-rewrite"),
        "both failures warn: {warning}"
    );
    Ok(())
}

#[test]
fn invalid_options_unborn_and_active_operations_refuse_without_mutation() -> Result<()> {
    let repo = fixture("amend-refusal")?;
    let old = repo.head_id()?;
    let before = index(repo.path())?;
    for args in [
        vec!["commit", "--amend"],
        vec!["commit", "--no-edit"],
        vec!["commit", "--amend", "--no-edit", "-m", "replacement"],
        vec!["commit", "--amend", "--no-edit", "-F", "missing"],
        vec!["commit", "--amend", "-m", "replacement", "-F", "missing"],
    ] {
        let out = run(repo.path(), true, &args, None)?;
        ensure!(!out.status.success(), "invalid {args:?} must refuse");
        assert_eq!(repo.head_id()?, old);
        assert_eq!(index(repo.path())?, before);
    }
    for state in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        let path = repo.path().join(".git").join(state);
        if state.contains("HEAD") {
            fs::write(&path, format!("{old}\n"))?;
        } else {
            fs::create_dir(&path)?;
        }
        ensure!(
            !run(repo.path(), true, &["commit", "--amend", "--no-edit"], None)?
                .status
                .success(),
            "active {state}"
        );
        assert_eq!(repo.head_id()?, old);
        assert_eq!(index(repo.path())?, before);
        if path.is_dir() {
            fs::remove_dir(path)?;
        } else {
            fs::remove_file(path)?;
        }
    }
    let unborn = tempfile::tempdir()?;
    git(unborn.path(), &["init", "-q", "--initial-branch=main"])?;
    fs::write(unborn.path().join("first"), "first\n")?;
    git(unborn.path(), &["add", "first"])?;
    let before = index(unborn.path())?;
    ensure!(!run(unborn.path(), true, &["commit", "--amend", "--no-edit"], None)?
        .status
        .success());
    assert_eq!(index(unborn.path())?, before);
    ensure!(!unborn.path().join(".git/refs/heads/main").exists());
    Ok(())
}

#[cfg(unix)]
#[test]
fn unsupported_metadata_and_policies_refuse_before_hooks() -> Result<()> {
    for metadata in ["encoding", "signature", "extra", "non-utf8", "signing", "notes"] {
        let repo = fixture("amend-metadata")?;
        let object = git(repo.path(), &["cat-file", "commit", "HEAD"])?;
        if metadata == "signing" {
            git(repo.path(), &["config", "commit.gpgSign", "true"])?;
        } else if metadata == "notes" {
            git(repo.path(), &["config", "notes.rewrite.amend", "true"])?;
            git(repo.path(), &["config", "notes.rewriteRef", "refs/notes/commits"])?;
        } else {
            let split = object.windows(2).position(|b| b == b"\n\n").expect("commit separator");
            let mut altered = object[..split + 1].to_vec();
            altered.extend_from_slice(match metadata {
                "encoding" => b"encoding ISO-8859-1\n",
                "signature" => b"gpgsig fake-signature\n continuation\n",
                "extra" => b"x-custom retained-required\n",
                _ => b"",
            });
            altered.push(b'\n');
            altered.extend_from_slice(if metadata == "non-utf8" {
                b"invalid \xff\n"
            } else {
                &object[split + 2..]
            });
            let out = run(
                repo.path(),
                false,
                &["hash-object", "-t", "commit", "-w", "--stdin"],
                Some(&altered),
            )?;
            ensure!(out.status.success());
            let oid = String::from_utf8(out.stdout)?;
            git(repo.path(), &["update-ref", "HEAD", oid.trim()])?;
        }
        hook(repo.path(), "pre-commit", "echo ran > hook-ran")?;
        let old = repo.head_id()?;
        let before = index(repo.path())?;
        let out = run(repo.path(), true, &["commit", "--amend", "-m", "replacement"], None)?;
        ensure!(!out.status.success(), "unsupported {metadata} must refuse");
        assert_eq!(repo.head_id()?, old);
        assert_eq!(index(repo.path())?, before);
        ensure!(
            !repo.path().join("hook-ran").exists(),
            "{metadata} must refuse before hooks"
        );
    }
    Ok(())
}

#[test]
fn alternate_indexes_and_linked_worktrees_stay_isolated() -> Result<()> {
    let repo = fixture("amend-alternate-index")?;
    let before = index(repo.path())?;
    let alt = repo.path().join(".git/alternate-index");
    fs::copy(repo.path().join(".git/index"), &alt)?;
    fs::write(repo.path().join("selected"), "alternate staged\n")?;
    let out = command(repo.path(), false)
        .env("GIT_INDEX_FILE", &alt)
        .args(["add", "selected"])
        .output()?;
    ensure!(out.status.success());
    let alt_before = fs::read(&alt)?;
    let out = command(repo.path(), true)
        .env("GIT_INDEX_FILE", &alt)
        .args(["commit", "--amend", "--no-edit"])
        .output()?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(git(repo.path(), &["show", "HEAD:selected"])?, b"alternate staged\n");
    assert_eq!(index(repo.path())?, before);
    assert_eq!(fs::read(alt)?, alt_before);
    let main_head = repo.head_id()?;
    let linked = tempfile::tempdir()?;
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.path().to_str().expect("temp path"),
        ],
    )?;
    fs::write(linked.path().join("selected"), "linked staged\n")?;
    git(linked.path(), &["add", "selected"])?;
    success(linked.path(), &["commit", "--amend", "-m", "linked amendment"])?;
    assert_eq!(git(linked.path(), &["show", "HEAD:selected"])?, b"linked staged\n");
    assert_eq!(repo.head_id()?, main_head);
    assert_eq!(index(repo.path())?, before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn publication_guards_original_attachment_and_oid_and_refuses_symbolic_chains() -> Result<()> {
    for change in ["branch", "detach", "oid"] {
        let repo = fixture("amend-race")?;
        let old = repo.head_id()?;
        git(repo.path(), &["branch", "other"])?;
        let body = match change {
            "branch" => "git symbolic-ref HEAD refs/heads/other",
            "detach" => "git update-ref --no-deref HEAD HEAD",
            _ => "git update-ref refs/heads/main HEAD~1",
        };
        hook(repo.path(), "prepare-commit-msg", body)?;
        let before = index(repo.path())?;
        let out = run(
            repo.path(),
            true,
            &["commit", "--amend", "-m", "must not publish"],
            None,
        )?;
        ensure!(
            !out.status.success(),
            "{change} race must refuse: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(index(repo.path())?, before);
        assert_eq!(
            String::from_utf8(git(repo.path(), &["rev-parse", "other"])?)?.trim(),
            old
        );
        let expected_main = if change == "oid" {
            git(repo.path(), &["rev-parse", "other~1"])?
        } else {
            format!("{old}\n").into_bytes()
        };
        assert_eq!(
            git(repo.path(), &["rev-parse", "main"])?,
            expected_main,
            "no ref chosen after hook may be published"
        );
    }
    let repo = fixture("amend-symbolic-chain")?;
    git(repo.path(), &["branch", "actual"])?;
    git(repo.path(), &["symbolic-ref", "refs/heads/main", "refs/heads/actual"])?;
    hook(repo.path(), "pre-commit", "echo ran > hook-ran")?;
    let old = repo.head_id()?;
    ensure!(!run(repo.path(), true, &["commit", "--amend", "--no-edit"], None)?
        .status
        .success());
    assert_eq!(repo.head_id()?, old);
    ensure!(!repo.path().join("hook-ran").exists());
    Ok(())
}

#[test]
fn head_and_branch_lock_contention_never_publish_or_remove_another_owners_lock() -> Result<()> {
    for name in ["HEAD.lock", "refs/heads/main.lock"] {
        let repo = fixture("amend-lock")?;
        let old = repo.head_id()?;
        let before = index(repo.path())?;
        let lock = repo.path().join(".git").join(name);
        fs::write(&lock, "other owner\n")?;
        ensure!(!run(repo.path(), true, &["commit", "--amend", "-m", "blocked"], None)?
            .status
            .success());
        assert_eq!(repo.head_id()?, old);
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(lock)?, b"other owner\n");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn interruption_during_hook_prevents_publication_but_after_publication_preserves_commit() -> Result<()> {
    for phase in ["pre-commit", "prepare-commit-msg", "commit-msg", "post-commit"] {
        let repo = fixture("amend-interrupt")?;
        let old = repo.head_id()?;
        let before = index(repo.path())?;
        // The hook delivers SIGINT while Brit waits for it. It then exits normally,
        // so success cannot accidentally be attributed to a failed hook status.
        hook(
            repo.path(),
            phase,
            "echo intentional > hook-owned\nkill -INT \"$PPID\"\nsleep 0.1",
        )?;
        hook(repo.path(), "post-rewrite", "cat > rewrite-input")?;
        let out = run(
            repo.path(),
            true,
            &["commit", "--amend", "-m", "interrupted amendment"],
            None,
        )?;
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(repo.path().join("hook-owned"))?, b"intentional\n");
        if phase == "post-commit" {
            assert_ne!(
                repo.head_id()?,
                old,
                "publication cannot be rolled back after post-commit"
            );
            assert_eq!(
                fs::read(repo.path().join("rewrite-input"))?,
                format!("{old} {}\n", repo.head_id()?).as_bytes()
            );
            let report = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            ensure!(
                report.contains(&repo.head_id()?[..7]) || report.contains("created") || report.contains("published"),
                "published outcome must be reported honestly: {report}"
            );
        } else {
            ensure!(
                !out.status.success(),
                "cancelled amendment must refuse before publication"
            );
            assert_eq!(repo.head_id()?, old);
            ensure!(!repo.path().join("rewrite-input").exists());
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn interruption_while_waiting_for_ref_lock_prevents_publication() -> Result<()> {
    use std::time::{Duration, Instant};
    let repo = fixture("amend-interrupt-lock")?;
    git(repo.path(), &["config", "core.filesRefLockTimeout", "3000"])?;
    hook(repo.path(), "commit-msg", "echo ready > hook-ready")?;
    let old = repo.head_id()?;
    let before = index(repo.path())?;
    let lock = repo.path().join(".git/refs/heads/main.lock");
    fs::write(&lock, "other owner\n")?;
    let mut child = command(repo.path(), true)
        .args(["commit", "--amend", "-m", "cancelled while waiting"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !repo.path().join("hook-ready").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !repo.path().join("hook-ready").exists() {
        let _ = child.kill();
        let out = child.wait_with_output()?;
        anyhow::bail!(
            "commit hook never became ready: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    // Give the prepared commit time to enter the contended native transaction.
    std::thread::sleep(Duration::from_millis(100));
    let signal = Command::new("kill").args(["-INT", &child.id().to_string()]).status()?;
    ensure!(signal.success(), "deliver interrupt to waiting Brit process");
    std::thread::sleep(Duration::from_millis(50));
    fs::remove_file(&lock)?;
    let out = child.wait_with_output()?;
    ensure!(!out.status.success(), "cancelled lock waiter must fail");
    assert_eq!(
        repo.head_id()?,
        old,
        "an interrupt received during ref locking must prevent publication"
    );
    assert_eq!(index(repo.path())?, before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn identical_oid_lock_failure_is_not_reported_as_successful_publication() -> Result<()> {
    let repo = fixture("amend-identical-oid")?;
    let old = repo.head_id()?;
    hook(repo.path(), "post-commit", "echo post > post-ran")?;
    hook(repo.path(), "post-rewrite", "cat > rewrite-input")?;
    let lock = repo.path().join(".git/refs/heads/main.lock");
    fs::write(&lock, "other owner\n")?;
    let out = command(repo.path(), true)
        .env("GIT_AUTHOR_NAME", "Sebastian Thiel")
        .env("GIT_AUTHOR_EMAIL", "git@example.com")
        .env("GIT_AUTHOR_DATE", "2020-09-09 09:06:03 +0800")
        .env("GIT_COMMITTER_NAME", "Sebastian Thiel")
        .env("GIT_COMMITTER_EMAIL", "git@example.com")
        .env("GIT_COMMITTER_DATE", "2020-09-09 09:06:03 +0800")
        .args(["commit", "--amend", "--no-edit"])
        .output()?;
    ensure!(
        !out.status.success(),
        "an unchanged target OID does not prove that our transaction published"
    );
    assert_eq!(repo.head_id()?, old);
    assert_eq!(fs::read(lock)?, b"other owner\n");
    ensure!(
        !repo.path().join("post-ran").exists(),
        "failed transaction must not run post-commit"
    );
    ensure!(
        !repo.path().join("rewrite-input").exists(),
        "failed transaction must not run post-rewrite"
    );
    Ok(())
}

#[test]
fn identical_oid_success_records_the_same_reflog_as_git() -> Result<()> {
    for detached in [true, false] {
        let actual = fixture("amend-identical-success")?;
        let oracle = fixture("amend-identical-success-oracle")?;
        let old = actual.head_id()?;
        for repo in [&actual, &oracle] {
            if detached {
                git(repo.path(), &["checkout", "--detach"])?;
            }
        }
        // Run Git first so --nocapture records the oracle even if Brit refuses.
        let before_head = fs::read(oracle.path().join(".git/logs/HEAD"))?;
        let before_branch = fs::read(oracle.path().join(".git/logs/refs/heads/main"))?;
        for (repo, native) in [(&oracle, false), (&actual, true)] {
            let out = command(repo.path(), native)
                .env("GIT_COMMITTER_NAME", "Sebastian Thiel")
                .env("GIT_COMMITTER_EMAIL", "git@example.com")
                .env("GIT_COMMITTER_DATE", "2020-09-09 09:06:03 +0800")
                .args(["commit", "--amend", "--no-edit"])
                .output()?;
            ensure!(
                out.status.success(),
                "identical amend detached={detached}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            assert_eq!(
                repo.head_id()?,
                old,
                "fixed-time amendment produces identical commit object"
            );
            if !native {
                eprintln!(
                    "Git identical amendment detached={detached}: HEAD reflog appended={}, branch reflog appended={}",
                    fs::read(repo.path().join(".git/logs/HEAD"))? != before_head,
                    fs::read(repo.path().join(".git/logs/refs/heads/main"))? != before_branch
                );
            }
        }
        for log in ["logs/HEAD", "logs/refs/heads/main"] {
            assert_eq!(
                fs::read(actual.path().join(".git").join(log))?,
                fs::read(oracle.path().join(".git").join(log))?,
                "identical OID amendment reflog parity: detached={detached}, log={log}"
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn attachment_cas_is_checked_after_waiting_for_the_branch_lock() -> Result<()> {
    use std::time::{Duration, Instant};
    let repo = fixture("amend-attachment-lock-race")?;
    git(repo.path(), &["config", "core.filesRefLockTimeout", "3000"])?;
    git(repo.path(), &["branch", "other"])?;
    hook(repo.path(), "commit-msg", "echo ready > hook-ready")?;
    let old = repo.head_id()?;
    let before = index(repo.path())?;
    let lock = repo.path().join(".git/refs/heads/main.lock");
    fs::write(&lock, "other owner\n")?;
    let mut child = command(repo.path(), true)
        .args(["commit", "--amend", "-m", "must not follow moved HEAD"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !repo.path().join("hook-ready").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    if !repo.path().join("hook-ready").exists() {
        let _ = child.kill();
        let out = child.wait_with_output()?;
        anyhow::bail!(
            "commit hook never became ready: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    std::thread::sleep(Duration::from_millis(100));
    git(repo.path(), &["symbolic-ref", "HEAD", "refs/heads/other"])?;
    fs::remove_file(lock)?;
    let out = child.wait_with_output()?;
    ensure!(
        !out.status.success(),
        "HEAD attachment CAS must reject a same-OID branch switch during lock wait"
    );
    for branch in ["main", "other"] {
        assert_eq!(
            String::from_utf8(git(repo.path(), &["rev-parse", branch])?)?.trim(),
            old
        );
    }
    assert_eq!(git(repo.path(), &["symbolic-ref", "HEAD"])?, b"refs/heads/other\n");
    assert_eq!(index(repo.path())?, before);
    Ok(())
}

#[test]
fn empty_root_and_merge_amendment_rules_match_git() -> Result<()> {
    for shape in ["root", "merge"] {
        for allow in [false, true] {
            let actual = TestRepo::new("amend-empty-shape")?;
            let oracle = TestRepo::new("amend-empty-shape-oracle")?;
            for repo in [&actual, &oracle] {
                if shape == "root" {
                    fs::write(repo.path().join("root-file"), "root\n")?;
                    git(repo.path(), &["add", "root-file"])?;
                    git(repo.path(), &["commit", "--amend", "-m", "root with content"])?;
                    git(repo.path(), &["rm", "root-file"])?;
                } else {
                    repo.commit_file("main-file", "main\n")?;
                    git(repo.path(), &["checkout", "-qb", "side", "HEAD~1"])?;
                    repo.commit_file("side-file", "side\n")?;
                    git(repo.path(), &["checkout", "main"])?;
                    git(repo.path(), &["merge", "--no-ff", "-m", "merge", "side"])?;
                    // The resulting tree equals the first parent; the second
                    // parent still carries meaning and must not be discarded.
                    git(repo.path(), &["rm", "side-file"])?;
                }
            }
            let mut args = vec!["commit", "--amend", "--no-edit"];
            if allow {
                args.push("--allow-empty");
            }
            compare(actual.path(), oracle.path(), &args, None)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn encoding_notes_environment_and_invalid_message_file_are_checked_before_hooks() -> Result<()> {
    for policy in ["encoding", "notes-env", "disabled-notes", "invalid-file"] {
        let repo = fixture("amend-policy")?;
        hook(repo.path(), "pre-commit", "echo ran > hook-ran")?;
        let old = repo.head_id()?;
        let before = index(repo.path())?;
        let mut cmd = command(repo.path(), true);
        let mut args = vec!["commit", "--amend", "--no-edit"];
        match policy {
            "encoding" => {
                git(repo.path(), &["config", "i18n.commitEncoding", "ISO-8859-1"])?;
            }
            "notes-env" => {
                cmd.env("GIT_NOTES_REWRITE_REF", "refs/notes/commits");
            }
            "disabled-notes" => {
                git(repo.path(), &["notes", "add", "-m", "original note", &old])?;
                git(repo.path(), &["config", "notes.rewrite.amend", "false"])?;
                git(repo.path(), &["config", "notes.rewriteRef", "refs/notes/commits"])?;
                cmd.env("GIT_NOTES_REWRITE_REF", "refs/notes/commits");
            }
            _ => {
                fs::write(repo.path().join("invalid-message"), b"invalid UTF-8 \xff\n")?;
                args = vec!["commit", "--amend", "-F", "invalid-message"];
            }
        }
        let out = cmd.args(args).output()?;
        if policy == "disabled-notes" {
            ensure!(
                out.status.success(),
                "explicitly disabled notes rewrite permits amendment: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            ensure!(repo.path().join("hook-ran").exists());
            assert_ne!(repo.head_id()?, old);
            let oracle = fixture("amend-disabled-notes-oracle")?;
            git(oracle.path(), &["notes", "add", "-m", "original note", &old])?;
            let expected = command(oracle.path(), false)
                .env("GIT_NOTES_REWRITE_REF", "refs/notes/commits")
                .args(["-c", "notes.rewrite.amend=false", "commit", "--amend", "--no-edit"])
                .output()?;
            ensure!(expected.status.success(), "Git disabled-notes oracle succeeds");
            assert_eq!(repo.head_id()?, oracle.head_id()?);
            for path in [repo.path(), oracle.path()] {
                ensure!(
                    !run(path, false, &["notes", "show", "HEAD"], None)?.status.success(),
                    "disabled rewrite does not copy notes, even with environment refs"
                );
                assert_eq!(git(path, &["notes", "show", &old])?, b"original note\n");
            }
        } else {
            ensure!(!out.status.success(), "{policy} must refuse before hooks");
            ensure!(!repo.path().join("hook-ran").exists(), "{policy} executed pre-commit");
            assert_eq!(repo.head_id()?, old);
        }
        assert_eq!(index(repo.path())?, before);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn reflog_failure_reports_whether_publication_occurred_and_runs_post_hooks_only_afterward() -> Result<()> {
    for log in ["HEAD", "refs/heads/main"] {
        let repo = fixture("amend-partial-publication")?;
        let old = repo.head_id()?;
        let before = index(repo.path())?;
        hook(repo.path(), "post-commit", "echo post > post-ran")?;
        hook(repo.path(), "post-rewrite", "cat > rewrite-input")?;
        let log_path = repo.path().join(".git/logs").join(log);
        let saved_log = log_path.with_extension("saved");
        fs::rename(&log_path, &saved_log)?;
        fs::create_dir(&log_path)?;
        // The native reflog writer legitimately prunes empty directories;
        // nonempty owned content forces the intended append failure.
        let obstruction = log_path.join("owned-content");
        fs::write(&obstruction, "other owner\n")?;
        let out = run(
            repo.path(),
            true,
            &["commit", "--amend", "-m", "partial publication proof"],
            None,
        )?;
        let stderr = String::from_utf8(out.stderr)?;
        if log == "HEAD" {
            ensure!(
                out.status.success(),
                "published commit must not be reported as refused: {stderr}"
            );
            let new = repo.head_id()?;
            assert_ne!(new, old, "pinned branch publishes before HEAD reflog failure");
            assert_eq!(
                String::from_utf8(git(repo.path(), &["rev-parse", "refs/heads/main"])?)?.trim(),
                new
            );
            ensure!(
                stderr.contains("was published") && stderr.contains("no rollback"),
                "partial publication warning names durable outcome: {stderr}"
            );
            assert_eq!(fs::read(repo.path().join("post-ran"))?, b"post\n");
            assert_eq!(
                fs::read(repo.path().join("rewrite-input"))?,
                format!("{old} {new}\n").as_bytes()
            );
        } else {
            ensure!(
                !out.status.success(),
                "branch reflog failure before publication refuses"
            );
            assert_eq!(repo.head_id()?, old, "failed branch reflog leaves branch unchanged");
            ensure!(!repo.path().join("post-ran").exists());
            ensure!(!repo.path().join("rewrite-input").exists());
        }
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
        ensure!(log_path.is_dir(), "another owner's obstructing path remains intact");
        assert_eq!(fs::read(obstruction)?, b"other owner\n");
        ensure!(
            saved_log.is_file(),
            "original reflog bytes remain available for recovery"
        );
    }
    Ok(())
}
