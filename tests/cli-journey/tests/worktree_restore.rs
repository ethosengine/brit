//! Scoped worktree restoration against stock Git, including preservation and recovery.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn command(repo: &Path, native: bool) -> Command {
    if native {
        let mut cmd = Command::new(brit_bin().expect("required Brit binary"));
        gix_testtools::configure_git_environment(&mut cmd, repo);
        cmd.current_dir(repo);
        cmd
    } else {
        gix_testtools::git_command(repo)
    }
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
fn index(repo: &Path) -> Result<Vec<u8>> {
    let path = String::from_utf8(git(repo, &["rev-parse", "--git-path", "index"])?)?;
    Ok(fs::read(repo.join(path.trim()))?)
}
fn fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("selected", "head\n")?;
    repo.commit_file("neighbor", "neighbor head\n")?;
    fs::write(repo.path().join("selected"), "index\n")?;
    git(repo.path(), &["add", "selected"])?;
    fs::write(repo.path().join("selected"), "dirty\n")?;
    fs::write(repo.path().join("neighbor"), "neighbor dirty\n")?;
    fs::write(repo.path().join("private"), "private\n")?;
    Ok(repo)
}

fn path_restore_fixture(linked: bool) -> Result<(TestRepo, PathBuf)> {
    let repo = TestRepo::new("restore-path")?;
    repo.commit_file("dir/selected", "older\n")?;
    repo.commit_file("neighbor", "neighbor head\n")?;
    repo.commit_file("dir/selected", "head\n")?;
    let checkout = if linked {
        git(repo.path(), &["worktree", "add", "--detach", "linked"])?;
        repo.commit_file("dir/selected", "primary head\n")?;
        repo.path().join("linked")
    } else {
        repo.path().to_owned()
    };
    fs::write(checkout.join("dir/selected"), "staged\n")?;
    git(&checkout, &["add", "dir/selected"])?;
    git(&checkout, &["update-index", "--chmod=+x", "dir/selected"])?;
    fs::write(checkout.join("dir/selected"), "dirty\n")?;
    fs::write(checkout.join("neighbor"), "neighbor dirty\n")?;
    fs::write(checkout.join("z-added"), "staged addition\n")?;
    git(&checkout, &["add", "z-added"])?;
    Ok((repo, checkout))
}

fn compare_restore_path(args: &[&str], changes_index: bool) -> Result<()> {
    for linked in [false, true] {
        let (primary, actual) = path_restore_fixture(linked)?;
        let (_oracle_primary, oracle) = path_restore_fixture(linked)?;
        let before = index(&actual)?;
        let primary_index = index(primary.path())?;
        git(&oracle, args)?;
        let out = run(&actual, true, args)?;
        ensure!(
            out.status.success(),
            "Brit {args:?} with linked={linked}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            git(&actual, &["ls-files", "--stage", "-z"])?,
            git(&oracle, &["ls-files", "--stage", "-z"])?,
            "index modes, oids and paths with linked={linked}"
        );
        for path in ["dir/selected", "neighbor", "z-added"] {
            assert_eq!(
                fs::read(actual.join(path))?,
                fs::read(oracle.join(path))?,
                "{path} bytes with linked={linked}"
            );
        }
        if !changes_index {
            assert_eq!(
                index(&actual)?,
                before,
                "worktree-only restore leaves the index file untouched"
            );
        }
        if linked {
            assert_eq!(index(primary.path())?, primary_index, "the primary index is untouched");
            assert_eq!(
                fs::read(primary.path().join("dir/selected"))?,
                b"primary head\n",
                "linked restore leaves the primary working file untouched"
            );
        }
    }
    Ok(())
}

#[test]
fn restore_path_from_index_matches_git() -> Result<()> {
    compare_restore_path(&["restore", "dir/selected"], false)
}

#[test]
fn restore_path_from_revision_matches_git() -> Result<()> {
    compare_restore_path(&["restore", "--source", "HEAD~2", "dir/selected"], false)
}

#[test]
fn restore_staged_path_matches_git() -> Result<()> {
    compare_restore_path(&["restore", "--staged", "dir/selected"], true)
}

#[test]
fn restore_staged_and_worktree_path_matches_git() -> Result<()> {
    compare_restore_path(&["restore", "--staged", "--worktree", "dir/selected"], true)?;
    compare_restore_path(
        &[
            "-C",
            "dir",
            "restore",
            "--staged",
            "--worktree",
            "--source=HEAD~2",
            "selected",
        ],
        true,
    )?;
    compare_restore_path(&["restore", "--staged", "--source=HEAD~2", "dir/selected"], true)
}

#[test]
fn restore_path_refuses_missing_and_outside_paths_without_writes() -> Result<()> {
    let outside = tempfile::NamedTempFile::new()?;
    fs::write(outside.path(), "outside\n")?;
    let outside_path = outside.path().to_str().expect("temporary path");
    for linked in [false, true] {
        let (_primary, actual) = path_restore_fixture(linked)?;
        let (_oracle_primary, oracle) = path_restore_fixture(linked)?;
        for options in [
            &[][..],
            &["--source", "HEAD~2"],
            &["--staged"],
            &["--staged", "--worktree"],
        ] {
            for tail in ["absent", "../outside", outside_path] {
                let args: Vec<_> = ["restore"]
                    .into_iter()
                    .chain(options.iter().copied())
                    .chain(["dir/selected", tail])
                    .collect();
                for (checkout, native) in [(&actual, true), (&oracle, false)] {
                    let before = index(checkout)?;
                    let out = run(checkout, native, &args)?;
                    ensure!(
                        !out.status.success(),
                        "{args:?} must refuse with linked={linked}, Brit={native}"
                    );
                    assert_eq!(index(checkout)?, before, "refusal leaves the index untouched");
                    assert_eq!(
                        fs::read(checkout.join("dir/selected"))?,
                        b"dirty\n",
                        "refusal leaves the valid path untouched"
                    );
                    assert!(!checkout.join("absent").exists(), "refusal creates no file");
                    assert_eq!(
                        fs::read(outside.path())?,
                        b"outside\n",
                        "refusal preserves outside files"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn restore_path_absent_from_source_refuses_even_if_in_index() -> Result<()> {
    for linked in [false, true] {
        let (_primary, checkout) = path_restore_fixture(linked)?;
        let before = index(&checkout)?;
        for options in [&["--source", "HEAD~2"][..], &["--staged"], &["--staged", "--worktree"]] {
            let args: Vec<_> = ["restore"]
                .into_iter()
                .chain(options.iter().copied())
                .chain(["--", "dir/selected", "z-added"])
                .collect();
            let out = run(&checkout, true, &args)?;
            ensure!(!out.status.success(), "{args:?} must refuse with linked={linked}");
            assert_eq!(index(&checkout)?, before, "refusal leaves the index untouched");
            assert_eq!(
                fs::read(checkout.join("dir/selected"))?,
                b"dirty\n",
                "refusal leaves the valid path untouched"
            );
            assert_eq!(
                fs::read(checkout.join("z-added"))?,
                b"staged addition\n",
                "refusal preserves the staged addition in the worktree"
            );
        }
    }
    Ok(())
}

fn compare(actual: &Path, oracle: &Path, args: &[&str], paths: &[&str]) -> Result<()> {
    let before = index(actual)?;
    let head = git(actual, &["rev-parse", "HEAD"])?;
    let expected = run(oracle, false, args)?;
    let result = run(actual, true, args)?;
    ensure!(
        expected.status.success(),
        "Git oracle: {}",
        String::from_utf8_lossy(&expected.stderr)
    );
    ensure!(
        result.status.success(),
        "Brit: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(index(actual)?, before, "restore preserves exact index bytes");
    assert_eq!(git(actual, &["rev-parse", "HEAD"])?, head, "restore preserves HEAD");
    for path in paths {
        let a = actual.join(path);
        let o = oracle.join(path);
        assert_eq!(
            fs::symlink_metadata(&a).is_ok(),
            fs::symlink_metadata(&o).is_ok(),
            "existence of {path}"
        );
        if let Ok(meta) = fs::symlink_metadata(&a) {
            if meta.file_type().is_symlink() {
                assert_eq!(fs::read_link(&a)?, fs::read_link(&o)?, "symlink target {path}");
            } else {
                assert_eq!(fs::read(&a)?, fs::read(&o)?, "file bytes {path}");
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    assert_eq!(
                        meta.permissions().mode() & 0o111,
                        fs::metadata(&o)?.permissions().mode() & 0o111,
                        "executable mode {path}"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn combined_restore_matches_git_conversion_when_restoring_attributes() -> Result<()> {
    let actual = fixture("restore-both-attributes")?;
    let oracle = fixture("restore-both-attributes-oracle")?;
    for repo in [&actual, &oracle] {
        repo.commit_file(".gitattributes", "selected text eol=crlf\n")?;
        fs::write(repo.path().join(".gitattributes"), "selected text eol=lf\n")?;
        git(repo.path(), &["add", ".gitattributes"])?;
    }
    let args = ["restore", "--staged", "--worktree", "--", ".gitattributes", "selected"];
    git(oracle.path(), &args)?;
    let result = run(actual.path(), true, &args)?;
    ensure!(result.status.success(), "restore both with attributes: {result:?}");
    assert_eq!(
        fs::read(actual.path().join(".gitattributes"))?,
        b"selected text eol=crlf\n",
        "combined restore reinstates HEAD's attributes"
    );
    assert_eq!(
        fs::read(actual.path().join("selected"))?,
        fs::read(oracle.path().join("selected"))?
    );
    assert_eq!(
        git(actual.path(), &["ls-files", "--stage"])?,
        git(oracle.path(), &["ls-files", "--stage"])?
    );
    Ok(())
}

#[test]
fn staged_and_worktree_restore_head_together_and_preserve_neighbors() -> Result<()> {
    for explicit_source in [false, true] {
        let actual = fixture("restore-both")?;
        let oracle = fixture("restore-both-oracle")?;
        for repo in [&actual, &oracle] {
            git(repo.path(), &["add", "neighbor"])?;
            fs::write(repo.path().join("neighbor"), "neighbor unstaged\n")?;
        }
        let head = git(actual.path(), &["rev-parse", "HEAD"])?;
        let mut args = vec!["restore", "--staged", "--worktree"];
        if explicit_source {
            args.push("--source=HEAD");
        }
        args.extend(["--", "selected"]);
        git(oracle.path(), &args)?;
        let result = run(actual.path(), true, &args)?;
        ensure!(result.status.success(), "restore both: {result:?}");
        assert_eq!(
            git(actual.path(), &["ls-files", "--stage"])?,
            git(oracle.path(), &["ls-files", "--stage"])?
        );
        assert_eq!(git(actual.path(), &["rev-parse", "HEAD"])?, head);
        for name in ["selected", "neighbor", "private"] {
            assert_eq!(fs::read(actual.path().join(name))?, fs::read(oracle.path().join(name))?);
        }
    }
    Ok(())
}

#[test]
fn combined_restore_refuses_before_writes_for_foreign_lock_or_invalid_selection() -> Result<()> {
    for locked in [false, true] {
        let repo = fixture("restore-both-refusal")?;
        let before = index(repo.path())?;
        if locked {
            fs::write(repo.path().join(".git/index.lock"), "foreign lock")?;
        }
        let mut args = vec!["restore", "--staged", "--worktree", "--", "selected"];
        if !locked {
            args.push("absent");
        }
        let output = run(repo.path(), true, &args)?;
        ensure!(!output.status.success(), "unsafe selection must refuse");
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n");
        if locked {
            assert_eq!(fs::read(repo.path().join(".git/index.lock"))?, b"foreign lock");
        }
    }
    Ok(())
}

#[test]
fn both_sources_preserve_dirty_neighbors_and_index() -> Result<()> {
    for source in [None, Some("--source=HEAD")] {
        let actual = fixture("restore-source")?;
        let oracle = fixture("restore-source-oracle")?;
        let mut args = vec!["restore", "--worktree"];
        args.extend(source);
        args.extend(["--", "selected"]);
        compare(
            actual.path(),
            oracle.path(),
            &args,
            &["selected", "neighbor", "private"],
        )?;
    }
    Ok(())
}

#[test]
fn deletions_and_missing_parent_directories_match_git() -> Result<()> {
    for source in [None, Some("--source=HEAD")] {
        let actual = fixture("restore-delete")?;
        let oracle = fixture("restore-delete-oracle")?;
        for repo in [&actual, &oracle] {
            repo.commit_file("nested/gone", "nested head\n")?;
            fs::remove_dir_all(repo.path().join("nested"))?;
            fs::remove_file(repo.path().join("selected"))?;
        }
        let mut args = vec!["restore", "--worktree"];
        args.extend(source);
        args.extend(["--", "selected", "nested/gone"]);
        compare(
            actual.path(),
            oracle.path(),
            &args,
            &["selected", "nested/gone", "neighbor", "private"],
        )?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn executable_modes_and_symlink_type_changes_match_git() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for source in [None, Some("--source=HEAD")] {
        let actual = fixture("restore-types")?;
        let oracle = fixture("restore-types-oracle")?;
        for repo in [&actual, &oracle] {
            symlink("private", repo.path().join("link"))?;
            git(repo.path(), &["add", "link"])?;
            git(repo.path(), &["update-index", "--chmod=+x", "selected"])?;
            // Commit via TestRepo so the fixture supplies its isolated identity.
            repo.commit_file("marker", "marker\n")?;
            fs::remove_file(repo.path().join("link"))?;
            fs::write(repo.path().join("link"), "dirty regular file\n")?;
            fs::remove_file(repo.path().join("selected"))?;
            symlink("private", repo.path().join("selected"))?;
            fs::set_permissions(repo.path().join("marker"), fs::Permissions::from_mode(0o755))?;
        }
        let mut args = vec!["restore", "--worktree"];
        args.extend(source);
        args.extend(["--", "selected", "link", "marker"]);
        compare(
            actual.path(),
            oracle.path(),
            &args,
            &["selected", "link", "marker", "private"],
        )?;
    }
    Ok(())
}

#[test]
fn linked_worktrees_and_alternate_indexes_remain_isolated() -> Result<()> {
    let repo = fixture("restore-isolation")?;
    let linked = tempfile::tempdir()?;
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "--detach",
            linked.path().to_str().expect("temporary path"),
        ],
    )?;
    fs::write(linked.path().join("selected"), "linked dirty\n")?;
    let primary = index(repo.path())?;
    let linked_index = index(linked.path())?;
    let out = run(linked.path(), true, &["restore", "--worktree", "--", "selected"])?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read(linked.path().join("selected"))?, b"head\n");
    assert_eq!(index(linked.path())?, linked_index);
    assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n");
    assert_eq!(index(repo.path())?, primary);
    let alt = repo.path().join(".git/alternate-index");
    fs::write(&alt, &primary)?;
    fs::write(repo.path().join("selected"), "alternate\n")?;
    ensure!(command(repo.path(), false)
        .env("GIT_INDEX_FILE", &alt)
        .args(["add", "selected"])
        .status()?
        .success());
    fs::write(repo.path().join("selected"), "dirty again\n")?;
    let alt_before = fs::read(&alt)?;
    let out = command(repo.path(), true)
        .env("GIT_INDEX_FILE", &alt)
        .args(["restore", "--worktree", "--", "selected"])
        .output()?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(fs::read(repo.path().join("selected"))?, b"alternate\n");
    assert_eq!(fs::read(&alt)?, alt_before);
    assert_eq!(index(repo.path())?, primary);
    Ok(())
}

#[test]
fn invalid_whole_selections_refuse_before_changing_any_file() -> Result<()> {
    let repo = fixture("restore-preflight")?;
    let before = index(repo.path())?;
    for tail in ["missing", ".", "../outside", "*.txt", ":(literal)selected", "neighbor/"] {
        let out = run(repo.path(), true, &["restore", "--worktree", "--", "selected", tail])?;
        ensure!(!out.status.success(), "must refuse literal selection {tail}");
        assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n");
        assert_eq!(index(repo.path())?, before);
    }
    for args in [
        vec!["restore", "--worktree"],
        vec!["restore", "--worktree", "--source=missing-revision", "--", "selected"],
        vec!["restore", "--worktree", "--source=HEAD:selected", "--", "selected"],
    ] {
        ensure!(!run(repo.path(), true, &args)?.status.success(), "must refuse {args:?}");
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n");
    }
    Ok(())
}

#[test]
fn unsupported_index_entries_and_external_filters_refuse_whole_selection() -> Result<()> {
    for shape in [
        "intent",
        "skip",
        "assume",
        "conflict",
        "submodule",
        "filter",
        "directory",
    ] {
        let repo = fixture("restore-unsupported")?;
        repo.commit_file("z-last", "last\n")?;
        fs::write(repo.path().join("selected"), "dirty\n")?;
        match shape {
            "intent" => {
                fs::write(repo.path().join("new"), "intent\n")?;
                git(repo.path(), &["add", "-N", "new"])?;
            }
            "skip" => {
                git(repo.path(), &["update-index", "--skip-worktree", "z-last"])?;
            }
            "assume" => {
                git(repo.path(), &["update-index", "--assume-unchanged", "z-last"])?;
            }
            "conflict" => {
                use std::io::Write;
                use std::process::Stdio;
                let blob = String::from_utf8(git(repo.path(), &["rev-parse", "HEAD:z-last"])?)?;
                git(repo.path(), &["update-index", "--force-remove", "z-last"])?;
                let mut child = command(repo.path(), false)
                    .args(["update-index", "--index-info"])
                    .stdin(Stdio::piped())
                    .spawn()?;
                child.stdin.take().expect("piped stdin").write_all(
                    format!("100644 {} 1\tz-last\n100644 {} 2\tz-last\n", blob.trim(), blob.trim()).as_bytes(),
                )?;
                ensure!(child.wait()?.success());
            }
            "submodule" => {
                let head = repo.head_id()?;
                git(
                    repo.path(),
                    &["update-index", "--add", "--cacheinfo", &format!("160000,{head},z-last")],
                )?;
            }
            "filter" => {
                fs::write(repo.path().join(".gitattributes"), "z-last filter=external\n")?;
                git(
                    repo.path(),
                    &["config", "filter.external.smudge", "touch filter-ran; cat"],
                )?;
            }
            "directory" => {
                fs::remove_file(repo.path().join("z-last"))?;
                fs::create_dir(repo.path().join("z-last"))?;
                fs::write(repo.path().join("z-last/private"), "preserve\n")?;
            }
            _ => unreachable!(),
        }
        let before = index(repo.path())?;
        let last = if shape == "intent" { "new" } else { "z-last" };
        let out = run(repo.path(), true, &["restore", "--worktree", "--", "selected", last])?;
        ensure!(!out.status.success(), "must refuse {shape}");
        assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n", "preflight {shape}");
        assert_eq!(index(repo.path())?, before, "index {shape}");
        assert!(
            !repo.path().join("filter-ran").exists(),
            "external filter must never execute"
        );
    }
    Ok(())
}

#[test]
fn staged_deletion_without_index_source_refuses_and_head_can_recover_it() -> Result<()> {
    let actual = fixture("restore-staged-delete")?;
    let oracle = fixture("restore-staged-delete-oracle")?;
    for repo in [&actual, &oracle] {
        git(repo.path(), &["rm", "--cached", "-f", "selected"])?;
    }
    let before = index(actual.path())?;
    for (repo, native) in [(actual.path(), true), (oracle.path(), false)] {
        ensure!(!run(repo, native, &["restore", "--worktree", "--", "selected"])?
            .status
            .success());
        assert_eq!(fs::read(repo.join("selected"))?, b"dirty\n");
    }
    assert_eq!(index(actual.path())?, before);
    compare(
        actual.path(),
        oracle.path(),
        &["restore", "--worktree", "--source=HEAD", "--", "selected"],
        &["selected", "neighbor", "private"],
    )?;
    Ok(())
}

#[test]
fn built_in_crlf_conversion_and_literal_spaces_match_git() -> Result<()> {
    for source in [None, Some("--source=HEAD")] {
        let actual = fixture("restore-crlf")?;
        let oracle = fixture("restore-crlf-oracle")?;
        for repo in [&actual, &oracle] {
            repo.commit_file(".gitattributes", "* text eol=crlf\n")?;
            repo.commit_file("a space.txt", "first\nsecond\n")?;
            fs::write(repo.path().join("a space.txt"), "dirty\n")?;
        }
        let mut args = vec!["restore", "--worktree"];
        args.extend(source);
        args.extend(["--", "a space.txt"]);
        compare(
            actual.path(),
            oracle.path(),
            &args,
            &["a space.txt", "neighbor", "private"],
        )?;
        assert_eq!(fs::read(actual.path().join("a space.txt"))?, b"first\r\nsecond\r\n");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
struct RestoreChild(Option<std::process::Child>);

#[cfg(target_os = "linux")]
impl std::ops::Deref for RestoreChild {
    type Target = std::process::Child;
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("live restore child")
    }
}
#[cfg(target_os = "linux")]
impl std::ops::DerefMut for RestoreChild {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("live restore child")
    }
}
#[cfg(target_os = "linux")]
impl RestoreChild {
    fn wait_with_output(mut self) -> std::io::Result<Output> {
        self.0.take().expect("live restore child").wait_with_output()
    }
}
#[cfg(target_os = "linux")]
impl Drop for RestoreChild {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Stop a real restore while its large temporary file is being written. This
/// synchronizes on the filesystem publication primitive, with no production
/// failpoints, filters, or timing-only sleeps.
#[cfg(target_os = "linux")]
fn suspended_restore(partial: bool) -> Result<(TestRepo, RestoreChild, Vec<u8>)> {
    // The child can finish its write between observing the temporary file and
    // delivery of SIGSTOP, especially on the emulated i386 runner. That means
    // the fixture was never established. Start a fresh fixture in that case;
    // errors and the caller's cancellation/publication assertions never retry.
    for _ in 0..8 {
        if let Some(fixture) = suspended_restore_attempt(partial)? {
            return Ok(fixture);
        }
    }
    anyhow::bail!("large restore published before suspension in all eight fixture attempts")
}

#[cfg(target_os = "linux")]
fn suspended_restore_attempt(partial: bool) -> Result<Option<(TestRepo, RestoreChild, Vec<u8>)>> {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    let repo = fixture("restore-publication")?;
    // A large compressed blob makes the temporary-write window observable even
    // on fast storage; the repository itself remains small.
    let large = vec![b'x'; 128 * 1024 * 1024];
    fs::write(repo.path().join("z-large"), &large)?;
    drop(large);
    git(repo.path(), &["add", "z-large"])?;
    fs::write(repo.path().join("z-large"), "large dirty\n")?;
    let before = index(repo.path())?;
    let mut cmd = command(repo.path(), true);
    cmd.args(["restore", "--worktree", "--"]);
    if partial {
        cmd.arg("selected");
    }
    cmd.arg("z-large").stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = RestoreChild(Some(cmd.spawn()?));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let temporary = fs::read_dir(repo.path())?.filter_map(Result::ok).any(|entry| {
            entry.file_name().to_string_lossy().starts_with(".brit-restore-")
                && entry.metadata().is_ok_and(|meta| meta.len() > 1024 * 1024)
        });
        if temporary {
            let pid = libc::pid_t::try_from(child.id())?;
            // Signal the child directly: spawning `kill` can take longer than
            // the remaining write on a busy/emulated 32-bit runner.
            // SAFETY: the positive PID names the live child retained above.
            ensure!(
                unsafe { libc::kill(pid, libc::SIGSTOP) } == 0,
                "could not stop restore: {}",
                std::io::Error::last_os_error()
            );
            // SIGSTOP is asynchronous. Observe the kernel's stopped state before
            // changing the target or queuing cancellation.
            let status_path = format!("/proc/{}/status", child.id());
            while !fs::read_to_string(&status_path)?
                .lines()
                .any(|line| line.starts_with("State:") && line.contains('T'))
            {
                ensure!(Instant::now() < deadline, "restore did not stop");
                std::thread::yield_now();
            }
            let contents = fs::read(repo.path().join("z-large"))?;
            if contents == b"large dirty\n" {
                return Ok(Some((repo, child, before)));
            }
            ensure!(
                contents.len() == 128 * 1024 * 1024 && contents.iter().all(|byte| *byte == b'x'),
                "early publication did not restore the expected object"
            );
            // Only successful early completion is a missed setup window. A
            // product error after publication must still fail this attempt.
            // SAFETY: this is the same retained child observed stopped above.
            ensure!(unsafe { libc::kill(pid, libc::SIGCONT) } == 0);
            let out = child.wait_with_output()?;
            ensure!(
                out.status.success(),
                "early restore failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            return Ok(None);
        }
        if child.try_wait()?.is_some() || Instant::now() >= deadline {
            let _ = child.kill();
            let out = child.wait_with_output()?;
            anyhow::bail!(
                "restore never exposed its temporary write: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        std::thread::yield_now();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn cancellation_before_publication_changes_nothing_and_partial_cancellation_reports_progress() -> Result<()> {
    for partial in [false, true] {
        let (repo, child, before) = suspended_restore(partial)?;
        ensure!(Command::new("kill")
            .args(["-INT", &child.id().to_string()])
            .status()?
            .success());
        ensure!(Command::new("kill")
            .args(["-CONT", &child.id().to_string()])
            .status()?
            .success());
        let out = child.wait_with_output()?;
        ensure!(!out.status.success(), "cancellation must fail");
        assert_eq!(
            fs::read(repo.path().join("selected"))?,
            if partial {
                b"index\n".as_slice()
            } else {
                b"dirty\n".as_slice()
            }
        );
        assert_eq!(fs::read(repo.path().join("z-large"))?, b"large dirty\n");
        assert_eq!(fs::read(repo.path().join("neighbor"))?, b"neighbor dirty\n");
        assert_eq!(index(repo.path())?, before);
        let diagnostic = String::from_utf8_lossy(&out.stderr);
        ensure!(
            diagnostic.contains("completed") && diagnostic.contains("remaining") && diagnostic.contains("z-large"),
            "progress report: {diagnostic}"
        );
        let completed = if partial {
            "completed paths: [\"selected\"]"
        } else {
            "completed paths: []"
        };
        ensure!(diagnostic.contains(completed), "completed set: {diagnostic}");
        ensure!(
            diagnostic.contains("remaining paths: [\"z-large\"]"),
            "remaining set: {diagnostic}"
        );
        ensure!(
            !fs::read_dir(repo.path())?
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with(".brit-restore-")),
            "temporary files are cleaned after cancellation"
        );
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn publication_failure_preserves_failed_file_neighbors_and_index_with_partial_progress() -> Result<()> {
    for partial in [false, true] {
        let (repo, child, before) = suspended_restore(partial)?;
        // A competing writer replaces the destination with a nonempty directory
        // after preflight. Rename must refuse without erasing that writer's bytes.
        fs::remove_file(repo.path().join("z-large"))?;
        fs::create_dir(repo.path().join("z-large"))?;
        fs::write(repo.path().join("z-large/private"), "concurrent owner\n")?;
        ensure!(Command::new("kill")
            .args(["-CONT", &child.id().to_string()])
            .status()?
            .success());
        let out = child.wait_with_output()?;
        ensure!(!out.status.success(), "publication failure must fail");
        assert_eq!(
            fs::read(repo.path().join("selected"))?,
            if partial {
                b"index\n".as_slice()
            } else {
                b"dirty\n".as_slice()
            }
        );
        assert_eq!(fs::read(repo.path().join("z-large/private"))?, b"concurrent owner\n");
        assert_eq!(fs::read(repo.path().join("neighbor"))?, b"neighbor dirty\n");
        assert_eq!(index(repo.path())?, before);
        let diagnostic = String::from_utf8_lossy(&out.stderr);
        ensure!(
            diagnostic.contains("completed")
                && (!partial || diagnostic.contains("selected"))
                && diagnostic.contains("remaining")
                && diagnostic.contains("z-large"),
            "partial failure report: {diagnostic}"
        );
        ensure!(
            diagnostic.contains(if partial {
                "completed paths: [\"selected\"]"
            } else {
                "completed paths: []"
            }) && diagnostic.contains("remaining paths: [\"z-large\"]"),
            "exact partial failure sets: {diagnostic}"
        );
        ensure!(
            !fs::read_dir(repo.path())?
                .filter_map(Result::ok)
                .any(|entry| entry.file_name().to_string_lossy().starts_with(".brit-restore-")),
            "temporary files are cleaned after failure"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn restrictive_umask_keeps_restored_files_private_like_git() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    for executable in [false, true] {
        let actual = fixture("restore-umask")?;
        let oracle = fixture("restore-umask-oracle")?;
        for (repo, native) in [(&actual, true), (&oracle, false)] {
            if executable {
                git(repo.path(), &["update-index", "--chmod=+x", "selected"])?;
            }
            fs::remove_file(repo.path().join("selected"))?;
            let program = command(repo.path(), native).get_program().to_os_string();
            let mut shell = Command::new("sh");
            gix_testtools::configure_git_environment(&mut shell, repo.path());
            let out = shell
                .current_dir(repo.path())
                .args(["-c", "umask 077; exec \"$@\"", "restore-private"])
                .arg(program)
                .args(["restore", "--worktree", "--", "selected"])
                .output()?;
            ensure!(
                out.status.success(),
                "umask restore: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        assert_eq!(
            fs::metadata(actual.path().join("selected"))?.permissions().mode() & 0o777,
            fs::metadata(oracle.path().join("selected"))?.permissions().mode() & 0o777,
            "restore honors umask even while recovering executable mode"
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_ancestors_are_refused_and_disabled_symlinks_match_git() -> Result<()> {
    use std::os::unix::fs::symlink;
    let repo = fixture("restore-symlink-parent")?;
    repo.commit_file("nested/file", "tracked\n")?;
    fs::write(repo.path().join("selected"), "dirty\n")?;
    fs::remove_dir_all(repo.path().join("nested"))?;
    let outside = tempfile::tempdir()?;
    fs::write(outside.path().join("file"), "outside\n")?;
    symlink(outside.path(), repo.path().join("nested"))?;
    let before = index(repo.path())?;
    ensure!(!run(
        repo.path(),
        true,
        &["restore", "--worktree", "--", "selected", "nested/file"]
    )?
    .status
    .success());
    assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n");
    assert_eq!(fs::read(outside.path().join("file"))?, b"outside\n");
    assert_eq!(index(repo.path())?, before);

    let actual = fixture("restore-no-symlinks")?;
    let oracle = fixture("restore-no-symlinks-oracle")?;
    for repo in [&actual, &oracle] {
        symlink("private", repo.path().join("link"))?;
        git(repo.path(), &["add", "link"])?;
        fs::remove_file(repo.path().join("link"))?;
        git(repo.path(), &["config", "core.symlinks", "false"])?;
    }
    compare(
        actual.path(),
        oracle.path(),
        &["restore", "--worktree", "--", "link"],
        &["link", "private"],
    )?;
    Ok(())
}

#[test]
fn split_and_sparse_indexes_are_refused_without_refreshing_them() -> Result<()> {
    for shape in ["split", "sparse"] {
        let repo = fixture("restore-index-form")?;
        repo.commit_file("kept/file", "kept\n")?;
        repo.commit_file("omitted/file", "omitted\n")?;
        git(repo.path(), &["reset", "--hard", "HEAD"])?;
        if shape == "split" {
            git(repo.path(), &["update-index", "--split-index"])?;
        } else {
            git(repo.path(), &["sparse-checkout", "init", "--cone", "--sparse-index"])?;
            git(repo.path(), &["sparse-checkout", "set", "kept"])?;
        }
        fs::write(repo.path().join("selected"), "dirty\n")?;
        let before = index(repo.path())?;
        let out = run(repo.path(), true, &["restore", "--worktree", "--", "selected"])?;
        ensure!(!out.status.success(), "must refuse {shape} index");
        assert_eq!(fs::read(repo.path().join("selected"))?, b"dirty\n");
        assert_eq!(index(repo.path())?, before);
    }
    Ok(())
}

#[test]
fn restoring_attributes_with_content_matches_git_conversion_order() -> Result<()> {
    for source in [None, Some("--source=HEAD")] {
        for index_eol in ["crlf", "lf"] {
            for worktree_eol in [Some("crlf"), Some("lf"), None] {
                for select_attributes in [false, true] {
                    let actual = fixture("restore-attributes")?;
                    let oracle = fixture("restore-attributes-oracle")?;
                    for repo in [&actual, &oracle] {
                        repo.commit_file(".gitattributes", "selected text eol=crlf\n")?;
                        fs::write(
                            repo.path().join(".gitattributes"),
                            format!("selected text eol={index_eol}\n"),
                        )?;
                        git(repo.path(), &["add", ".gitattributes"])?;
                        if let Some(eol) = worktree_eol {
                            fs::write(repo.path().join(".gitattributes"), format!("selected text eol={eol}\n"))?;
                        } else {
                            fs::remove_file(repo.path().join(".gitattributes"))?;
                        }
                        fs::write(repo.path().join("selected"), "dirty\n")?;
                    }
                    let mut args = vec!["restore", "--worktree"];
                    args.extend(source);
                    args.push("--");
                    if select_attributes {
                        args.push(".gitattributes");
                    }
                    args.push("selected");
                    // Selected attributes take their restored source bytes. Unselected
                    // attributes use the worktree, falling back to the index even
                    // when the selected file itself is restored from HEAD.
                    compare(
                        actual.path(),
                        oracle.path(),
                        &args,
                        &[".gitattributes", "selected", "neighbor", "private"],
                    )?;
                }
            }
        }
    }
    Ok(())
}
