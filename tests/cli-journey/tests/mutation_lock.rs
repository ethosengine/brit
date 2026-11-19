//! Brit's writer guards: a paused hook in one worktree leaves other lanes writable,
//! and a refused writer names its holder or waits for it.
#![cfg(target_os = "linux")]
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = gix_testtools::git_command(repo)
        .args(["-c", &format!("safe.directory={}", repo.display())])
        .args(args)
        .output()?;
    ensure!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn brit(repo: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required binary"))
        .args(args.iter().copied())
        .current_dir(repo)
        .run()
}

/// A Brit process left running in the background, killed if the test fails early.
struct Background {
    child: Child,
    _config: tempfile::TempDir,
}

impl Background {
    fn spawn(repo: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Result<Self> {
        let config = tempfile::tempdir()?;
        let mut command = Command::new(brit_bin().expect("required binary"));
        gix_testtools::configure_git_environment(&mut command, repo);
        command
            .env("GIT_CONFIG_GLOBAL", config.path().join("missing.gitconfig"))
            .args(args)
            .current_dir(repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in envs {
            command.env(key, value);
        }
        Ok(Self {
            child: command.spawn()?,
            _config: config,
        })
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn running(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_none())
    }

    fn finish(mut self) -> Result<Capture> {
        let mut stdout = String::new();
        let mut stderr = String::new();
        self.child.stdout.take().expect("piped").read_to_string(&mut stdout)?;
        self.child.stderr.take().expect("piped").read_to_string(&mut stderr)?;
        let status = self.child.wait()?;
        Ok(Capture { stdout, stderr, status })
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Install `name` as a hook that pauses only for a process given `BRIT_TEST_PAUSE`:
/// it records `started`, then waits for `release` to appear in that directory.
fn pausing_hook(repo: &Path, hooks: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(hooks)?;
    git(
        repo,
        &["config", "core.hooksPath", hooks.to_str().expect("utf8 temp path")],
    )?;
    let path = hooks.join(name);
    fs::write(
        &path,
        "#!/bin/sh\n\
         cat >/dev/null\n\
         test -n \"$BRIT_TEST_PAUSE\" || exit 0\n\
         : >\"$BRIT_TEST_PAUSE/started\"\n\
         while ! test -e \"$BRIT_TEST_PAUSE/release\"; do sleep 0.05; done\n",
    )?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

fn await_file(path: &Path, process: &mut Background) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !path.exists() {
        if !process.running()? {
            bail!("process exited before {}", path.display());
        }
        ensure!(Instant::now() < deadline, "timed out waiting for {}", path.display());
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

struct Lanes {
    repo: TestRepo,
    remote: tempfile::TempDir,
    other: PathBuf,
    scratch: tempfile::TempDir,
}

/// A main worktree on `main` with a bare remote, and a linked worktree on `lane-b`.
fn lanes(label: &str) -> Result<Lanes> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("base", "base\n")?;
    let remote = tempfile::tempdir()?;
    git(remote.path(), &["init", "--bare", "-q"])?;
    git(
        repo.path(),
        &["remote", "add", "origin", remote.path().to_str().expect("temp path")],
    )?;
    let scratch = tempfile::tempdir()?;
    let other = scratch.path().join("lane-b");
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "lane-b",
            other.to_str().expect("temp path"),
        ],
    )?;
    Ok(Lanes {
        repo,
        remote,
        other,
        scratch,
    })
}

#[test]
fn push_paused_in_pre_push_hook_leaves_other_worktrees_writable() -> Result<()> {
    let Lanes {
        repo,
        remote,
        other,
        scratch,
    } = lanes("lock-push-hook")?;
    let pause = scratch.path().join("pause");
    fs::create_dir(&pause)?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-push")?;
    let pushed = repo.commit_file("payload", "pushed\n")?;

    let mut push = Background::spawn(
        repo.path(),
        &["push", "origin", "main:main"],
        &[("BRIT_TEST_PAUSE", &pause)],
    )?;
    await_file(&pause.join("started"), &mut push)?;

    fs::write(other.join("lane-file"), "lane b\n")?;
    let add = brit(&other, &["add", "lane-file"])?;
    ensure!(add.status.success(), "add beside a paused push: {}", add.stderr);
    let commit = brit(&other, &["commit", "-m", "lane b work"])?;
    ensure!(
        commit.status.success(),
        "commit beside a paused push: {}",
        commit.stderr
    );
    assert_eq!(git(&other, &["log", "-1", "--format=%s"])?, "lane b work");
    let third = scratch.path().join("lane-c");
    let worktree = brit(
        &other,
        &["worktree", "add", "-b", "lane-c", third.to_str().expect("temp path")],
    )?;
    ensure!(
        worktree.status.success(),
        "worktree add beside a paused push: {}",
        worktree.stderr
    );
    ensure!(third.join("lane-file").is_file(), "new worktree is checked out");
    let branch = brit(&other, &["branch", "create", "side"])?;
    ensure!(
        branch.status.success(),
        "shared ref write beside a paused push: {}",
        branch.stderr
    );
    ensure!(push.running()?, "the push is still inside its hook");

    fs::write(pause.join("release"), "")?;
    let push = push.finish()?;
    ensure!(push.status.success(), "released push: {}", push.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/main"])?, pushed);
    assert_eq!(repo.head_id()?, pushed, "push leaves the local branch unchanged");
    git(remote.path(), &["fsck", "--full"])?;
    Ok(())
}

#[test]
fn paused_commit_hook_keeps_lanes_writable_names_its_holder_and_can_be_awaited() -> Result<()> {
    let Lanes {
        repo, other, scratch, ..
    } = lanes("lock-commit-hook")?;
    let pause = scratch.path().join("pause");
    fs::create_dir(&pause)?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-commit")?;
    fs::write(repo.path().join("held"), "held\n")?;
    git(repo.path(), &["add", "held"])?;

    let mut held = Background::spawn(
        repo.path(),
        &["commit", "-m", "held commit"],
        &[("BRIT_TEST_PAUSE", &pause)],
    )?;
    await_file(&pause.join("started"), &mut held)?;
    let holder = held.pid().to_string();

    // Worktree-local writes in another lane proceed beside the paused commit.
    fs::write(other.join("lane-file"), "lane b\n")?;
    let add = brit(&other, &["add", "lane-file"])?;
    ensure!(add.status.success(), "add beside a paused commit: {}", add.stderr);
    let commit = brit(&other, &["commit", "-m", "lane b work"])?;
    ensure!(
        commit.status.success(),
        "commit beside a paused commit: {}",
        commit.stderr
    );

    // The paused lane's own worktree stays exclusive, and the refusal names its holder.
    fs::write(repo.path().join("second"), "second\n")?;
    let same = brit(repo.path(), &["add", "second"])?;
    ensure!(!same.status.success(), "same-worktree add must refuse");
    for expected in ["another Brit mutation is active", &holder, "commit"] {
        ensure!(
            same.stderr.contains(expected),
            "refusal names {expected:?}: {}",
            same.stderr
        );
    }

    // A shared write refuses at once by default, naming the holder.
    let refs_before = git(repo.path(), &["show-ref"])?;
    let refused = brit(&other, &["branch", "create", "blocked"])?;
    ensure!(!refused.status.success(), "shared write must refuse");
    for expected in ["another Brit mutation is active", &holder, "commit", "--wait"] {
        ensure!(
            refused.stderr.contains(expected),
            "refusal names {expected:?}: {}",
            refused.stderr
        );
    }
    assert_eq!(git(repo.path(), &["show-ref"])?, refs_before);

    // A bounded wait gives up after its budget, still naming the holder.
    let started = Instant::now();
    let timed_out = brit(&other, &["--wait=1", "branch", "create", "blocked"])?;
    ensure!(!timed_out.status.success(), "a bounded wait must give up");
    ensure!(started.elapsed() >= Duration::from_secs(1), "the wait was honoured");
    ensure!(
        timed_out.stderr.contains(&holder),
        "timed-out refusal names the holder: {}",
        timed_out.stderr
    );

    // An open-ended wait completes once the holder finishes.
    let waiting = Background::spawn(&other, &["--wait", "branch", "create", "awaited"], &[])?;
    std::thread::sleep(Duration::from_millis(500));
    fs::write(pause.join("release"), "")?;
    let held = held.finish()?;
    ensure!(held.status.success(), "released commit: {}", held.stderr);
    let waiting = waiting.finish()?;
    ensure!(waiting.status.success(), "awaited shared write: {}", waiting.stderr);
    ensure!(
        waiting.stderr.contains("waiting"),
        "a waiting writer says so: {}",
        waiting.stderr
    );
    assert_eq!(git(repo.path(), &["log", "-1", "--format=%s"])?, "held commit");
    git(repo.path(), &["rev-parse", "--verify", "refs/heads/awaited"])?;
    Ok(())
}

#[test]
fn wait_environment_default_is_validated() -> Result<()> {
    let repo = TestRepo::new("lock-wait-env")?;
    let output = BritInvocation::new(brit_bin().expect("required binary"))
        .args(["branch", "create", "x"])
        .env("BRIT_MUTATION_WAIT", "soon")
        .current_dir(repo.path())
        .run()?;
    ensure!(!output.status.success(), "an invalid wait default refuses");
    ensure!(
        output.stderr.contains("BRIT_MUTATION_WAIT"),
        "names the variable: {}",
        output.stderr
    );
    let output = BritInvocation::new(brit_bin().expect("required binary"))
        .args(["branch", "create", "x"])
        .env("BRIT_MUTATION_WAIT", "5")
        .current_dir(repo.path())
        .run()?;
    ensure!(output.status.success(), "{}", output.stderr);
    Ok(())
}
