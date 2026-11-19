//! Revision ranges use Git's reachable-set exclusion, including skewed dates.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        out.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

fn commit(root: &Path, tree: &str, parents: &[&str], time: &str) -> Result<String> {
    let mut command = gix_testtools::git_command(root);
    command.args(["commit-tree", tree, "-m", time]);
    for parent in parents {
        command.args(["-p", parent]);
    }
    let out = command
        .env("GIT_AUTHOR_DATE", time)
        .env("GIT_COMMITTER_DATE", time)
        .output()?;
    ensure!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

#[test]
fn ranges_exclude_ancestry_without_timestamp_cutoffs() -> Result<()> {
    let repo = TestRepo::new("revision-ranges")?;
    let root = repo.path();
    let tree = git(root, &["write-tree"])?;
    let base = commit(root, &tree, &[], "1000000300 +0000")?;
    let left = commit(root, &tree, &[&base], "1000000200 +0000")?;
    let right = commit(root, &tree, &[&base], "1000000100 +0000")?;
    let merge = commit(root, &tree, &[&left, &right], "1000000400 +0000")?;
    let unrelated = commit(root, &tree, &[], "1000000500 +0000")?;
    git(root, &["update-ref", "refs/heads/main", &merge])?;
    git(root, &["symbolic-ref", "HEAD", "refs/heads/main"])?;
    git(root, &["update-ref", "refs/remotes/origin/dev", &right])?;
    git(root, &["tag", "-a", "left", "-m", "annotated boundary", &left])?;
    for spec in [
        "left..main".to_owned(),
        "main..left".to_owned(),
        "HEAD..origin/dev".to_owned(),
        "origin/dev..HEAD".to_owned(),
        "HEAD..HEAD".to_owned(),
        "..origin/dev".to_owned(),
        "origin/dev..".to_owned(),
        format!("{unrelated}..HEAD"),
        format!("HEAD..{unrelated}"),
    ] {
        let expected = git(root, &["rev-list", &spec])?;
        for limit in ["1", "16777216"] {
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            let out = command
                .current_dir(root)
                .args([
                    "-c",
                    &format!("gitoxide.objects.allocLimit={limit}"),
                    "rev",
                    "list",
                    "-l",
                    &spec,
                ])
                .output()?;
            ensure!(
                out.status.success(),
                "Brit {spec}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let actual = String::from_utf8(out.stdout)?;
            let actual: Vec<_> = actual
                .lines()
                .map(|row| row.split_whitespace().next().expect("OID"))
                .collect();
            assert_eq!(actual.join("\n"), expected, "{spec}, policy {limit}");
        }
    }
    Ok(())
}

/// `brit log` takes revisions and ranges, with `git rev-list --date-order` as the
/// oracle for the selected commits and their order. Before this, every positional
/// argument was taken as a path and refused, so `brit log origin/dev` and
/// `brit log A..B` could not be used at all.
#[test]
fn log_lists_revisions_and_ranges_like_rev_list_date_order() -> Result<()> {
    let repo = TestRepo::new("log-revision-ranges")?;
    let root = repo.path();
    let tree = git(root, &["write-tree"])?;
    // Dates are skewed against ancestry so that a timestamp cutoff would be wrong.
    let base = commit(root, &tree, &[], "1000000300 +0000")?;
    let left = commit(root, &tree, &[&base], "1000000200 +0000")?;
    let right = commit(root, &tree, &[&base], "1000000100 +0000")?;
    let merge = commit(root, &tree, &[&left, &right], "1000000400 +0000")?;
    let after = commit(root, &tree, &[&merge], "1000000050 +0000")?;
    let unrelated = commit(root, &tree, &[], "1000000500 +0000")?;
    git(root, &["update-ref", "refs/heads/main", &after])?;
    git(root, &["symbolic-ref", "HEAD", "refs/heads/main"])?;
    git(root, &["update-ref", "refs/remotes/origin/dev", &right])?;
    git(root, &["tag", "-a", "left", "-m", "annotated boundary", &left])?;
    let unrelated_range = format!("{unrelated}..HEAD");
    let cases: Vec<Vec<&str>> = vec![
        vec!["HEAD"],
        vec!["origin/dev"],
        vec!["left"],
        vec!["left..main"],
        vec!["main..left"],
        vec!["origin/dev..HEAD"],
        vec!["HEAD..origin/dev"],
        vec!["HEAD..HEAD"],
        vec!["main..main~1"],
        vec!["main~2..main"],
        vec!["main", "^main~1", "^main"],
        vec!["origin/dev.."],
        vec![&unrelated_range],
        vec!["left...origin/dev"],
        vec!["origin/dev...main"],
        vec!["main^@"],
        vec!["main~1^!"],
        vec!["main", "^left"],
        vec!["left", "origin/dev"],
        vec!["^origin/dev"],
    ];
    for args in cases {
        let mut oracle = vec!["rev-list", "--date-order"];
        oracle.extend(&args);
        let expected: Vec<String> = git(root, &oracle)?.lines().map(|id| id[..8].to_owned()).collect();
        for limit in [None, Some(2usize)] {
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            command.current_dir(root).arg("log").args(&args);
            if let Some(limit) = limit {
                command.args(["-n", &limit.to_string()]);
            }
            let out = command.output()?;
            ensure!(
                out.status.success(),
                "Brit log {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let actual: Vec<String> = String::from_utf8(out.stdout)?
                .lines()
                .map(|row| row.split_whitespace().next().expect("abbreviated OID").to_owned())
                .collect();
            let expected = &expected[..limit.unwrap_or(usize::MAX).min(expected.len())];
            assert_eq!(
                actual, expected,
                "log {args:?} selects rev-list's commits in its order, limit {limit:?}"
            );
        }
    }
    Ok(())
}

/// A positional argument that names neither a revision nor a worktree path is refused
/// rather than read as an empty range, and a spec with `..` or a leading `^` is always a
/// revision. A lone existing path is a path, as in Git, and paths after `--` limit the log.
#[test]
fn log_refuses_unknown_revisions_and_reads_paths() -> Result<()> {
    let repo = TestRepo::new("log-revision-refusals")?;
    repo.commit_file("tracked", "content\n")?;
    repo.commit_file("other", "content\n")?;
    let run = |args: &[&str]| -> Result<std::process::Output> {
        let mut command = Command::new(brit_bin().expect("built Brit"));
        gix_testtools::configure_git_environment(&mut command, repo.path());
        Ok(command.current_dir(repo.path()).args(args).output()?)
    };
    for args in [
        &["log", "no-such-ref"][..],
        &["log", "tracked..HEAD"][..],
        &["log", "^tracked"][..],
    ] {
        let out = run(args)?;
        assert!(!out.status.success(), "{args:?} must fail");
        assert!(out.stdout.is_empty(), "{args:?} prints no commits");
    }
    for (args, oracle) in [
        (
            &["log", "tracked"][..],
            &["log", "--format=%h", "--abbrev=8", "tracked"][..],
        ),
        (
            &["log", "--", "tracked"][..],
            &["log", "--format=%h", "--abbrev=8", "--", "tracked"][..],
        ),
    ] {
        let out = run(args)?;
        ensure!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let actual: Vec<_> = String::from_utf8(out.stdout)?
            .lines()
            .map(|row| row.split_whitespace().next().expect("abbreviated OID").to_owned())
            .collect();
        let expected: Vec<_> = git(repo.path(), oracle)?.lines().map(ToOwned::to_owned).collect();
        assert_eq!(actual, expected, "{args:?} lists the commits that changed the path");
        assert_eq!(actual.len(), 1);
    }
    Ok(())
}
