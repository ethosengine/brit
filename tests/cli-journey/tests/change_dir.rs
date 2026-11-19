//! Global `-C <dir>` means what Git's does: run as if started in `<dir>`, moving repository
//! discovery and relative path arguments, repeatable, and composing with `-r`. Proven in a normal
//! repository, a linked worktree, and from a superproject into a submodule.
use anyhow::Result;
use cli_journey::support::parity::{assert_same, brit, git_ok};

fn s(path: &std::path::Path) -> &str {
    path.to_str().expect("UTF-8 temporary path")
}

#[test]
fn change_dir_matches_git_in_a_repository_a_linked_worktree_and_a_submodule() -> Result<()> {
    let top_dir = tempfile::tempdir()?;
    let top = top_dir.path().canonicalize()?;
    let away = tempfile::tempdir()?;
    let away = away.path();
    git_ok(&top, &["init", "-q", "--initial-branch=main", "repo"])?;
    let repo = top.join("repo");
    std::fs::create_dir_all(repo.join("sub"))?;
    std::fs::write(repo.join("sub/f"), "f\n")?;
    git_ok(&repo, &["add", "sub/f"])?;
    git_ok(&repo, &["commit", "-q", "-m", "init"])?;

    // A normal repository, entered from an unrelated directory.
    assert_same(
        away,
        &["-C", s(&repo), "rev-parse", "--show-toplevel", "--git-dir", "HEAD"],
    )?;
    assert_same(
        away,
        &[
            "-C",
            s(&repo.join("sub")),
            "rev-parse",
            "--show-toplevel",
            "--git-common-dir",
        ],
    )?;
    assert_same(
        away,
        &["-C", s(&top), "-C", "repo", "-C", "sub", "rev-parse", "--show-toplevel"],
    )?;
    assert_same(&repo, &["-C", "", "rev-parse", "--show-toplevel"])?;
    assert_same(away, &["-C", s(&repo), "for-each-ref", "refs/heads"])?;

    // Relative path arguments move with -C, never with -r alone.
    assert_same(away, &["-C", s(&repo), "rev-parse", "sub/f"])?;
    let not_moved = brit(away, &["-r", s(&repo), "rev-parse", "sub/f"])?;
    assert_eq!(not_moved.code, Some(128), "-r does not move `sub/f`: {not_moved:?}");
    let added = brit(away, &["-C", s(&repo), "worktree", "add", "-b", "linked", "../linked"])?;
    assert_eq!(added.code, Some(0), "{added:?}");
    let linked = top.join("linked");
    assert!(
        linked.join("sub/f").is_file(),
        "`../linked` is relative to -C: {added:?}"
    );
    assert!(
        !away.join("../linked").exists(),
        "nothing is created relative to the start directory"
    );

    // -r is resolved relative to the directory -C selects, whatever their order.
    for args in [
        &["-C", s(&top), "-r", "repo", "rev-parse", "--show-toplevel"][..],
        &["-r", "repo", "-C", s(&top), "rev-parse", "--show-toplevel"][..],
    ] {
        let out = brit(away, args)?;
        assert_eq!(out.stdout, format!("{}\n", repo.display()), "{args:?}: {out:?}");
    }
    let missing = brit(away, &["-C", "does-not-exist", "rev-parse", "HEAD"])?;
    assert_ne!(missing.code, Some(0), "{missing:?}");
    assert!(
        missing.stderr.contains("cannot change to 'does-not-exist'"),
        "{missing:?}"
    );

    // A linked worktree: its private Git directory differs from the shared one.
    for args in [
        &[
            "-C",
            s(&linked),
            "rev-parse",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
        ][..],
        &[
            "-C",
            s(&linked.join("sub")),
            "rev-parse",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
        ][..],
        &["-C", s(&linked), "rev-parse", "--abbrev-ref", "HEAD"][..],
        &[
            "-C",
            s(&linked),
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads/",
        ][..],
    ] {
        assert_same(away, args)?;
    }

    // From a superproject into its submodule.
    git_ok(&top, &["init", "-q", "--initial-branch=main", "super"])?;
    let superproject = top.join("super");
    git_ok(
        &superproject,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "-q",
            s(&repo),
            "module",
        ],
    )?;
    let module = superproject.join("module");
    for args in [
        &[
            "-C",
            s(&module),
            "rev-parse",
            "--show-toplevel",
            "--git-dir",
            "--git-common-dir",
            "HEAD",
        ][..],
        &[
            "-C",
            s(&superproject),
            "-C",
            "module",
            "rev-parse",
            "--show-toplevel",
            "--git-dir",
        ][..],
        &[
            "-C",
            s(&module.join("sub")),
            "rev-parse",
            "--show-toplevel",
            "--git-common-dir",
        ][..],
        &["-C", s(&superproject), "rev-parse", "--show-toplevel", "--git-dir"][..],
        &["-C", s(&module), "for-each-ref", "refs/heads"][..],
    ] {
        assert_same(away, args)?;
    }
    Ok(())
}
