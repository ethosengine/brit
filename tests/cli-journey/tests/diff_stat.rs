//! Read-only diff summaries, with stock Git as the byte-for-byte oracle.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn command(repo: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(brit_bin().expect("required Brit binary"));
    gix_testtools::configure_git_environment(&mut cmd, repo);
    cmd.current_dir(repo).args(args);
    cmd
}

fn brit(repo: &Path, args: &[&str]) -> Result<Output> {
    Ok(command(repo, args).output()?)
}

// Includes HEAD, refs, reflogs, object inventory, indexes, worktree bytes and locks.
// Do not follow symlinks, including broken symlinks in the comparison fixture.
fn snapshot(root: &Path) -> Result<Vec<(PathBuf, Vec<u8>)>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            result.push((
                entry.path(),
                fs::read_link(entry.path())?.as_os_str().as_encoded_bytes().to_vec(),
            ));
        } else if entry.file_type()?.is_dir() {
            result.extend(snapshot(&entry.path())?);
        } else {
            result.push((entry.path(), fs::read(entry.path())?));
        }
    }
    result.sort();
    Ok(result)
}

fn parity(repo: &Path, native: &[&str], oracle: &[&str]) -> Result<Vec<u8>> {
    let mut args = vec![
        "-c",
        "color.ui=false",
        "diff",
        "--no-ext-diff",
        "--no-renames",
        "--stat",
        "--stat-width=80",
    ];
    args.extend_from_slice(oracle);
    let expected = git(repo, &args)?;
    // Git may refresh its index stat cache. Only Brit's subsequent read is measured.
    let before = snapshot(repo)?;
    let actual = brit(repo, native)?;
    ensure!(
        actual.status.success(),
        "{native:?}: {}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(actual.stdout, expected, "summary parity for {native:?}");
    assert_eq!(snapshot(repo)?, before, "inspection must not write repository state");
    Ok(actual.stdout)
}

fn refuse(repo: &Path, args: &[&str], reason: &str) -> Result<()> {
    let before = snapshot(repo)?;
    let out = brit(repo, args)?;
    ensure!(!out.status.success(), "must refuse {args:?}");
    ensure!(
        String::from_utf8_lossy(&out.stderr).contains(reason),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    ensure!(out.stdout.is_empty(), "refusal cannot emit a partial summary");
    assert_eq!(snapshot(repo)?, before, "refusal preserves all state");
    Ok(())
}

#[test]
fn working_staged_and_tree_summaries_match_git() -> Result<()> {
    let repo = TestRepo::new("stat-content")?;
    for (path, data) in [
        ("replacement", "a\nb\nc\n"),
        ("deleted", "gone\n"),
        ("no-newline", "old"),
        ("newline-only", "line"),
        ("binary", "old\0bytes"),
        ("binary-deleted", "deleted\0bytes"),
        ("empty-deleted", ""),
        ("emptied", "old\n"),
        ("filled", ""),
    ] {
        repo.commit_file(path, data)?;
    }
    let old = repo.head_id()?;
    for path in ["deleted", "binary-deleted", "empty-deleted"] {
        fs::remove_file(repo.path().join(path))?;
    }
    for (path, data) in [
        ("replacement", "a\nnew\nc\nextra\n"),
        ("no-newline", "new"),
        ("newline-only", "line\n"),
        ("binary", "new\0longer bytes"),
        ("binary-added", "\0new"),
        ("empty-added", ""),
        ("added", "new\n"),
        ("emptied", ""),
        ("filled", "now filled"),
    ] {
        fs::write(repo.path().join(path), data)?;
    }
    parity(repo.path(), &["diff", "--stat"], &[])?;
    git(repo.path(), &["add", "--all"])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    parity(repo.path(), &["diff", "--staged", "--stat"], &["--cached"])?;
    fs::write(repo.path().join("replacement"), "unstaged\n".repeat(80))?;
    fs::write(repo.path().join("untracked"), "untracked\n".repeat(100))?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    let output = parity(repo.path(), &["diff", "--stat"], &[])?;
    ensure!(!String::from_utf8_lossy(&output).contains("untracked"));
    git(repo.path(), &["commit", "-m", "stage"])?;
    let new = repo.head_id()?;
    parity(repo.path(), &["diff", "tree", "--stat", &old, &new], &[&old, &new])?;
    parity(repo.path(), &["diff", "tree", "--stat", &new, &old], &[&new, &old])?;
    Ok(())
}

#[test]
fn empty_unborn_detached_and_linked_worktrees() -> Result<()> {
    let repo = TestRepo::new("stat-heads")?;
    ensure!(parity(repo.path(), &["diff", "--stat"], &[])?.is_empty());
    ensure!(parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?.is_empty());
    ensure!(parity(
        repo.path(),
        &["diff", "tree", "--stat", "HEAD", "HEAD"],
        &["HEAD", "HEAD"]
    )?
    .is_empty());
    git(repo.path(), &["checkout", "--orphan", "unborn"])?;
    fs::write(repo.path().join("new"), "new\n")?;
    fs::write(repo.path().join("empty"), "")?;
    git(repo.path(), &["add", "new", "empty"])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    git(repo.path(), &["commit", "-m", "born"])?;
    git(repo.path(), &["checkout", "--detach"])?;
    fs::write(repo.path().join("new"), "changed\n")?;
    parity(repo.path(), &["diff", "--stat"], &[])?;
    git(repo.path(), &["add", "new"])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    git(repo.path(), &["worktree", "add", "-b", "linked", "linked"])?;
    let linked = repo.path().join("linked");
    fs::write(linked.join("new"), "linked\nchange\n")?;
    parity(&linked, &["diff", "--stat"], &[])?;
    git(&linked, &["add", "new"])?;
    let before = snapshot(repo.path())?;
    parity(&linked, &["diff", "--cached", "--stat"], &["--cached"])?;
    assert_eq!(
        snapshot(repo.path())?,
        before,
        "linked inspection preserves common metadata too"
    );
    Ok(())
}

#[test]
fn names_scaling_literal_selection_and_renames() -> Result<()> {
    let repo = TestRepo::new("stat-names")?;
    let long = format!("directory/{}/other/long-name", "long".repeat(30));
    let flat = "long".repeat(35);
    for path in [
        &long,
        &flat,
        "space name",
        #[cfg(unix)]
        "tab\tname",
        #[cfg(unix)]
        "quote\"name",
        #[cfg(unix)]
        "back\\slash",
        "café",
        "literal[ab]",
        "literala",
        "tiny",
        "rename-old",
    ] {
        repo.commit_file(path, "old\n")?;
        fs::write(repo.path().join(path), "new\n")?;
    }
    fs::write(repo.path().join(&long), "many lines\n".repeat(1000))?;
    fs::write(repo.path().join(&flat), "old\nextra\n")?;
    fs::rename(repo.path().join("rename-old"), repo.path().join("rename-new"))?;
    for quote in ["true", "false"] {
        git(repo.path(), &["config", "core.quotePath", quote])?;
        parity(repo.path(), &["diff", "--stat"], &[])?;
    }
    refuse(
        repo.path(),
        &["diff", "--stat", "--", "literal[ab]"],
        "literal file paths",
    )?;
    parity(
        repo.path(),
        &["diff", "--stat", "--", "space name"],
        &["--", "space name"],
    )?;
    git(repo.path(), &["add", "--all"])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    parity(
        repo.path(),
        &["diff", "--cached", "--stat", "--", "tiny"],
        &["--cached", "--", "tiny"],
    )?;
    // Formatter config and terminal dimensions cannot change the fixed public surface.
    let expected = brit(repo.path(), &["diff", "--cached", "--stat"])?.stdout;
    git(repo.path(), &["config", "diff.statGraphWidth", "5"])?;
    git(repo.path(), &["config", "diff.statNameWidth", "8"])?;
    let out = command(repo.path(), &["diff", "--cached", "--stat"])
        .env("COLUMNS", "20")
        .output()?;
    ensure!(out.status.success());
    assert_eq!(out.stdout, expected);
    Ok(())
}

#[test]
fn graph_rounding_and_unicode_name_widths() -> Result<()> {
    let repo = TestRepo::new("stat-widths")?;
    for (name, lines) in [("mostly-deleted", 900), ("equal", 3), ("mostly-added", 1), ("tiny", 1)] {
        repo.commit_file(name, &"old\n".repeat(lines))?;
    }
    let wide = format!("parent/{}/leaf", "界".repeat(35));
    let flat = "é".repeat(70);
    // x + acute has no precomposed form, so Git's macOS normalization preserves it.
    let combining = "x\u{301}".repeat(45);
    for name in [&wide, &flat, &combining] {
        repo.commit_file(name, "old\n")?;
        fs::write(repo.path().join(name), "new\n")?;
    }
    for (name, lines) in [("mostly-deleted", 1), ("equal", 3), ("mostly-added", 1100), ("tiny", 1)] {
        fs::write(repo.path().join(name), "new\n".repeat(lines))?;
    }
    git(repo.path(), &["config", "core.quotePath", "true"])?;
    parity(repo.path(), &["diff", "--stat"], &[])?;
    git(repo.path(), &["config", "core.quotePath", "false"])?;
    // Git 2.56 clips by display width. Older hosts split UTF-8 sequences here,
    // so retain the expected 2.56 output instead of accepting their output.
    let before = snapshot(repo.path())?;
    let actual = brit(repo.path(), &["diff", "--stat"])?;
    ensure!(actual.status.success(), "{}", String::from_utf8_lossy(&actual.stderr));
    assert_eq!(actual.stdout, include_bytes!("../fixtures/diff-stat-unicode.txt"));
    assert_eq!(
        snapshot(repo.path())?,
        before,
        "Unicode clipping preserves repository state"
    );
    Ok(())
}

#[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
#[test]
fn cancellation_before_output_preserves_state_and_prints_nothing() -> Result<()> {
    use std::{
        process::{Child, Stdio},
        time::{Duration, Instant},
    };
    struct Running(Option<Child>);
    impl Drop for Running {
        fn drop(&mut self) {
            if let Some(child) = &mut self.0 {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    let repo = TestRepo::new("stat-cancel")?;
    repo.commit_file("file", "old\n")?;
    repo.commit_file("z-archive", "unchanged\n")?;
    repo.commit_file(".gitattributes", "z-archive diff=lfs\n")?;
    fs::write(repo.path().join("file"), "staged\n")?;
    git(repo.path(), &["add", "file"])?;
    fs::write(repo.path().join("file"), "unstaged\n")?;
    for args in [
        &["diff"][..],
        &["diff", "--stat"][..],
        &["diff", "--cached", "--stat"],
        &["diff", "tree", "--stat", "HEAD~", "HEAD"],
    ] {
        let checkpoint = tempfile::tempdir()?;
        let before = snapshot(repo.path())?;
        let mut child = Running(Some(
            command(repo.path(), args)
                .env("BRIT_SWITCH_CHECKPOINT", "diff-before-output")
                .env("BRIT_SWITCH_CHECKPOINT_DIR", checkpoint.path())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()?,
        ));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !checkpoint.path().join("diff-before-output.reached").exists() {
            ensure!(Instant::now() < deadline, "diff output checkpoint timed out");
            ensure!(
                child.0.as_mut().expect("child").try_wait()?.is_none(),
                "diff exited before checkpoint"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        ensure!(Command::new("kill")
            .args(["-INT", &child.0.as_ref().expect("child").id().to_string()])
            .status()?
            .success());
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.0.as_mut().expect("child").try_wait()?.is_none() {
            ensure!(Instant::now() < deadline, "diff ignored cancellation");
            std::thread::sleep(Duration::from_millis(10));
        }
        let out = child.0.take().expect("child").wait_with_output()?;
        ensure!(!out.status.success());
        ensure!(out.stdout.is_empty(), "cancelled summary cannot reach stdout");
        assert_eq!(snapshot(repo.path())?, before);
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_modes_types_and_byte_paths() -> Result<()> {
    use std::{
        ffi::OsStr,
        os::unix::{
            ffi::OsStrExt,
            fs::{symlink, PermissionsExt},
        },
    };
    let repo = TestRepo::new("stat-types")?;
    for (name, data) in [
        ("exec", "plain\n"),
        ("binary-mode", "\0binary"),
        ("to-link", "file\n"),
        ("empty-mode", ""),
    ] {
        repo.commit_file(name, data)?;
    }
    symlink("old-target", repo.path().join("link"))?;
    symlink("old-target", repo.path().join("to-file"))?;
    // macOS filesystems reject malformed UTF-8; keep newline quoting covered there.
    let bytes = OsStr::from_bytes(if cfg!(target_os = "macos") {
        b"line\n".as_slice()
    } else {
        b"invalid-\xff\n"
    });
    fs::write(repo.path().join(bytes), "old\n")?;
    git(repo.path(), &["add", "--all"])?;
    git(repo.path(), &["commit", "-m", "types"])?;
    for name in ["exec", "binary-mode", "empty-mode"] {
        fs::set_permissions(repo.path().join(name), fs::Permissions::from_mode(0o755))?;
    }
    for name in ["link", "to-link", "to-file"] {
        fs::remove_file(repo.path().join(name))?;
    }
    symlink("new-target", repo.path().join("link"))?;
    symlink("target", repo.path().join("to-link"))?;
    symlink("added-target", repo.path().join("new-link"))?;
    fs::write(repo.path().join("to-file"), "regular\nfile\n")?;
    fs::write(repo.path().join(bytes), "new\n")?;
    for quote in ["true", "false"] {
        git(repo.path(), &["config", "core.quotePath", quote])?;
        parity(repo.path(), &["diff", "--stat"], &[])?;
    }
    parity(
        repo.path(),
        &["diff", "--stat", "--", "binary-mode"],
        &["--", "binary-mode"],
    )?;
    parity(
        repo.path(),
        &["diff", "--stat", "--", "empty-mode"],
        &["--", "empty-mode"],
    )?;
    git(repo.path(), &["add", "--all"])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    git(repo.path(), &["commit", "-m", "changed types"])?;
    parity(
        repo.path(),
        &["diff", "tree", "--stat", "HEAD~", "HEAD"],
        &["HEAD~", "HEAD"],
    )?;
    Ok(())
}

#[test]
fn unsupported_indexes_attributes_and_external_commands_refuse_atomically() -> Result<()> {
    for state in [
        "--skip-worktree",
        "--assume-unchanged",
        "intent",
        "conflict",
        "attributes",
        "external",
    ] {
        let repo = TestRepo::new("stat-refusal")?;
        repo.commit_file("a-ordinary", "old\n")?;
        repo.commit_file("z-refused", "old\n")?;
        fs::write(repo.path().join("a-ordinary"), "new\n")?;
        fs::write(repo.path().join("z-refused"), "new\n")?;
        git(repo.path(), &["add", "a-ordinary", "z-refused"])?;
        match state {
            "intent" => {
                fs::write(repo.path().join("intent"), "new")?;
                git(repo.path(), &["add", "-N", "intent"])?;
            }
            "conflict" => {
                git(repo.path(), &["update-index", "--force-remove", "z-refused"])?;
                let blob = String::from_utf8(git(repo.path(), &["rev-parse", "HEAD:z-refused"])?)?;
                use std::io::Write;
                let mut child = gix_testtools::git_command(repo.path())
                    .args(["update-index", "--index-info"])
                    .stdin(std::process::Stdio::piped())
                    .spawn()?;
                writeln!(
                    child.stdin.take().expect("piped stdin"),
                    "100644 {} 1\tz-refused",
                    blob.trim()
                )?;
                ensure!(child.wait()?.success());
            }
            "attributes" => {
                fs::write(repo.path().join(".gitattributes"), "z-refused diff=custom\n")?;
                fs::write(repo.path().join("z-refused"), "unstaged change\n")?;
            }
            "external" => {
                git(repo.path(), &["config", "diff.external", "must-not-run"])?;
            }
            flag => {
                git(repo.path(), &["update-index", flag, "z-refused"])?;
            }
        }
        fs::write(repo.path().join(".git/index.lock"), "foreign lock\n")?;
        for args in [&["diff", "--stat"][..], &["diff", "--cached", "--stat"]] {
            refuse(repo.path(), args, "")?;
        }
    }
    Ok(())
}

#[test]
fn unchanged_attributed_files_do_not_block_working_statistics() -> Result<()> {
    let repo = TestRepo::new("stat-unchanged-attributes")?;
    repo.commit_file("a-ordinary", "old\n")?;
    repo.commit_file("z-archive", "archive\n")?;
    repo.commit_file("z-binary", "binary\0bytes\n")?;
    repo.commit_file("z-lines", "canonical\nlines\n")?;
    repo.commit_file(
        ".gitattributes",
        "z-* diff=lfs filter=lfs-disabled\nz-lines text eol=lf\n",
    )?;
    // Filesystem bytes differ, but conversion to index form proves equality.
    fs::write(repo.path().join("z-lines"), "canonical\r\nlines\r\n")?;
    fs::write(repo.path().join(".git/index.lock"), "foreign lock\n")?;
    ensure!(parity(repo.path(), &["diff", "--stat"], &[])?.is_empty());
    for path in ["z-archive", "z-binary", "z-lines"] {
        ensure!(parity(repo.path(), &["diff", "--stat", "--", path], &["--", path])?.is_empty());
    }
    fs::write(repo.path().join("a-ordinary"), "new\n")?;
    ensure!(!parity(repo.path(), &["diff", "--stat"], &[])?.is_empty());
    for data in ["changed\n", "changed\0bytes\n"] {
        fs::write(repo.path().join("z-archive"), data)?;
        for args in [&["diff"][..], &["diff", "--stat"]] {
            refuse(repo.path(), args, "diff attributes")?;
        }
    }
    fs::remove_file(repo.path().join("z-archive"))?;
    for args in [&["diff"][..], &["diff", "--stat"]] {
        refuse(repo.path(), args, "diff attributes")?;
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn attributed_symlinks_and_effective_modes_still_require_exact_equality() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};

    let repo = TestRepo::new("attributed-modes")?;
    let patch_parity = || -> Result<()> {
        let expected = git(repo.path(), &["diff", "--no-ext-diff", "--no-color"])?;
        let before = snapshot(repo.path())?;
        let out = brit(repo.path(), &["diff"])?;
        ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        assert_eq!(out.stdout, expected);
        assert_eq!(snapshot(repo.path())?, before);
        Ok(())
    };
    repo.commit_file("a-ordinary", "old\n")?;
    repo.commit_file("z-file", "target")?;
    symlink("target", repo.path().join("z-link"))?;
    git(repo.path(), &["add", "z-link"])?;
    repo.commit_file(".gitattributes", "z-* diff=lfs\n")?;
    ensure!(parity(repo.path(), &["diff", "--stat"], &[])?.is_empty());
    patch_parity()?;
    fs::write(repo.path().join("a-ordinary"), "new\n")?;
    parity(repo.path(), &["diff", "--stat"], &[])?;
    patch_parity()?;

    fs::set_permissions(repo.path().join("z-file"), fs::Permissions::from_mode(0o755))?;
    git(repo.path(), &["config", "core.filemode", "false"])?;
    parity(repo.path(), &["diff", "--stat"], &[])?;
    patch_parity()?;
    git(repo.path(), &["config", "core.filemode", "true"])?;
    for args in [&["diff"][..], &["diff", "--stat"]] {
        refuse(repo.path(), args, "diff attributes")?;
    }
    // An unchanged executable remains skippable when executable bits matter.
    git(repo.path(), &["add", "z-file"])?;
    parity(repo.path(), &["diff", "--stat"], &[])?;
    patch_parity()?;
    // File and symlink bytes can agree without their types agreeing.
    fs::remove_file(repo.path().join("z-file"))?;
    symlink("target", repo.path().join("z-file"))?;
    for args in [&["diff"][..], &["diff", "--stat"]] {
        refuse(repo.path(), args, "diff attributes")?;
    }
    fs::remove_file(repo.path().join("z-file"))?;
    fs::write(repo.path().join("z-file"), "target")?;
    git(repo.path(), &["add", "z-file"])?;
    fs::remove_file(repo.path().join("z-link"))?;
    symlink("changed-target", repo.path().join("z-link"))?;
    for args in [&["diff"][..], &["diff", "--stat"]] {
        refuse(repo.path(), args, "diff attributes")?;
    }
    fs::remove_file(repo.path().join("z-link"))?;
    fs::write(repo.path().join("z-link"), "target")?;
    for args in [&["diff"][..], &["diff", "--stat"]] {
        refuse(repo.path(), args, "diff attributes")?;
    }
    Ok(())
}

#[test]
fn unchanged_attributed_files_never_execute_external_commands() -> Result<()> {
    for setting in [
        "filter.external.clean",
        "filter.external.process",
        "diff.external",
        "diff.lfs.command",
        "diff.lfs.textconv",
    ] {
        let repo = TestRepo::new("attributed-external-sentinel")?;
        repo.commit_file("a-ordinary", "old\n")?;
        repo.commit_file("z-archive", "unchanged\n")?;
        repo.commit_file(".gitattributes", "z-archive diff=lfs filter=external\n")?;
        git(repo.path(), &["config", setting, "echo executed > .git/executed; cat"])?;
        fs::write(repo.path().join("a-ordinary"), "new\n")?;
        if setting.starts_with("diff.lfs.") {
            let before = snapshot(repo.path())?;
            for args in [&["diff"][..], &["diff", "--stat"]] {
                let out = brit(repo.path(), args)?;
                ensure!(out.status.success() && !out.stdout.is_empty());
                assert_eq!(snapshot(repo.path())?, before);
            }
            fs::write(repo.path().join("z-archive"), "changed\n")?;
        }
        for args in [&["diff"][..], &["diff", "--stat"]] {
            let reason = if setting.starts_with("filter.") {
                "External filter"
            } else if setting.starts_with("diff.lfs.") {
                "diff attributes"
            } else {
                "external diff"
            };
            refuse(repo.path(), args, reason)?;
        }
        ensure!(
            !repo.path().join(".git/executed").exists(),
            "no external command may run"
        );
    }
    Ok(())
}

#[test]
fn tree_attributes_come_from_both_requested_trees_and_ignore_working_index() -> Result<()> {
    let repo = TestRepo::new("stat-tree-attrs")?;
    repo.commit_file("a-ordinary", "base\n")?;
    repo.commit_file("z-custom", "old\n")?;
    let old = repo.head_id()?;
    repo.commit_file("z-custom", "new\n")?;
    let new = repo.head_id()?;
    fs::write(repo.path().join(".gitattributes"), "* diff=working-only\n")?;
    fs::write(repo.path().join(".git/index"), "invalid working index")?;
    parity(repo.path(), &["diff", "tree", "--stat", &old, &new], &[&old, &new])?;
    // Restore only the disposable fixture index, then construct a tree-only driver.
    fs::remove_file(repo.path().join(".git/index"))?;
    git(repo.path(), &["read-tree", "HEAD"])?;
    repo.commit_file(".gitattributes", "z-custom diff=custom\n")?;
    repo.commit_file("a-ordinary", "changed\n")?;
    repo.commit_file("z-custom", "changed\n")?;
    let custom = repo.head_id()?;
    fs::remove_file(repo.path().join(".gitattributes"))?;
    for (a, b) in [(&new, &custom), (&custom, &new)] {
        refuse(repo.path(), &["diff", "tree", "--stat", a, b], "diff attributes")?;
    }
    Ok(())
}

#[test]
fn changed_submodules_refuse_and_unchanged_staged_neighbors_are_skipped() -> Result<()> {
    let repo = TestRepo::new("stat-submodule")?;
    repo.commit_file("ordinary", "old\n")?;
    let old = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--add", "--cacheinfo", &format!("160000,{old},sub")],
    )?;
    git(repo.path(), &["commit", "-m", "gitlink"])?;
    fs::write(repo.path().join("ordinary"), "new\n")?;
    git(repo.path(), &["add", "ordinary"])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    let new = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--cacheinfo", &format!("160000,{new},sub")],
    )?;
    refuse(repo.path(), &["diff", "--cached", "--stat"], "submodule")?;
    git(repo.path(), &["commit", "-m", "changed gitlink"])?;
    refuse(repo.path(), &["diff", "tree", "--stat", "HEAD~", "HEAD"], "submodule")?;
    Ok(())
}

#[test]
fn split_sparse_indexes_and_environment_drivers_refuse() -> Result<()> {
    let repo = TestRepo::new("stat-special-index")?;
    repo.commit_file("included/file", "old\n")?;
    repo.commit_file("excluded/file", "old\n")?;
    fs::write(repo.path().join("included/file"), "new\n")?;
    git(repo.path(), &["update-index", "--split-index"])?;
    for args in [&["diff", "--stat"][..], &["diff", "--cached", "--stat"]] {
        refuse(repo.path(), args, "Split indexes")?;
    }
    git(repo.path(), &["update-index", "--no-split-index"])?;
    git(
        repo.path(),
        &["sparse-checkout", "set", "--cone", "--sparse-index", "included"],
    )?;
    for args in [&["diff", "--stat"][..], &["diff", "--cached", "--stat"]] {
        refuse(repo.path(), args, "Sparse")?;
    }
    git(repo.path(), &["sparse-checkout", "disable"])?;
    let before = snapshot(repo.path())?;
    for args in [&["diff", "--stat"][..], &["diff", "tree", "--stat", "HEAD~", "HEAD"]] {
        let out = command(repo.path(), args)
            .env("GIT_EXTERNAL_DIFF", "must-not-run")
            .output()?;
        ensure!(!out.status.success() && out.stdout.is_empty());
        ensure!(String::from_utf8_lossy(&out.stderr).contains("external diff"));
        assert_eq!(snapshot(repo.path())?, before);
    }
    Ok(())
}

#[test]
fn bare_tree_summaries_and_invalid_arguments() -> Result<()> {
    let repo = TestRepo::new("stat-bare")?;
    repo.commit_file("file", "old\n")?;
    repo.commit_file("file", "new\n")?;
    let bare = tempfile::tempdir()?;
    git(
        repo.path(),
        &[
            "clone",
            "--bare",
            ".",
            bare.path().to_str().expect("UTF-8 temporary path"),
        ],
    )?;
    parity(
        bare.path(),
        &["diff", "tree", "--stat", "HEAD~", "HEAD"],
        &["HEAD~", "HEAD"],
    )?;
    for args in [
        vec!["diff", "--stat", "HEAD"],
        vec!["diff", "--stat", "--width=80"],
        vec!["diff", "--numstat"],
        vec!["diff", "--shortstat"],
        vec!["diff", "--cached", "tree", "--stat", "HEAD~", "HEAD"],
        vec!["diff", "tree", "--stat", "HEAD~", "HEAD", "--", "file"],
        vec!["diff", "file", "--stat", "HEAD~:file", "HEAD:file"],
        vec!["diff", "--stat", "--", "untracked"],
        vec!["diff", "tree", "--stat", "HEAD", "missing-revision"],
    ] {
        refuse(repo.path(), &args, "")?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn inspection_stays_available_during_a_mutation_with_foreign_locks() -> Result<()> {
    use std::{io::Read, process::Stdio};
    let repo = TestRepo::new("stat-guard")?;
    repo.commit_file("file", "old\n")?;
    fs::write(repo.path().join("file"), "new\n")?;
    git(repo.path(), &["add", "file"])?;
    fs::write(repo.path().join("file"), "unstaged\n")?;
    let mut cmd = Command::new("flock");
    gix_testtools::configure_git_environment(&mut cmd, repo.path());
    let mut holder = cmd
        .arg(repo.path().join(".git/brit-mutation.guard"))
        .args(["sh", "-c", "printf ready; cat"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut ready = [0; 5];
    holder.stdout.take().expect("piped stdout").read_exact(&mut ready)?;
    assert_eq!(&ready, b"ready");
    fs::write(repo.path().join(".git/index.lock"), "foreign index")?;
    fs::write(repo.path().join(".git/HEAD.lock"), "foreign head")?;
    parity(repo.path(), &["diff", "--stat"], &[])?;
    parity(repo.path(), &["diff", "--cached", "--stat"], &["--cached"])?;
    parity(
        repo.path(),
        &["diff", "tree", "--stat", "HEAD~", "HEAD"],
        &["HEAD~", "HEAD"],
    )?;
    drop(holder.stdin.take());
    ensure!(holder.wait()?.success());
    Ok(())
}
