//! Explicit-path native authoring, compared with stock Git in disposable repositories.
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
        .args(args.iter().copied())
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_NAME", "Author")
        .env("GIT_COMMITTER_EMAIL", "author@example.com")
        .run()
}

fn success(repo: &Path, args: &[&str]) -> Result<Capture> {
    let out = brit(repo, args)?;
    ensure!(out.status.success(), "brit {args:?}: {}", out.stderr);
    Ok(out)
}

fn index(repo: &Path) -> Result<Vec<u8>> {
    let path = String::from_utf8(git(repo, &["rev-parse", "--git-path", "index"])?)?;
    Ok(fs::read(repo.join(path.trim()))?)
}

fn entries(repo: &Path) -> Result<Vec<u8>> {
    git(repo, &["ls-files", "--stage", "-z"])
}

fn fixture(label: &str) -> Result<TestRepo> {
    let repo = TestRepo::new(label)?;
    repo.commit_file("selected", "base\n")?;
    repo.commit_file("neighbor", "base neighbor\n")?;
    fs::write(repo.path().join("neighbor"), "staged neighbor\n")?;
    git(repo.path(), &["add", "--", "neighbor"])?;
    fs::write(repo.path().join("neighbor"), "unstaged neighbor\n")?;
    fs::write(repo.path().join("private"), "private\n")?;
    fs::write(repo.path().join("selected"), "selected change\n")?;
    Ok(repo)
}

#[test]
fn stage_inspect_unstage_and_commit_preserves_neighbors() -> Result<()> {
    let actual = fixture("native")?;
    let oracle = fixture("oracle")?;
    success(actual.path(), &["add", "--", "selected"])?;
    git(oracle.path(), &["add", "--", "selected"])?;
    assert_eq!(
        entries(actual.path())?,
        entries(oracle.path())?,
        "staging matches Git entries"
    );
    fs::write(actual.path().join("selected"), "later worktree change\n")?;
    let before = index(actual.path())?;
    let staged = success(actual.path(), &["diff", "--cached", "--", "selected"])?;
    ensure!(
        staged.stdout.contains("+selected change") && !staged.stdout.contains("later worktree"),
        "{}",
        staged.stdout
    );
    let working = success(actual.path(), &["diff", "--", "selected"])?;
    ensure!(
        working.stdout.contains("+later worktree change") && working.stdout.contains("-selected change"),
        "{}",
        working.stdout
    );
    assert_eq!(index(actual.path())?, before, "diff never refreshes index bytes");
    success(actual.path(), &["restore", "--staged", "--", "selected"])?;
    git(oracle.path(), &["restore", "--staged", "--", "selected"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    assert_eq!(fs::read(actual.path().join("selected"))?, b"later worktree change\n");
    assert_eq!(fs::read(actual.path().join("neighbor"))?, b"unstaged neighbor\n");
    assert_eq!(fs::read(actual.path().join("private"))?, b"private\n");
    success(actual.path(), &["add", "--", "selected"])?;
    let tree = git(actual.path(), &["write-tree"])?;
    success(actual.path(), &["commit", "-m", "native authoring"])?;
    assert_eq!(
        git(actual.path(), &["rev-parse", "HEAD^{tree}"])?,
        tree,
        "commit consumes exactly staged tree"
    );
    Ok(())
}

#[test]
fn invalid_batches_and_lock_contention_preserve_index_and_worktree() -> Result<()> {
    let repo = fixture("atomic")?;
    let before = index(repo.path())?;
    for args in [
        vec!["add", "--", "selected", "missing"],
        vec!["restore", "--staged", "--", "selected", "missing"],
        vec!["add", "--", "*.txt"],
        vec!["add", "selected"],
        vec!["restore", "--staged", "--worktree", "--", "selected"],
        vec!["restore", "--", "selected"],
    ] {
        ensure!(!brit(repo.path(), &args)?.status.success(), "must refuse {args:?}");
        assert_eq!(index(repo.path())?, before, "refusal is index-atomic: {args:?}");
        assert_eq!(fs::read(repo.path().join("selected"))?, b"selected change\n");
        assert_eq!(fs::read(repo.path().join("neighbor"))?, b"unstaged neighbor\n");
    }
    let lock = repo.path().join(".git/index.lock");
    fs::write(&lock, "other owner")?;
    for args in [
        vec!["add", "--", "selected"],
        vec!["restore", "--staged", "--", "selected"],
    ] {
        ensure!(
            !brit(repo.path(), &args)?.status.success(),
            "index lock must refuse {args:?}"
        );
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(&lock)?, b"other owner");
    }
    Ok(())
}

#[test]
fn new_files_and_deletions_match_git_and_restore_without_touching_files() -> Result<()> {
    let actual = fixture("new-delete")?;
    let oracle = fixture("new-delete-oracle")?;
    for repo in [&actual, &oracle] {
        fs::remove_file(repo.path().join("selected"))?;
        fs::write(repo.path().join("new file"), "new content\n")?;
    }
    success(actual.path(), &["add", "--", "selected", "new file"])?;
    git(oracle.path(), &["add", "--", "selected", "new file"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    success(actual.path(), &["restore", "--staged", "--", "selected", "new file"])?;
    git(oracle.path(), &["restore", "--staged", "--", "selected", "new file"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    ensure!(!actual.path().join("selected").exists());
    assert_eq!(fs::read(actual.path().join("new file"))?, b"new content\n");
    Ok(())
}

#[test]
fn unborn_index_and_staged_alias_are_usable() -> Result<()> {
    let temp = tempfile::tempdir()?;
    git(temp.path(), &["init", "-q", "--initial-branch=main"])?;
    fs::write(temp.path().join("first"), "first content\n")?;
    success(temp.path(), &["add", "--", "first"])?;
    let before = index(temp.path())?;
    let out = success(temp.path(), &["diff", "--staged"])?;
    ensure!(out.stdout.contains("+first content"), "{}", out.stdout);
    assert_eq!(index(temp.path())?, before);
    ensure!(!brit(temp.path(), &["restore", "--staged", "--", "first"])?
        .status
        .success());
    assert_eq!(
        index(temp.path())?,
        before,
        "unborn restore refuses without an authoritative HEAD"
    );
    assert_eq!(fs::read(temp.path().join("first"))?, b"first content\n");
    Ok(())
}

#[test]
fn linked_worktree_uses_its_own_index() -> Result<()> {
    let repo = fixture("linked-parent")?;
    let before = index(repo.path())?;
    let linked = tempfile::tempdir()?;
    git(
        repo.path(),
        &[
            "worktree",
            "add",
            "-b",
            "authoring",
            linked.path().to_str().expect("temp path"),
        ],
    )?;
    fs::write(linked.path().join("selected"), "linked change\n")?;
    success(linked.path(), &["add", "--", "selected"])?;
    let out = success(linked.path(), &["diff", "--cached"])?;
    ensure!(out.stdout.contains("+linked change") && !out.stdout.contains("staged neighbor"));
    success(linked.path(), &["restore", "--staged", "--", "selected"])?;
    assert_eq!(index(repo.path())?, before);
    assert_eq!(fs::read(linked.path().join("selected"))?, b"linked change\n");
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_and_executable_modes_match_git() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let actual = fixture("modes")?;
    let oracle = fixture("modes-oracle")?;
    for repo in [&actual, &oracle] {
        git(repo.path(), &["config", "core.filemode", "true"])?;
        symlink("selected", repo.path().join("link"))?;
        fs::set_permissions(repo.path().join("selected"), fs::Permissions::from_mode(0o755))?;
    }
    success(actual.path(), &["add", "--", "selected", "link"])?;
    git(oracle.path(), &["add", "--", "selected", "link"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    success(actual.path(), &["restore", "--staged", "--", "selected", "link"])?;
    git(oracle.path(), &["restore", "--staged", "--", "selected", "link"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    assert_eq!(fs::read_link(actual.path().join("link"))?, Path::new("selected"));
    assert_ne!(
        fs::metadata(actual.path().join("selected"))?.permissions().mode() & 0o111,
        0
    );
    Ok(())
}

#[test]
fn patches_match_git_and_leave_index_and_objects_unchanged() -> Result<()> {
    fn object_files(dir: &Path) -> Result<Vec<(std::path::PathBuf, Vec<u8>)>> {
        let mut result = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                result.extend(object_files(&entry.path())?);
            } else {
                result.push((entry.path(), fs::read(entry.path())?));
            }
        }
        result.sort();
        Ok(result)
    }
    let repo = fixture("patches")?;
    repo.commit_file("deleted", "deleted content\n")?;
    repo.commit_file("no-newline", "old unterminated")?;
    repo.commit_file("binary", "old\0binary")?;
    fs::remove_file(repo.path().join("deleted"))?;
    fs::write(repo.path().join("no-newline"), "new unterminated")?;
    fs::write(repo.path().join("binary"), "new\0binary")?;
    fs::write(repo.path().join("added"), "new content\n")?;
    success(repo.path(), &["add", "--", "deleted", "no-newline", "binary", "added"])?;
    let before_objects = object_files(&repo.path().join(".git/objects"))?;
    for (native, oracle) in [
        (
            vec!["diff", "--cached"],
            vec!["diff", "--cached", "--no-ext-diff", "--no-color", "--no-renames"],
        ),
        (
            vec!["diff"],
            vec!["diff", "--no-ext-diff", "--no-color", "--no-renames"],
        ),
    ] {
        let expected = git(repo.path(), &oracle)?;
        // Stock Git may refresh stat-cache entries while computing worktree diff.
        // Snapshot after that oracle read so only Brit's read-only behavior is measured.
        let before_index = index(repo.path())?;
        let actual = success(repo.path(), &native)?;
        assert_eq!(actual.stdout.as_bytes(), expected, "patch parity for {native:?}");
        assert_eq!(index(repo.path())?, before_index, "diff is read-only for index");
        assert_eq!(
            object_files(&repo.path().join(".git/objects"))?,
            before_objects,
            "diff writes no objects"
        );
    }
    let selected = success(repo.path(), &["diff", "--cached", "--", "deleted"])?;
    assert_eq!(
        selected.stdout.as_bytes(),
        git(
            repo.path(),
            &["diff", "--cached", "--no-ext-diff", "--no-color", "--", "deleted"]
        )?
    );
    Ok(())
}

#[test]
fn working_diff_skips_only_canonically_unchanged_attributed_files() -> Result<()> {
    let repo = TestRepo::new("working-unsupported-neighbor")?;
    repo.commit_file("a-ordinary", "base\n")?;
    repo.commit_file("z-archive", "archive\n")?;
    repo.commit_file("z-binary", "binary\0bytes\n")?;
    repo.commit_file("z-lines", "canonical\nlines\n")?;
    repo.commit_file(
        ".gitattributes",
        "z-* diff=lfs filter=lfs-disabled\nz-lines text eol=lf\n",
    )?;
    fs::write(repo.path().join("z-lines"), "canonical\r\nlines\r\n")?;
    ensure!(success(repo.path(), &["diff"])?.stdout.is_empty());
    for path in ["z-archive", "z-binary", "z-lines"] {
        ensure!(success(repo.path(), &["diff", "--", path])?.stdout.is_empty());
    }
    fs::write(repo.path().join("a-ordinary"), "changed\n")?;
    let expected = git(repo.path(), &["diff", "--no-ext-diff", "--no-color"])?;
    let before = index(repo.path())?;
    fs::write(repo.path().join(".git/index.lock"), "foreign lock\n")?;
    assert_eq!(success(repo.path(), &["diff"])?.stdout.as_bytes(), expected);
    assert_eq!(
        index(repo.path())?,
        before,
        "canonical comparison leaves index untouched"
    );
    // Same length is not equality; the attribute refusal must still be atomic.
    fs::write(repo.path().join("z-archive"), "changed\n")?;
    for args in [&["diff"][..], &["diff", "--", "z-archive"]] {
        let refused = brit(repo.path(), args)?;
        ensure!(!refused.status.success() && refused.stderr.contains("diff attributes"));
        ensure!(refused.stdout.is_empty(), "refusal emits no partial patch");
    }
    assert_eq!(index(repo.path())?, before);
    Ok(())
}

#[test]
fn staged_diff_skips_unchanged_unsupported_paths_but_refuses_changed_ones() -> Result<()> {
    let repo = TestRepo::new("cached-unsupported-neighbor")?;
    repo.commit_file("ordinary", "base\n")?;
    repo.commit_file("archive", "archive base\n")?;
    repo.commit_file(".gitattributes", "archive diff=lfs\n")?;
    let head = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")],
    )?;
    git(repo.path(), &["commit", "-m", "unchanged gitlink"])?;
    ensure!(success(repo.path(), &["diff", "--cached"])?.stdout.is_empty());
    fs::write(repo.path().join("ordinary"), "staged change\n")?;
    success(repo.path(), &["add", "--", "ordinary"])?;
    let before = index(repo.path())?;
    let patch = success(repo.path(), &["diff", "--cached"])?;
    assert_eq!(
        patch.stdout.as_bytes(),
        git(repo.path(), &["diff", "--cached", "--no-ext-diff", "--no-color"])?
    );
    ensure!(patch.stdout.contains("+staged change"));
    assert_eq!(index(repo.path())?, before, "inspection preserves index bytes");

    // A staged attribute-driven change must still refuse without a partial patch.
    fs::write(repo.path().join("archive"), "changed archive\n")?;
    git(repo.path(), &["add", "--", "archive"])?;
    let before = index(repo.path())?;
    let refused = brit(repo.path(), &["diff", "--cached"])?;
    ensure!(!refused.status.success() && refused.stderr.contains("diff attributes"));
    ensure!(refused.stdout.is_empty(), "refusal emits no partial patch");
    assert_eq!(index(repo.path())?, before);
    git(repo.path(), &["restore", "--staged", "--", "archive"])?;
    // Equal content alone is insufficient: staged mode changes still need a patch.
    git(repo.path(), &["update-index", "--chmod=+x", "archive"])?;
    let refused = brit(repo.path(), &["diff", "--cached"])?;
    ensure!(!refused.status.success() && refused.stderr.contains("diff attributes"));
    ensure!(refused.stdout.is_empty());
    git(repo.path(), &["restore", "--staged", "--", "archive"])?;
    let new_head = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--cacheinfo", &format!("160000,{new_head},sub")],
    )?;
    let refused = brit(repo.path(), &["diff", "--cached"])?;
    ensure!(!refused.status.success() && refused.stderr.contains("submodule patches"));
    ensure!(refused.stdout.is_empty());
    Ok(())
}

#[test]
fn ignored_paths_and_external_filters_refuse_atomically() -> Result<()> {
    let repo = fixture("filters")?;
    fs::write(repo.path().join(".gitignore"), "ignored\n")?;
    fs::write(repo.path().join("ignored"), "private ignored content\n")?;
    fs::write(repo.path().join(".gitattributes"), "filtered filter=external\n")?;
    fs::write(repo.path().join("filtered"), "filter input\n")?;
    git(
        repo.path(),
        &[
            "config",
            "filter.external.clean",
            "echo executed > .git/filter-executed; cat",
        ],
    )?;
    let before = index(repo.path())?;
    for path in ["ignored", "filtered", "../outside"] {
        let output = brit(repo.path(), &["add", "--", "selected", path])?;
        ensure!(!output.status.success(), "unsupported path must refuse: {path}");
        assert_eq!(index(repo.path())?, before, "batch refusal retains index");
    }
    assert_eq!(fs::read(repo.path().join("filtered"))?, b"filter input\n");
    ensure!(
        !repo.path().join(".git/filter-executed").exists(),
        "external filter must never execute"
    );
    Ok(())
}

#[test]
fn text_attributes_and_subdirectory_paths_match_git() -> Result<()> {
    let actual = fixture("text-attrs")?;
    let oracle = fixture("text-attrs-oracle")?;
    for repo in [&actual, &oracle] {
        fs::create_dir(repo.path().join("nested"))?;
        fs::write(repo.path().join(".gitattributes"), "*.txt text eol=lf\n")?;
        fs::write(repo.path().join("nested/file.txt"), "first\r\nsecond\r\n")?;
    }
    success(&actual.path().join("nested"), &["add", "--", "file.txt"])?;
    git(&oracle.path().join("nested"), &["add", "--", "file.txt"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    assert_eq!(git(actual.path(), &["show", ":nested/file.txt"])?, b"first\nsecond\n");
    assert_eq!(fs::read(actual.path().join("nested/file.txt"))?, b"first\r\nsecond\r\n");
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlink_ancestors_refuse_and_disabled_filemode_matches_git() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let actual = fixture("symlink-ancestor")?;
    let oracle = fixture("filemode-oracle")?;
    let external = tempfile::tempdir()?;
    fs::write(external.path().join("file"), "external content\n")?;
    symlink(external.path(), actual.path().join("escape"))?;
    let before = index(actual.path())?;
    ensure!(!brit(actual.path(), &["add", "--", "selected", "escape/file"])?
        .status
        .success());
    assert_eq!(index(actual.path())?, before);
    assert_eq!(fs::read(external.path().join("file"))?, b"external content\n");
    for repo in [&actual, &oracle] {
        git(repo.path(), &["config", "core.filemode", "false"])?;
        fs::set_permissions(repo.path().join("selected"), fs::Permissions::from_mode(0o755))?;
    }
    success(actual.path(), &["add", "--", "selected"])?;
    git(oracle.path(), &["add", "--", "selected"])?;
    assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    Ok(())
}

#[test]
fn legacy_diff_subcommands_remain_available_and_mixed_modes_refuse() -> Result<()> {
    let repo = fixture("legacy-diff")?;
    success(repo.path(), &["diff", "tree", "HEAD", "HEAD"])?;
    success(repo.path(), &["diff", "file", "HEAD:selected", "HEAD:selected"])?;
    let before = index(repo.path())?;
    for args in [
        vec!["diff", "--cached", "tree", "HEAD", "HEAD"],
        vec!["diff", "file", "--stat", "HEAD:selected", "HEAD:selected"],
        vec!["diff", "--", "private"],
    ] {
        ensure!(
            !brit(repo.path(), &args)?.status.success(),
            "unsupported diff must refuse: {args:?}"
        );
        assert_eq!(index(repo.path())?, before);
    }
    Ok(())
}

#[test]
fn selected_file_operations_preserve_unrelated_gitlinks() -> Result<()> {
    let repo = fixture("gitlink-neighbor")?;
    let head = repo.head_id()?;
    git(
        repo.path(),
        &["update-index", "--add", "--cacheinfo", &format!("160000,{head},sub")],
    )?;
    let gitlink = git(repo.path(), &["ls-files", "--stage", "--", "sub"])?;
    success(repo.path(), &["add", "--", "selected"])?;
    success(repo.path(), &["diff", "--cached", "--", "selected"])?;
    fs::write(repo.path().join("selected"), "new unstaged content\n")?;
    success(repo.path(), &["diff", "--", "selected"])?;
    success(repo.path(), &["restore", "--staged", "--", "selected"])?;
    assert_eq!(git(repo.path(), &["ls-files", "--stage", "--", "sub"])?, gitlink);
    assert_eq!(fs::read(repo.path().join("selected"))?, b"new unstaged content\n");
    Ok(())
}

#[test]
fn special_index_states_refuse_without_rewriting_any_entries() -> Result<()> {
    use std::{io::Write, process::Stdio};
    for state in ["--skip-worktree", "--assume-unchanged", "intent", "conflict"] {
        let repo = fixture("special-index")?;
        match state {
            "intent" => {
                fs::write(repo.path().join("pending"), "intent to add\n")?;
                git(repo.path(), &["add", "--intent-to-add", "--", "pending"])?;
            }
            "conflict" => {
                let blob = String::from_utf8(git(repo.path(), &["rev-parse", "HEAD:neighbor"])?)?;
                let mut command = gix_testtools::git_command(repo.path());
                let mut child = command
                    .args(["update-index", "--index-info"])
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()?;
                {
                    let input = child.stdin.as_mut().expect("piped index-info input");
                    writeln!(input, "0 {}\tneighbor", "0".repeat(40))?;
                    writeln!(input, "100644 {} 1\tneighbor", blob.trim())?;
                    writeln!(input, "100644 {} 2\tneighbor", blob.trim())?;
                }
                drop(child.stdin.take());
                let out = child.wait_with_output()?;
                ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            }
            flag => {
                git(repo.path(), &["update-index", flag, "--", "neighbor"])?;
            }
        }
        let before = index(repo.path())?;
        for args in [
            vec!["add", "--", "selected"],
            vec!["restore", "--staged", "--", "selected"],
            vec!["diff", "--cached", "--", "selected"],
            vec!["diff", "--", "selected"],
        ] {
            if state == "conflict" && args[0] == "add" {
                continue; // Conflict-tolerant staging has its own Git parity journey.
            }
            ensure!(
                !brit(repo.path(), &args)?.status.success(),
                "{state} must refuse {args:?}"
            );
            assert_eq!(index(repo.path())?, before, "{state}: index bytes remain intact");
            assert_eq!(fs::read(repo.path().join("selected"))?, b"selected change\n");
        }
    }
    Ok(())
}

#[test]
fn staged_changes_invalidate_cached_tree_before_commit() -> Result<()> {
    let actual = fixture("cached-tree")?;
    let oracle = fixture("cached-tree-oracle")?;
    let old_tree = git(actual.path(), &["write-tree"])?;
    ensure!(
        index(actual.path())?.windows(4).any(|bytes| bytes == b"TREE"),
        "fixture has cached TREE extension"
    );
    success(actual.path(), &["add", "--", "selected"])?;
    git(oracle.path(), &["add", "--", "selected"])?;
    let expected_tree = git(oracle.path(), &["write-tree"])?;
    assert_ne!(old_tree, expected_tree);
    success(actual.path(), &["commit", "-m", "updated cached tree"])?;
    assert_eq!(git(actual.path(), &["rev-parse", "HEAD^{tree}"])?, expected_tree);
    Ok(())
}

#[cfg(unix)]
#[test]
fn quoted_names_and_file_type_changes_match_git_patches() -> Result<()> {
    use std::os::unix::fs::symlink;
    let repo = fixture("quoted-patches")?;
    // Exact byte-path support is under test, not macOS Unicode normalization.
    git(repo.path(), &["config", "core.precomposeunicode", "false"])?;
    fs::remove_file(repo.path().join("selected"))?;
    symlink("neighbor", repo.path().join("selected"))?;
    for name in ["space name", "line\nbreak", "caf\u{e9}"] {
        fs::write(repo.path().join(name), "quoted path content\n")?;
    }
    success(
        repo.path(),
        &["add", "--", "selected", "space name", "line\nbreak", "caf\u{e9}"],
    )?;
    for quote in ["true", "false"] {
        git(repo.path(), &["config", "core.quotePath", quote])?;
        git(
            repo.path(),
            &["config", "core.abbrev", if quote == "true" { "7" } else { "12" }],
        )?;
        let before = index(repo.path())?;
        let output = success(repo.path(), &["diff", "--cached"])?;
        assert_eq!(
            output.stdout.as_bytes(),
            git(
                repo.path(),
                &["diff", "--cached", "--no-ext-diff", "--no-color", "--no-renames"]
            )?,
            "quotePath={quote}"
        );
        assert_eq!(index(repo.path())?, before);
    }
    Ok(())
}

#[test]
fn split_indexes_refuse_before_publication() -> Result<()> {
    let repo = fixture("split-index")?;
    git(repo.path(), &["update-index", "--split-index"])?;
    let before = index(repo.path())?;
    ensure!(
        before.windows(4).any(|bytes| bytes == b"link"),
        "fixture contains split-index link extension"
    );
    for args in [
        vec!["add", "--", "selected"],
        vec!["restore", "--staged", "--", "selected"],
    ] {
        ensure!(
            !brit(repo.path(), &args)?.status.success(),
            "split index must refuse {args:?}"
        );
        assert_eq!(index(repo.path())?, before);
        assert_eq!(fs::read(repo.path().join("selected"))?, b"selected change\n");
    }
    Ok(())
}

#[test]
fn case_aliases_and_unicode_precomposition_refuse_without_index_changes() -> Result<()> {
    let repo = fixture("path-aliases")?;
    git(repo.path(), &["config", "core.ignorecase", "true"])?;
    fs::write(repo.path().join("Selected"), "case alias content\n")?;
    let before = index(repo.path())?;
    for args in [
        vec!["add", "--", "Selected"],
        vec!["restore", "--staged", "--", "Selected"],
    ] {
        ensure!(
            !brit(repo.path(), &args)?.status.success(),
            "case alias must refuse {args:?}"
        );
        assert_eq!(index(repo.path())?, before);
    }
    git(repo.path(), &["config", "core.precomposeunicode", "true"])?;
    fs::write(repo.path().join("caf\u{e9}"), "unicode content\n")?;
    ensure!(!brit(repo.path(), &["add", "--", "caf\u{e9}"])?.status.success());
    assert_eq!(index(repo.path())?, before);
    assert_eq!(fs::read(repo.path().join("caf\u{e9}"))?, b"unicode content\n");
    Ok(())
}

// Construct all Git conflict shapes without a merge driver or shared repository state.
fn conflict_entries(repo: &Path, path: &str, stages: &[u8]) -> Result<()> {
    use std::{io::Write, process::Stdio};
    let blob = String::from_utf8(git(repo, &["rev-parse", "HEAD:selected"])?)?;
    let mut child = gix_testtools::git_command(repo)
        .args(["update-index", "--index-info"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    {
        let input = child.stdin.as_mut().expect("piped index input");
        writeln!(input, "0 {}\t{path}", "0".repeat(40))?;
        for stage in stages {
            writeln!(input, "100644 {} {stage}\t{path}", blob.trim())?;
        }
    }
    drop(child.stdin.take());
    let out = child.wait_with_output()?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Ok(())
}

#[test]
fn scoped_conflict_staging_and_resolve_undo_match_git() -> Result<()> {
    for stages in [&[1, 2, 3][..], &[2, 3], &[1, 2], &[1, 3], &[3]] {
        for deletion in [false, true] {
            for update in [false, true] {
                let actual = fixture("conflict-actual")?;
                let oracle = fixture("conflict-oracle")?;
                for repo in [&actual, &oracle] {
                    conflict_entries(repo.path(), "selected", stages)?;
                    conflict_entries(repo.path(), "unresolved", &[1, 2, 3])?;
                    conflict_entries(repo.path(), "previous", &[2, 3])?;
                    fs::write(repo.path().join("previous"), "earlier resolution\n")?;
                    git(repo.path(), &["add", "previous"])?;
                    if deletion {
                        fs::remove_file(repo.path().join("selected"))?;
                    }
                }
                let before = index(actual.path())?;
                ensure!(!brit(actual.path(), &["add", "--", "selected", "../outside"])?
                    .status
                    .success());
                assert_eq!(index(actual.path())?, before, "invalid batch must be atomic");
                let args = if update {
                    vec!["add", "-u", "--", "selected"]
                } else {
                    vec!["add", "--", "selected"]
                };
                success(actual.path(), &args)?;
                git(oracle.path(), &args)?;
                assert_eq!(
                    entries(actual.path())?,
                    entries(oracle.path())?,
                    "all index stages match Git"
                );
                assert_eq!(
                    git(actual.path(), &["ls-files", "--resolve-undo"])?,
                    git(oracle.path(), &["ls-files", "--resolve-undo"])?
                );
                assert_eq!(fs::read(actual.path().join("neighbor"))?, b"unstaged neighbor\n");
                if !deletion && stages == [1, 2, 3] {
                    git(actual.path(), &["checkout", "-m", "--", "selected"])?;
                    git(oracle.path(), &["checkout", "-m", "--", "selected"])?;
                    assert_eq!(
                        entries(actual.path())?,
                        entries(oracle.path())?,
                        "Git recreates resolved conflict"
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn staging_unrelated_file_preserves_conflicts_and_directory_selection_resolves_them() -> Result<()> {
    let actual = fixture("directory-conflicts")?;
    let oracle = fixture("directory-oracle")?;
    for repo in [&actual, &oracle] {
        fs::create_dir(repo.path().join("dir"))?;
        conflict_entries(repo.path(), "dir/file", &[1, 2, 3])?;
        fs::write(repo.path().join("dir/file"), "<<<<<<< deliberate bytes\n")?;
    }
    for args in [vec!["add", "--", "selected"], vec!["add", "--", "dir"]] {
        success(actual.path(), &args)?;
        git(oracle.path(), &args)?;
        assert_eq!(entries(actual.path())?, entries(oracle.path())?);
        assert_eq!(
            git(actual.path(), &["ls-files", "--resolve-undo"])?,
            git(oracle.path(), &["ls-files", "--resolve-undo"])?
        );
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn conflict_resolution_modes_and_lock_refusal_match_git() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    for link in [false, true] {
        let actual = fixture("conflict-mode")?;
        let oracle = fixture("conflict-mode-oracle")?;
        for repo in [&actual, &oracle] {
            conflict_entries(repo.path(), "selected", &[1, 2, 3])?;
            if link {
                fs::remove_file(repo.path().join("selected"))?;
                symlink("neighbor", repo.path().join("selected"))?;
            } else {
                fs::set_permissions(repo.path().join("selected"), fs::Permissions::from_mode(0o755))?;
            }
        }
        let before = index(actual.path())?;
        fs::write(actual.path().join(".git/index.lock"), "held")?;
        ensure!(!brit(actual.path(), &["add", "--", "selected"])?.status.success());
        assert_eq!(index(actual.path())?, before);
        fs::remove_file(actual.path().join(".git/index.lock"))?;
        success(actual.path(), &["add", "--", "selected"])?;
        git(oracle.path(), &["add", "--", "selected"])?;
        assert_eq!(entries(actual.path())?, entries(oracle.path())?);
    }
    Ok(())
}
