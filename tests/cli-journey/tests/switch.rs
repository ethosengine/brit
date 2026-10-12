//! Safe branch switching: disposable stock-Git oracles and preservation boundaries.
#[cfg(not(target_os = "linux"))]
#[test]
fn unsupported_platform_refuses_switch_without_mutation() -> anyhow::Result<()> {
    use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
    let repo = TestRepo::new("switch-unsupported-platform")?;
    let before = std::fs::read(repo.path().join(".git/index")).ok();
    let head = std::fs::read(repo.path().join(".git/HEAD"))?;
    let mut cmd = std::process::Command::new(brit_bin().expect("required Brit executable"));
    gix_testtools::configure_git_environment(&mut cmd, repo.path());
    let out = cmd.current_dir(repo.path()).args(["switch", "main"]).output()?;
    anyhow::ensure!(!out.status.success(), "unsupported durability platform must refuse");
    let diagnostic = String::from_utf8_lossy(&out.stderr);
    anyhow::ensure!(
        diagnostic.contains("Linux") || diagnostic.contains("durability"),
        "explicit unsupported platform: {diagnostic}"
    );
    assert_eq!(std::fs::read(repo.path().join(".git/index")).ok(), before);
    assert_eq!(std::fs::read(repo.path().join(".git/HEAD"))?, head);
    Ok(())
}

#[cfg(target_os = "linux")]
mod linux {

    use anyhow::{ensure, Result};
    use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
    use std::{
        fs,
        path::Path,
        process::{Command, Output},
    };

    fn command(repo: &Path, native: bool) -> Command {
        let mut cmd = if native {
            let mut cmd = Command::new(brit_bin().expect("required Brit executable"));
            gix_testtools::configure_git_environment(&mut cmd, repo);
            cmd.current_dir(repo);
            cmd
        } else {
            gix_testtools::git_command(repo)
        };
        cmd.env("GIT_COMMITTER_NAME", "Switch Journey")
            .env("GIT_COMMITTER_EMAIL", "switch@example.com")
            .env("GIT_COMMITTER_DATE", "2026-01-02 03:04:05 +0000");
        cmd
    }
    fn run(repo: &Path, native: bool, args: &[&str]) -> Result<Output> {
        Ok(command(repo, native).args(args).output()?)
    }
    fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
        let out = run(repo, false, args)?;
        ensure!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out.stdout)
    }
    fn success(repo: &Path, args: &[&str]) -> Result<Output> {
        let out = run(repo, true, args)?;
        ensure!(
            out.status.success(),
            "brit {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out)
    }
    fn fixture(label: &str) -> Result<TestRepo> {
        let repo = TestRepo::new(label)?;
        repo.commit_file("selected", "original\n")?;
        repo.commit_file("neighbor", "neighbor original\n")?;
        repo.commit_file(".gitignore", "ignored\n")?;
        git(repo.path(), &["switch", "-c", "destination"])?;
        repo.commit_file("selected", "destination\n")?;
        repo.commit_file("added", "added\n")?;
        git(repo.path(), &["switch", "main"])?;
        Ok(repo)
    }
    fn index(repo: &Path) -> Result<Vec<u8>> {
        Ok(fs::read(repo.join(".git/index"))?)
    }
    fn unchanged(repo: &Path, before: &[u8], head: &[u8]) -> Result<()> {
        assert_eq!(index(repo)?, before, "refusal preserves exact original index bytes");
        assert_eq!(git(repo, &["rev-parse", "HEAD"])?, head, "refusal preserves HEAD");
        Ok(())
    }
    fn compare(repo: &Path, oracle: &Path) -> Result<()> {
        for args in [
            vec!["rev-parse", "HEAD"],
            vec!["symbolic-ref", "HEAD"],
            vec!["ls-files", "--stage", "-z"],
            vec!["status", "--porcelain=v1", "-z"],
        ] {
            assert_eq!(git(repo, &args)?, git(oracle, &args)?, "Git oracle {args:?}");
        }
        for path in ["selected", "neighbor", "added", "private", "ignored"] {
            assert_eq!(
                fs::read(repo.join(path)).ok(),
                fs::read(oracle.join(path)).ok(),
                "bytes {path}"
            );
        }
        Ok(())
    }

    #[test]
    fn ordinary_same_branch_and_detached_start_match_git() -> Result<()> {
        for detached in [false, true] {
            let actual = fixture("switch-basic")?;
            let oracle = fixture("switch-basic-oracle")?;
            for repo in [&actual, &oracle] {
                if detached {
                    git(repo.path(), &["switch", "--detach"])?;
                }
            }
            let branches = git(actual.path(), &["show-ref", "--heads"])?;
            success(actual.path(), &["switch", "destination"])?;
            git(oracle.path(), &["switch", "destination"])?;
            compare(actual.path(), oracle.path())?;
            success(actual.path(), &["switch", "destination"])?;
            git(oracle.path(), &["switch", "destination"])?;
            compare(actual.path(), oracle.path())?;
            assert_eq!(
                git(actual.path(), &["reflog", "show", "--format=%gs", "HEAD"])?,
                git(oracle.path(), &["reflog", "show", "--format=%gs", "HEAD"])?,
                "same-branch HEAD reflog matches Git"
            );
            assert_eq!(
                git(actual.path(), &["show-ref", "--heads"])?,
                branches,
                "switch never moves branch tips"
            );
            let log = git(actual.path(), &["reflog", "show", "--format=%gs", "HEAD"])?;
            ensure!(
                String::from_utf8_lossy(&log).contains("checkout: moving from"),
                "HEAD checkout reflog"
            );
        }
        Ok(())
    }

    #[test]
    fn full_local_ref_and_unrelated_staged_unstaged_private_files_survive() -> Result<()> {
        let actual = fixture("switch-neighbors")?;
        let oracle = fixture("switch-neighbors-oracle")?;
        for repo in [&actual, &oracle] {
            fs::write(repo.path().join("neighbor"), "staged\n")?;
            git(repo.path(), &["add", "--", "neighbor"])?;
            fs::write(repo.path().join("neighbor"), "unstaged\n")?;
            fs::write(repo.path().join("private"), "private\n")?;
            fs::write(repo.path().join("ignored"), "ignored private\n")?;
        }
        success(actual.path(), &["switch", "refs/heads/destination"])?;
        git(oracle.path(), &["switch", "destination"])?;
        compare(actual.path(), oracle.path())
    }

    #[test]
    fn conflicts_and_untracked_or_ignored_collisions_refuse_before_mutation() -> Result<()> {
        for shape in ["unstaged", "staged", "untracked", "ignored", "directory"] {
            let repo = fixture("switch-collision")?;
            match shape {
                "unstaged" | "staged" => {
                    fs::write(repo.path().join("selected"), "private edit\n")?;
                    if shape == "staged" {
                        git(repo.path(), &["add", "selected"])?;
                    }
                }
                "untracked" | "ignored" => {
                    fs::write(repo.path().join("added"), "private collision\n")?;
                    if shape == "ignored" {
                        fs::write(repo.path().join(".git/info/exclude"), "added\n")?;
                    }
                }
                "directory" => {
                    fs::create_dir(repo.path().join("added"))?;
                    fs::write(repo.path().join("added/private"), "nested private\n")?;
                }
                _ => unreachable!(),
            }
            let before = index(repo.path())?;
            let head = git(repo.path(), &["rev-parse", "HEAD"])?;
            let selected = fs::read(repo.path().join("selected"))?;
            let out = run(repo.path(), true, &["switch", "destination"])?;
            ensure!(!out.status.success(), "must refuse {shape}");
            unchanged(repo.path(), &before, &head)?;
            assert_eq!(fs::read(repo.path().join("selected"))?, selected);
            if shape == "directory" {
                assert_eq!(fs::read(repo.path().join("added/private"))?, b"nested private\n");
            }
            if shape == "untracked" || shape == "ignored" {
                assert_eq!(fs::read(repo.path().join("added"))?, b"private collision\n");
            }
        }
        Ok(())
    }

    #[test]
    fn new_nested_paths_never_overwrite_untracked_or_ignored_ancestors() -> Result<()> {
        for ignored in [false, true] {
            let repo = fixture("switch-ancestor-collision")?;
            git(repo.path(), &["switch", "destination"])?;
            repo.commit_file("directory/child", "destination child\n")?;
            git(repo.path(), &["switch", "main"])?;
            fs::write(repo.path().join("directory"), "private ancestor\n")?;
            if ignored {
                fs::write(repo.path().join(".git/info/exclude"), "directory\n")?;
            }
            let before = index(repo.path())?;
            let head = git(repo.path(), &["rev-parse", "HEAD"])?;
            let out = run(repo.path(), true, &["switch", "destination"])?;
            ensure!(
                !out.status.success(),
                "untracked ancestor must refuse (ignored={ignored})"
            );
            unchanged(repo.path(), &before, &head)?;
            assert_eq!(fs::read(repo.path().join("directory"))?, b"private ancestor\n");
            assert_eq!(
                fs::read(repo.path().join("selected"))?,
                b"original\n",
                "whole preflight before changes"
            );
            assert!(!repo.path().join("added").exists());
        }
        Ok(())
    }

    #[test]
    fn unsupported_forms_and_alternate_index_are_non_mutating() -> Result<()> {
        let repo = fixture("switch-forms")?;
        let before = index(repo.path())?;
        let head = git(repo.path(), &["rev-parse", "HEAD"])?;
        for args in [
            vec!["switch", "-c", "new"],
            vec!["switch", "-C", "new"],
            vec!["switch", "--detach", "destination"],
            vec!["switch", "--force", "destination"],
            vec!["switch", "--merge", "destination"],
            vec!["switch", "-"],
            vec!["switch", "HEAD~1"],
            vec!["switch", "refs/remotes/origin/main"],
            vec!["switch", "missing"],
        ] {
            ensure!(!run(repo.path(), true, &args)?.status.success(), "must refuse {args:?}");
            unchanged(repo.path(), &before, &head)?;
        }
        let alternate = repo.path().join(".git/alternate");
        fs::write(&alternate, &before)?;
        let out = command(repo.path(), true)
            .env("GIT_INDEX_FILE", &alternate)
            .args(["switch", "destination"])
            .output()?;
        ensure!(!out.status.success(), "alternate index is unsupported");
        unchanged(repo.path(), &before, &head)?;
        assert_eq!(fs::read(alternate)?, before);
        Ok(())
    }

    #[test]
    fn occupied_or_reserved_branch_and_foreign_locks_are_preserved() -> Result<()> {
        for reservation in ["attached", "rebase", "bisect", "index-lock", "head-lock"] {
            let repo = fixture("switch-occupancy")?;
            let linked = tempfile::tempdir()?;
            if reservation.ends_with("lock") {
                let name = if reservation == "index-lock" {
                    "index.lock"
                } else {
                    "HEAD.lock"
                };
                fs::write(repo.path().join(".git").join(name), "foreign lock\n")?;
            } else {
                git(
                    repo.path(),
                    &[
                        "worktree",
                        "add",
                        linked.path().to_str().expect("temporary path"),
                        "destination",
                    ],
                )?;
                if reservation != "attached" {
                    git(linked.path(), &["switch", "--detach"])?;
                    let private = String::from_utf8(git(linked.path(), &["rev-parse", "--absolute-git-dir"])?)?;
                    if reservation == "rebase" {
                        fs::create_dir(Path::new(private.trim()).join("rebase-merge"))?;
                        fs::write(
                            Path::new(private.trim()).join("rebase-merge/head-name"),
                            "refs/heads/destination\n",
                        )?;
                    } else {
                        fs::write(Path::new(private.trim()).join("BISECT_START"), "destination\n")?;
                        fs::write(Path::new(private.trim()).join("BISECT_LOG"), "# bisect started\n")?;
                    }
                }
            }
            let before = index(repo.path())?;
            let head = git(repo.path(), &["rev-parse", "HEAD"])?;
            ensure!(
                !run(repo.path(), true, &["switch", "destination"])?.status.success(),
                "must refuse {reservation}"
            );
            unchanged(repo.path(), &before, &head)?;
            assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
            if reservation.ends_with("lock") {
                let name = if reservation == "index-lock" {
                    "index.lock"
                } else {
                    "HEAD.lock"
                };
                assert_eq!(fs::read(repo.path().join(".git").join(name))?, b"foreign lock\n");
            }
        }
        Ok(())
    }

    #[test]
    fn unsupported_indexes_and_external_filters_refuse_without_side_effects() -> Result<()> {
        for shape in ["split", "sparse", "skip", "assume", "filter"] {
            let repo = fixture("switch-preflight")?;
            match shape {
                "split" => {
                    git(repo.path(), &["update-index", "--split-index"])?;
                }
                "sparse" => {
                    git(repo.path(), &["sparse-checkout", "init", "--cone", "--sparse-index"])?;
                }
                "skip" => {
                    git(repo.path(), &["update-index", "--skip-worktree", "selected"])?;
                }
                "assume" => {
                    git(repo.path(), &["update-index", "--assume-unchanged", "selected"])?;
                }
                "filter" => {
                    fs::write(repo.path().join(".git/info/attributes"), "selected filter=external\n")?;
                    git(
                        repo.path(),
                        &["config", "filter.external.smudge", "touch filter-ran; cat"],
                    )?;
                }
                _ => unreachable!(),
            }
            let before = index(repo.path())?;
            let head = git(repo.path(), &["rev-parse", "HEAD"])?;
            ensure!(
                !run(repo.path(), true, &["switch", "destination"])?.status.success(),
                "must refuse {shape}"
            );
            unchanged(repo.path(), &before, &head)?;
            assert!(!repo.path().join("filter-ran").exists());
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_executable_modes_and_destination_attributes_match_git() -> Result<()> {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let actual = fixture("switch-types")?;
        let oracle = fixture("switch-types-oracle")?;
        for repo in [&actual, &oracle] {
            git(repo.path(), &["switch", "destination"])?;
            symlink("selected", repo.path().join("link"))?;
            git(repo.path(), &["add", "link"])?;
            git(repo.path(), &["update-index", "--chmod=+x", "selected"])?;
            fs::set_permissions(repo.path().join("selected"), fs::Permissions::from_mode(0o755))?;
            repo.commit_file(".gitattributes", "added text eol=crlf\n")?;
            git(repo.path(), &["switch", "main"])?;
        }
        success(actual.path(), &["switch", "destination"])?;
        git(oracle.path(), &["switch", "destination"])?;
        compare(actual.path(), oracle.path())?;
        assert_eq!(
            fs::read_link(actual.path().join("link"))?,
            fs::read_link(oracle.path().join("link"))?
        );
        assert_eq!(
            fs::metadata(actual.path().join("selected"))?.permissions().mode() & 0o111,
            fs::metadata(oracle.path().join("selected"))?.permissions().mode() & 0o111
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn post_checkout_receives_old_new_and_branch_flag_and_failure_keeps_commit() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        for exit_code in [0, 1, 7] {
            let repo = fixture("switch-hook")?;
            let oracle = fixture("switch-hook-oracle")?;
            let old = repo.head_id()?;
            let target = String::from_utf8(git(repo.path(), &["rev-parse", "destination"])?)?;
            for fixture in [&repo, &oracle] {
                let hook = fixture.path().join(".git/hooks/post-checkout");
                fs::write(
                    &hook,
                    format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > hook-args\nexit {exit_code}\n"),
                )?;
                fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
            }
            let out = run(repo.path(), true, &["switch", "destination"])?;
            let expected = run(oracle.path(), false, &["switch", "destination"])?;
            assert_eq!(
                out.status.code(),
                expected.status.code(),
                "exact hook exit code matches Git"
            );
            assert_eq!(
                expected.status.success(),
                exit_code == 0,
                "Git reports hook failure without rolling back"
            );
            if exit_code != 0 {
                ensure!(
                    String::from_utf8_lossy(&out.stderr).contains("committed"),
                    "hook failure must identify committed outcome"
                );
            }
            assert_eq!(
                fs::read_to_string(repo.path().join("hook-args"))?,
                format!("{old}\n{}\n1\n", target.trim())
            );
            assert_eq!(
                fs::read(repo.path().join("hook-args"))?,
                fs::read(oracle.path().join("hook-args"))?
            );
            assert_eq!(
                repo.head_id()?,
                target.trim(),
                "hook failure does not roll back committed switch"
            );
            let same = run(repo.path(), true, &["switch", "destination"])?;
            let oracle_same = run(oracle.path(), false, &["switch", "destination"])?;
            assert_eq!(
                same.status.code(),
                oracle_same.status.code(),
                "same-branch hook exit matches Git"
            );
            assert_eq!(
                fs::read_to_string(repo.path().join("hook-args"))?,
                format!("{0}\n{0}\n1\n", target.trim()),
                "same-branch switch still invokes post-checkout"
            );
        }
        Ok(())
    }

    #[test]
    fn post_checkout_can_stage_its_edits_through_git_like_the_oracle() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let actual = fixture("switch-hook-authoring")?;
        let oracle = fixture("switch-hook-authoring-oracle")?;
        for repo in [&actual, &oracle] {
            let hook = repo.path().join(".git/hooks/post-checkout");
            fs::write(&hook, "#!/bin/sh\necho hook-staged > selected\ngit add -- selected\n")?;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
        }
        let expected = run(oracle.path(), false, &["switch", "destination"])?;
        let actual_out = run(actual.path(), true, &["switch", "destination"])?;
        ensure!(
            expected.status.success(),
            "oracle authoring hook: {}",
            String::from_utf8_lossy(&expected.stderr)
        );
        ensure!(
            actual_out.status.success(),
            "Brit authoring hook: {}",
            String::from_utf8_lossy(&actual_out.stderr)
        );
        compare(actual.path(), oracle.path())?;
        assert_eq!(fs::read(actual.path().join("selected"))?, b"hook-staged\n");
        assert_eq!(
            git(actual.path(), &["show", ":selected"])?,
            b"hook-staged\n",
            "ordinary index lock released before post-checkout"
        );
        Ok(())
    }

    #[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
    mod recovery {
        use super::*;
        use std::{
            process::{Child, Stdio},
            time::{Duration, Instant},
        };

        struct ReleaseHook(std::path::PathBuf);
        impl Drop for ReleaseHook {
            fn drop(&mut self) {
                let _ = fs::write(&self.0, b"release\n");
            }
        }
        struct Paused {
            child: Child,
            _markers: tempfile::TempDir,
        }
        impl Drop for Paused {
            fn drop(&mut self) {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
        fn pause(repo: &Path, args: &[&str], point: &str) -> Result<Paused> {
            let markers = tempfile::tempdir()?;
            let child = command(repo, true)
                .args(args)
                .env("BRIT_SWITCH_CHECKPOINT", point)
                .env("BRIT_SWITCH_CHECKPOINT_DIR", markers.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let mut paused = Paused {
                child,
                _markers: markers,
            };
            let deadline = Instant::now() + Duration::from_secs(30);
            while !paused._markers.path().join(format!("{point}.reached")).exists() {
                ensure!(
                    paused.child.try_wait()?.is_none(),
                    "switch exited before deterministic checkpoint {point}; BRIT_BIN must enable switch-test-faults"
                );
                ensure!(Instant::now() < deadline, "switch did not reach {point}");
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(paused)
        }
        fn interrupt(paused: &mut Paused, signal: &str) -> Result<()> {
            ensure!(Command::new("kill")
                .args([signal, &paused.child.id().to_string()])
                .status()?
                .success());
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if let Some(status) = paused.child.try_wait()? {
                    ensure!(!status.success(), "interruption must report incomplete operation");
                    return Ok(());
                }
                ensure!(Instant::now() < deadline, "interrupted switch failed to terminate");
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        fn journal(repo: &Path) -> std::path::PathBuf {
            repo.join(".git/brit-switch")
        }

        #[test]
        fn real_kill_and_interrupt_at_each_publication_boundary_continue_explicitly() -> Result<()> {
            for signal in ["-KILL", "-INT"] {
                for point in [
                    "journal-prepared",
                    "file-published",
                    "commit-decision",
                    "index-published",
                    "head-published",
                    "reflog-published",
                    "hook-started",
                    "cleanup",
                ] {
                    let repo = fixture("switch-recovery")?;
                    // A hook ensures its checkpoint is traversed, without calling Git.
                    use std::os::unix::fs::PermissionsExt;
                    let hook = repo.path().join(".git/hooks/post-checkout");
                    fs::write(&hook, "#!/bin/sh\necho invoked >> hook-count\n")?;
                    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
                    let target = String::from_utf8(git(repo.path(), &["rev-parse", "destination"])?)?;
                    let mut paused = pause(repo.path(), &["switch", "destination"], point)?;
                    interrupt(&mut paused, signal)?;
                    success(repo.path(), &["switch", "--status"])?;
                    ensure!(
                        !run(repo.path(), true, &["switch", "destination"])?.status.success(),
                        "new switch must not resume interrupted {point}"
                    );
                    success(repo.path(), &["switch", "--continue"])?;
                    assert_eq!(repo.head_id()?, target.trim(), "roll-forward at {point}");
                    assert_eq!(fs::read(repo.path().join("selected"))?, b"destination\n");
                    assert_eq!(fs::read(repo.path().join("added"))?, b"added\n");
                    ensure!(
                        !journal(repo.path()).exists(),
                        "durable completion removes journal at {point}"
                    );
                    // Repeated explicit recovery cannot replay a hook or a reflog row.
                    let log = git(repo.path(), &["reflog", "show", "--format=%gs", "HEAD"])?;
                    let hooks = fs::read(repo.path().join("hook-count")).ok();
                    let _ = run(repo.path(), true, &["switch", "--continue"])?;
                    assert_eq!(git(repo.path(), &["reflog", "show", "--format=%gs", "HEAD"])?, log);
                    assert_eq!(fs::read(repo.path().join("hook-count")).ok(), hooks);
                    ensure!(
                        hooks
                            .as_ref()
                            .map_or(0, |bytes| bytes.iter().filter(|b| **b == b'\n').count())
                            <= 1,
                        "hook never replays"
                    );
                }
            }
            Ok(())
        }

        #[test]
        fn killing_initial_and_recovery_lock_acquisition_preserves_an_explicit_way_out() -> Result<()> {
            for point in ["lock-intent", "lock-acquired"] {
                for action in ["--continue", "--abort"] {
                    let repo = fixture("switch-lock-window")?;
                    let before = index(repo.path())?;
                    let head = git(repo.path(), &["rev-parse", "HEAD"])?;
                    let mut paused = pause(repo.path(), &["switch", "destination"], point)?;
                    interrupt(&mut paused, "-KILL")?;
                    success(repo.path(), &["switch", "--status"])?;
                    success(repo.path(), &["switch", action])?;
                    if action == "--abort" {
                        unchanged(repo.path(), &before, &head)?;
                        assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
                    } else {
                        assert_eq!(fs::read(repo.path().join("selected"))?, b"destination\n");
                        assert_eq!(
                            git(repo.path(), &["symbolic-ref", "HEAD"])?,
                            b"refs/heads/destination\n"
                        );
                    }
                    assert!(!journal(repo.path()).exists());
                }
                let repo = fixture("switch-recovery-lock-window")?;
                let mut paused = pause(repo.path(), &["switch", "destination"], "journal-prepared")?;
                interrupt(&mut paused, "-KILL")?;
                let mut recovering = pause(repo.path(), &["switch", "--continue"], point)?;
                interrupt(&mut recovering, "-KILL")?;
                success(repo.path(), &["switch", "--continue"])?;
                assert_eq!(fs::read(repo.path().join("selected"))?, b"destination\n");
                assert!(!journal(repo.path()).exists());
            }
            Ok(())
        }

        #[test]
        fn abort_restores_exact_original_state_and_interrupted_abort_is_restartable() -> Result<()> {
            for point in ["journal-prepared", "file-published"] {
                let repo = fixture("switch-abort")?;
                let before = index(repo.path())?;
                let head = git(repo.path(), &["rev-parse", "HEAD"])?;
                let mut paused = pause(repo.path(), &["switch", "destination"], point)?;
                interrupt(&mut paused, "-KILL")?;
                if point == "file-published" {
                    let mut abort = pause(repo.path(), &["switch", "--abort"], "abort-file")?;
                    interrupt(&mut abort, "-KILL")?;
                }
                success(repo.path(), &["switch", "--abort"])?;
                unchanged(repo.path(), &before, &head)?;
                assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
                assert!(!repo.path().join("added").exists());
                assert!(!journal(repo.path()).exists());
            }
            Ok(())
        }

        #[test]
        fn commit_decision_refuses_abort_and_preserves_recovery_evidence() -> Result<()> {
            let repo = fixture("switch-decision")?;
            let mut paused = pause(repo.path(), &["switch", "destination"], "commit-decision")?;
            interrupt(&mut paused, "-KILL")?;
            let out = run(repo.path(), true, &["switch", "--abort"])?;
            ensure!(!out.status.success(), "commit decision makes abort unavailable");
            ensure!(journal(repo.path()).exists(), "refusal retains recovery evidence");
            success(repo.path(), &["switch", "--continue"])?;
            Ok(())
        }

        #[test]
        fn foreign_worktree_index_and_branch_updates_are_never_overwritten() -> Result<()> {
            for foreign in ["file", "index", "source", "target", "index-lock"] {
                let repo = fixture("switch-foreign")?;
                let mut paused = pause(repo.path(), &["switch", "destination"], "journal-prepared")?;
                interrupt(&mut paused, "-KILL")?;
                match foreign {
                    "file" => fs::write(repo.path().join("selected"), "foreign writer\n")?,
                    "index" => fs::write(repo.path().join(".git/index"), "foreign index bytes\n")?,
                    "source" => {
                        fs::write(
                            repo.path().join(".git/refs/heads/main"),
                            git(repo.path(), &["rev-parse", "destination"])?,
                        )?;
                    }
                    "target" => {
                        fs::write(
                            repo.path().join(".git/refs/heads/destination"),
                            git(repo.path(), &["rev-parse", "main"])?,
                        )?;
                    }
                    "index-lock" => {
                        fs::write(repo.path().join(".git/index.lock"), "foreign lock bytes\n")?;
                    }
                    _ => unreachable!(),
                }
                let before = index(repo.path())?;
                let selected = fs::read(repo.path().join("selected"))?;
                let refs = git(repo.path(), &["show-ref", "--heads"])?;
                ensure!(
                    !run(repo.path(), true, &["switch", "--continue"])?.status.success(),
                    "must detect foreign {foreign}"
                );
                assert_eq!(index(repo.path())?, before);
                assert_eq!(fs::read(repo.path().join("selected"))?, selected);
                assert_eq!(git(repo.path(), &["show-ref", "--heads"])?, refs);
                assert!(journal(repo.path()).exists());
                if foreign == "index-lock" {
                    assert_eq!(fs::read(repo.path().join(".git/index.lock"))?, b"foreign lock bytes\n");
                }
            }
            Ok(())
        }

        #[test]
        fn linked_worktree_journal_is_private_to_its_git_directory() -> Result<()> {
            let repo = fixture("switch-linked-recovery")?;
            let linked = tempfile::tempdir()?;
            git(
                repo.path(),
                &[
                    "worktree",
                    "add",
                    "--detach",
                    linked.path().to_str().expect("temporary path"),
                    "main",
                ],
            )?;
            let private = String::from_utf8(git(linked.path(), &["rev-parse", "--absolute-git-dir"])?)?;
            let primary_index = index(repo.path())?;
            let primary_head = git(repo.path(), &["rev-parse", "HEAD"])?;
            let mut paused = pause(linked.path(), &["switch", "destination"], "file-published")?;
            interrupt(&mut paused, "-KILL")?;
            assert!(Path::new(private.trim()).join("brit-switch/manifest.json").exists());
            assert!(
                !journal(repo.path()).exists(),
                "journal is never shared or stored in worktree"
            );
            ensure!(
                !run(repo.path(), true, &["add", "--", "neighbor"])?.status.success(),
                "shared mutation guard observes linked journal"
            );
            success(linked.path(), &["switch", "--continue"])?;
            unchanged(repo.path(), &primary_index, &primary_head)?;
            assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
            assert_eq!(fs::read(linked.path().join("selected"))?, b"destination\n");
            Ok(())
        }

        #[test]
        fn pending_journal_blocks_mutations_but_keeps_read_only_inspection_available() -> Result<()> {
            use std::os::unix::fs::PermissionsExt;
            let repo = fixture("switch-pending")?;
            let outside = tempfile::tempdir()?;
            let store = outside.path().join("store");
            let sealed = success(
                repo.path(),
                &["snapshot", "seal", "--store", store.to_str().expect("temporary path")],
            )?;
            let sealed: serde_json::Value = serde_json::from_slice(&sealed.stdout)?;
            let root = sealed["root"].as_str().expect("sealed root");
            let expected_tree = sealed["gitTree"].as_str().expect("sealed Git tree");
            let mut paused = pause(repo.path(), &["switch", "destination"], "journal-prepared")?;
            interrupt(&mut paused, "-KILL")?;
            let before = index(repo.path())?;
            let head = git(repo.path(), &["rev-parse", "HEAD"])?;
            for args in [
                vec!["add", "--", "neighbor"],
                vec!["commit", "-m", "must not commit"],
                vec!["restore", "--worktree", "--", "neighbor"],
                vec!["build", "baseline", "write", "switch-test", "HEAD"],
                vec![
                    "snapshot",
                    "export-git",
                    "--root",
                    root,
                    "--store",
                    store.to_str().expect("temporary path"),
                    "--repo",
                    repo.path().to_str().expect("temporary path"),
                    "--expected-tree",
                    expected_tree,
                ],
            ] {
                let out = run(repo.path(), true, &args)?;
                ensure!(!out.status.success(), "pending switch blocks {args:?}");
                let diagnostic = String::from_utf8_lossy(&out.stderr);
                ensure!(
                    diagnostic.contains("--continue") && diagnostic.contains("--abort"),
                    "recovery guidance for {args:?}: {diagnostic}"
                );
                unchanged(repo.path(), &before, &head)?;
            }
            let fake_epr = outside.path().join("epr");
            fs::write(&fake_epr, "#!/bin/sh\necho invoked > epr-invoked\n")?;
            fs::set_permissions(&fake_epr, fs::Permissions::from_mode(0o755))?;
            let out = command(outside.path(), true)
                .env("PATH", outside.path())
                .args(["integrate", "--root", repo.path().to_str().expect("temporary path")])
                .output()?;
            ensure!(!out.status.success(), "forwarded root must be guarded");
            ensure!(
                String::from_utf8_lossy(&out.stderr).contains("--continue"),
                "integration must name recovery"
            );
            assert!(
                !outside.path().join("epr-invoked").exists(),
                "pending switch prevents child invocation"
            );
            let names = outside.path().join("names");
            fs::write(&names, "foreign-file\n")?;
            let out = command(outside.path(), true)
                .args(["free", "index", "from-list", "--force", "--index-output-path"])
                .arg(repo.path().join(".git/index"))
                .arg(&names)
                .output()?;
            ensure!(
                !out.status.success(),
                "free index writer must guard explicit output repository"
            );
            ensure!(
                String::from_utf8_lossy(&out.stderr).contains("--continue"),
                "explicit index output must name recovery"
            );
            unchanged(repo.path(), &before, &head)?;
            let surface = success(outside.path(), &["--cli-surface-json"])?;
            let surface: serde_json::Value = serde_json::from_slice(&surface.stdout)?;
            if surface["command"]["subcommands"]
                .as_array()
                .expect("public commands")
                .iter()
                .any(|cmd| cmd["name"] == "corpus")
            {
                let database = repo.path().join("guarded-corpus.db");
                let out = command(outside.path(), true)
                    .args(["corpus", "--db"])
                    .arg(&database)
                    .args(["--path"])
                    .arg(outside.path())
                    .args(["run", "--dry-run"])
                    .output()?;
                ensure!(
                    !out.status.success(),
                    "corpus dry-run still writes its explicit database"
                );
                ensure!(
                    String::from_utf8_lossy(&out.stderr).contains("--continue"),
                    "corpus database writer must name recovery"
                );
                assert!(
                    !database.exists(),
                    "no SQLite database may be created inside pending repository"
                );
            }
            if surface["command"]["subcommands"]
                .as_array()
                .expect("public commands")
                .iter()
                .any(|cmd| cmd["name"] == "tix")
            {
                let other = fixture("switch-cross-repo-output")?;
                let destination = repo.path().join("guarded-tix-output");
                let output_flag = format!("--materialize-conflicts={}", destination.display());
                for args in [
                    vec![
                        "tix",
                        "transplant",
                        "HEAD",
                        "--copy",
                        "--fork",
                        "--above",
                        "HEAD~1",
                        &output_flag,
                    ],
                    vec!["tix", "rebase", "apply", &output_flag],
                    vec!["tix", "rebase", "continue", &output_flag],
                    vec!["tix", "rebase", "todo", "--edit-and-apply", &output_flag],
                    vec![
                        "tix",
                        "worktrunk",
                        "switch",
                        "--detach",
                        "HEAD",
                        "--path",
                        destination.to_str().expect("temporary path"),
                    ],
                ] {
                    let out = run(other.path(), true, &args)?;
                    ensure!(!out.status.success(), "cross-repository output must refuse: {args:?}");
                    ensure!(
                        String::from_utf8_lossy(&out.stderr).contains("--continue"),
                        "output guard must name recovery: {args:?}: {}",
                        String::from_utf8_lossy(&out.stderr)
                    );
                    assert!(
                        !destination.exists(),
                        "Tix must not create output in pending repository"
                    );
                    unchanged(repo.path(), &before, &head)?;
                }
            }
            success(repo.path(), &["status"])?;
            unchanged(repo.path(), &before, &head)?;
            success(repo.path(), &["log"])?;
            unchanged(repo.path(), &before, &head)?;
            success(repo.path(), &["switch", "--abort"])?;
            Ok(())
        }

        #[test]
        fn crash_during_running_hook_reports_unknown_and_never_replays_it() -> Result<()> {
            use std::os::unix::fs::PermissionsExt;
            let repo = fixture("switch-running-hook")?;
            let _release_hook = ReleaseHook(repo.path().join("hook-release"));
            let hook = repo.path().join(".git/hooks/post-checkout");
            fs::write(&hook, "#!/bin/sh\necho invoked >> hook-count\necho hook-edited > selected\necho $$ > hook-pid\nwhile test ! -f hook-release; do sleep 0.01; done\n")?;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
            let child = command(repo.path(), true)
                .args(["switch", "destination"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let mut running = Paused {
                child,
                _markers: tempfile::tempdir()?,
            };
            let deadline = Instant::now() + Duration::from_secs(30);
            while !repo.path().join("hook-pid").exists() {
                ensure!(
                    running.child.try_wait()?.is_none(),
                    "switch exited before invoking hook"
                );
                ensure!(Instant::now() < deadline, "hook never started");
                std::thread::sleep(Duration::from_millis(5));
            }
            interrupt(&mut running, "-KILL")?;
            // Release the orphaned hook so no process escapes this disposable fixture.
            fs::write(repo.path().join("hook-release"), "release\n")?;
            let status = success(repo.path(), &["switch", "--status"])?;
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&status.stdout),
                String::from_utf8_lossy(&status.stderr)
            );
            ensure!(
                diagnostic.contains("unknown"),
                "interrupted hook outcome must be explicit: {diagnostic}"
            );
            success(repo.path(), &["switch", "--continue"])?;
            assert_eq!(
                fs::read(repo.path().join("hook-count"))?,
                b"invoked\n",
                "never replay a started hook"
            );
            assert_eq!(fs::read(repo.path().join("selected"))?, b"hook-edited\n");
            Ok(())
        }

        #[test]
        fn sigint_during_running_hook_preserves_commit_and_never_replays_hook() -> Result<()> {
            use std::os::unix::{fs::PermissionsExt, process::CommandExt};
            let repo = fixture("switch-interrupted-hook")?;
            let _release_hook = ReleaseHook(repo.path().join("hook-release"));
            let hook = repo.path().join(".git/hooks/post-checkout");
            fs::write(&hook, "#!/bin/sh\ntrap 'exit 130' INT\necho invoked >> hook-count\necho hook-edited > selected\necho ready > hook-ready\nwhile test ! -f hook-release; do sleep 0.01; done\n")?;
            fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))?;
            let child = command(repo.path(), true)
                .process_group(0)
                .args(["switch", "destination"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let mut running = Paused {
                child,
                _markers: tempfile::tempdir()?,
            };
            let deadline = Instant::now() + Duration::from_secs(30);
            while !repo.path().join("hook-ready").exists() {
                ensure!(
                    running.child.try_wait()?.is_none(),
                    "switch exited before hook readiness"
                );
                ensure!(Instant::now() < deadline, "hook did not reach deterministic readiness");
                std::thread::sleep(Duration::from_millis(5));
            }
            ensure!(Command::new("kill")
                .args(["-INT", "--", &format!("-{}", running.child.id())])
                .status()?
                .success());
            let deadline = Instant::now() + Duration::from_secs(30);
            loop {
                if let Some(status) = running.child.try_wait()? {
                    ensure!(!status.success(), "SIGINT must report interrupted hook");
                    break;
                }
                ensure!(Instant::now() < deadline, "interrupted hook did not exit");
                std::thread::sleep(Duration::from_millis(5));
            }
            fs::write(repo.path().join("hook-release"), "release\n")?;
            let status = success(repo.path(), &["switch", "--status"])?;
            let diagnostic = format!(
                "{}{}",
                String::from_utf8_lossy(&status.stdout),
                String::from_utf8_lossy(&status.stderr)
            );
            ensure!(
                diagnostic.contains("failed") || diagnostic.contains("unknown"),
                "hook outcome must be truthful: {diagnostic}"
            );
            let recovered = run(repo.path(), true, &["switch", "--continue"])?;
            if !recovered.status.success() {
                ensure!(
                    String::from_utf8_lossy(&recovered.stderr).contains("hook failed"),
                    "known hook failure is preserved"
                );
            }
            assert_eq!(fs::read(repo.path().join("hook-count"))?, b"invoked\n");
            assert_eq!(fs::read(repo.path().join("selected"))?, b"hook-edited\n");
            assert_eq!(
                git(repo.path(), &["symbolic-ref", "HEAD"])?,
                b"refs/heads/destination\n"
            );
            assert!(!journal(repo.path()).exists());
            Ok(())
        }

        #[test]
        fn foreign_and_symlinked_journals_refuse_without_deleting_evidence() -> Result<()> {
            use std::os::unix::fs::{symlink, PermissionsExt};
            for shape in ["foreign", "journal-symlink", "manifest-symlink"] {
                let repo = fixture("switch-journal-boundaries")?;
                let mut paused = pause(repo.path(), &["switch", "destination"], "journal-prepared")?;
                interrupt(&mut paused, "-KILL")?;
                let outside = tempfile::tempdir()?;
                let (recovery_root, retained) = if shape == "foreign" {
                    let other = fixture("switch-foreign-journal")?;
                    // Move this disposable fixture under the retained temporary directory.
                    let foreign_path = outside.path().join("foreign-repository");
                    fs::rename(other.path(), &foreign_path)?;
                    let target = journal(&foreign_path);
                    fs::create_dir(&target)?;
                    fs::set_permissions(&target, fs::Permissions::from_mode(0o700))?;
                    fs::copy(journal(repo.path()).join("manifest.json"), target.join("manifest.json"))?;
                    (foreign_path, target.join("manifest.json"))
                } else if shape == "journal-symlink" {
                    let retained = outside.path().join("private-journal");
                    fs::rename(journal(repo.path()), &retained)?;
                    symlink(&retained, journal(repo.path()))?;
                    (repo.path().to_owned(), retained.join("manifest.json"))
                } else {
                    let retained = outside.path().join("private-manifest.json");
                    fs::rename(journal(repo.path()).join("manifest.json"), &retained)?;
                    symlink(&retained, journal(repo.path()).join("manifest.json"))?;
                    (repo.path().to_owned(), retained)
                };
                let evidence = fs::read(&retained)?;
                let before = index(&recovery_root)?;
                let head = git(&recovery_root, &["rev-parse", "HEAD"])?;
                for action in ["--continue", "--abort"] {
                    ensure!(
                        !run(&recovery_root, true, &["switch", action])?.status.success(),
                        "must reject {shape}"
                    );
                    unchanged(&recovery_root, &before, &head)?;
                    assert_eq!(fs::read(&retained)?, evidence, "never delete foreign recovery evidence");
                }
            }
            Ok(())
        }

        #[test]
        fn private_journal_corruption_refuses_and_never_deletes_evidence() -> Result<()> {
            use std::os::unix::fs::PermissionsExt;
            for corrupt in ["manifest", "backup"] {
                let repo = fixture("switch-corrupt")?;
                let mut paused = pause(repo.path(), &["switch", "destination"], "journal-prepared")?;
                interrupt(&mut paused, "-KILL")?;
                let dir = journal(repo.path());
                assert_eq!(
                    fs::metadata(&dir)?.permissions().mode() & 0o077,
                    0,
                    "journal is private"
                );
                let files: Vec<_> = fs::read_dir(&dir)?.collect::<std::io::Result<Vec<_>>>()?;
                for file in &files {
                    assert_eq!(
                        file.metadata()?.permissions().mode() & 0o077,
                        0,
                        "recovery bytes are private"
                    );
                }
                let path = dir.join("manifest.json");
                let corrupted = if corrupt == "manifest" {
                    b"corrupt recovery evidence\n".to_vec()
                } else {
                    // Backups are exact bytes inside the checksum-protected payload.
                    let mut envelope: serde_json::Value = serde_json::from_slice(&fs::read(&path)?)?;
                    let payload = envelope["payload"].as_array_mut().expect("journal payload bytes");
                    let byte = payload.last_mut().expect("nonempty recovery payload");
                    *byte = serde_json::json!(byte.as_u64().expect("byte") ^ 1);
                    serde_json::to_vec(&envelope)?
                };
                fs::write(&path, &corrupted)?;
                let before = index(repo.path())?;
                let head = git(repo.path(), &["rev-parse", "HEAD"])?;
                for recovery in ["--continue", "--abort"] {
                    ensure!(
                        !run(repo.path(), true, &["switch", recovery])?.status.success(),
                        "corrupt {corrupt} must refuse {recovery}"
                    );
                    unchanged(repo.path(), &before, &head)?;
                    assert_eq!(fs::read(&path)?, corrupted);
                }
            }
            Ok(())
        }
    }

    #[test]
    fn directory_transitions_and_deletions_match_git_and_refuse_dirty_descendants() -> Result<()> {
        for direction in ["file-to-directory", "directory-to-file", "deletion"] {
            for dirty in [false, true] {
                let actual = fixture("switch-directories")?;
                let oracle = fixture("switch-directories-oracle")?;
                for repo in [&actual, &oracle] {
                    let source_path = if direction == "directory-to-file" {
                        "shape/child"
                    } else {
                        "shape"
                    };
                    repo.commit_file(source_path, "source\n")?;
                    git(repo.path(), &["switch", "destination"])?;
                    if direction == "file-to-directory" {
                        repo.commit_file("shape/child", "destination\n")?;
                    }
                    if direction == "directory-to-file" {
                        repo.commit_file("shape", "destination\n")?;
                    }
                    git(repo.path(), &["switch", "main"])?;
                    if dirty {
                        fs::write(repo.path().join(source_path), "dirty ancestor or descendant\n")?;
                    }
                }
                if dirty {
                    let before = index(actual.path())?;
                    let head = git(actual.path(), &["rev-parse", "HEAD"])?;
                    ensure!(!run(actual.path(), true, &["switch", "destination"])?.status.success());
                    unchanged(actual.path(), &before, &head)?;
                    let source_path = if direction == "directory-to-file" {
                        "shape/child"
                    } else {
                        "shape"
                    };
                    assert_eq!(
                        fs::read(actual.path().join(source_path))?,
                        b"dirty ancestor or descendant\n"
                    );
                } else {
                    success(actual.path(), &["switch", "destination"])?;
                    git(oracle.path(), &["switch", "destination"])?;
                    compare(actual.path(), oracle.path())?;
                    for path in ["shape", "shape/child"] {
                        assert_eq!(
                            fs::read(actual.path().join(path)).ok(),
                            fs::read(oracle.path().join(path)).ok()
                        );
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn unborn_active_operations_gitlinks_and_nested_repositories_refuse() -> Result<()> {
        for shape in ["unborn", "merge", "rebase", "gitlink", "nested"] {
            let repo = fixture("switch-boundaries")?;
            match shape {
                "unborn" => {
                    git(repo.path(), &["symbolic-ref", "HEAD", "refs/heads/unborn"])?;
                }
                "merge" => {
                    fs::write(repo.path().join(".git/MERGE_HEAD"), format!("{}\n", repo.head_id()?))?;
                }
                "rebase" => {
                    fs::create_dir(repo.path().join(".git/rebase-merge"))?;
                }
                "gitlink" => {
                    git(repo.path(), &["switch", "destination"])?;
                    let head = repo.head_id()?;
                    git(
                        repo.path(),
                        &[
                            "update-index",
                            "--add",
                            "--cacheinfo",
                            &format!("160000,{head},submodule"),
                        ],
                    )?;
                    repo.commit_file("marker", "gitlink\n")?;
                    git(repo.path(), &["switch", "main"])?;
                }
                "nested" => {
                    fs::create_dir(repo.path().join("added"))?;
                    git(&repo.path().join("added"), &["init", "-q"])?;
                    fs::write(repo.path().join("added/private"), "nested private\n")?;
                }
                _ => unreachable!(),
            }
            let before = index(repo.path())?;
            let attachment = fs::read(repo.path().join(".git/HEAD"))?;
            ensure!(
                !run(repo.path(), true, &["switch", "destination"])?.status.success(),
                "must refuse {shape}"
            );
            assert_eq!(index(repo.path())?, before);
            assert_eq!(fs::read(repo.path().join(".git/HEAD"))?, attachment);
            assert_eq!(fs::read(repo.path().join("selected"))?, b"original\n");
            if shape == "nested" {
                assert_eq!(fs::read(repo.path().join("added/private"))?, b"nested private\n");
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn byte_paths_survive_switch_without_lossy_names() -> Result<()> {
        use std::os::unix::ffi::OsStrExt;
        let repo = fixture("switch-byte-paths")?;
        let name = std::ffi::OsStr::from_bytes(b"byte-\xff");
        git(repo.path(), &["switch", "destination"])?;
        fs::write(repo.path().join(name), "byte-path destination\n")?;
        let out = command(repo.path(), false).arg("add").arg("--").arg(name).output()?;
        ensure!(out.status.success());
        repo.commit_file("marker", "byte paths\n")?;
        git(repo.path(), &["switch", "main"])?;
        success(repo.path(), &["switch", "destination"])?;
        assert_eq!(fs::read(repo.path().join(name))?, b"byte-path destination\n");
        success(repo.path(), &["switch", "main"])?;
        assert!(!repo.path().join(name).exists());
        Ok(())
    }

    #[test]
    fn unchanged_attributes_with_staged_and_unstaged_edits_control_conversion_like_git() -> Result<()> {
        let actual = TestRepo::new("switch-dirty-attributes")?;
        let oracle = TestRepo::new("switch-dirty-attributes-oracle")?;
        for repo in [&actual, &oracle] {
            repo.commit_file("selected", "original\n")?;
            repo.commit_file(".gitattributes", "selected text eol=lf\n")?;
            git(repo.path(), &["switch", "-c", "destination"])?;
            repo.commit_file("selected", "destination\n")?;
            git(repo.path(), &["switch", "main"])?;
            fs::write(repo.path().join(".gitattributes"), "selected text eol=lf\n# staged\n")?;
            git(repo.path(), &["add", ".gitattributes"])?;
            fs::write(
                repo.path().join(".gitattributes"),
                "selected text eol=crlf\n# unstaged\n",
            )?;
        }
        success(actual.path(), &["switch", "destination"])?;
        git(oracle.path(), &["switch", "destination"])?;
        compare(actual.path(), oracle.path())?;
        assert_eq!(
            fs::read(actual.path().join(".gitattributes"))?,
            fs::read(oracle.path().join(".gitattributes"))?
        );
        assert_eq!(
            fs::read(actual.path().join("selected"))?,
            b"destination\n",
            "Git uses index attributes during branch checkout, preserving dirty worktree attributes"
        );
        Ok(())
    }
}
