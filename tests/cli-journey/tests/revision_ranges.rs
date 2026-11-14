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
