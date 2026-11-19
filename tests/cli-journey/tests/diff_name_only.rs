//! Changed paths between two revisions, with `git diff --name-only --no-renames` as the oracle.
//!
//! Agents select gates and reviewers from "which files differ between A and the
//! merge base of A and B". Without this, each such question fell back to Git.
use std::{fs, path::Path, process::Command};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        out.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?)
}

fn brit(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    Ok(command.current_dir(root).args(args).output()?)
}

fn commit_all(root: &Path, message: &str) -> Result<()> {
    git(root, &["add", "-A"])?;
    git(root, &["commit", "-q", "-m", message])?;
    Ok(())
}

/// The fixture diverges `topic` from `main` after a shared base so that `A..B`
/// (two-tree) and `A...B` (merge-base) select different path sets. The names
/// `a-b`, `a.txt` and `a/c` sit on both sides of `/` in byte order, a rename is
/// listed as deletion plus addition, and a gitlink is reported by its path.
#[test]
fn name_only_lists_changed_paths_for_two_trees_ranges_and_merge_bases() -> Result<()> {
    let repo = TestRepo::new("diff-name-only")?;
    let root = repo.path();
    fs::create_dir_all(root.join("a"))?;
    for (path, content) in [("a-b", "1\n"), ("a.txt", "1\n"), ("a/c", "1\n"), ("moved", "keep\n")] {
        fs::write(root.join(path), content)?;
    }
    commit_all(root, "base")?;
    git(root, &["branch", "topic"])?;
    fs::write(root.join("a.txt"), "main\n")?;
    commit_all(root, "main side")?;
    git(root, &["switch", "-q", "topic"])?;
    fs::write(root.join("a-b"), "topic\n")?;
    fs::write(root.join("a/c"), "topic\n")?;
    fs::rename(root.join("moved"), root.join("renamed"))?;
    fs::write(root.join("new file"), "added\n")?;
    // Non-ASCII names are quoted under the default `core.quotePath`.
    fs::write(root.join("caf\u{e9}"), "added\n")?;
    commit_all(root, "topic side")?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},sub", head.trim()),
        ],
    )?;
    git(root, &["commit", "-q", "-m", "gitlink"])?;

    let cases: [(&[&str], &[&str]); 5] = [
        (&["main", "topic"], &["main", "topic"]),
        (&["topic", "main"], &["topic", "main"]),
        (&["main..topic"], &["main..topic"]),
        (&["main...topic"], &["main...topic"]),
        (&["--merge-base", "main", "topic"], &["--merge-base", "main", "topic"]),
    ];
    for (native, oracle) in cases {
        let mut oracle_args = vec!["diff", "--no-ext-diff", "--no-renames", "--name-only"];
        oracle_args.extend(oracle);
        let expected = git(root, &oracle_args)?;
        ensure!(!expected.is_empty(), "fixture yields changes for {oracle:?}");
        let mut native_args = vec!["diff", "tree", "--name-only"];
        native_args.extend(native);
        let out = brit(root, &native_args)?;
        ensure!(
            out.status.success(),
            "Brit {native:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8(out.stdout)?,
            expected,
            "brit diff tree --name-only {native:?} lists Git's paths in Git's order"
        );
    }
    Ok(())
}

/// One-argument forms must be ranges, and `--name-only` excludes `--stat`.
#[test]
fn name_only_refuses_ambiguous_forms() -> Result<()> {
    let repo = TestRepo::new("diff-name-only-refusals")?;
    repo.commit_file("tracked", "one\n")?;
    repo.commit_file("tracked", "two\n")?;
    for args in [
        &["diff", "tree", "--name-only", "HEAD"][..],
        &["diff", "tree", "--name-only", "--stat", "HEAD~1", "HEAD"][..],
        &["diff", "tree", "--name-only", "--merge-base", "HEAD~1..HEAD"][..],
    ] {
        let out = brit(repo.path(), args)?;
        assert!(!out.status.success(), "{args:?} must be refused");
        assert!(out.stdout.is_empty(), "{args:?} prints no paths");
    }
    Ok(())
}
