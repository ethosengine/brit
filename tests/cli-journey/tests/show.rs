//! `brit show` for commits, merges, annotated tags, blobs and several revisions, with
//! `git show --no-renames` as the byte-for-byte oracle.
#![cfg(unix)]

use std::{cell::Cell, fs, path::Path, process::Output};

use anyhow::{ensure, Result};
use cli_journey::support::runner::brit_bin;

/// A scratch repository whose commits get distinct, increasing dates.
struct Fixture {
    dir: tempfile::TempDir,
    clock: Cell<u64>,
}

impl Fixture {
    fn new() -> Result<Self> {
        let fixture = Fixture {
            dir: tempfile::tempdir()?,
            clock: Cell::new(0),
        };
        fixture.git(&["init", "-q", "--initial-branch=main"])?;
        Ok(fixture)
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn run(&self, args: &[&str]) -> Result<Output> {
        let tick = self.clock.get() + 1;
        self.clock.set(tick);
        let date = format!("{} -0130", 1_700_000_000 + tick * 3600);
        Ok(gix_testtools::git_command(self.root())
            .args(args)
            .env("GIT_AUTHOR_NAME", "Author Name")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "Committer Name")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .output()?)
    }

    fn git(&self, args: &[&str]) -> Result<Vec<u8>> {
        let out = self.run(args)?;
        ensure!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out.stdout)
    }

    fn write(&self, path: &str, data: &str) -> Result<()> {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().expect("inside the fixture"))?;
        fs::write(path, data)?;
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<()> {
        self.git(&["add", "-A"])?;
        self.git(&["commit", "-q", "--allow-empty", "--cleanup=verbatim", "-m", message])?;
        Ok(())
    }
}

fn brit(root: &Path, args: &[&str]) -> Result<Output> {
    let mut command = std::process::Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    Ok(command.current_dir(root).args(args).output()?)
}

fn parity(f: &Fixture, native: &[&str]) -> Result<()> {
    let mut oracle = vec!["-c", "color.ui=false", "show", "--no-ext-diff", "--no-renames"];
    for arg in native {
        oracle.push(arg);
        if *arg == "--stat" {
            oracle.push("--stat-width=80");
        }
    }
    let expected = f.git(&oracle)?;
    let mut args = vec!["show"];
    args.extend_from_slice(native);
    let actual = brit(f.root(), &args)?;
    ensure!(
        actual.status.success(),
        "brit show {native:?}: {}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout),
        String::from_utf8_lossy(&expected),
        "brit show {native:?}"
    );
    assert_eq!(actual.stdout, expected, "bytes of brit show {native:?}");
    Ok(())
}

const FORMATS: &[&[&str]] = &[
    &[],
    &["--stat"],
    &["--name-only"],
    &["--name-status"],
    &["--numstat"],
    &["--no-patch"],
    &["-s"],
    &["--oneline"],
    &["--oneline", "--stat"],
    &["--oneline", "-s"],
];

/// A root commit, ordinary commits with multi-paragraph and tab-indented messages, an empty
/// commit, a clean merge and an annotated tag.
fn history() -> Result<Fixture> {
    let f = Fixture::new()?;
    f.write("file", "1\n2\n3\n")?;
    f.write("dir/inner", "inner\n")?;
    f.commit("root commit")?;
    f.write("file", "1\ntwo\n3\n")?;
    f.write("added", "new\n")?;
    f.commit("subject line\ncontinued subject\n\nBody paragraph\n\twith a tab\n\n  indented\n")?;
    f.commit("empty commit")?;
    f.git(&["switch", "-q", "-c", "topic"])?;
    f.write("topic-file", "t\n")?;
    f.commit("topic work")?;
    f.git(&["switch", "-q", "main"])?;
    f.write("dir/inner", "changed on main\n")?;
    f.commit("main work")?;
    f.git(&["merge", "-q", "--no-ff", "-m", "merge topic\n\nmerge body", "topic"])?;
    f.git(&["tag", "-a", "v1", "-m", "release one\n\ntag body", "HEAD~1"])?;
    Ok(f)
}

#[test]
fn commits_merges_and_tags_match_git_in_every_format() -> Result<()> {
    let f = history()?;
    let revisions: &[&[&str]] = &[
        &[],
        &["HEAD"],
        &["HEAD~1"],
        &[":/root commit"],
        &[":/subject line"],
        &[":/empty commit"],
        &["v1"],
        &["HEAD~1", "HEAD~2", ":/root commit"],
        &["v1", "HEAD"],
    ];
    for revs in revisions {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revs);
            parity(&f, &args)?;
        }
    }
    Ok(())
}

#[test]
fn paths_restrict_commits_and_hide_those_that_do_not_touch_them() -> Result<()> {
    let f = history()?;
    for (revs, paths) in [
        (&[":/subject line"][..], &["file"][..]),
        (&[":/subject line"], &["dir"]),
        (&[":/subject line", ":/root commit"], &["dir/"]),
        (&["HEAD"], &["topic-file"]),
        (&["HEAD"], &["dir"]),
        (&["HEAD~1", "HEAD"], &["nonexistent"]),
    ] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revs);
            args.push("--");
            args.extend_from_slice(paths);
            parity(&f, &args)?;
        }
    }
    Ok(())
}

#[test]
fn blobs_print_their_content_between_commits() -> Result<()> {
    let f = history()?;
    for args in [
        &["HEAD:file"][..],
        &["HEAD:file", "HEAD~1"],
        &["HEAD~1", "HEAD:dir/inner"],
    ] {
        parity(&f, args)?;
    }
    // A tree is listed like `brit cat`, not with Git's `tree` heading.
    let tree = brit(f.root(), &["show", "HEAD:dir"])?;
    let cat = brit(f.root(), &["cat", "HEAD:dir"])?;
    ensure!(tree.status.success());
    assert_eq!(tree.stdout, cat.stdout);
    Ok(())
}

/// A merge whose result differs from both parents needs Git's combined diff, which Brit does not
/// print; it refuses instead of printing a different patch.
#[test]
fn merges_needing_a_combined_diff_refuse() -> Result<()> {
    let f = Fixture::new()?;
    f.write("file", "base\n")?;
    f.commit("base")?;
    f.git(&["switch", "-q", "-c", "topic"])?;
    f.write("file", "topic\n")?;
    f.commit("topic")?;
    f.git(&["switch", "-q", "main"])?;
    f.write("file", "main\n")?;
    f.commit("main")?;
    let conflicted = f.run(&["merge", "-q", "topic"])?;
    ensure!(!conflicted.status.success(), "the fixture conflicts");
    f.write("file", "resolved\n")?;
    f.commit("resolved merge")?;
    for format in [&[][..], &["--name-only"], &["--name-status"]] {
        let mut args = vec!["show"];
        args.extend_from_slice(format);
        let out = brit(f.root(), &args)?;
        ensure!(!out.status.success() && out.stdout.is_empty(), "{format:?} refuses");
        ensure!(String::from_utf8_lossy(&out.stderr).contains("combined diff"));
    }
    // A first-parent summary and the header alone remain available.
    for format in [&["--stat"][..], &["--numstat"], &["-s"]] {
        parity(&f, format)?;
    }
    Ok(())
}

/// Like Git, a shallow boundary commit is shown against the empty tree.
#[test]
fn shallow_boundary_commits_are_roots() -> Result<()> {
    let f = history()?;
    let boundary = String::from_utf8(f.git(&["rev-parse", ":/subject line"])?)?;
    fs::write(f.root().join(".git/shallow"), &boundary)?;
    for format in [&[][..], &["--stat"], &["--name-status"]] {
        let mut args: Vec<&str> = format.to_vec();
        args.push(":/subject line");
        parity(&f, &args)?;
    }
    Ok(())
}
