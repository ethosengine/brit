//! Compare native fast-forwards with Git, including index-only submodule pin moves.
#![cfg(target_os = "linux")]

use std::{fs, path::Path, process::Output};

use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use gix_testtools::TestResult;

fn git(path: &Path, args: &[&str]) -> TestResult<Vec<u8>> {
    let out = gix_testtools::git_command(path).args(args).output()?;
    assert!(out.status.success(), "Git {args:?}: {}", stderr(&out));
    Ok(out.stdout)
}

fn brit_command(path: &Path, args: &[&str]) -> std::process::Command {
    let mut command = std::process::Command::new(brit_bin().expect("required Brit executable"));
    gix_testtools::configure_git_environment(&mut command, path);
    command.current_dir(path).args(args);
    command
}

fn brit(path: &Path, args: &[&str]) -> TestResult<Output> {
    Ok(brit_command(path, args).output()?)
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn clone(source: &TestRepo, destination: &Path, name: &str) -> TestResult<()> {
    git(
        destination,
        &["clone", "-q", "--", source.path().to_str().expect("UTF-8"), name],
    )?;
    Ok(())
}

fn compare(actual: &Path, expected: &Path, files: &[&str]) -> TestResult<()> {
    for args in [
        ["symbolic-ref", "HEAD"].as_slice(),
        &["rev-parse", "HEAD", "ORIG_HEAD"],
        &["ls-files", "-s"],
        &["status", "--porcelain"],
    ] {
        assert_eq!(git(actual, args)?, git(expected, args)?, "Git oracle {args:?}");
    }
    for name in files {
        assert_eq!(fs::read(actual.join(name))?, fs::read(expected.join(name))?, "{name}");
    }
    Ok(())
}

struct Fixture {
    clones: tempfile::TempDir,
}

impl Fixture {
    fn new() -> TestResult<Self> {
        let source = TestRepo::new("ff-source")?;
        source.commit_file("selected", "original\n")?;
        source.commit_file("neighbor", "original neighbor\n")?;
        git(source.path(), &["switch", "-c", "incoming"])?;
        source.commit_file("other", "added\n")?;
        source.commit_file("selected", "destination\n")?;
        git(source.path(), &["switch", "main"])?;
        let clones = tempfile::tempdir()?;
        for name in ["actual", "expected"] {
            clone(&source, clones.path(), name)?;
        }
        Ok(Self { clones })
    }

    fn path(&self, name: &str) -> std::path::PathBuf {
        self.clones.path().join(name)
    }
}

fn compare_merge(dirty_neighbor: bool) -> TestResult<()> {
    let f = Fixture::new()?;
    let actual = f.path("actual");
    let expected = f.path("expected");
    if dirty_neighbor {
        for path in [&actual, &expected] {
            fs::write(path.join("neighbor"), "dirty neighbor\n")?;
        }
    }
    let args = ["merge", "--ff-only", "origin/incoming"];
    git(&expected, &args)?;
    let out = brit(&actual, &args)?;
    assert!(out.status.success(), "Brit merge: {}", stderr(&out));
    compare(&actual, &expected, &["selected", "neighbor", "other"])?;
    for reference in ["HEAD", "refs/heads/main"] {
        let args = ["reflog", "show", "-1", "--format=%gD %gs", reference];
        assert_eq!(git(&actual, &args)?, git(&expected, &args)?, "{reference} reflog");
    }
    if dirty_neighbor {
        assert_eq!(fs::read(actual.join("neighbor"))?, b"dirty neighbor\n");
    }
    Ok(())
}

#[test]
fn merge_fast_forwards_the_checked_out_branch_like_git() -> TestResult<()> {
    compare_merge(false)
}

#[test]
fn unrelated_dirty_file_survives() -> TestResult<()> {
    compare_merge(true)
}

fn assert_refusal(path: &Path, diagnostic: &str) -> TestResult<()> {
    let files = [
        ".git/HEAD",
        ".git/refs/heads/main",
        ".git/index",
        "selected",
        "neighbor",
    ];
    let before = files
        .map(|name| fs::read(path.join(name)))
        .into_iter()
        .collect::<std::io::Result<Vec<_>>>()?;
    let out = brit(path, &["merge", "--ff-only", "origin/incoming"])?;
    assert!(
        !out.status.success() && stderr(&out).contains(diagnostic),
        "Brit merge should refuse: {}",
        stderr(&out)
    );
    for (name, bytes) in files.into_iter().zip(before) {
        assert_eq!(fs::read(path.join(name))?, bytes, "{name} must remain unchanged");
    }
    for name in ["other", ".git/ORIG_HEAD", ".git/brit-switch"] {
        assert!(!path.join(name).exists(), "{name} must not be written");
    }
    Ok(())
}

#[test]
fn divergence_refuses_without_writing() -> TestResult<()> {
    let f = Fixture::new()?;
    let actual = f.path("actual");
    git(&actual, &["commit", "--allow-empty", "-m", "diverged"])?;
    assert_refusal(&actual, "Not possible to fast-forward")
}

#[test]
fn affected_dirty_file_refuses_without_writing_and_names_the_path() -> TestResult<()> {
    for staged in [false, true] {
        let f = Fixture::new()?;
        let actual = f.path("actual");
        fs::write(actual.join("selected"), "dirty selected\n")?;
        if staged {
            git(&actual, &["add", "selected"])?;
        }
        assert_refusal(&actual, "selected")?;
    }
    Ok(())
}

fn compare_pin_move(initialized: bool) -> TestResult<()> {
    let submodule = TestRepo::new("ff-submodule")?;
    let old = submodule.commit_file("contents", "old pin\n")?;
    let new = submodule.commit_file("contents", "new pin\n")?;
    let remote = TestRepo::new("ff-superproject")?;
    let url = submodule.path().to_str().expect("UTF-8");
    git(
        remote.path(),
        &["-c", "protocol.file.allow=always", "submodule", "add", url, "sub"],
    )?;
    git(
        remote.path(),
        &["update-index", "--cacheinfo", &format!("160000,{old},sub")],
    )?;
    git(remote.path(), &["commit", "-m", "old pin"])?;
    let clones = tempfile::tempdir()?;
    let actual = clones.path().join("actual");
    let expected = clones.path().join("expected");
    for name in ["actual", "expected"] {
        clone(&remote, clones.path(), name)?;
        let path = clones.path().join(name);
        if initialized {
            git(
                &path,
                &["-c", "protocol.file.allow=always", "submodule", "update", "--init"],
            )?;
        } else {
            assert_eq!(fs::read_dir(path.join("sub"))?.count(), 0, "submodule starts empty");
        }
    }
    git(
        remote.path(),
        &["update-index", "--cacheinfo", &format!("160000,{new},sub")],
    )?;
    remote.commit_file("updated", "superproject updated\n")?;
    if initialized {
        for path in [&actual, &expected] {
            git(path, &["fetch", "origin"])?;
        }
        let args = ["merge", "--ff-only", "origin/main"];
        git(&expected, &args)?;
        let out = brit(&actual, &args)?;
        assert!(out.status.success(), "Brit merge: {}", stderr(&out));
        compare(&actual, &expected, &[".gitmodules", "updated", "sub/contents"])?;
        assert_eq!(
            git(&actual.join("sub"), &["rev-parse", "HEAD"])?,
            format!("{old}\n").as_bytes()
        );
    } else {
        let args = ["pull", "--ff-only", "origin", "main"];
        git(&expected, &args)?;
        let out = brit(&actual, &args)?;
        assert!(out.status.success(), "Brit pull: {}", stderr(&out));
        compare(&actual, &expected, &[".gitmodules", "updated"])?;
        for path in [&actual, &expected] {
            assert_eq!(fs::read_dir(path.join("sub"))?.count(), 0, "submodule stays empty");
        }
    }
    Ok(())
}

#[test]
fn pull_moves_an_uninitialized_submodule_pin_like_git() -> TestResult<()> {
    compare_pin_move(false)
}

#[test]
fn merge_moves_an_initialized_submodule_pin_without_touching_its_checkout() -> TestResult<()> {
    compare_pin_move(true)
}

#[test]
fn packed_branch_fast_forwards_like_git() -> TestResult<()> {
    let f = Fixture::new()?;
    let actual = f.path("actual");
    let expected = f.path("expected");
    for path in [&actual, &expected] {
        git(path, &["switch", "-c", "topic/nested"])?;
        git(path, &["tag", "commit", "origin/incoming"])?;
        git(path, &["pack-refs", "--all", "--prune"])?;
        assert!(
            !path.join(".git/refs/heads/topic").exists(),
            "loose branch parent was pruned"
        );
    }
    let args = ["merge", "--ff-only", "commit"];
    git(&expected, &args)?;
    let out = brit(&actual, &args)?;
    assert!(out.status.success(), "Brit merge: {}", stderr(&out));
    compare(&actual, &expected, &["selected", "neighbor", "other"])
}

#[test]
fn already_up_to_date_preserves_index_and_writes_orig_head_like_git() -> TestResult<()> {
    let f = Fixture::new()?;
    let actual = f.path("actual");
    let expected = f.path("expected");
    let index = fs::read(actual.join(".git/index"))?;
    let args = ["merge", "--ff-only", "HEAD~1"];
    git(&expected, &args)?;
    let out = brit(&actual, &args)?;
    assert!(out.status.success(), "Brit merge: {}", stderr(&out));
    assert_eq!(fs::read(actual.join(".git/index"))?, index, "no-op preserves raw index");
    compare(&actual, &expected, &["selected", "neighbor"])
}

#[cfg(feature = "switch-test-faults")]
#[test]
fn interrupted_merge_holds_writer_guards_and_recovers_through_switch() -> TestResult<()> {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };

    for (checkpoint, recover) in [("journal-prepared", "--abort"), ("branch-published", "--continue")] {
        let f = Fixture::new()?;
        let actual = f.path("actual");
        let expected = f.path("expected");
        let args = ["merge", "--ff-only", "origin/incoming"];
        let preserved_paths = [
            ".git/HEAD",
            ".git/refs/heads/main",
            ".git/index",
            "selected",
            "neighbor",
        ];
        let before = preserved_paths
            .map(|name| fs::read(actual.join(name)))
            .into_iter()
            .collect::<std::io::Result<Vec<_>>>()?;
        let marker = f.clones.path();
        let mut child = brit_command(&actual, &args)
            .env("BRIT_SWITCH_CHECKPOINT", checkpoint)
            .env("BRIT_SWITCH_CHECKPOINT_DIR", marker)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let guarded = (|| -> anyhow::Result<()> {
            let deadline = Instant::now() + Duration::from_secs(30);
            while !marker.join(format!("{checkpoint}.reached")).exists() {
                anyhow::ensure!(child.try_wait()?.is_none(), "merge exited before reaching {checkpoint}");
                anyhow::ensure!(Instant::now() < deadline, "merge did not reach {checkpoint}");
                std::thread::sleep(Duration::from_millis(5));
            }
            let out = brit_command(&actual, &args).output()?;
            anyhow::ensure!(
                !out.status.success() && stderr(&out).contains("held"),
                "writer guard refusal: {}",
                stderr(&out)
            );
            Ok(())
        })();
        let _ = child.kill();
        child.wait()?;
        guarded?;

        let out = brit(&actual, &args)?;
        assert!(
            !out.status.success() && stderr(&out).contains("pending"),
            "recovery refusal: {}",
            stderr(&out)
        );
        let out = brit(&actual, &["switch", "--status"])?;
        assert!(String::from_utf8_lossy(&out.stdout).contains("Merge recovery pending"));
        let out = brit(&actual, &["switch", recover])?;
        assert!(out.status.success(), "Brit switch {recover}: {}", stderr(&out));
        assert!(!actual.join(".git/brit-switch").exists(), "recovery clears the journal");
        if recover == "--continue" {
            git(&expected, &args)?;
            compare(&actual, &expected, &["selected", "neighbor", "other"])?;
        } else {
            for (name, bytes) in preserved_paths.into_iter().zip(before) {
                assert_eq!(fs::read(actual.join(name))?, bytes, "{name} after abort");
            }
            assert!(!actual.join("other").exists());
        }
    }
    Ok(())
}
