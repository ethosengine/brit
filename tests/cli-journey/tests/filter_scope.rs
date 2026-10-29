//! External filter refusal is scoped per path. A configured driver alone never refuses:
//! brit refuses only when a path it converts resolves `filter=<configured driver>`, when an
//! attribute macro assigns `filter`, or when an attribute source cannot be parsed in full.
//! Everything else is compared with stock Git under the same global configuration.
#![cfg(target_os = "linux")]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

/// A global Git configuration shared by brit and the Git oracle.
struct Global {
    _dir: tempfile::TempDir,
    config: PathBuf,
    marker: PathBuf,
}

impl Global {
    /// The `filter.lfs.*` section `git lfs install` writes globally.
    fn lfs() -> Result<Self> {
        Self::with(
            "[filter \"lfs\"]\n\tclean = git-lfs clean -- %f\n\tsmudge = git-lfs smudge -- %f\n\
             \tprocess = git-lfs filter-process\n\trequired = true\n",
        )
    }

    /// An `lfs` driver that leaves a marker whenever it runs, so a refusal can prove it never did.
    fn witnessed_lfs() -> Result<Self> {
        Self::with("")
    }

    fn with(section: &str) -> Result<Self> {
        let dir = tempfile::tempdir()?;
        let config = dir.path().join("gitconfig");
        let marker = dir.path().join("filter-ran");
        let section = if section.is_empty() {
            format!(
                "[filter \"lfs\"]\n\tclean = \"touch '{0}'; cat\"\n\tsmudge = \"touch '{0}'; cat\"\n\trequired = true\n",
                marker.display()
            )
        } else {
            section.to_owned()
        };
        fs::write(&config, section)?;
        Ok(Self {
            _dir: dir,
            config,
            marker,
        })
    }

    fn command(&self, repo: &Path, native: bool) -> Command {
        let mut cmd = if native {
            let mut cmd = Command::new(brit_bin().expect("required Brit executable"));
            gix_testtools::configure_git_environment(&mut cmd, repo);
            cmd.current_dir(repo);
            cmd
        } else {
            gix_testtools::git_command(repo)
        };
        cmd.env("GIT_CONFIG_GLOBAL", &self.config)
            .env("GIT_COMMITTER_NAME", "Filter Journey")
            .env("GIT_COMMITTER_EMAIL", "filter@example.com")
            .env("GIT_COMMITTER_DATE", "2026-01-02 03:04:05 +0000")
            .env("GIT_EDITOR", ":")
            .env("GIT_SEQUENCE_EDITOR", ":");
        cmd
    }

    fn run(&self, repo: &Path, native: bool, args: &[&str]) -> Result<Output> {
        Ok(self.command(repo, native).args(args).output()?)
    }

    fn git(&self, repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
        let out = self.run(repo, false, args)?;
        ensure!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out.stdout)
    }

    fn brit(&self, repo: &Path, args: &[&str]) -> Result<Output> {
        let out = self.run(repo, true, args)?;
        ensure!(
            out.status.success(),
            "brit {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out)
    }

    /// Brit must refuse, leaving the index byte-identical and HEAD where it was, without
    /// running the driver.
    fn refused(&self, repo: &Path, args: &[&str]) -> Result<String> {
        let index = index(repo)?;
        let head = self.git(repo, &["rev-parse", "HEAD"])?;
        let out = self.run(repo, true, args)?;
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        ensure!(!out.status.success(), "brit {args:?} must refuse");
        ensure!(index == self::index(repo)?, "refusal of {args:?} changed the index");
        ensure!(
            head == self.git(repo, &["rev-parse", "HEAD"])?,
            "refusal of {args:?} moved HEAD"
        );
        ensure!(!self.marker.exists(), "the filter driver ran for {args:?}");
        Ok(stderr)
    }

    /// The observable result of an operation, comparable between brit and Git.
    fn state(&self, repo: &Path, files: &[&str]) -> Result<Vec<Vec<u8>>> {
        let mut state = vec![
            self.git(repo, &["rev-parse", "HEAD", "HEAD^{tree}"])?,
            self.git(repo, &["symbolic-ref", "-q", "HEAD"]).unwrap_or_default(),
            self.git(repo, &["ls-files", "--stage", "-z"])?,
            self.git(repo, &["status", "--porcelain=v1", "-z", "--untracked-files=all"])?,
        ];
        for file in files {
            state.push(fs::read(repo.join(file)).unwrap_or_else(|_| b"<absent>".to_vec()));
        }
        Ok(state)
    }
}

fn index(repo: &Path) -> Result<Vec<u8>> {
    Ok(fs::read(repo.join(".git/index"))?)
}

fn authoring_fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("tracked", "base\n")?;
    fs::write(repo.path().join("tracked"), "changed\n")?;
    fs::write(repo.path().join("data.bin"), b"\x00binary payload\x01\n")?;
    fs::write(repo.path().join("notes.txt"), "notes\n")?;
    Ok(repo)
}

#[test]
fn global_lfs_without_attributes_add_and_restore_match_git() -> Result<()> {
    let global = Global::lfs()?;
    let actual = authoring_fixture("lfs-add")?;
    let oracle = authoring_fixture("lfs-add-oracle")?;
    let files = ["tracked", "data.bin", "notes.txt"];
    let add = ["add", "--", "tracked", "data.bin", "notes.txt"];
    global.brit(actual.path(), &add)?;
    global.git(oracle.path(), &add)?;
    assert_eq!(
        global.state(actual.path(), &files)?,
        global.state(oracle.path(), &files)?
    );

    for repo in [&actual, &oracle] {
        fs::write(repo.path().join("tracked"), "dirty\n")?;
        fs::write(repo.path().join("data.bin"), "dirty binary\n")?;
    }
    let restore = ["restore", "--worktree", "--", "tracked", "data.bin"];
    global.brit(actual.path(), &restore)?;
    global.git(oracle.path(), &restore)?;
    assert_eq!(
        global.state(actual.path(), &files)?,
        global.state(oracle.path(), &files)?
    );
    Ok(())
}

fn switch_fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("selected", "original\n")?;
    repo.commit_file("neighbor", "neighbor original\n")?;
    let git = |args: &[&str]| -> Result<()> {
        let out = gix_testtools::git_command(repo.path()).args(args).output()?;
        ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        Ok(())
    };
    git(&["switch", "-c", "destination"])?;
    repo.commit_file("selected", "destination\n")?;
    repo.commit_file("added.bin", "added\n")?;
    git(&["switch", "main"])?;
    Ok(repo)
}

#[test]
fn global_lfs_without_attributes_switch_matches_git() -> Result<()> {
    let global = Global::lfs()?;
    let actual = switch_fixture("lfs-switch")?;
    let oracle = switch_fixture("lfs-switch-oracle")?;
    let files = ["selected", "neighbor", "added.bin"];
    global.brit(actual.path(), &["switch", "destination"])?;
    global.git(oracle.path(), &["switch", "destination"])?;
    assert_eq!(
        global.state(actual.path(), &files)?,
        global.state(oracle.path(), &files)?
    );
    Ok(())
}

#[test]
fn switch_ignores_a_filter_on_a_path_it_does_not_convert() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let actual = switch_fixture("filter-outside-switch")?;
    let oracle = switch_fixture("filter-outside-switch-oracle")?;
    for repo in [&actual, &oracle] {
        fs::write(repo.path().join(".git/info/attributes"), "neighbor filter=lfs\n")?;
    }
    let files = ["selected", "neighbor", "added.bin"];
    global.brit(actual.path(), &["switch", "destination"])?;
    ensure!(!global.marker.exists(), "no converted path names the driver");
    global.git(oracle.path(), &["switch", "destination"])?;
    assert_eq!(
        global.state(actual.path(), &files)?,
        global.state(oracle.path(), &files)?
    );
    Ok(())
}

#[test]
fn switch_refuses_when_a_changed_path_resolves_a_configured_filter() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let repo = switch_fixture("filter-inside-switch")?;
    fs::write(repo.path().join(".git/info/attributes"), "*.bin filter=lfs\n")?;
    let stderr = global.refused(repo.path(), &["switch", "destination"])?;
    ensure!(stderr.contains("added.bin"), "refusal names the path: {stderr}");
    assert!(!repo.path().join("added.bin").exists());
    assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
    // The refusal leaves no operation behind: without the attribute the same switch proceeds.
    fs::remove_file(repo.path().join(".git/info/attributes"))?;
    global.brit(repo.path(), &["switch", "destination"])?;
    assert_eq!(fs::read(repo.path().join("added.bin"))?, b"added\n");
    Ok(())
}

struct Remote {
    remote: TestRepo,
    actual: TestRepo,
    oracle: TestRepo,
}

fn pull_fixture(label: &str) -> Result<Remote> {
    let remote = TestRepo::new(&format!("{label}-remote"))?;
    remote.commit_file("selected", "original\n")?;
    remote.commit_file("neighbor", "original neighbor\n")?;
    let mut locals = Vec::new();
    for side in ["actual", "oracle"] {
        let local = TestRepo::new(&format!("{label}-{side}"))?;
        let git = |args: &[&str]| -> Result<()> {
            let out = gix_testtools::git_command(local.path()).args(args).output()?;
            ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            Ok(())
        };
        git(&[
            "remote",
            "add",
            "origin",
            remote.path().to_str().expect("UTF-8 fixture"),
        ])?;
        git(&["fetch", "origin"])?;
        git(&["reset", "--hard", "origin/main"])?;
        locals.push(local);
    }
    remote.commit_file("selected", "destination\n")?;
    remote.commit_file("added.bin", "added\n")?;
    let oracle = locals.pop().expect("two locals");
    let actual = locals.pop().expect("two locals");
    Ok(Remote { remote, actual, oracle })
}

#[test]
fn global_lfs_without_attributes_pull_ff_only_matches_git() -> Result<()> {
    let global = Global::lfs()?;
    let f = pull_fixture("lfs-pull")?;
    let files = ["selected", "neighbor", "added.bin"];
    let pull = ["pull", "--ff-only", "origin", "main"];
    global.brit(f.actual.path(), &pull)?;
    global.git(f.oracle.path(), &pull)?;
    assert_eq!(
        global.state(f.actual.path(), &files)?,
        global.state(f.oracle.path(), &files)?
    );
    assert_eq!(global.git(f.actual.path(), &["rev-parse", "HEAD"])?, {
        let mut head = f.remote.head_id()?.into_bytes();
        head.push(b'\n');
        head
    });
    Ok(())
}

#[test]
fn pull_refuses_when_an_incoming_path_resolves_a_configured_filter() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let f = pull_fixture("filter-pull")?;
    fs::write(f.actual.path().join(".git/info/attributes"), "*.bin filter=lfs\n")?;
    let stderr = global.refused(f.actual.path(), &["pull", "--ff-only", "origin", "main"])?;
    ensure!(stderr.contains("added.bin"), "refusal names the path: {stderr}");
    assert!(!f.actual.path().join("added.bin").exists());
    assert_eq!(fs::read(f.actual.path().join("selected"))?, b"original\n");
    fs::remove_file(f.actual.path().join(".git/info/attributes"))?;
    global.brit(f.actual.path(), &["pull", "--ff-only", "origin", "main"])?;
    assert_eq!(fs::read(f.actual.path().join("added.bin"))?, b"added\n");
    Ok(())
}

#[test]
fn add_refuses_only_the_paths_that_resolve_a_configured_filter() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let actual = authoring_fixture("filter-add")?;
    let oracle = authoring_fixture("filter-add-oracle")?;
    for repo in [&actual, &oracle] {
        fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs\n")?;
    }
    let stderr = global.refused(actual.path(), &["add", "--", "data.bin"])?;
    ensure!(
        stderr.contains("data.bin") && stderr.contains("lfs"),
        "refusal names the path and driver: {stderr}"
    );
    global.refused(actual.path(), &["add", "--", "notes.txt", "data.bin"])?;

    let files = ["tracked", "notes.txt", ".gitattributes"];
    let add = ["add", "--", "tracked", "notes.txt", ".gitattributes"];
    global.brit(actual.path(), &add)?;
    ensure!(!global.marker.exists(), "no added path names the driver");
    global.git(oracle.path(), &add)?;
    assert_eq!(
        global.state(actual.path(), &files)?,
        global.state(oracle.path(), &files)?
    );
    Ok(())
}

#[test]
fn an_attribute_macro_that_assigns_filter_refuses() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let applied = authoring_fixture("filter-macro")?;
    fs::write(
        applied.path().join(".gitattributes"),
        "[attr]large filter=lfs -text\n*.bin large\n",
    )?;
    global.refused(applied.path(), &["add", "--", "data.bin"])?;

    // A macro that could assign a driver is in play even where no path uses it.
    let defined = authoring_fixture("filter-macro-defined")?;
    fs::write(defined.path().join(".gitattributes"), "[attr]large filter=lfs\n")?;
    let stderr = global.refused(defined.path(), &["add", "--", "notes.txt"])?;
    ensure!(stderr.contains("macro"), "refusal names the macro: {stderr}");
    Ok(())
}

#[test]
fn an_attribute_source_with_rejected_lines_fails_closed() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let repo = authoring_fixture("filter-unparseable")?;
    // gix drops a whole line for one invalid attribute name; what it dropped is unknown.
    fs::write(repo.path().join(".gitattributes"), "*.bin filter=lfs bad/name\n")?;
    let stderr = global.refused(repo.path(), &["add", "--", "notes.txt"])?;
    ensure!(stderr.contains(".gitattributes"), "refusal names the source: {stderr}");

    // Without a configured driver no attribute can run a filter, and nothing refuses.
    let unconfigured = Global::with("[core]\n\tbare = false\n")?;
    unconfigured.brit(repo.path(), &["add", "--", "notes.txt"])?;
    Ok(())
}

#[test]
fn a_committed_attribute_source_with_rejected_lines_fails_closed_on_switch() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let repo = TestRepo::new("filter-committed-unparseable")?;
    repo.commit_file("selected", "original\n")?;
    let git = |args: &[&str]| -> Result<()> {
        let out = gix_testtools::git_command(repo.path()).args(args).output()?;
        ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        Ok(())
    };
    git(&["switch", "-c", "destination"])?;
    // Read from the destination tree, not the worktree: the dropped line's assignments are unknown.
    repo.commit_file(".gitattributes", "*.bin filter=lfs bad/name\n")?;
    repo.commit_file("selected", "destination\n")?;
    git(&["switch", "main"])?;
    let stderr = global.refused(repo.path(), &["switch", "destination"])?;
    ensure!(stderr.contains(".gitattributes"), "refusal names the source: {stderr}");
    assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
    Ok(())
}

fn replay_fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("selected", "base\n")?;
    repo.commit_file("neighbor", "base neighbor\n")?;
    let out = gix_testtools::git_command(repo.path())
        .args(["switch", "-c", "upstream"])
        .output()?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    repo.commit_file("upstream.bin", "upstream\n")?;
    let out = gix_testtools::git_command(repo.path())
        .args(["switch", "main"])
        .output()?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    repo.commit_file("selected", "topic\n")?;
    repo.commit_file("second", "second\n")?;
    Ok(repo)
}

#[test]
fn global_lfs_without_attributes_replay_matches_git() -> Result<()> {
    let global = Global::lfs()?;
    let actual = replay_fixture("lfs-replay")?;
    let oracle = replay_fixture("lfs-replay-oracle")?;
    global.brit(actual.path(), &["rebase", "upstream"])?;
    global.git(oracle.path(), &["rebase", "upstream"])?;
    for args in [
        &["rev-parse", "HEAD^{tree}", "HEAD~2", "upstream"][..],
        &["ls-files", "--stage", "-z"][..],
        &["status", "--porcelain=v1", "-z"][..],
    ] {
        assert_eq!(
            global.git(actual.path(), args)?,
            global.git(oracle.path(), args)?,
            "{args:?}"
        );
    }
    Ok(())
}

#[test]
fn replay_refuses_when_a_changed_path_resolves_a_configured_filter() -> Result<()> {
    let global = Global::witnessed_lfs()?;
    let repo = replay_fixture("filter-replay")?;
    fs::write(repo.path().join(".git/info/attributes"), "*.bin filter=lfs\n")?;
    let stderr = global.refused(repo.path(), &["rebase", "upstream"])?;
    ensure!(stderr.contains("upstream.bin"), "refusal names the path: {stderr}");
    assert!(!repo.path().join("upstream.bin").exists());
    assert_eq!(fs::read(repo.path().join("selected"))?, b"topic\n");
    fs::remove_file(repo.path().join(".git/info/attributes"))?;
    global.brit(repo.path(), &["rebase", "upstream"])?;
    assert_eq!(fs::read(repo.path().join("upstream.bin"))?, b"upstream\n");
    Ok(())
}
