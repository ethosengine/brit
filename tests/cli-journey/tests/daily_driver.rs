//! Bounded daily workflow: Git bridges isolate/stage/abort; Brit authors scoped commits.
//! The operator equivalents are `git worktree add -b scoped <path>`,
//! `git add -- <path>`, `brit commit -m <message>`, and explicit
//! `git restore --source=HEAD --staged --worktree -- <path>` to abort later edits.
use std::{collections::BTreeMap, fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn brit(repo: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required installed or built binary"))
        .args(args.iter().copied())
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Developer Two")
        .env("GIT_AUTHOR_EMAIL", "two@example.com")
        .env("GIT_COMMITTER_NAME", "Developer Two")
        .env("GIT_COMMITTER_EMAIL", "two@example.com")
        .run()
}

#[derive(Debug, PartialEq, Eq)]
struct State {
    head: String,
    index: Vec<u8>,
    files: BTreeMap<String, Vec<u8>>,
}

fn state(repo: &Path) -> Result<State> {
    fn files(root: &Path, dir: &Path, entries: &mut BTreeMap<String, Vec<u8>>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_name() == ".git" {
                continue;
            }
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                files(root, &path, entries)?;
            } else {
                entries.insert(path.strip_prefix(root)?.to_string_lossy().into_owned(), fs::read(path)?);
            }
        }
        Ok(())
    }
    let mut content = BTreeMap::new();
    files(repo, repo, &mut content)?;
    Ok(State {
        head: git(repo, &["rev-parse", "HEAD"])?,
        index: fs::read(repo.join(git(repo, &["rev-parse", "--git-path", "index"])?))?,
        files: content,
    })
}

#[test]
fn unsupported_public_mutations_refuse_and_preserve_all_bytes() -> Result<()> {
    let repo = TestRepo::new("daily-refusal")?;
    repo.commit_file("tracked", "original")?;
    fs::write(repo.path().join("tracked"), "staged")?;
    git(repo.path(), &["add", "--", "tracked"])?;
    fs::write(repo.path().join("tracked"), "unstaged")?;
    fs::write(repo.path().join("untracked"), "keep")?;
    let before = state(repo.path())?;
    for args in [
        vec!["add", "tracked"],
        vec!["restore", "tracked"],
        vec!["reset", "--hard", "HEAD"],
        vec!["rm", "--force", "tracked"],
        vec!["mv", "tracked", "moved"],
    ] {
        let output = brit(repo.path(), &args)?;
        ensure!(
            !output.status.success(),
            "unsupported mutation must not claim success: {args:?}"
        );
        ensure!(output.stderr.contains("unrecognized subcommand"), "{}", output.stderr);
        assert_eq!(
            state(repo.path())?,
            before,
            "unsupported {args:?} must preserve HEAD, index and all file bytes"
        );
    }
    Ok(())
}

#[cfg(unix)]
mod interrupted {
    use super::*;
    use std::{
        os::unix::{fs::PermissionsExt, process::CommandExt},
        process::{Child, Command, Stdio},
        thread,
        time::{Duration, Instant},
    };

    // Own exactly one new process group. Drop reaps both Brit and the sleeping
    // hook on test failure too, so interruption fixtures cannot leak processes.
    struct Operation(Child);
    impl Drop for Operation {
        fn drop(&mut self) {
            let _ = Command::new("kill")
                .args(["-KILL", "--", &format!("-{}", self.0.id())])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            let _ = self.0.wait();
        }
    }

    fn hook(common: &Path, name: &str, body: &str) -> Result<()> {
        let path = common.join(".git/hooks").join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
        Ok(())
    }

    fn interrupt_commit(repo: &Path, marker: &Path) -> Result<()> {
        let config = tempfile::tempdir()?;
        let mut command = Command::new(brit_bin().expect("required binary"));
        gix_testtools::configure_git_environment(&mut command, repo);
        command
            .env("GIT_CONFIG_GLOBAL", config.path().join("missing.gitconfig"))
            .env("GIT_AUTHOR_NAME", "Developer One")
            .env("GIT_AUTHOR_EMAIL", "one@example.com")
            .env("GIT_COMMITTER_NAME", "Developer One")
            .env("GIT_COMMITTER_EMAIL", "one@example.com")
            .args(["commit", "-m", "scoped work"])
            .current_dir(repo)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        let mut operation = Operation(command.spawn()?);
        let deadline = Instant::now() + Duration::from_secs(15);
        while !marker.exists() {
            ensure!(
                operation.0.try_wait()?.is_none(),
                "commit exited before the interruption boundary"
            );
            ensure!(
                Instant::now() < deadline,
                "hook did not reach the interruption boundary"
            );
            thread::sleep(Duration::from_millis(20));
        }
        drop(operation);
        Ok(())
    }

    fn fixture() -> Result<(TestRepo, tempfile::TempDir)> {
        let repo = TestRepo::new("daily-isolation")?;
        repo.commit_file("neighbor", "committed neighbor")?;
        repo.commit_file("scoped", "committed scoped")?;
        fs::write(repo.path().join("neighbor"), "another developer's staged bytes")?;
        git(repo.path(), &["add", "--", "neighbor"])?;
        fs::write(repo.path().join("neighbor"), "another developer's unstaged bytes")?;
        fs::write(repo.path().join("private"), "another developer's untracked bytes")?;
        let linked = tempfile::tempdir()?;
        // Explicit stock-Git bridge: isolated worktree rather than shared-index surgery.
        git(
            repo.path(),
            &[
                "worktree",
                "add",
                "-b",
                "scoped",
                linked.path().to_str().expect("temp path"),
            ],
        )?;
        fs::write(linked.path().join("scoped"), "scoped contribution")?;
        git(linked.path(), &["add", "--", "scoped"])?;
        Ok((repo, linked))
    }

    #[test]
    fn two_developer_resume_after_rejection_and_prepublication_interruption() -> Result<()> {
        let (repo, linked) = fixture()?;
        let neighbor = state(repo.path())?;
        let staged = state(linked.path())?;
        hook(repo.path(), "pre-commit", "echo rejected >&2; exit 1")?;
        let rejected = brit(linked.path(), &["commit", "-m", "scoped work"])?;
        ensure!(!rejected.status.success() && rejected.stderr.contains("rejected"));
        assert_eq!(
            state(linked.path())?,
            staged,
            "rejection retains the scoped staged change"
        );
        let marker = repo.path().join(".git/pre-publication-ready");
        hook(
            repo.path(),
            "pre-commit",
            "common=$(git rev-parse --git-common-dir)\nprintf ready > \"$common/pre-publication-ready\"\nexec sleep 60",
        )?;
        interrupt_commit(linked.path(), &marker)?;
        assert_eq!(
            state(linked.path())?,
            staged,
            "termination before publication leaves HEAD, index and files intact"
        );
        fs::remove_file(repo.path().join(".git/hooks/pre-commit"))?;
        // Developer Two resumes from persisted state using a fresh public process.
        let resumed = brit(linked.path(), &["commit", "-m", "scoped work"])?;
        ensure!(resumed.status.success(), "{}", resumed.stderr);
        assert_eq!(
            git(
                linked.path(),
                &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"]
            )?,
            "scoped"
        );
        assert_eq!(git(linked.path(), &["log", "-1", "--format=%an"])?, "Developer Two");
        assert_eq!(git(linked.path(), &["show", "HEAD:scoped"])?, "scoped contribution");
        assert_eq!(
            state(repo.path())?,
            neighbor,
            "another developer's staged/unstaged/untracked bytes survive"
        );
        // Explicit abort affects only the later named path in the isolated worktree.
        let accepted = state(linked.path())?;
        fs::write(linked.path().join("scoped"), "abandoned edit")?;
        git(linked.path(), &["add", "--", "scoped"])?;
        git(
            linked.path(),
            &["restore", "--source=HEAD", "--staged", "--worktree", "--", "scoped"],
        )?;
        assert_eq!(state(linked.path())?.head, accepted.head);
        assert_eq!(state(linked.path())?.files, accepted.files);
        assert_eq!(git(linked.path(), &["diff", "--cached", "--name-only"])?, "");
        assert_eq!(state(repo.path())?, neighbor);
        git(repo.path(), &["fsck", "--no-dangling"])?;
        Ok(())
    }

    #[test]
    fn postpublication_interruption_is_recognized_as_committed_without_duplicate() -> Result<()> {
        let (repo, linked) = fixture()?;
        let neighbor = state(repo.path())?;
        let before = git(linked.path(), &["rev-parse", "HEAD"])?;
        let marker = repo.path().join(".git/post-publication-ready");
        hook(repo.path(), "post-commit", "common=$(git rev-parse --git-common-dir)\nprintf ready > \"$common/post-publication-ready\"\nexec sleep 60")?;
        interrupt_commit(linked.path(), &marker)?;
        let committed = git(linked.path(), &["rev-parse", "HEAD"])?;
        ensure!(
            committed != before,
            "post-commit interruption must retain the already-published commit"
        );
        assert_eq!(git(linked.path(), &["rev-parse", "HEAD^"])?, before);
        assert_eq!(git(linked.path(), &["show", "HEAD:scoped"])?, "scoped contribution");
        fs::remove_file(repo.path().join(".git/hooks/post-commit"))?;
        let retry = brit(linked.path(), &["commit", "-m", "scoped work"])?;
        ensure!(
            !retry.status.success() && retry.stderr.contains("nothing to commit"),
            "{}",
            retry.stderr
        );
        assert_eq!(
            git(linked.path(), &["rev-parse", "HEAD"])?,
            committed,
            "retry must not create a duplicate commit"
        );
        assert_eq!(state(repo.path())?, neighbor);
        git(linked.path(), &["fsck", "--no-dangling"])?;
        Ok(())
    }
}
