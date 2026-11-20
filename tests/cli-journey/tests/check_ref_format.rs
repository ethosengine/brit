//! `brit check-ref-format` answers like `git check-ref-format`, through its exit code alone.
use anyhow::Result;
use cli_journey::support::parity::{assert_same, assert_same_streams};

#[test]
fn check_ref_format_matches_git() -> Result<()> {
    let cwd = tempfile::tempdir()?;
    for name in [
        "refs/heads/main",
        "refs/notes/brit/devices/x/build/gate%3Aelohim-storage",
        "heads/x",
        "main",
        "HEAD",
        "refs/heads/a..b",
        "refs/heads/x.lock",
        "refs/heads/",
        "refs//x",
        "/refs/x",
        "refs/heads/a b",
        "refs/heads/a~b",
        "refs/heads/a:b",
        "refs/@{x}",
        "refs/*",
        "refs/heads/.hidden",
        "refs/heads/end.",
        "@",
    ] {
        let out = assert_same(cwd.path(), &["check-ref-format", name])?;
        assert!(out.stdout.is_empty(), "{name}: {out:?}");
        assert_same(cwd.path(), &["check-ref-format", "--allow-onelevel", name])?;
    }
    // A missing name, or `-` in its place, is a usage error: Git's 129 and a usage line on stderr.
    for args in [
        &["check-ref-format"][..],
        &["check-ref-format", "--allow-onelevel"],
        &["check-ref-format", "-"],
        &["check-ref-format", "--allow-onelevel", "-"],
        &["check-ref-format", "-x"],
    ] {
        let out = assert_same_streams(cwd.path(), args)?;
        assert_eq!(out.code, Some(129), "{args:?}: {out:?}");
    }
    Ok(())
}
