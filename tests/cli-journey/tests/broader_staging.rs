//! Whole-selection staging compared with stock Git; every fixture is disposable.
use std::{fs, path::Path};

use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation, Capture},
    test_repo::TestRepo,
};

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}
fn brit(repo: &Path, args: &[&str]) -> Result<Capture> {
    BritInvocation::new(brit_bin().expect("required Brit binary"))
        .current_dir(repo)
        .args(args.iter().copied())
        .run()
}
fn index(repo: &Path) -> Result<Vec<u8>> {
    let path = String::from_utf8(git(repo, &["rev-parse", "--git-path", "index"])?)?;
    Ok(fs::read(repo.join(path.trim()))?)
}
fn entries(repo: &Path) -> Result<Vec<u8>> {
    git(repo, &["ls-files", "--stage", "-z"])
}
fn fixture() -> Result<TestRepo> {
    let repo = TestRepo::new("broader-staging")?;
    for path in ["scope/modified", "scope/deleted", "outside", "outside-deleted"] {
        repo.commit_file(path, "base\n")?;
    }
    fs::write(repo.path().join("outside"), "staged neighbor\n")?;
    git(repo.path(), &["add", "outside"])?;
    fs::write(repo.path().join("outside"), "unstaged neighbor\n")?;
    fs::write(repo.path().join("scope/modified"), "changed\n")?;
    fs::remove_file(repo.path().join("scope/deleted"))?;
    fs::remove_file(repo.path().join("outside-deleted"))?;
    fs::write(repo.path().join("scope/new"), "new\n")?;
    fs::write(repo.path().join("outside-new"), "outside new\n")?;
    Ok(repo)
}
fn parity(actual: &Path, oracle: &Path, args: &[&str]) -> Result<()> {
    let result = brit(actual, args)?;
    ensure!(result.status.success(), "brit {args:?}: {}", result.stderr);
    git(oracle, args)?;
    assert_eq!(entries(actual)?, entries(oracle)?, "index entry parity for {args:?}");
    assert_eq!(
        git(actual, &["write-tree"])?,
        git(oracle, &["write-tree"])?,
        "cached tree parity"
    );
    Ok(())
}

#[test]
fn directories_and_flags_select_exactly_the_git_scope() -> Result<()> {
    for args in [
        vec!["add", "--", "scope"],
        vec!["add", "--", "scope/."],
        vec!["add", "-u", "--", "scope"],
        vec!["add", "-A", "--", "scope/"],
        vec!["add", "--", "."],
        vec!["add", "-u"],
        vec!["add", "-A"],
    ] {
        let actual = fixture()?;
        let oracle = fixture()?;
        git(actual.path(), &["write-tree"])?;
        parity(actual.path(), oracle.path(), &args)?;
        assert_eq!(fs::read(actual.path().join("outside"))?, b"unstaged neighbor\n");
        assert_eq!(fs::read(actual.path().join("scope/new"))?, b"new\n");
        ensure!(!actual.path().join("scope/deleted").exists());
    }
    Ok(())
}

#[test]
fn pathless_flags_from_subdirectory_cover_the_entire_repository() -> Result<()> {
    for flag in ["-u", "-A"] {
        let actual = fixture()?;
        let oracle = fixture()?;
        parity(
            &actual.path().join("scope"),
            &oracle.path().join("scope"),
            &["add", flag],
        )?;
        assert_eq!(git(actual.path(), &["show", ":outside"])?, b"unstaged neighbor\n");
        assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    }
    Ok(())
}

#[test]
fn overlapping_directories_and_literal_names_are_deduplicated() -> Result<()> {
    let actual = fixture()?;
    let oracle = fixture()?;
    for repo in [&actual, &oracle] {
        fs::write(repo.path().join("scope/-literal name"), "literal\n")?;
    }
    parity(
        actual.path(),
        oracle.path(),
        &["add", "--", "scope", "scope/modified", "scope", "scope/-literal name"],
    )
}

#[test]
fn discovered_ignores_skip_untracked_but_keep_tracked_descendants() -> Result<()> {
    for args in [vec!["add", "--", "."], vec!["add", "-u"], vec!["add", "-A"]] {
        let actual = fixture()?;
        let oracle = fixture()?;
        for repo in [&actual, &oracle] {
            fs::write(repo.path().join(".gitignore"), "scope/\n*.secret\n")?;
            fs::write(repo.path().join("private.secret"), "secret\n")?;
        }
        parity(actual.path(), oracle.path(), &args)?;
        assert_eq!(git(actual.path(), &["show", ":scope/modified"])?, b"changed\n");
        ensure!(git(actual.path(), &["ls-files", "--", "private.secret", "scope/new"])?.is_empty());
    }
    Ok(())
}

#[test]
fn explicit_ignored_file_and_invalid_batch_preserve_original_index() -> Result<()> {
    let repo = fixture()?;
    fs::write(repo.path().join(".gitignore"), "ignored\n")?;
    fs::write(repo.path().join("ignored"), "private\n")?;
    let before = index(repo.path())?;
    for path in ["ignored", "missing", "*.txt", "../escape"] {
        ensure!(
            !brit(repo.path(), &["add", "--", "scope", path])?.status.success(),
            "refuse {path}"
        );
        assert_eq!(index(repo.path())?, before, "whole batch must remain unpublished");
    }
    assert_eq!(fs::read(repo.path().join("ignored"))?, b"private\n");
    Ok(())
}

#[test]
fn file_directory_transitions_and_deleted_directories_match_git() -> Result<()> {
    for flag in ["-u", "-A"] {
        for ignored in [false, true] {
            let actual = fixture()?;
            let oracle = fixture()?;
            for repo in [&actual, &oracle] {
                repo.commit_file("becomes-dir", "old file\n")?;
                repo.commit_file("becomes-file/child", "old child\n")?;
                repo.commit_file("removed/child", "deleted\n")?;
                fs::remove_file(repo.path().join("becomes-dir"))?;
                fs::create_dir(repo.path().join("becomes-dir"))?;
                fs::write(repo.path().join("becomes-dir/new"), "new child\n")?;
                fs::remove_dir_all(repo.path().join("becomes-file"))?;
                fs::write(repo.path().join("becomes-file"), "new parent\n")?;
                fs::remove_dir_all(repo.path().join("removed"))?;
                if ignored {
                    fs::write(repo.path().join(".gitignore"), "becomes-dir/*\n")?;
                }
            }
            parity(
                actual.path(),
                oracle.path(),
                &["add", flag, "--", "becomes-dir", "becomes-file", "removed"],
            )?;
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn recursive_selection_preserves_symlinks_and_executable_modes() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for filemode in ["true", "false"] {
        let actual = fixture()?;
        let oracle = fixture()?;
        let external = tempfile::tempdir()?;
        fs::write(external.path().join("private"), "external\n")?;
        for repo in [&actual, &oracle] {
            git(repo.path(), &["config", "core.filemode", filemode])?;
            symlink(external.path(), repo.path().join("scope/link-dir"))?;
            symlink("missing", repo.path().join("scope/dangling"))?;
            fs::set_permissions(repo.path().join("scope/modified"), fs::Permissions::from_mode(0o755))?;
        }
        parity(actual.path(), oracle.path(), &["add", "--", "scope"])?;
        assert_eq!(fs::read(external.path().join("private"))?, b"external\n");
        assert_eq!(fs::read_link(actual.path().join("scope/link-dir"))?, external.path());
        let before = index(actual.path())?;
        for path in [
            "scope/link-dir/private",
            "scope/link-dir/",
            "scope/modified/",
            "scope/link-dir/.",
            "scope/link-dir/./.",
            "scope/modified/.",
        ] {
            let oracle_before = index(oracle.path())?;
            let expected = gix_testtools::git_command(oracle.path())
                .args(["add", "--", path])
                .output()?;
            ensure!(!expected.status.success(), "Git must refuse {path}");
            assert_eq!(index(oracle.path())?, oracle_before);
            ensure!(
                !brit(actual.path(), &["add", "--", path])?.status.success(),
                "Brit must refuse {path}"
            );
            assert_eq!(index(actual.path())?, before);
        }
    }
    Ok(())
}

#[test]
fn nested_repositories_and_gitlinks_refuse_when_selected() -> Result<()> {
    let repo = fixture()?;
    fs::create_dir(repo.path().join("scope/nested"))?;
    git(&repo.path().join("scope/nested"), &["init", "-q"])?;
    fs::write(repo.path().join("scope/nested/private"), "nested\n")?;
    let before = index(repo.path())?;
    for path in ["scope/nested", "scope/nested/private"] {
        ensure!(!brit(repo.path(), &["add", "--", "scope/modified", path])?
            .status
            .success());
        assert_eq!(index(repo.path())?, before);
    }
    // Discovery skips the boundary; it must not turn nested files into parent entries.
    let result = brit(repo.path(), &["add", "--", "scope"])?;
    ensure!(result.status.success(), "{}", result.stderr);
    ensure!(git(repo.path(), &["ls-files", "--", "scope/nested"])?.is_empty());
    let head = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")],
    )?;
    let before = index(repo.path())?;
    for args in [vec!["add", "--", "sub"], vec!["add", "-u"], vec!["add", "-A"]] {
        ensure!(
            !brit(repo.path(), &args)?.status.success(),
            "deleted gitlink must refuse {args:?}"
        );
        assert_eq!(index(repo.path())?, before);
    }
    assert_eq!(fs::read(repo.path().join("scope/nested/private"))?, b"nested\n");
    Ok(())
}

#[test]
fn alternate_indexes_and_linked_worktrees_preserve_the_primary_index() -> Result<()> {
    let actual = fixture()?;
    let oracle = fixture()?;
    let before = index(actual.path())?;
    let actual_index = actual.path().join(".git/alternate");
    let oracle_index = oracle.path().join(".git/alternate");
    fs::write(&actual_index, &before)?;
    fs::write(&oracle_index, index(oracle.path())?)?;
    let out = BritInvocation::new(brit_bin().expect("binary"))
        .current_dir(actual.path())
        .args(["add", "-A"])
        .env("GIT_INDEX_FILE", actual_index.as_os_str())
        .run()?;
    ensure!(out.status.success(), "{}", out.stderr);
    let out = gix_testtools::git_command(oracle.path())
        .args(["add", "-A"])
        .env("GIT_INDEX_FILE", &oracle_index)
        .output()?;
    ensure!(out.status.success());
    let actual_entries = gix_testtools::git_command(actual.path())
        .args(["ls-files", "--stage", "-z"])
        .env("GIT_INDEX_FILE", &actual_index)
        .output()?;
    let oracle_entries = gix_testtools::git_command(oracle.path())
        .args(["ls-files", "--stage", "-z"])
        .env("GIT_INDEX_FILE", &oracle_index)
        .output()?;
    ensure!(actual_entries.status.success() && oracle_entries.status.success());
    assert_eq!(actual_entries.stdout, oracle_entries.stdout);
    assert_eq!(index(actual.path())?, before);
    let a = tempfile::tempdir()?;
    let b = tempfile::tempdir()?;
    for (repo, linked) in [(&actual, &a), (&oracle, &b)] {
        git(
            repo.path(),
            &[
                "worktree",
                "add",
                "--detach",
                linked.path().to_str().expect("temp path"),
            ],
        )?;
        fs::write(linked.path().join("scope/modified"), "linked\n")?;
        fs::write(linked.path().join("scope/new"), "linked new\n")?;
    }
    parity(a.path(), b.path(), &["add", "-A"])?;
    assert_eq!(index(actual.path())?, before);
    Ok(())
}

#[test]
fn locks_filters_and_case_aliases_refuse_atomically() -> Result<()> {
    let repo = fixture()?;
    let before = index(repo.path())?;
    fs::write(repo.path().join(".git/index.lock"), "other owner\n")?;
    ensure!(!brit(repo.path(), &["add", "-A"])?.status.success());
    assert_eq!(index(repo.path())?, before);
    assert_eq!(fs::read(repo.path().join(".git/index.lock"))?, b"other owner\n");
    fs::remove_file(repo.path().join(".git/index.lock"))?;
    fs::write(repo.path().join("scope/.gitattributes"), "new filter=external\n")?;
    git(
        repo.path(),
        &["config", "filter.external.clean", "echo ran > .git/filter-ran; cat"],
    )?;
    ensure!(!brit(repo.path(), &["add", "-A"])?.status.success());
    assert_eq!(index(repo.path())?, before);
    ensure!(!repo.path().join(".git/filter-ran").exists());
    fs::remove_file(repo.path().join("scope/.gitattributes"))?;
    git(repo.path(), &["config", "--remove-section", "filter.external"])?;
    git(repo.path(), &["config", "core.ignorecase", "true"])?;
    fs::write(repo.path().join("scope/Modified"), "alias\n")?;
    let alias = brit(repo.path(), &["add", "--", "scope"])?;
    ensure!(
        !alias.status.success() && alias.stderr.contains("Case aliases"),
        "{}",
        alias.stderr
    );
    assert_eq!(index(repo.path())?, before);
    Ok(())
}

#[test]
fn object_write_failure_preserves_index_and_working_files() -> Result<()> {
    let repo = fixture()?;
    let before = index(repo.path())?;
    let hash = String::from_utf8(git(repo.path(), &["hash-object", "scope/new"])?)?;
    let obstruction = repo.path().join(".git/objects").join(&hash[..2]);
    ensure!(!obstruction.exists(), "fixture requires unused loose-object fanout");
    fs::write(&obstruction, "another owner's obstruction\n")?;
    ensure!(!brit(repo.path(), &["add", "--", "scope"])?.status.success());
    assert_eq!(index(repo.path())?, before);
    assert_eq!(fs::read(obstruction)?, b"another owner's obstruction\n");
    assert_eq!(fs::read(repo.path().join("scope/new"))?, b"new\n");
    ensure!(!repo.path().join(".git/index.lock").exists());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn real_interrupt_under_index_lock_prevents_all_publication() -> Result<()> {
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    struct ChildGuard(Option<Child>);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let repo = fixture()?;
    // Sparse zeros provide a long hashing window without an equally large fixture archive.
    fs::File::create(repo.path().join("scope/large"))?.set_len(256 * 1024 * 1024)?;
    let before = index(repo.path())?;
    let mut cmd = Command::new(brit_bin().expect("binary"));
    gix_testtools::configure_git_environment(&mut cmd, repo.path());
    cmd.env("GIT_CONFIG_GLOBAL", repo.path().join(".git/missing-config"))
        .current_dir(repo.path())
        .args(["add", "--", "scope"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = ChildGuard(Some(cmd.spawn()?));
    let pid = child.0.as_ref().expect("child").id().to_string();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !repo.path().join(".git/index.lock").exists() {
        ensure!(
            child.0.as_mut().expect("child").try_wait()?.is_none(),
            "add exited before lock observation"
        );
        ensure!(Instant::now() < deadline, "add did not acquire index lock");
        std::thread::yield_now();
    }
    ensure!(Command::new("kill").args(["-STOP", &pid]).status()?.success());
    while !fs::read_to_string(format!("/proc/{pid}/status"))?
        .lines()
        .any(|line| line.starts_with("State:") && line.contains('T'))
    {
        ensure!(Instant::now() < deadline, "add did not stop");
        std::thread::yield_now();
    }
    assert_eq!(index(repo.path())?, before, "stop happened before publication");
    ensure!(
        repo.path().join(".git/index.lock").exists(),
        "process owns lock when interrupted"
    );
    ensure!(Command::new("kill").args(["-INT", &pid]).status()?.success());
    ensure!(Command::new("kill").args(["-CONT", &pid]).status()?.success());
    let out = child.0.take().expect("child").wait_with_output()?;
    ensure!(!out.status.success(), "cancelled staging cannot claim success");
    assert_eq!(
        index(repo.path())?,
        before,
        "cancellation preserves the entire original index"
    );
    ensure!(
        !repo.path().join(".git/index.lock").exists(),
        "cancellation releases owned lock"
    );
    assert_eq!(fs::metadata(repo.path().join("scope/large"))?.len(), 256 * 1024 * 1024);
    assert_eq!(fs::read(repo.path().join("scope/modified"))?, b"changed\n");
    Ok(())
}

#[test]
fn tracked_paths_becoming_nested_repositories_refuse_without_deletions() -> Result<()> {
    for (was_file, ignored) in [(false, false), (false, true), (true, false), (true, true)] {
        let repo = fixture()?;
        if was_file {
            fs::remove_dir_all(repo.path().join("scope"))?;
            git(repo.path(), &["rm", "-r", "--cached", "scope"])?;
            fs::write(repo.path().join("scope"), "tracked parent\n")?;
            git(repo.path(), &["add", "--", "scope"])?;
            fs::remove_file(repo.path().join("scope"))?;
            fs::create_dir(repo.path().join("scope"))?;
            fs::write(repo.path().join("scope/modified"), "changed\n")?;
        }
        git(&repo.path().join("scope"), &["init", "-q"])?;
        if ignored {
            fs::write(repo.path().join(".gitignore"), "scope/\n")?;
        }
        let before = index(repo.path())?;
        for args in [
            vec!["add", "-A"],
            vec!["add", "-u"],
            vec!["add", "--", "."],
            vec!["add", "--", "scope"],
            vec!["add", "--", "scope/modified"],
        ] {
            ensure!(
                !brit(repo.path(), &args)?.status.success(),
                "tracked nested boundary (was_file={was_file}, ignored={ignored}) refuses {args:?}"
            );
            assert_eq!(index(repo.path())?, before);
        }
        assert_eq!(fs::read(repo.path().join("scope/modified"))?, b"changed\n");
    }
    Ok(())
}

#[test]
fn tracked_only_selection_skips_untracked_nested_repository() -> Result<()> {
    let actual = fixture()?;
    let oracle = fixture()?;
    for repo in [&actual, &oracle] {
        fs::create_dir(repo.path().join("nested"))?;
        git(&repo.path().join("nested"), &["init", "-q"])?;
        fs::write(repo.path().join("nested/private"), "private\n")?;
    }
    parity(actual.path(), oracle.path(), &["add", "-u"])?;
    assert_eq!(fs::read(actual.path().join("nested/private"))?, b"private\n");
    Ok(())
}

#[test]
fn tracked_only_unmatched_scopes_refuse_atomically_like_git() -> Result<()> {
    for path in ["scope/new", "empty"] {
        for mixed in [false, true] {
            let actual = fixture()?;
            let oracle = fixture()?;
            fs::create_dir(actual.path().join("empty"))?;
            fs::create_dir(oracle.path().join("empty"))?;
            let before = index(actual.path())?;
            let oracle_before = index(oracle.path())?;
            let mut args = vec!["add", "-u", "--"];
            if mixed {
                args.push("scope/modified");
            }
            args.push(path);
            let expected = gix_testtools::git_command(oracle.path()).args(&args).output()?;
            ensure!(!expected.status.success(), "Git must refuse unmatched scope {args:?}");
            assert_eq!(index(oracle.path())?, oracle_before);
            let result = brit(actual.path(), &args)?;
            ensure!(!result.status.success(), "Brit must refuse unmatched scope {args:?}");
            assert_eq!(index(actual.path())?, before, "whole selection remains unpublished");
            assert_eq!(fs::read(actual.path().join("scope/modified"))?, b"changed\n");
        }
    }
    Ok(())
}
