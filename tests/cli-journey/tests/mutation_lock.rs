//! Brit's writer guards: a paused hook in one worktree leaves other lanes writable,
//! a refused writer names its holder or waits for it, a hook never waits for its own
//! command, and a waiting shared writer is not starved by overlapping worktree writers.
#![cfg(target_os = "linux")]
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
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

fn brit_with(repo: &Path, args: &[&str], envs: &[(&str, &str)]) -> Result<Capture> {
    let mut invocation = BritInvocation::new(brit_bin().expect("required binary"))
        .args(args.iter().copied())
        .current_dir(repo);
    for (key, value) in envs {
        invocation = invocation.env(key, value);
    }
    invocation.run()
}

const PATIENCE: Duration = Duration::from_secs(60);

/// A Brit process left running in the background, killed if the test fails early.
/// Its stderr goes to a file, so a test can watch for a line while it runs.
struct Background {
    child: Child,
    stderr: PathBuf,
    _scratch: tempfile::TempDir,
}

impl Background {
    fn spawn(repo: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Result<Self> {
        let scratch = tempfile::tempdir()?;
        let stderr = scratch.path().join("stderr");
        let mut command = Command::new(brit_bin().expect("required binary"));
        gix_testtools::configure_git_environment(&mut command, repo);
        command
            .env("GIT_CONFIG_GLOBAL", scratch.path().join("missing.gitconfig"))
            .env("BRIT_TEST_BIN", brit_bin().expect("required binary"))
            .args(args)
            .current_dir(repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(&stderr)?);
        for (key, value) in envs {
            command.env(key, value);
        }
        Ok(Self {
            child: command.spawn()?,
            stderr,
            _scratch: scratch,
        })
    }

    fn pid(&self) -> u32 {
        self.child.id()
    }

    fn running(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_none())
    }

    fn stderr(&self) -> Result<String> {
        Ok(fs::read_to_string(&self.stderr)?)
    }

    /// Block until the process has written `text` to stderr.
    fn await_stderr(&mut self, text: &str) -> Result<()> {
        let deadline = Instant::now() + PATIENCE;
        while !self.stderr()?.contains(text) {
            if !self.running()? {
                bail!("process exited before printing {text:?}: {}", self.stderr()?);
            }
            ensure!(Instant::now() < deadline, "timed out waiting for {text:?}");
            std::thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }

    /// Wait for the process to exit, failing (and killing it) if it takes longer than `limit`.
    fn finish_within(mut self, limit: Duration) -> Result<Capture> {
        let deadline = Instant::now() + limit;
        while self.running()? {
            ensure!(
                Instant::now() < deadline,
                "process did not finish within {limit:?}: {}",
                self.stderr()?
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut stdout = String::new();
        self.child.stdout.take().expect("piped").read_to_string(&mut stdout)?;
        let status = self.child.wait()?;
        Ok(Capture {
            stdout,
            stderr: self.stderr()?,
            status,
        })
    }

    fn finish(self) -> Result<Capture> {
        self.finish_within(PATIENCE)
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

/// Hold the repository guard with flock(1) until dropped.
struct ExternalHolder {
    child: Child,
    stdin: Option<ChildStdin>,
}

impl ExternalHolder {
    fn hold(repo: &Path) -> Result<Self> {
        let guard = PathBuf::from(git(repo, &["rev-parse", "--path-format=absolute", "--git-common-dir"])?)
            .join("brit-mutation.guard");
        let mut command = Command::new("flock");
        gix_testtools::configure_git_environment(&mut command, repo);
        let mut child = command
            .arg(&guard)
            .args(["sh", "-c", "printf ready; cat"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = [0; 5];
        child.stdout.take().expect("piped").read_exact(&mut ready)?;
        ensure!(&ready == b"ready", "flock holds the guard");
        let stdin = child.stdin.take();
        Ok(Self { child, stdin })
    }

    fn release(mut self) -> Result<()> {
        drop(self.stdin.take());
        ensure!(self.child.wait()?.success(), "flock holder exits cleanly");
        Ok(())
    }
}

impl Drop for ExternalHolder {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.wait();
    }
}

fn install_hook(repo: &Path, hooks: &Path, name: &str, body: &str) -> Result<()> {
    fs::create_dir_all(hooks)?;
    git(
        repo,
        &["config", "core.hooksPath", hooks.to_str().expect("utf8 temp path")],
    )?;
    let path = hooks.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n"))?;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))?;
    Ok(())
}

/// A hook that pauses only for a process given `BRIT_TEST_PAUSE`: it records whether
/// `refs/heads/fair` exists, marks `started`, then waits for `release` in that directory.
const PAUSING_HOOK: &str = "cat >/dev/null\n\
     test -n \"$BRIT_TEST_PAUSE\" || exit 0\n\
     git rev-parse -q --verify refs/heads/fair >\"$BRIT_TEST_PAUSE/fair-seen\" || :\n\
     : >\"$BRIT_TEST_PAUSE/started\"\n\
     while ! test -e \"$BRIT_TEST_PAUSE/release\"; do sleep 0.05; done";

fn pausing_hook(repo: &Path, hooks: &Path, name: &str) -> Result<()> {
    install_hook(repo, hooks, name, PAUSING_HOOK)
}

fn await_file(path: &Path, process: &mut Background) -> Result<()> {
    let deadline = Instant::now() + PATIENCE;
    while !path.exists() {
        if !process.running()? {
            bail!("process exited before {}: {}", path.display(), process.stderr()?);
        }
        ensure!(Instant::now() < deadline, "timed out waiting for {}", path.display());
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

fn ensure_refusal_names(output: &Capture, what: &str, expected: &[&str]) -> Result<()> {
    ensure!(!output.status.success(), "{what} must refuse");
    for text in expected {
        ensure!(
            output.stderr.contains(text),
            "{what}: refusal names {text:?}: {}",
            output.stderr
        );
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

fn pause_dir(scratch: &tempfile::TempDir, name: &str) -> Result<PathBuf> {
    let pause = scratch.path().join(name);
    fs::create_dir(&pause)?;
    Ok(pause)
}

#[test]
fn push_paused_in_pre_push_hook_holds_its_worktree_and_leaves_others_writable() -> Result<()> {
    let Lanes {
        repo,
        remote,
        other,
        scratch,
    } = lanes("lock-push-hook")?;
    let pause = pause_dir(&scratch, "pause")?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-push")?;
    git(repo.path(), &["branch", "spare"])?;
    let pushed = repo.commit_file("payload", "pushed\n")?;

    let mut push = Background::spawn(
        repo.path(),
        &["push", "origin", "main:main"],
        &[("BRIT_TEST_PAUSE", &pause)],
    )?;
    await_file(&pause.join("started"), &mut push)?;
    let holder = push.pid().to_string();

    // Another worktree's lane stays writable, including shared writes.
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

    // The pushing worktree cannot change under its gate.
    let head = repo.head_id()?;
    fs::write(repo.path().join("late"), "late\n")?;
    let expected = ["another Brit mutation is active", holder.as_str(), "push"];
    ensure_refusal_names(
        &brit(repo.path(), &["add", "late"])?,
        "add in the pushing worktree",
        &expected,
    )?;
    ensure_refusal_names(
        &brit(repo.path(), &["commit", "--allow-empty", "-m", "late"])?,
        "commit in the pushing worktree",
        &expected,
    )?;
    ensure_refusal_names(
        &brit(repo.path(), &["switch", "spare"])?,
        "switch in the pushing worktree",
        &expected,
    )?;
    assert_eq!(repo.head_id()?, head, "the pushing worktree is unchanged");
    assert_eq!(git(repo.path(), &["symbolic-ref", "HEAD"])?, "refs/heads/main");
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
fn push_waits_for_the_repository_guard_after_its_hook_before_sending() -> Result<()> {
    let Lanes {
        repo, remote, scratch, ..
    } = lanes("lock-push-reacquire")?;
    let pause = pause_dir(&scratch, "pause")?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-push")?;
    let pushed = repo.commit_file("payload", "pushed\n")?;
    let mut push = Background::spawn(
        repo.path(),
        &["push", "origin", "main:main"],
        &[("BRIT_TEST_PAUSE", &pause)],
    )?;
    await_file(&pause.join("started"), &mut push)?;
    // The hook runs without the repository guard, so another writer can take it now.
    let holder = ExternalHolder::hold(repo.path())?;
    fs::write(pause.join("release"), "")?;
    push.await_stderr("waiting for the Brit mutation guard")?;
    ensure!(push.running()?, "push waits for the guard after its hook");
    ensure!(
        git(
            remote.path(),
            &["for-each-ref", "--format=%(refname)", "refs/heads/main"]
        )?
        .is_empty(),
        "nothing is sent while another writer holds the guard"
    );
    holder.release()?;
    let push = push.finish()?;
    ensure!(push.status.success(), "push after the guard came back: {}", push.stderr);
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/main"])?, pushed);
    Ok(())
}

#[test]
fn paused_commit_hook_keeps_lanes_writable_names_its_holder_and_can_be_awaited() -> Result<()> {
    let Lanes {
        repo, other, scratch, ..
    } = lanes("lock-commit-hook")?;
    let pause = pause_dir(&scratch, "pause")?;
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
    ensure_refusal_names(
        &brit(repo.path(), &["add", "second"])?,
        "same-worktree add",
        &["another Brit mutation is active", &holder, "commit"],
    )?;

    // A shared write refuses at once by default, naming the holder.
    let refs_before = git(repo.path(), &["show-ref"])?;
    ensure_refusal_names(
        &brit(&other, &["branch", "create", "blocked"])?,
        "shared write",
        &["another Brit mutation is active", &holder, "commit", "--wait"],
    )?;
    assert_eq!(git(repo.path(), &["show-ref"])?, refs_before);

    // A bounded wait gives up after one budget across all its locks, still naming the holder.
    let started = Instant::now();
    let timed_out = brit(&other, &["--wait=1", "branch", "create", "blocked"])?;
    ensure_refusal_names(&timed_out, "bounded wait", &[&holder, "after waiting"])?;
    let elapsed = started.elapsed();
    ensure!(
        elapsed >= Duration::from_secs(1) && elapsed < Duration::from_secs(4),
        "the one-second budget was honoured: {elapsed:?}"
    );

    // An open-ended wait completes once the holder finishes.
    let mut waiting = Background::spawn(&other, &["--wait", "branch", "create", "awaited"], &[])?;
    waiting.await_stderr("waiting for the Brit mutation guard")?;
    fs::write(pause.join("release"), "")?;
    let held = held.finish()?;
    ensure!(held.status.success(), "released commit: {}", held.stderr);
    let waiting = waiting.finish()?;
    ensure!(waiting.status.success(), "awaited shared write: {}", waiting.stderr);
    assert_eq!(git(repo.path(), &["log", "-1", "--format=%s"])?, "held commit");
    git(repo.path(), &["rev-parse", "--verify", "refs/heads/awaited"])?;
    Ok(())
}

#[test]
fn a_hook_never_waits_for_the_command_it_runs_under() -> Result<()> {
    let Lanes {
        repo, remote, scratch, ..
    } = lanes("lock-hook-reentry")?;
    let hooks = scratch.path().join("hooks");
    let out = pause_dir(&scratch, "out")?;
    fs::write(repo.path().join("hookfile"), "hook\n")?;

    // pre-commit: the commit holds this worktree; the hook's own add must refuse at once.
    install_hook(
        repo.path(),
        &hooks,
        "pre-commit",
        "\"$BRIT_TEST_BIN\" add hookfile 2>\"$BRIT_TEST_OUT/commit-add.err\"",
    )?;
    let commit = Background::spawn(
        repo.path(),
        &["commit", "--allow-empty", "-m", "hooked"],
        &[("BRIT_TEST_OUT", &out), ("BRIT_MUTATION_WAIT", Path::new("forever"))],
    )?
    .finish_within(Duration::from_secs(30))?;
    ensure!(!commit.status.success(), "the hook's refusal fails the commit");
    let refusal = fs::read_to_string(out.join("commit-add.err"))?;
    ensure!(
        refusal.contains("the command this hook runs under") && refusal.contains("commit"),
        "the hook's writer names its own command: {refusal}"
    );

    // pre-push: shared writes work during the hook, the pushing worktree does not.
    install_hook(
        repo.path(),
        &hooks,
        "pre-push",
        "cat >/dev/null\n\
         \"$BRIT_TEST_BIN\" branch create from-hook || exit 3\n\
         \"$BRIT_TEST_BIN\" add hookfile 2>\"$BRIT_TEST_OUT/push-add.err\"\n\
         echo $? >\"$BRIT_TEST_OUT/push-add.status\"",
    )?;
    let push = Background::spawn(
        repo.path(),
        &["push", "origin", "main:main"],
        &[("BRIT_TEST_OUT", &out), ("BRIT_MUTATION_WAIT", Path::new("forever"))],
    )?
    .finish_within(Duration::from_secs(30))?;
    ensure!(push.status.success(), "push with a passing hook: {}", push.stderr);
    ensure!(
        fs::read_to_string(out.join("push-add.status"))?.trim() != "0",
        "the hook's add in the pushing worktree refused"
    );
    let refusal = fs::read_to_string(out.join("push-add.err"))?;
    ensure!(
        refusal.contains("the command this hook runs under") && refusal.contains("push"),
        "the hook's writer names the push: {refusal}"
    );
    git(repo.path(), &["rev-parse", "--verify", "refs/heads/from-hook"])?;
    assert_eq!(git(remote.path(), &["rev-parse", "refs/heads/main"])?, repo.head_id()?);
    Ok(())
}

#[test]
fn a_waiting_shared_writer_is_not_starved_by_overlapping_worktree_writers() -> Result<()> {
    let Lanes {
        repo, other, scratch, ..
    } = lanes("lock-fairness")?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-commit")?;
    let first = pause_dir(&scratch, "first")?;
    let second = pause_dir(&scratch, "second")?;

    let mut a = Background::spawn(
        repo.path(),
        &["commit", "--allow-empty", "-m", "a"],
        &[("BRIT_TEST_PAUSE", &first)],
    )?;
    await_file(&first.join("started"), &mut a)?;
    // The exclusive writer queues behind the first shared holder.
    let mut fair = Background::spawn(&other, &["--wait=60", "branch", "create", "fair"], &[])?;
    fair.await_stderr("waiting for the Brit mutation guard")?;
    // A later shared writer arrives while the exclusive writer is queued.
    let mut b = Background::spawn(
        &other,
        &["--wait=60", "commit", "--allow-empty", "-m", "b"],
        &[("BRIT_TEST_PAUSE", &second)],
    )?;
    b.await_stderr("waiting for the Brit mutation guard")?;
    ensure!(
        !second.join("started").exists(),
        "the later writer queues behind the waiter"
    );

    fs::write(first.join("release"), "")?;
    let a = a.finish()?;
    ensure!(a.status.success(), "first writer: {}", a.stderr);
    let fair = fair.finish()?;
    ensure!(fair.status.success(), "queued exclusive writer: {}", fair.stderr);
    await_file(&second.join("started"), &mut b)?;
    ensure!(
        !fs::read_to_string(second.join("fair-seen"))?.trim().is_empty(),
        "the later shared writer ran only after the queued exclusive writer"
    );
    fs::write(second.join("release"), "")?;
    let b = b.finish()?;
    ensure!(b.status.success(), "later writer: {}", b.stderr);
    Ok(())
}

#[test]
fn wait_environment_default_is_validated_and_honoured_under_contention() -> Result<()> {
    let repo = TestRepo::new("lock-wait-env")?;
    let output = brit_with(
        repo.path(),
        &["branch", "create", "x"],
        &[("BRIT_MUTATION_WAIT", "soon")],
    )?;
    ensure!(!output.status.success(), "an invalid wait default refuses");
    ensure!(
        output.stderr.contains("BRIT_MUTATION_WAIT"),
        "names the variable: {}",
        output.stderr
    );

    let holder = ExternalHolder::hold(repo.path())?;
    let started = Instant::now();
    let output = brit_with(repo.path(), &["branch", "create", "x"], &[("BRIT_MUTATION_WAIT", "1")])?;
    let elapsed = started.elapsed();
    ensure_refusal_names(
        &output,
        "contended wait from the environment",
        &["after waiting", "flock"],
    )?;
    ensure!(output.stderr.contains("brit: waiting"), "it waited: {}", output.stderr);
    ensure!(
        elapsed >= Duration::from_secs(1) && elapsed < Duration::from_secs(4),
        "{elapsed:?}"
    );
    ensure!(
        !output.stderr.contains("ready; cat"),
        "only the holder's program name is shown: {}",
        output.stderr
    );
    holder.release()?;

    let output = brit_with(repo.path(), &["branch", "create", "x"], &[("BRIT_MUTATION_WAIT", "5")])?;
    ensure!(output.status.success(), "{}", output.stderr);
    Ok(())
}

#[test]
fn status_stays_read_only_when_its_refresh_guard_is_contended() -> Result<()> {
    let repo = TestRepo::new("lock-status-contended")?;
    repo.commit_file("tracked", "tracked\n")?;
    // A rewritten file with the same bytes leaves stale stat data an index refresh would update.
    std::thread::sleep(Duration::from_millis(1100));
    fs::write(repo.path().join("tracked"), "tracked\n")?;
    let index = repo.path().join(".git/index");
    let before = fs::read(&index)?;
    let holder = ExternalHolder::hold(repo.path())?;
    let status = brit(repo.path(), &["status"])?;
    ensure!(status.status.success(), "status under contention: {}", status.stderr);
    assert_eq!(fs::read(&index)?, before, "the index is untouched under contention");
    holder.release()?;
    Ok(())
}

#[test]
fn native_namespaces_take_wait_only_before_their_passed_through_arguments() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let repo = TestRepo::new("lock-context-wait")?;
    let bin = tempfile::tempdir()?;
    let record = bin.path().join("epr-args");
    let epr = bin.path().join("epr");
    fs::write(
        &epr,
        format!("#!/bin/sh\nprintf '%s\\n' \"$@\" >\"{}\"\n", record.display()),
    )?;
    fs::set_permissions(&epr, fs::Permissions::from_mode(0o755))?;
    let path = format!("{}:{}", bin.path().display(), std::env::var("PATH").unwrap_or_default());
    for (args, expected) in [
        (&["--wait=1", "context", "probe"][..], "flow\ncontext\nprobe\n"),
        (
            &["context", "probe", "--wait=1"][..],
            "flow\ncontext\nprobe\n--wait=1\n",
        ),
    ] {
        let output = brit_with(repo.path(), args, &[("PATH", &path)])?;
        ensure!(output.status.success(), "{args:?}: {}", output.stderr);
        assert_eq!(fs::read_to_string(&record)?, expected, "{args:?}");
    }
    Ok(())
}

#[test]
fn removing_a_worktree_that_another_writer_holds_refuses() -> Result<()> {
    let Lanes {
        repo, other, scratch, ..
    } = lanes("lock-worktree-remove")?;
    let pause = pause_dir(&scratch, "pause")?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-commit")?;
    let mut held = Background::spawn(
        &other,
        &["commit", "--allow-empty", "-m", "held"],
        &[("BRIT_TEST_PAUSE", &pause)],
    )?;
    await_file(&pause.join("started"), &mut held)?;
    // The repository guard is shared by the paused commit; wait for it, then refuse on the worktree.
    let mut remove = Background::spawn(
        repo.path(),
        &[
            "--wait=60",
            "worktree",
            "remove",
            "--force",
            other.to_str().expect("utf8"),
        ],
        &[],
    )?;
    remove.await_stderr("waiting for the Brit mutation guard")?;
    fs::write(pause.join("release"), "")?;
    let held = held.finish()?;
    ensure!(held.status.success(), "{}", held.stderr);
    let remove = remove.finish()?;
    ensure!(
        remove.status.success(),
        "removal after the writer finished: {}",
        remove.stderr
    );
    ensure!(!other.exists(), "the worktree was removed");

    // A worktree whose own writer holds it while the repository guard is free refuses at once.
    let again = scratch.path().join("again");
    git(
        repo.path(),
        &["worktree", "add", "-q", "-b", "again", again.to_str().expect("utf8")],
    )?;
    let dir_guard = PathBuf::from(git(&again, &["rev-parse", "--absolute-git-dir"])?);
    let mut command = Command::new("flock");
    let mut holder = command
        .arg(&dir_guard)
        .args(["sh", "-c", "printf ready; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut ready = [0; 5];
    holder.stdout.take().expect("piped").read_exact(&mut ready)?;
    let refused = brit(
        repo.path(),
        &["worktree", "remove", "--force", again.to_str().expect("utf8")],
    )?;
    ensure_refusal_names(
        &refused,
        "removing a held worktree",
        &["in use by another Brit writer", "flock"],
    )?;
    ensure!(again.exists(), "the held worktree was kept");
    drop(holder.stdin.take());
    holder.wait()?;
    Ok(())
}
