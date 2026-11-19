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

    fn running(&mut self) -> Result<bool> {
        Ok(self.child.try_wait()?.is_none())
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

fn hex(path: &Path) -> String {
    use std::{fmt::Write as _, os::unix::ffi::OsStrExt};
    let path = path.canonicalize().expect("canonical path");
    path.as_os_str().as_bytes().iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn common_dir(worktree: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from(git(
        worktree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?))
}

fn git_dir(worktree: &Path) -> Result<PathBuf> {
    Ok(PathBuf::from(git(worktree, &["rev-parse", "--absolute-git-dir"])?))
}

fn start_ticks(pid: u32) -> Result<String> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat"))?;
    let after = &stat[stat.rfind(')').expect("stat has a command name") + 2..];
    Ok(after.split_whitespace().nth(19).expect("start time field").to_owned())
}

/// A writer that holds the repository guard shared and `worktree`'s guard until released:
/// a push paused in the remote's pre-receive hook, during send-pack.
struct SharedHolder {
    process: Background,
    pause: PathBuf,
}

impl SharedHolder {
    fn start(worktree: &Path, remote: &Path, scratch: &tempfile::TempDir, name: &str) -> Result<Self> {
        install_hook(
            remote,
            &remote.join("hooks"),
            "pre-receive",
            "cat >/dev/null\n\
             test -n \"$BRIT_TEST_REMOTE_PAUSE\" || exit 0\n\
             : >\"$BRIT_TEST_REMOTE_PAUSE/started\"\n\
             while ! test -e \"$BRIT_TEST_REMOTE_PAUSE/release\"; do sleep 0.05; done",
        )?;
        let pause = pause_dir(scratch, name)?;
        let spec = format!("HEAD:refs/heads/holder-{name}");
        let mut process = Background::spawn(
            worktree,
            &["push", "origin", &spec],
            &[("BRIT_TEST_REMOTE_PAUSE", &pause)],
        )?;
        await_file(&pause.join("started"), &mut process)?;
        Ok(Self { process, pause })
    }

    fn pid(&self) -> u32 {
        self.process.pid()
    }

    fn release(self) -> Result<()> {
        fs::write(self.pause.join("release"), "")?;
        let done = self.process.finish()?;
        ensure!(done.status.success(), "shared holder: {}", done.stderr);
        Ok(())
    }
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
fn a_commit_in_its_hooks_holds_only_its_worktree() -> Result<()> {
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
    // So do exclusive writers: the commit holds no repository guard while its hooks run.
    let branch = brit(&other, &["branch", "create", "beside"])?;
    ensure!(
        branch.status.success(),
        "exclusive write beside a paused commit: {}",
        branch.stderr
    );

    // The paused lane's own worktree stays exclusive, and the refusal names its holder.
    fs::write(repo.path().join("second"), "second\n")?;
    ensure_refusal_names(
        &brit(repo.path(), &["add", "second"])?,
        "same-worktree add",
        &["another Brit mutation is active", &holder, "commit"],
    )?;

    fs::write(pause.join("release"), "")?;
    let held = held.finish()?;
    ensure!(held.status.success(), "released commit: {}", held.stderr);
    assert_eq!(git(repo.path(), &["log", "-1", "--format=%s"])?, "held commit");
    Ok(())
}

#[test]
fn a_refused_exclusive_write_names_its_holder_and_can_be_awaited() -> Result<()> {
    let Lanes {
        repo,
        remote,
        other,
        scratch,
    } = lanes("lock-refusal-names")?;
    let holder = SharedHolder::start(repo.path(), remote.path(), &scratch, "names")?;
    let pid = holder.pid().to_string();

    // A shared write refuses at once by default, naming the holder.
    let refs_before = git(repo.path(), &["show-ref"])?;
    ensure_refusal_names(
        &brit(&other, &["branch", "create", "blocked"])?,
        "exclusive write",
        &["another Brit mutation is active", &pid, "push", "--wait"],
    )?;
    assert_eq!(git(repo.path(), &["show-ref"])?, refs_before);

    // A bounded wait gives up after one budget across all its locks, still naming the holder.
    let started = Instant::now();
    let timed_out = brit(&other, &["--wait=1", "branch", "create", "blocked"])?;
    ensure_refusal_names(&timed_out, "bounded wait", &[&pid, "after waiting"])?;
    ensure!(
        started.elapsed() >= Duration::from_secs(1),
        "the one-second budget was honoured"
    );

    // An open-ended wait completes once the holder finishes.
    let mut waiting = Background::spawn(&other, &["--wait", "branch", "create", "awaited"], &[])?;
    waiting.await_stderr("waiting for the Brit mutation guard")?;
    holder.release()?;
    let waiting = waiting.finish()?;
    ensure!(waiting.status.success(), "awaited write: {}", waiting.stderr);
    git(repo.path(), &["rev-parse", "--verify", "refs/heads/awaited"])?;
    Ok(())
}

#[test]
fn a_commit_publishes_under_the_repository_guard_after_its_hooks() -> Result<()> {
    let Lanes { repo, scratch, .. } = lanes("lock-commit-publish")?;
    let pause = pause_dir(&scratch, "pause")?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-commit")?;
    let head = repo.head_id()?;

    // An exclusive holder present when the hooks finish makes the commit wait, then publish.
    let mut commit = Background::spawn(
        repo.path(),
        &["commit", "--allow-empty", "-m", "published"],
        &[("BRIT_TEST_PAUSE", &pause)],
    )?;
    await_file(&pause.join("started"), &mut commit)?;
    let mut external = ExternalHolder::hold(repo.path())?;
    fs::write(pause.join("release"), "")?;
    commit.await_stderr("waiting for the Brit mutation guard")?;
    ensure!(external.running()?, "the exclusive holder is still there");
    assert_eq!(repo.head_id()?, head, "nothing is published while the guard is held");
    external.release()?;
    let commit = commit.finish()?;
    ensure!(
        commit.status.success(),
        "commit after the guard came back: {}",
        commit.stderr
    );
    assert_eq!(git(repo.path(), &["log", "-1", "--format=%s"])?, "published");

    // A pending recovery that appears during the hooks refuses publication; nothing changes.
    let head = repo.head_id()?;
    install_hook(
        repo.path(),
        &scratch.path().join("hooks"),
        "pre-commit",
        "mkdir \"$GIT_DIR/brit-switch\"",
    )?;
    let refused = brit(repo.path(), &["commit", "--allow-empty", "-m", "refused"])?;
    ensure_refusal_names(
        &refused,
        "publication after a recovery appeared",
        &["nothing was committed", "pending switch or pull"],
    )?;
    assert_eq!(repo.head_id()?, head, "HEAD is unchanged");
    fs::remove_dir(git_dir(repo.path())?.join("brit-switch"))?;
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
        refusal.contains("the command this hook runs under")
            && refusal.contains("commit")
            && refusal.contains("must write the index directly"),
        "the hook's writer names its own command and how to stage instead: {refusal}"
    );

    // A hook child whose environment was cleared (no marker) still refuses on direct re-entry.
    install_hook(
        repo.path(),
        &hooks,
        "pre-commit",
        "env -i PATH=\"$PATH\" HOME=\"$HOME\" \"$BRIT_TEST_BIN\" --wait add hookfile 2>\"$BRIT_TEST_OUT/cleared.err\"",
    )?;
    let commit = Background::spawn(
        repo.path(),
        &["commit", "--allow-empty", "-m", "cleared"],
        &[("BRIT_TEST_OUT", &out)],
    )?
    .finish_within(Duration::from_secs(30))?;
    ensure!(
        !commit.status.success(),
        "the cleared hook child's refusal fails the commit"
    );
    let refusal = fs::read_to_string(out.join("cleared.err"))?;
    ensure!(
        refusal.contains("the command this hook runs under"),
        "found through the parent chain without a marker: {refusal}"
    );
    fs::remove_file(hooks.join("pre-commit"))?;

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
fn a_waiting_exclusive_writer_is_not_starved_by_overlapping_shared_holds() -> Result<()> {
    let Lanes {
        repo,
        remote,
        other,
        scratch,
    } = lanes("lock-fairness")?;
    let holder = SharedHolder::start(repo.path(), remote.path(), &scratch, "first")?;
    // The exclusive writer queues behind the first shared holder.
    let mut fair = Background::spawn(&other, &["--wait=60", "branch", "create", "fair"], &[])?;
    fair.await_stderr("waiting for the Brit mutation guard")?;
    // A later shared writer arrives while the exclusive writer is queued. Its request is
    // compatible with the shared hold, so only the turnstile can make it wait.
    fs::write(other.join("later"), "later\n")?;
    let mut later = Background::spawn(&other, &["--wait=60", "add", "later"], &[])?;
    later.await_stderr("queued for the repository guard")?;
    ensure!(later.running()?, "the later writer queues behind the waiter");

    holder.release()?;
    let fair = fair.finish()?;
    ensure!(fair.status.success(), "queued exclusive writer: {}", fair.stderr);
    let later = later.finish()?;
    ensure!(later.status.success(), "later writer: {}", later.stderr);
    git(repo.path(), &["rev-parse", "--verify", "refs/heads/fair"])?;
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

    let mut holder = ExternalHolder::hold(repo.path())?;
    let started = Instant::now();
    let output = brit_with(repo.path(), &["branch", "create", "x"], &[("BRIT_MUTATION_WAIT", "1")])?;
    let elapsed = started.elapsed();
    ensure_refusal_names(
        &output,
        "contended wait from the environment",
        &["after waiting", "flock"],
    )?;
    ensure!(output.stderr.contains("brit: waiting"), "it waited: {}", output.stderr);
    ensure!(elapsed >= Duration::from_secs(1), "{elapsed:?}");
    ensure!(holder.running()?, "it gave up while the holder still held the guard");
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
    let index = repo.path().join(".git/index");
    // A rewritten file with the same bytes leaves stale stat data an index refresh updates.
    let make_stale = || -> Result<Vec<u8>> {
        std::thread::sleep(Duration::from_millis(1100));
        fs::write(repo.path().join("tracked"), "tracked\n")?;
        Ok(fs::read(&index)?)
    };
    // Control: uncontended, status does refresh the index.
    let before = make_stale()?;
    let status = brit(repo.path(), &["status"])?;
    ensure!(status.status.success(), "status: {}", status.stderr);
    ensure!(fs::read(&index)? != before, "an uncontended status refreshes the index");
    let before = make_stale()?;
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
    let holder = held.pid().to_string();
    // The commit in that worktree holds its guard; removal refuses at once, naming it.
    let refused = brit(
        repo.path(),
        &[
            "--wait=60",
            "worktree",
            "remove",
            "--force",
            other.to_str().expect("utf8"),
        ],
    )?;
    ensure_refusal_names(
        &refused,
        "removing a worktree in use",
        &["in use by another Brit writer", &holder, "commit"],
    )?;
    ensure!(other.exists(), "the worktree in use was kept");
    fs::write(pause.join("release"), "")?;
    let held = held.finish()?;
    ensure!(held.status.success(), "{}", held.stderr);
    let removed = brit(
        repo.path(),
        &["worktree", "remove", "--force", other.to_str().expect("utf8")],
    )?;
    ensure!(
        removed.status.success(),
        "removal after the writer finished: {}",
        removed.stderr
    );
    ensure!(!other.exists(), "the worktree was removed");
    Ok(())
}

#[test]
fn a_hook_refuses_at_once_when_its_command_holds_the_guard_it_needs() -> Result<()> {
    let Lanes {
        repo, other, scratch, ..
    } = lanes("lock-hook-exclusive")?;
    git(repo.path(), &["branch", "spare"])?;
    let pause = pause_dir(&scratch, "pause")?;
    let out = pause_dir(&scratch, "out")?;
    // post-checkout runs while switch holds the repository guard exclusively.
    install_hook(
        repo.path(),
        &scratch.path().join("hooks"),
        "post-checkout",
        "test -n \"$BRIT_TEST_PAUSE\" || exit 0\n\
         : >\"$BRIT_TEST_PAUSE/started\"\n\
         while ! test -e \"$BRIT_TEST_PAUSE/release\"; do sleep 0.05; done\n\
         \"$BRIT_TEST_BIN\" branch create from-hook 2>\"$BRIT_TEST_OUT/hook.err\"\n\
         echo $? >\"$BRIT_TEST_OUT/hook.status\"",
    )?;
    let mut switch = Background::spawn(
        repo.path(),
        &["switch", "spare"],
        &[
            ("BRIT_TEST_PAUSE", &pause),
            ("BRIT_TEST_OUT", &out),
            ("BRIT_MUTATION_WAIT", Path::new("forever")),
        ],
    )?;
    await_file(&pause.join("started"), &mut switch)?;
    // Another writer queues at the turnstile, waiting for the switch.
    let mut queued = Background::spawn(&other, &["--wait", "branch", "create", "queued-b"], &[])?;
    queued.await_stderr("waiting for the Brit mutation guard")?;
    fs::write(pause.join("release"), "")?;
    let switch = switch.finish_within(Duration::from_secs(30))?;
    ensure!(switch.status.success(), "switch: {}", switch.stderr);
    ensure!(
        fs::read_to_string(out.join("hook.status"))?.trim() != "0",
        "the hook's exclusive request refused"
    );
    let refusal = fs::read_to_string(out.join("hook.err"))?;
    ensure!(
        refusal.contains("the command this hook runs under") && refusal.contains("switch"),
        "names the switch: {refusal}"
    );
    let queued = queued.finish()?;
    ensure!(queued.status.success(), "the queued writer proceeds: {}", queued.stderr);
    Ok(())
}

#[test]
fn a_hook_wait_is_capped_and_no_cycle_forms_through_another_worktree() -> Result<()> {
    let Lanes {
        repo, other, scratch, ..
    } = lanes("lock-hook-cycle")?;
    git(&other, &["branch", "lane-x"])?;
    fs::write(other.join("f"), "f\n")?;
    let pause = pause_dir(&scratch, "pause")?;
    let out = pause_dir(&scratch, "out")?;
    let hooks = scratch.path().join("hooks");
    install_hook(
        repo.path(),
        &hooks,
        "pre-commit",
        &format!(
            "test -n \"$BRIT_TEST_PAUSE\" || exit 0\n\
             : >\"$BRIT_TEST_PAUSE/started\"\n\
             while ! test -e \"$BRIT_TEST_PAUSE/release\"; do sleep 0.05; done\n\
             cd \"{}\" && env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE \"$BRIT_TEST_BIN\" add f 2>\"$BRIT_TEST_OUT/add.err\"",
            other.display()
        ),
    )?;
    let envs = [
        ("BRIT_TEST_PAUSE", pause.as_path()),
        ("BRIT_TEST_OUT", out.as_path()),
        ("BRIT_MUTATION_WAIT", Path::new("forever")),
        ("BRIT_HOOK_GUARD_WAIT", Path::new("2")),
    ];

    // The second review's cycle: a commit in its hook, a switch in the other worktree that
    // needs the repository guard, and the hook writing into that other worktree. The commit
    // holds no repository guard during its hooks, so the switch completes meanwhile.
    let mut commit = Background::spawn(repo.path(), &["commit", "--allow-empty", "-m", "cycle"], &envs)?;
    await_file(&pause.join("started"), &mut commit)?;
    let switch = Background::spawn(&other, &["--wait", "switch", "lane-x"], &[])?.finish()?;
    ensure!(
        switch.status.success(),
        "the switch completes while the commit is in its hook: {}",
        switch.stderr
    );
    ensure!(commit.running()?, "the commit is still in its hook");
    fs::write(pause.join("release"), "")?;
    let commit = commit.finish_within(Duration::from_secs(30))?;
    ensure!(
        commit.status.success(),
        "the hook's write and the commit succeed: {}",
        commit.stderr
    );
    git(&other, &["diff", "--cached", "--quiet", "HEAD", "--", "f"]).expect_err("f was staged by the hook");

    // Any other wait a hook child makes is capped: a worktree guard held by an unrelated process.
    let head = repo.head_id()?;
    let pause = pause_dir(&scratch, "pause-2")?;
    let blocker_dir = git_dir(&other)?;
    let mut blocker = Command::new("flock")
        .arg(&blocker_dir)
        .args(["sh", "-c", "printf ready; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut ready = [0; 5];
    blocker.stdout.take().expect("piped").read_exact(&mut ready)?;
    let envs = [
        ("BRIT_TEST_PAUSE", pause.as_path()),
        ("BRIT_TEST_OUT", out.as_path()),
        ("BRIT_MUTATION_WAIT", Path::new("forever")),
        ("BRIT_HOOK_GUARD_WAIT", Path::new("2")),
    ];
    let mut commit = Background::spawn(repo.path(), &["commit", "--allow-empty", "-m", "capped"], &envs)?;
    await_file(&pause.join("started"), &mut commit)?;
    fs::write(pause.join("release"), "")?;
    let commit = commit.finish_within(Duration::from_secs(30))?;
    drop(blocker.stdin.take());
    blocker.wait()?;
    ensure!(!commit.status.success(), "the capped hook fails the commit");
    let refusal = fs::read_to_string(out.join("add.err"))?;
    ensure!(refusal.contains("capped at 2s"), "the cap is named: {refusal}");
    assert_eq!(repo.head_id()?, head, "nothing was committed");
    Ok(())
}

#[test]
fn forged_hook_markers_are_ignored() -> Result<()> {
    let Lanes {
        repo,
        remote,
        other,
        scratch,
    } = lanes("lock-forged-marker")?;
    let holder = SharedHolder::start(repo.path(), remote.path(), &scratch, "holder")?;
    let common = hex(&common_dir(repo.path())?);
    let worktree = hex(&git_dir(repo.path())?);

    // A marker naming the real holder, but with a wrong start time, does not make a waiter refuse.
    let wrong_start = format!("v1 {} 1 s:{common}:{worktree}", holder.pid());
    // A marker with the holder's real start time, but not this process's ancestor, neither.
    let not_ancestor = format!(
        "v1 {} {} s:{common}:{worktree}",
        holder.pid(),
        start_ticks(holder.pid())?
    );
    let mut waiters = Vec::new();
    for (name, marker) in [("wrong-start", &wrong_start), ("not-ancestor", &not_ancestor)] {
        fs::write(repo.path().join(name), name)?;
        let mut waiter = Background::spawn(
            repo.path(),
            &["--wait=60", "add", name],
            &[("BRIT_GUARD_HELD_BY", Path::new(marker))],
        )?;
        waiter.await_stderr("waiting for the Brit mutation guard")?;
        waiters.push(waiter);
    }

    // A forged marker claiming a shared ancestor does not skip an engaged turnstile.
    let mut queued = Background::spawn(&other, &["--wait", "branch", "create", "queued"], &[])?;
    queued.await_stderr("waiting for the Brit mutation guard")?;
    fs::write(other.join("forged"), "forged\n")?;
    let forged = format!("v1 1 0 s:{common}:-");
    let skipped = brit_with(&other, &["add", "forged"], &[("BRIT_GUARD_HELD_BY", &forged)])?;
    ensure_refusal_names(&skipped, "a forged marker at the turnstile", &["branch create"])?;

    holder.release()?;
    for waiter in waiters {
        let done = waiter.finish()?;
        ensure!(
            done.status.success(),
            "the waiter proceeds after the holder: {}",
            done.stderr
        );
    }
    let queued = queued.finish()?;
    ensure!(queued.status.success(), "{}", queued.stderr);
    Ok(())
}

#[test]
fn hook_markers_round_trip_through_a_real_hook() -> Result<()> {
    let Lanes { repo, scratch, .. } = lanes("lock-marker-shape")?;
    let out = pause_dir(&scratch, "out")?;
    install_hook(
        repo.path(),
        &scratch.path().join("hooks"),
        "pre-commit",
        "printf '%s' \"$BRIT_GUARD_HELD_BY\" >\"$BRIT_TEST_OUT/marker\"",
    )?;
    let commit = Background::spawn(
        repo.path(),
        &["commit", "--allow-empty", "-m", "m"],
        &[("BRIT_TEST_OUT", &out)],
    )?;
    let pid = commit.pid();
    let done = commit.finish()?;
    ensure!(done.status.success(), "{}", done.stderr);
    let marker = fs::read_to_string(out.join("marker"))?;
    let fields: Vec<_> = marker.split(' ').collect();
    assert_eq!(fields[0], "v1");
    assert_eq!(fields[1], pid.to_string(), "names the spawning brit");
    let worktree = format!(":{}", hex(&git_dir(repo.path())?));
    ensure!(
        fields[3..].iter().any(|holding| holding.ends_with(&worktree)),
        "names the worktree guard it holds: {marker}"
    );
    Ok(())
}

#[test]
fn push_send_pack_is_shared_and_a_queued_writer_neither_blocks_lanes_for_long_nor_hides_the_holder() -> Result<()> {
    let Lanes {
        repo,
        remote,
        other,
        scratch,
    } = lanes("lock-queued-writer")?;
    let holder = SharedHolder::start(repo.path(), remote.path(), &scratch, "send-pack")?;
    let push = holder.pid().to_string();

    // During send-pack other lanes commit; exclusive writers refuse naming the push.
    fs::write(other.join("lane-file"), "lane b\n")?;
    let add = brit(&other, &["add", "lane-file"])?;
    ensure!(add.status.success(), "add during send-pack: {}", add.stderr);
    let commit = brit(&other, &["commit", "-m", "during send-pack"])?;
    ensure!(commit.status.success(), "commit during send-pack: {}", commit.stderr);
    ensure_refusal_names(
        &brit(&other, &["branch", "create", "blocked"])?,
        "exclusive write during send-pack",
        &[push.as_str(), "push"],
    )?;

    // A queued exclusive writer gives non-waiting lanes a grace, then a refusal naming both.
    let mut queued = Background::spawn(&other, &["--wait", "branch", "create", "queued"], &[])?;
    queued.await_stderr("waiting for the Brit mutation guard")?;
    fs::write(other.join("second"), "second\n")?;
    let started = Instant::now();
    let refused = brit(&other, &["add", "second"])?;
    ensure_refusal_names(
        &refused,
        "a non-waiting add behind a queued writer",
        &[
            "branch create",
            "queued for the repository guard",
            push.as_str(),
            "push",
        ],
    )?;
    ensure!(
        started.elapsed() >= Duration::from_millis(900),
        "the turnstile gave a grace"
    );

    // A stopped queued writer does not hold the lanes.
    let signal = |name: &str| -> Result<()> {
        let status = Command::new("kill").args([name, &queued.pid().to_string()]).status()?;
        ensure!(status.success(), "kill {name}");
        Ok(())
    };
    signal("-STOP")?;
    let passed = brit(&other, &["add", "second"]);
    signal("-CONT")?;
    let passed = passed?;
    ensure!(passed.status.success(), "a stopped waiter is passed: {}", passed.stderr);

    holder.release()?;
    let queued = queued.finish()?;
    ensure!(queued.status.success(), "the queued writer proceeds: {}", queued.stderr);
    Ok(())
}

#[test]
fn add_and_commit_starts_wait_briefly_for_a_momentary_exclusive_writer() -> Result<()> {
    let repo = TestRepo::new("lock-start-patience")?;
    fs::write(repo.path().join("f"), "f\n")?;
    for args in [&["add", "f"][..], &["commit", "-m", "patient"]] {
        let holder = ExternalHolder::hold(repo.path())?;
        let mut writer = Background::spawn(repo.path(), args, &[])?;
        writer.await_stderr("waiting for the Brit mutation guard")?;
        holder.release()?;
        let writer = writer.finish()?;
        ensure!(
            writer.status.success(),
            "{args:?} after a momentary holder: {}",
            writer.stderr
        );
    }
    assert_eq!(git(repo.path(), &["log", "-1", "--format=%s"])?, "patient");
    // An explicit `--wait=0` still refuses at once.
    let holder = ExternalHolder::hold(repo.path())?;
    let refused = brit(repo.path(), &["--wait=0", "commit", "--allow-empty", "-m", "refused"])?;
    ensure_refusal_names(
        &refused,
        "commit with --wait=0",
        &["another Brit mutation is active", "flock"],
    )?;
    ensure!(
        !refused.stderr.contains("brit: waiting"),
        "it did not wait: {}",
        refused.stderr
    );
    holder.release()?;
    Ok(())
}

#[test]
fn a_branch_moved_during_the_hooks_refuses_publication() -> Result<()> {
    let repo = TestRepo::new("lock-branch-moved")?;
    let scratch = tempfile::tempdir()?;
    // A second worktree on the same branch, as `git worktree add --force` allows.
    let twin = scratch.path().join("twin");
    git(
        repo.path(),
        &["worktree", "add", "-q", "--force", twin.to_str().expect("utf8"), "main"],
    )?;
    pausing_hook(repo.path(), &scratch.path().join("hooks"), "pre-commit")?;
    fs::write(repo.path().join("mine"), "mine\n")?;
    git(repo.path(), &["add", "mine"])?;
    let index = git_dir(repo.path())?.join("index");

    let movers: [&dyn Fn() -> Result<()>; 2] = [
        // Another worktree's commit to the same branch.
        &|| {
            fs::write(twin.join("theirs"), "theirs\n")?;
            let add = brit(&twin, &["add", "theirs"])?;
            ensure!(add.status.success(), "{}", add.stderr);
            let commit = brit(&twin, &["commit", "-m", "theirs"])?;
            ensure!(
                commit.status.success(),
                "the other worktree publishes: {}",
                commit.stderr
            );
            Ok(())
        },
        // A fetch-like reference update.
        &|| {
            let tree = git(repo.path(), &["rev-parse", "HEAD^{tree}"])?;
            let parent = git(repo.path(), &["rev-parse", "HEAD"])?;
            let moved = git(repo.path(), &["commit-tree", &tree, "-p", &parent, "-m", "fetched"])?;
            git(repo.path(), &["update-ref", "refs/heads/main", &moved])?;
            Ok(())
        },
    ];
    for (round, mover) in movers.iter().enumerate() {
        let pause = pause_dir(&scratch, &format!("pause-{round}"))?;
        let mut commit = Background::spawn(repo.path(), &["commit", "-m", "mine"], &[("BRIT_TEST_PAUSE", &pause)])?;
        await_file(&pause.join("started"), &mut commit)?;
        mover()?;
        let moved_to = git(repo.path(), &["rev-parse", "refs/heads/main"])?;
        let index_before = fs::read(&index)?;
        fs::write(pause.join("release"), "")?;
        let commit = commit.finish()?;
        ensure_refusal_names(
            &commit,
            "a commit whose branch moved",
            &["HEAD changed while commit hooks ran"],
        )?;
        assert_eq!(
            git(repo.path(), &["rev-parse", "refs/heads/main"])?,
            moved_to,
            "nothing was published"
        );
        assert_eq!(fs::read(&index)?, index_before, "the index is as the hooks left it");
        assert_eq!(fs::read_to_string(repo.path().join("mine"))?, "mine\n");
    }
    Ok(())
}

#[test]
fn commit_hooks_stage_through_the_index_and_post_commit_hooks_may_write() -> Result<()> {
    let Lanes { repo, scratch, .. } = lanes("lock-hook-staging")?;
    let hooks = scratch.path().join("hooks");
    let out = pause_dir(&scratch, "out")?;
    fs::write(repo.path().join("hooked"), "hooked\n")?;
    fs::write(repo.path().join("after"), "after\n")?;
    install_hook(repo.path(), &hooks, "pre-commit", "git add hooked")?;
    install_hook(
        repo.path(),
        &hooks,
        "post-commit",
        "\"$BRIT_TEST_BIN\" add after 2>\"$BRIT_TEST_OUT/post.err\"\n\
         echo $? >\"$BRIT_TEST_OUT/post.status\"",
    )?;
    let commit = Background::spawn(
        repo.path(),
        &["commit", "-m", "staged by hook"],
        &[("BRIT_TEST_OUT", &out)],
    )?
    .finish_within(Duration::from_secs(30))?;
    ensure!(commit.status.success(), "{}", commit.stderr);
    ensure!(
        git(repo.path(), &["show", "--name-only", "--format=", "HEAD"])?
            .lines()
            .any(|name| name == "hooked"),
        "a pre-commit `git add` is committed"
    );
    // post-commit runs holding no guard, so a Brit writer in this worktree succeeds.
    assert_eq!(
        fs::read_to_string(out.join("post.status"))?.trim(),
        "0",
        "post-commit's brit add: {}",
        fs::read_to_string(out.join("post.err"))?
    );
    ensure!(
        git(repo.path(), &["diff", "--cached", "--name-only"])?
            .lines()
            .any(|name| name == "after"),
        "post-commit staged its file"
    );
    Ok(())
}
