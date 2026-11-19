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
