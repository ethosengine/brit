//! `brit log [REV] [-n N] [--oneline] [-- PATH…]`: start revision, literal path filters and the
//! one-line listing, checked against Git as the oracle on linear history.
use std::{
    path::{Path, PathBuf},
    process::Command,
};

fn git(dir: &Path, args: &[&str]) -> String {
    let mut command = Command::new("git");
    gix_testtools::configure_git_environment(&mut command, dir);
    let out = command.current_dir(dir).args(args).output().expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

fn brit_output(dir: &Path, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_brit"));
    gix_testtools::configure_git_environment(&mut command, dir);
    command
        .current_dir(dir)
        .env_remove("EPR_FEEDBACK_ROOT")
        .args(args)
        .output()
        .expect("brit")
}

fn brit(dir: &Path, args: &[&str]) -> String {
    let out = brit_output(dir, args);
    assert!(
        out.status.success(),
        "brit {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("utf8")
}

fn titles(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(|line| line.split_once(' ').expect("hash and title").1.to_owned())
        .collect()
}

/// Git's subjects for the same arguments, the oracle for linear history.
fn git_titles(dir: &Path, args: &[&str]) -> Vec<String> {
    let mut full = vec!["log", "--format=%s"];
    full.extend_from_slice(args);
    git(dir, &full).lines().map(ToOwned::to_owned).collect()
}

fn commit(dir: &Path, n: u32, message: &str) {
    let mut command = Command::new("git");
    gix_testtools::configure_git_environment(&mut command, dir);
    let date = format!("2000-01-{:02} 00:00:00 +0000", n + 2);
    let out = command
        .current_dir(dir)
        .env("GIT_COMMITTER_DATE", &date)
        .env("GIT_AUTHOR_DATE", &date)
        .args(["commit", "-q", "-m", message])
        .output()
        .expect("git commit");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
}

fn write(dir: &Path, path: &str, content: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    std::fs::write(path, content).expect("write");
}

/// c0: add a/x.txt and top.txt; c1: change top.txt; c2: change a/x.txt; c3: add b/y.txt;
/// c4: make top.txt executable (a mode-only change).
fn fixture() -> (gix_testtools::tempfile::TempDir, PathBuf) {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = tmp.path().join("repo");
    std::fs::create_dir(&dir).expect("repo dir");
    git(&dir, &["init", "-q", "-b", "main"]);
    write(&dir, "a/x.txt", "x0\n");
    write(&dir, "top.txt", "t0\n");
    git(&dir, &["add", "."]);
    commit(&dir, 0, "c0 add a and top");
    write(&dir, "top.txt", "t1\n");
    git(&dir, &["add", "."]);
    commit(&dir, 1, "c1 change top");
    write(&dir, "a/x.txt", "x2\n");
    git(&dir, &["add", "."]);
    commit(&dir, 2, "c2 change a/x");
    write(&dir, "b/y.txt", "y3\n");
    git(&dir, &["add", "."]);
    commit(&dir, 3, "c3 add b");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir.join("top.txt"), std::fs::Permissions::from_mode(0o755)).expect("chmod");
        git(&dir, &["add", "top.txt"]);
    }
    #[cfg(not(unix))]
    git(&dir, &["update-index", "--chmod=+x", "top.txt"]);
    commit(&dir, 4, "c4 chmod top");
    (tmp, dir)
}

#[test]
fn revision_argument_starts_the_walk() {
    let (_tmp, dir) = fixture();
    for args in [
        &["log", "HEAD~2"][..],
        &["log", "-n", "1", "HEAD~3"][..],
        &["log", "main"][..],
        &["log", "HEAD"][..],
    ] {
        assert_eq!(titles(&brit(&dir, args)), git_titles(&dir, &args[1..]), "{args:?}");
    }
    assert_eq!(
        titles(&brit(&dir, &["log", "HEAD~2"])),
        ["c2 change a/x", "c1 change top", "c0 add a and top"]
    );
    git(&dir, &["tag", "-a", "v1", "-m", "annotated", "HEAD~1"]);
    assert_eq!(titles(&brit(&dir, &["log", "-n", "1", "v1"])), ["c3 add b"]);

    let out = brit_output(&dir, &["log", "no-such-revision"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let tree = git(&dir, &["rev-parse", "HEAD^{tree}"]);
    let out = brit_output(&dir, &["log", tree.trim()]);
    assert!(!out.status.success(), "a tree is not a commit");
    assert!(String::from_utf8_lossy(&out.stderr).contains("does not point to a commit"));
}

#[test]
fn paths_filter_files_directories_and_revisions() {
    let (_tmp, dir) = fixture();
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["--", "top.txt"],
            &["c4 chmod top", "c1 change top", "c0 add a and top"],
        ),
        (&["--", "a"], &["c2 change a/x", "c0 add a and top"]),
        (&["--", "a/"], &["c2 change a/x", "c0 add a and top"]),
        (&["--", "./a/x.txt"], &["c2 change a/x", "c0 add a and top"]),
        (&["--", "a", "b"], &["c3 add b", "c2 change a/x", "c0 add a and top"]),
        (&["HEAD~3", "--", "a"], &["c0 add a and top"]),
        (&["HEAD~1", "--", "top.txt"], &["c1 change top", "c0 add a and top"]),
        (&["-n", "1", "--", "a"], &["c2 change a/x"]),
        (&["-1", "--", "top.txt"], &["c4 chmod top"]),
        (&["--", "missing"], &[]),
        (&["--", "a/missing"], &[]),
        (
            &["--", "."],
            &[
                "c4 chmod top",
                "c3 add b",
                "c2 change a/x",
                "c1 change top",
                "c0 add a and top",
            ],
        ),
    ];
    for (args, expected) in cases {
        let mut full = vec!["log"];
        full.extend_from_slice(args);
        let actual = titles(&brit(&dir, &full));
        assert_eq!(actual, *expected, "{full:?}");
        let oracle_args: Vec<_> = args.iter().map(|a| if *a == "-1" { "-n1" } else { a }).collect();
        assert_eq!(actual, git_titles(&dir, &oracle_args), "git parity for {full:?}");
    }
    // An existing worktree path is accepted without `--` when it is not a revision, like Git.
    assert_eq!(titles(&brit(&dir, &["log", "top.txt"])), git_titles(&dir, &["top.txt"]));
    // Paths are relative to the current directory inside the worktree.
    assert_eq!(
        titles(&brit(&dir.join("a"), &["log", "--", "x.txt"])),
        ["c2 change a/x", "c0 add a and top"]
    );
    assert_eq!(
        titles(&brit(&dir.join("a"), &["log", "--", "../top.txt"])),
        git_titles(&dir, &["--", "top.txt"])
    );
    let out = brit_output(&dir, &["log", "--", "../outside"]);
    assert!(!out.status.success(), "a path outside the worktree is refused");
}

#[test]
fn oneline_abbreviates_like_git_and_default_output_is_unchanged() {
    let (_tmp, dir) = fixture();
    assert_eq!(brit(&dir, &["log", "--oneline"]), git(&dir, &["log", "--oneline"]));
    assert_eq!(
        brit(&dir, &["log", "--oneline", "-n", "2", "--", "a"]),
        git(&dir, &["log", "--oneline", "-n", "2", "--", "a"])
    );
    assert_eq!(
        brit(&dir, &["log", "--oneline", "HEAD~1"]),
        git(&dir, &["log", "--oneline", "HEAD~1"])
    );
    // The default listing keeps its fixed eight-digit hashes.
    assert_eq!(
        brit(&dir, &["log", "-n", "2"]),
        git(&dir, &["log", "-n", "2", "--abbrev=8", "--format=%h %s"])
    );
}

#[test]
fn shallow_boundary_ends_the_walk_cleanly() {
    let (tmp, dir) = fixture();
    let clone = tmp.path().join("shallow");
    let url = format!("file://{}", dir.display());
    git(
        tmp.path(),
        &["clone", "-q", "--depth", "2", &url, clone.to_str().expect("utf8 path")],
    );
    assert!(clone.join(".git/shallow").is_file());
    for args in [
        &["log"][..],
        &["log", "HEAD"][..],
        &["log", "-n", "5", "HEAD"][..],
        &["log", "--", "top.txt"][..],
        &["log", "--", "b"][..],
        &["log", "--", "a"][..],
        &["log", "HEAD~1", "--", "a"][..],
    ] {
        assert_eq!(titles(&brit(&clone, args)), git_titles(&clone, &args[1..]), "{args:?}");
    }
    // The boundary commit counts as a root: every path it contains is listed there.
    assert_eq!(titles(&brit(&clone, &["log", "--", "a"])), ["c3 add b"]);
}

#[test]
fn linked_worktree_reads_shared_history() {
    let (tmp, dir) = fixture();
    let linked = tmp.path().join("linked");
    git(
        &dir,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "side",
            linked.to_str().expect("utf8 path"),
            "HEAD~2",
        ],
    );
    assert_eq!(
        titles(&brit(&linked, &["log", "--", "a"])),
        ["c2 change a/x", "c0 add a and top"]
    );
    assert_eq!(titles(&brit(&linked, &["log", "-n", "1", "main"])), ["c4 chmod top"]);
    assert_eq!(
        brit(&linked, &["log", "--oneline", "main", "--", "b"]),
        git(&linked, &["log", "--oneline", "main", "--", "b"])
    );
}

#[test]
fn merges_matching_any_parent_are_not_listed() {
    let (_tmp, dir) = fixture();
    git(&dir, &["switch", "-q", "-c", "topic", "HEAD~1"]);
    write(&dir, "side.txt", "s\n");
    git(&dir, &["add", "."]);
    commit(&dir, 5, "s1 add side");
    git(&dir, &["switch", "-q", "main"]);
    let mut command = Command::new("git");
    gix_testtools::configure_git_environment(&mut command, &dir);
    let out = command
        .current_dir(&dir)
        .env("GIT_COMMITTER_DATE", "2000-01-09 00:00:00 +0000")
        .args(["merge", "-q", "--no-ff", "-m", "m1 merge topic", "topic"])
        .output()
        .expect("merge");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // The merge differs from its first parent at side.txt but matches the side it came from,
    // so, as in Git, only the commit that made the change is listed.
    assert_eq!(titles(&brit(&dir, &["log", "--", "side.txt"])), ["s1 add side"]);
    assert_eq!(
        titles(&brit(&dir, &["log", "--", "top.txt"])),
        ["c4 chmod top", "c1 change top", "c0 add a and top"]
    );
    for args in [
        &["--", "side.txt"][..],
        &["--", "top.txt"][..],
        &["--", "a", "side.txt"][..],
        &["--", "."][..],
        &[][..],
    ] {
        let mut full = vec!["log", "HEAD"];
        full.extend_from_slice(args);
        assert_eq!(titles(&brit(&dir, &full)), git_titles(&dir, &full[1..]), "{full:?}");
    }
    // An evil merge that changes a path relative to both parents is listed.
    git(&dir, &["switch", "-q", "-c", "evil", "topic"]);
    git(&dir, &["merge", "-q", "--no-ff", "--no-commit", "main"]);
    write(&dir, "side.txt", "evil\n");
    git(&dir, &["add", "."]);
    commit(&dir, 8, "e1 evil merge");
    assert_eq!(
        titles(&brit(&dir, &["log", "-n", "1", "--", "side.txt"])),
        ["e1 evil merge"]
    );
    assert_eq!(
        titles(&brit(&dir, &["log", "--", "side.txt"])),
        git_titles(&dir, &["--", "side.txt"])
    );
}

fn git_at(dir: &Path, date: &str, args: &[&str]) {
    let mut command = Command::new("git");
    gix_testtools::configure_git_environment(&mut command, dir);
    let out = command
        .current_dir(dir)
        .env("GIT_COMMITTER_DATE", date)
        .env("GIT_AUTHOR_DATE", date)
        .args(args)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn day(n: u32) -> String {
    format!("2000-02-{n:02} 00:00:00 +0000")
}

fn commit_all(dir: &Path, n: u32, message: &str) {
    git(dir, &["add", "-A", "."]);
    git_at(dir, &day(n), &["commit", "-q", "--allow-empty", "-m", message]);
}

fn merge(dir: &Path, n: u32, branch: &str, message: &str) {
    git_at(dir, &day(n), &["merge", "-q", "--no-ff", "-m", message, branch]);
}

fn init(tmp: &gix_testtools::tempfile::TempDir) -> PathBuf {
    let dir = tmp.path().join("repo");
    std::fs::create_dir(&dir).expect("repo dir");
    git(&dir, &["init", "-q", "-b", "main"]);
    dir
}

/// Every listing of `paths` (also with each `-n` up to its length) matches `git log`.
fn assert_git_parity(dir: &Path, paths: &[&str]) {
    let mut args = vec!["--"];
    args.extend_from_slice(paths);
    let expected = git_titles(dir, &args);
    let mut full = vec!["log", "--"];
    full.extend_from_slice(paths);
    assert_eq!(titles(&brit(dir, &full)), expected, "{full:?}");
    for n in 1..=expected.len() {
        let count = n.to_string();
        let mut limited = vec!["log", "-n", &count, "--"];
        limited.extend_from_slice(paths);
        assert_eq!(titles(&brit(dir, &limited)), expected[..n], "{limited:?}");
    }
    let mut oneline = vec!["log", "--oneline", "--"];
    oneline.extend_from_slice(paths);
    assert_eq!(brit(dir, &oneline), git(dir, &oneline), "{oneline:?}");
}

#[test]
fn a_merge_matching_its_first_parent_prunes_the_side_branch() {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = init(&tmp);
    write(&dir, "a", "a\n");
    commit_all(&dir, 1, "root adds a");
    git(&dir, &["switch", "-q", "-c", "side"]);
    write(&dir, "f", "F\n");
    commit_all(&dir, 2, "side-adds-f");
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "u", "u\n");
    commit_all(&dir, 3, "main-unrelated");
    write(&dir, "f", "F\n");
    commit_all(&dir, 4, "main-adds-f-same");
    merge(&dir, 5, "side", "merge side");
    assert_eq!(titles(&brit(&dir, &["log", "--", "f"])), ["main-adds-f-same"]);
    assert_git_parity(&dir, &["f"]);
    assert_git_parity(&dir, &["a", "f"]);
}

#[test]
fn a_side_branch_that_restores_the_path_is_pruned() {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = init(&tmp);
    write(&dir, "f", "0\n");
    commit_all(&dir, 1, "root adds f");
    git(&dir, &["switch", "-q", "-c", "side"]);
    write(&dir, "f", "1\n");
    commit_all(&dir, 2, "side changes f");
    write(&dir, "f", "0\n");
    commit_all(&dir, 3, "side restores f");
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "u", "u\n");
    commit_all(&dir, 4, "main unrelated");
    merge(&dir, 5, "side", "merge side");
    assert_eq!(titles(&brit(&dir, &["log", "--", "f"])), ["root adds f"]);
    assert_git_parity(&dir, &["f"]);

    // An evil merge keeps both parents, yet the side branch pruned further down stays pruned.
    git(&dir, &["switch", "-q", "-c", "side2"]);
    write(&dir, "f", "2\n");
    commit_all(&dir, 6, "side2 changes f");
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "f", "3\n");
    commit_all(&dir, 7, "main changes f");
    // Both sides changed f, so this merge stops with a conflict that the next commit resolves.
    let mut command = Command::new("git");
    gix_testtools::configure_git_environment(&mut command, &dir);
    let out = command
        .current_dir(&dir)
        .args(["merge", "-q", "--no-ff", "--no-commit", "side2"])
        .output()
        .expect("merge");
    assert!(!out.status.success(), "the merge conflicts at f");
    write(&dir, "f", "4\n");
    commit_all(&dir, 9, "evil merge resolves f");
    assert_eq!(
        titles(&brit(&dir, &["log", "--", "f"])),
        [
            "evil merge resolves f",
            "main changes f",
            "side2 changes f",
            "root adds f"
        ]
    );
    assert_git_parity(&dir, &["f"]);
    assert_git_parity(&dir, &["u", "f"]);
}

#[test]
fn trailing_slash_matches_only_directories() {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = init(&tmp);
    write(&dir, "f", "x\n");
    write(&dir, "d/x", "x\n");
    commit_all(&dir, 1, "c0 f file, d dir");
    write(&dir, "f", "y\n");
    commit_all(&dir, 2, "c1 f file changes");
    std::fs::remove_file(dir.join("f")).expect("rm f");
    write(&dir, "f/g", "g\n");
    commit_all(&dir, 3, "c2 f becomes a directory");
    write(&dir, "f/g", "h\n");
    std::fs::remove_dir_all(dir.join("d")).expect("rm d");
    commit_all(&dir, 4, "c3 f/g changes, d deleted");
    std::fs::remove_dir_all(dir.join("f")).expect("rm f/");
    write(&dir, "f", "z\n");
    commit_all(&dir, 5, "c4 f becomes a file again");
    write(&dir, "f", "w\n");
    commit_all(&dir, 6, "c5 f file changes");
    std::fs::remove_file(dir.join("f")).expect("rm f");
    commit_all(&dir, 7, "c6 f deleted");
    assert_eq!(
        titles(&brit(&dir, &["log", "--", "f/"])),
        [
            "c4 f becomes a file again",
            "c3 f/g changes, d deleted",
            "c2 f becomes a directory"
        ]
    );
    for paths in [&["f/"][..], &["f"], &["f/g"], &["d"], &["d/"], &["d/x"], &["f/", "d/"]] {
        assert_git_parity(&dir, paths);
    }
}

#[test]
fn a_gitlink_is_a_leaf() {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = init(&tmp);
    write(&dir, "file", "0\n");
    git(&dir, &["add", "file"]);
    let first = "1111111111111111111111111111111111111111";
    let second = "2222222222222222222222222222222222222222";
    git(
        &dir,
        &["update-index", "--add", "--cacheinfo", &format!("160000,{first},sub")],
    );
    git_at(&dir, &day(1), &["commit", "-q", "-m", "add gitlink"]);
    git(&dir, &["update-index", "--cacheinfo", &format!("160000,{second},sub")]);
    git_at(&dir, &day(2), &["commit", "-q", "-m", "move gitlink"]);
    write(&dir, "file", "1\n");
    git(&dir, &["add", "file"]);
    git_at(&dir, &day(3), &["commit", "-q", "-m", "unrelated"]);
    assert_eq!(
        titles(&brit(&dir, &["log", "--", "sub"])),
        ["move gitlink", "add gitlink"]
    );
    for paths in [&["sub"][..], &["sub/"], &["sub/inner"]] {
        assert_git_parity(&dir, paths);
    }
}

#[test]
fn clock_skew_keeps_every_commit_on_the_path() {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = init(&tmp);
    write(&dir, "f", "0\n");
    commit_all(&dir, 10, "f0 newer root");
    write(&dir, "f", "1\n");
    commit_all(&dir, 5, "f1 older than its parent");
    write(&dir, "g", "1\n");
    commit_all(&dir, 20, "g1 newest");
    write(&dir, "f", "2\n");
    commit_all(&dir, 3, "f2 oldest tip");
    assert_eq!(
        titles(&brit(&dir, &["log", "--", "f"])),
        ["f2 oldest tip", "f1 older than its parent", "f0 newer root"]
    );
    assert_git_parity(&dir, &["f"]);
    assert_git_parity(&dir, &["f", "g"]);
}

/// `brit log <specs> [-- <paths>]` matches `git log --date-order <specs>` without paths and
/// `git log <specs> -- <paths>` with them, for every `-n` and with `--oneline`.
fn assert_specs_parity(dir: &Path, specs: &[&str], paths: &[&str]) {
    let mut selection: Vec<&str> = specs.to_vec();
    if paths.is_empty() {
        selection.insert(0, "--date-order");
    } else {
        selection.push("--");
        selection.extend_from_slice(paths);
    }
    let expected = git_titles(dir, &selection);
    fn log_args<'a>(prefix: &[&'a str], specs: &[&'a str], paths: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec!["log"];
        args.extend_from_slice(prefix);
        args.extend_from_slice(specs);
        if !paths.is_empty() {
            args.push("--");
            args.extend_from_slice(paths);
        }
        args
    }
    let native = |prefix: &[&'static str]| log_args(prefix, specs, paths);
    assert_eq!(titles(&brit(dir, &native(&[]))), expected, "{:?}", native(&[]));
    for n in 1..=expected.len() {
        let count = n.to_string();
        let args = log_args(&["-n", &count], specs, paths);
        assert_eq!(titles(&brit(dir, &args)), expected[..n], "{args:?}");
    }
    let mut oneline = vec!["log", "--oneline"];
    oneline.extend(&selection);
    assert_eq!(
        brit(dir, &native(&["--oneline"])),
        git(dir, &oneline),
        "{:?}",
        native(&["--oneline"])
    );
}

/// Two branches off a root, a merge, a second topic off an older commit and a commit dated
/// before its parent, so that ranges, exclusions and three-dot specs select different sets.
fn branchy() -> (gix_testtools::tempfile::TempDir, PathBuf) {
    let tmp = gix_testtools::tempfile::tempdir().expect("fixture");
    let dir = init(&tmp);
    for name in ["f", "g", "h"] {
        write(&dir, name, "0\n");
    }
    commit_all(&dir, 1, "c1 root");
    write(&dir, "f", "1\n");
    commit_all(&dir, 2, "c2 main f");
    git(&dir, &["branch", "old"]);
    git(&dir, &["switch", "-q", "-c", "topic", "HEAD~1"]);
    write(&dir, "g", "1\n");
    commit_all(&dir, 3, "t1 topic g");
    write(&dir, "h", "1\n");
    commit_all(&dir, 4, "t2 topic h");
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "f", "2\n");
    commit_all(&dir, 5, "c3 main f");
    merge(&dir, 6, "topic", "m1 merge topic");
    write(&dir, "g", "2\n");
    commit_all(&dir, 7, "c4 main g");
    git(&dir, &["switch", "-q", "-c", "topic2", "old"]);
    write(&dir, "k", "1\n");
    commit_all(&dir, 8, "u1 topic2 k");
    write(&dir, "h", "5\n");
    commit_all(&dir, 9, "u2 topic2 h");
    git(&dir, &["switch", "-q", "main"]);
    write(&dir, "h", "2\n");
    commit_all(&dir, 10, "c5 main h");
    write(&dir, "f", "3\n");
    commit_all(&dir, 4, "c6 main f, dated before its parent");
    (tmp, dir)
}

const SPEC_CASES: &[(&[&str], &[&str])] = &[
    (&["old..main"], &[]),
    (&["topic...main"], &[]),
    (&["main", "^topic", "topic2"], &[]),
    (&["old..main"], &["f"]),
    (&["old..main"], &["g", "h"]),
    (&["main", "^old"], &["g"]),
    (&["main", "^topic"], &["f", "g", "h"]),
    (&["topic...main"], &["f"]),
    (&["topic...main"], &["g"]),
    (&["main...topic2"], &["k", "f", "h"]),
    (&["topic2...main"], &["h"]),
    (&["topic..main"], &["h"]),
    (&["main~1^!"], &["g"]),
    (&["main^@"], &["f"]),
    (&["old..main", "topic2"], &["h"]),
    (&["^main", "topic2"], &["f", "h", "k"]),
    (&["main..main"], &["f"]),
    (&["main", "topic2"], &["h"]),
];

#[test]
fn ranges_exclusions_and_three_dot_specs_combine_with_paths() {
    let (_tmp, dir) = branchy();
    for (specs, paths) in SPEC_CASES {
        assert_specs_parity(&dir, specs, paths);
    }
    assert_eq!(
        titles(&brit(&dir, &["log", "old..main", "--", "f"])),
        ["c6 main f, dated before its parent", "c3 main f"]
    );
}

#[test]
fn ranges_work_in_shallow_clones_and_linked_worktrees() {
    let (tmp, dir) = branchy();
    let clone = tmp.path().join("shallow");
    let url = format!("file://{}", dir.display());
    git(
        tmp.path(),
        &[
            "clone",
            "-q",
            "--depth",
            "4",
            "--no-single-branch",
            &url,
            clone.to_str().expect("utf8 path"),
        ],
    );
    assert!(clone.join(".git/shallow").is_file());
    for (specs, paths) in [
        (&["HEAD~2..HEAD"][..], &[][..]),
        (&["HEAD~2..HEAD"], &["f"]),
        (&["HEAD~1..HEAD"], &["h"]),
        (&[], &["g"]),
        (&["origin/topic2..HEAD"], &[]),
        (&["origin/topic2..HEAD"], &["h"]),
        (&["HEAD...origin/topic2"], &["h", "k"]),
        (&["HEAD"], &[]),
    ] {
        assert_specs_parity(&clone, specs, paths);
    }

    let linked = tmp.path().join("linked");
    git(
        &dir,
        &["worktree", "add", "-q", linked.to_str().expect("utf8 path"), "topic2"],
    );
    for (specs, paths) in [
        (&["old..main"][..], &["f"][..]),
        (&["HEAD..main"], &[]),
        (&[], &["k"]),
        (&["main...HEAD"], &["h"]),
        (&["main", "^HEAD"], &["f", "g"]),
    ] {
        assert_specs_parity(&linked, specs, paths);
    }
}
