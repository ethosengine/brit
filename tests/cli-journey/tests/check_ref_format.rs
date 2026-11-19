//! `brit check-ref-format` answers like `git check-ref-format`, through its exit code alone.
use anyhow::Result;
use cli_journey::support::parity::assert_same;

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
    Ok(())
}
