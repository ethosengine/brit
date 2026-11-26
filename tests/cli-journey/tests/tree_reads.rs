//! Git is the byte-for-byte oracle for the tree and object read spellings.

use std::{path::Path, process::Command};

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};

fn parity(repo: &Path, args: &[&str]) -> Result<()> {
    let expected = gix_testtools::git_command(repo).args(args).output()?;
    let mut command = Command::new(brit_bin().expect("required Brit binary"));
    gix_testtools::configure_git_environment(&mut command, repo);
    let actual = command.current_dir(repo).args(args).output()?;
    assert_eq!(
        actual.status.code(),
        expected.status.code(),
        "{args:?}, Brit stderr: {}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(actual.stdout, expected.stdout, "stdout bytes for {args:?}");
    assert_eq!(actual.stderr, expected.stderr, "stderr bytes for {args:?}");
    Ok(())
}

#[test]
fn ls_tree_matches_git_with_directories_and_gitlinks() -> Result<()> {
    let repo = TestRepo::new("ls-tree")?;
    let source = TestRepo::new("ls-tree-submodule")?;
    repo.commit_file("dir/nested/file", "nested\n")?;
    repo.commit_file("dir-sibling", "outside dir\n")?;
    #[cfg(unix)]
    repo.commit_file("dir/café\tfile", "quoted\n")?;
    let source_path = source.path().to_str().expect("UTF-8 fixture path");
    let submodule = gix_testtools::git_command(repo.path())
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            source_path,
            "sub",
        ])
        .output()?;
    ensure!(
        submodule.status.success(),
        "{}",
        String::from_utf8_lossy(&submodule.stderr)
    );
    repo.commit_file("root", "root\n")?;
    for args in [
        &["ls-tree", "HEAD"][..],
        &["ls-tree", "-r", "HEAD"],
        &["ls-tree", "-r", "--name-only", "-z", "HEAD"],
        &["ls-tree", "-rz", "HEAD"],
        &["ls-tree", "HEAD", "--", "dir", "sub"],
        &["ls-tree", "HEAD", "--", "dir/nested/file"],
        &["ls-tree", "HEAD", "--", ":(literal)dir/nested/file"],
        &["ls-tree", "-r", "HEAD", "--", "dir/", "sub", "missing"],
        &["ls-tree", "-r", "HEAD:dir"],
    ] {
        parity(repo.path(), args)?;
    }
    parity(&repo.path().join("dir"), &["ls-tree", "-r", "HEAD"])?;
    parity(&repo.path().join("dir"), &["ls-tree", "HEAD", "--", "nested/file"])?;
    parity(
        &repo.path().join("dir"),
        &["ls-tree", "HEAD", "--", ":(top)dir-sibling"],
    )?;
    parity(repo.path(), &["cat-file", "-e", "HEAD:sub"])?;
    Ok(())
}

#[test]
fn cat_file_matches_git_for_object_data_headers_and_existence() -> Result<()> {
    let repo = TestRepo::new("cat-file")?;
    repo.commit_file("dir/file", "contents\0without final newline")?;
    for object in ["HEAD", "HEAD^{tree}", "HEAD:dir", "HEAD:dir/file"] {
        for flag in ["-p", "-t", "-s", "-e"] {
            parity(repo.path(), &["cat-file", flag, object])?;
        }
    }
    for object in [
        "HEAD:missing",
        "ffffffffffffffffffffffffffffffffffffffff",
        "missing-ref",
    ] {
        parity(repo.path(), &["cat-file", "-e", object])?;
    }
    parity(repo.path(), &["cat-file", "-t", "missing-ref"])?;
    Ok(())
}
