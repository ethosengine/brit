//! Bridged Git operations print the stock Git command, exit 2 and touch nothing.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    process::Command,
};

/// The documented usage exit code (`CliError::Args`), which every bridge refusal uses.
const EXIT_CODE: i32 = 2;

/// (arguments after `brit`, the exact stderr line). The lines are written out in full rather than
/// derived, so the test does not repeat the implementation's formatting.
const ROWS: &[(&[&str], &str)] = &[
    (
        &["reset", "--hard", "HEAD"],
        "brit: reset is a Git bridge operation; run: git reset --hard HEAD [fb brit/capability-absent:reset]",
    ),
    (
        &["rm", "--cached", "file"],
        "brit: rm is a Git bridge operation; run: git rm --cached file [fb brit/capability-absent:rm]",
    ),
    (&["mv", "a", "b"], "brit: mv is a Git bridge operation; run: git mv a b [fb brit/capability-absent:mv]"),
    (
        &["stash", "push", "-m", "wip item"],
        "brit: stash is a Git bridge operation; run: git stash push -m 'wip item' [fb brit/capability-absent:stash]",
    ),
    (
        &["revert", "--no-edit", "HEAD"],
        "brit: revert is a Git bridge operation; run: git revert --no-edit HEAD [fb brit/capability-absent:revert]",
    ),
    (
        &["checkout", "-b", "feature"],
        "brit: checkout is a Git bridge operation; run: git checkout -b feature [fb brit/capability-absent:checkout]",
    ),
    (
        &["notes", "add", "-m", "note", "HEAD"],
        "brit: notes is a Git bridge operation; run: git notes add -m note HEAD [fb brit/capability-absent:notes]",
    ),
    (
        &["submodule", "update", "--init"],
        "brit: submodule update is a Git bridge operation; run: git submodule update --init [fb brit/capability-absent:submodule.update]",
    ),
    (
        &["-C", ".", "submodule", "update", "--merge", "child"],
        "brit: submodule update is a Git bridge operation; run: git -C . submodule update --merge child [fb brit/capability-absent:submodule.update]",
    ),
    (
        &["submodule", "init"],
        "brit: submodule init is a Git bridge operation; run: git submodule init [fb brit/capability-absent:submodule.init]",
    ),
    (
        &["submodule", "sync", "--recursive"],
        "brit: submodule sync is a Git bridge operation; run: git submodule sync --recursive [fb brit/capability-absent:submodule.sync]",
    ),
];

fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).expect("read dir") {
            let path = entry.expect("dir entry").path();
            let relative = path.strip_prefix(root).expect("under root").to_owned();
            if path.is_dir() {
                out.insert(relative, Vec::new());
                walk(root, &path, out);
            } else {
                out.insert(relative, std::fs::read(&path).expect("read file"));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// A repository on which the mutating rows (reset, rm, mv, stash, revert, checkout, notes), had
/// Git run them, would change bytes: two commits, a side branch, a tracked file with an unstaged
/// edit and an untracked file.
fn repository_git_would_change() -> gix_testtools::tempfile::TempDir {
    let repo = gix_testtools::tempfile::tempdir().expect("temporary repository");
    let dir = repo.path();
    gix_testtools::git(dir, "init").expect("initialize fixture");
    std::fs::write(dir.join("file"), "content\n").expect("write file");
    std::fs::write(dir.join("a"), "a\n").expect("write file");
    gix_testtools::git(dir, "add file a").expect("stage fixture");
    gix_testtools::git(dir, "commit -m initial").expect("commit fixture");
    gix_testtools::git(dir, "branch topic").expect("branch fixture");
    std::fs::write(dir.join("a"), "a\nsecond\n").expect("write file");
    gix_testtools::git(dir, "commit -am second").expect("commit fixture");
    std::fs::write(dir.join("file"), "content\nunstaged\n").expect("dirty file");
    std::fs::write(dir.join("untracked"), "untracked\n").expect("write untracked");
    repo
}

fn brit(repo: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, repo);
    command
        .current_dir(repo)
        .env_remove("EPR_FEEDBACK_ROOT")
        .args(args)
        .output()
        .expect("run brit")
}

#[test]
fn every_bridged_row_prints_its_git_line_exits_two_and_leaves_the_repository_unchanged() {
    let repo = repository_git_would_change();
    let before = snapshot(repo.path());
    for (args, line) in ROWS {
        let output = brit(repo.path(), args);
        assert_eq!(output.status.code(), Some(EXIT_CODE), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: nothing on stdout: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stderr), format!("{line}\n"), "{args:?}");
        assert_eq!(snapshot(repo.path()), before, "{args:?} must not change the repository");
    }
}

/// The unchanged-repository assertion above is only meaningful if Git itself would have changed
/// the fixture: run each mutating row with stock Git on a fresh fixture and require a change.
#[test]
fn stock_git_would_change_the_fixture_for_each_mutating_row() {
    for name in ["reset", "rm", "mv", "stash", "revert", "checkout", "notes"] {
        let (args, _) = ROWS.iter().find(|(args, _)| args[0] == name).expect("row exists");
        let repo = repository_git_would_change();
        let before = snapshot(repo.path());
        let status = gix_testtools::git_command(repo.path())
            .args(*args)
            .output()
            .expect("run git")
            .status;
        assert!(status.success(), "git {args:?} must succeed on the fixture");
        assert_ne!(snapshot(repo.path()), before, "git {args:?} must change the fixture");
    }
}

#[test]
fn owned_submodule_list_is_not_a_bridge() {
    let repo = repository_git_would_change();
    let output = brit(repo.path(), &["submodule", "list"]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("Git bridge operation"),
        "{output:?}"
    );
}
