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

    fn command(&self, args: &[&str]) -> std::process::Command {
        let tick = self.clock.get() + 1;
        self.clock.set(tick);
        let date = format!("{} -0130", 1_700_000_000 + tick * 3600);
        let mut command = gix_testtools::git_command(self.root());
        command
            .args(args)
            .env("GIT_AUTHOR_NAME", "Author Name")
            .env("GIT_AUTHOR_EMAIL", "author@example.com")
            .env("GIT_COMMITTER_NAME", "Committer Name")
            .env("GIT_COMMITTER_EMAIL", "committer@example.com")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date);
        command
    }

    fn run(&self, args: &[&str]) -> Result<Output> {
        Ok(self.command(args).output()?)
    }

    /// Write a raw object, for commits whose headers or bytes `git commit` would not produce.
    fn object(&self, kind: &str, data: &[u8]) -> Result<String> {
        use std::{io::Write, process::Stdio};
        let mut child = self
            .command(&["hash-object", "-t", kind, "-w", "--stdin"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        child.stdin.take().expect("piped").write_all(data)?;
        let out = child.wait_with_output()?;
        ensure!(out.status.success(), "hash-object");
        Ok(String::from_utf8(out.stdout)?.trim().to_owned())
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
fn blobs_trees_and_tags_of_them_match_git() -> Result<()> {
    let f = history()?;
    f.write("space name", "s\n")?;
    f.commit("a name with a space")?;
    let tree = String::from_utf8(f.git(&["rev-parse", "HEAD^{tree}"])?)?;
    f.git(&["tag", "-a", "tree-tag", "-m", "a tagged tree", tree.trim()])?;
    f.git(&["tag", "-a", "blob-tag", "-m", "a tagged blob", "HEAD:file"])?;
    f.git(&["tag", "-a", "nested", "-m", "a tag of a tag", "v1"])?;
    f.git(&["tag", "light-tree", tree.trim()])?;
    for args in [
        &["HEAD:file"][..],
        &["HEAD:file", "HEAD~1"],
        &["HEAD~1", "HEAD:dir/inner"],
        &["HEAD:"],
        &["HEAD^{tree}"],
        &["HEAD:dir"],
        &["HEAD:", "HEAD:"],
        &["-s", "HEAD", "HEAD:"],
        &["-s", "HEAD:dir", "HEAD"],
        &["tree-tag"],
        &["--oneline", "tree-tag"],
        &["light-tree"],
        &["blob-tag"],
        &["nested"],
        &["-s", "nested"],
        &["--oneline", "nested"],
        &["v1^{}"],
        &["-s", "v1^{}", "v1"],
        // A commit prints once per call, a tag or tree each time it is named.
        &[
            "-s", "v1", "v1^{}", "v1", "HEAD", "HEAD~1", "HEAD", "HEAD:dir", "HEAD:dir",
        ],
    ] {
        parity(&f, args)?;
    }
    Ok(())
}

/// Git re-encodes a commit's names and message from its declared encoding to UTF-8; bytes without
/// a declaration, or that do not decode, print unchanged.
#[test]
fn declared_encodings_are_reencoded_like_git() -> Result<()> {
    let f = history()?;
    let tree = String::from_utf8(f.git(&["rev-parse", "HEAD^{tree}"])?)?;
    let tree = tree.trim();
    let commit = |encoding: Option<&str>, message: &[u8]| -> Result<String> {
        let mut data = format!("tree {tree}\n").into_bytes();
        data.extend_from_slice(b"author Andr\xe9 <a@example.com> 1700000000 +0100\n");
        data.extend_from_slice(b"committer C <c@example.com> 1700000000 +0100\n");
        if let Some(encoding) = encoding {
            data.extend_from_slice(format!("encoding {encoding}\n").as_bytes());
        }
        data.push(b'\n');
        data.extend_from_slice(message);
        f.object("commit", &data)
    };
    let latin1 = commit(Some("ISO-8859-1"), b"caf\xe9 \x80\x9f\n\nbody \xe9\n")?;
    let sjis_invalid = commit(Some("Shift_JIS"), b"bad \x81\n")?;
    let undeclared = commit(None, b"raw \xe9\n")?;
    for id in [&latin1, &sjis_invalid, &undeclared] {
        for format in [&[][..], &["-s"], &["--oneline", "-s"]] {
            let mut args: Vec<&str> = format.to_vec();
            args.push(id);
            parity(&f, &args)?;
        }
    }
    // An encoding Brit cannot decode exactly as iconv is refused; Git prints the bytes unchanged.
    let unknown = commit(Some("x-no-such-encoding"), b"unknown \xe9\n")?;
    ensure!(f.run(&["show", "-s", &unknown])?.status.success());
    let out = brit(f.root(), &["show", "-s", &unknown])?;
    ensure!(!out.status.success() && out.stdout.is_empty());
    ensure!(String::from_utf8_lossy(&out.stderr)
        .contains("declares the encoding 'x-no-such-encoding', which brit cannot decode as Git would"));
    // Git prints in i18n.logOutputEncoding, else i18n.commitEncoding; Brit prints UTF-8 only.
    for (key, value) in [
        ("i18n.logOutputEncoding", "ISO-8859-1"),
        ("i18n.commitEncoding", "ISO-8859-1"),
    ] {
        f.git(&["config", key, value])?;
        let out = brit(f.root(), &["show", "-s", &latin1])?;
        ensure!(!out.status.success() && out.stdout.is_empty());
        ensure!(String::from_utf8_lossy(&out.stderr).contains(&format!(
            "{key} asks for '{value}' output, but brit show prints UTF-8 only; use git show"
        )));
        f.git(&["config", "--unset", key])?;
    }
    f.git(&["config", "i18n.commitEncoding", "ISO-8859-1"])?;
    f.git(&["config", "i18n.logOutputEncoding", "UTF-8"])?;
    parity(&f, &[&latin1])?;
    Ok(())
}

/// A signature header is not part of the default output, and author and committer are separate.
#[test]
fn signatures_and_distinct_committers_match_git() -> Result<()> {
    let f = history()?;
    let tree = String::from_utf8(f.git(&["rev-parse", "HEAD^{tree}"])?)?;
    let head = String::from_utf8(f.git(&["rev-parse", "HEAD"])?)?;
    let signed = f.object(
        "commit",
        format!(
            "tree {}\nparent {}\nauthor A <a@example.com> 1700000000 +0545\n\
             committer C <c@example.com> 1700009999 -0800\n\
             gpgsig -----BEGIN PGP SIGNATURE-----\n \n iQEzBAABCAAdFiEE\n -----END PGP SIGNATURE-----\n\n\
             signed commit\n",
            tree.trim(),
            head.trim()
        )
        .as_bytes(),
    )?;
    for format in FORMATS {
        let mut args: Vec<&str> = format.to_vec();
        args.push(&signed);
        parity(&f, &args)?;
    }
    f.write("file", "by someone else\n")?;
    f.git(&["add", "-A"])?;
    let out = f
        .command(&["commit", "-q", "-m", "author and committer differ"])
        .env("GIT_AUTHOR_NAME", "Other Author")
        .env("GIT_AUTHOR_DATE", "1700000000 +1245")
        .env("GIT_COMMITTER_DATE", "1700100000 -0930")
        .output()?;
    ensure!(out.status.success());
    for format in [&[][..], &["-s"], &["--oneline"], &["--stat"]] {
        parity(&f, format)?;
    }
    Ok(())
}

/// `-s ours` keeps the first parent's tree; an octopus merges several parents at once.
#[test]
fn ours_and_octopus_merges_match_git() -> Result<()> {
    let f = Fixture::new()?;
    f.write("base", "base\n")?;
    f.commit("base")?;
    for branch in ["a", "b", "c"] {
        f.git(&["switch", "-q", "-c", branch, "main"])?;
        f.write(&format!("{branch}-file"), &format!("{branch}\n"))?;
        f.commit(&format!("work on {branch}"))?;
    }
    f.git(&["switch", "-q", "main"])?;
    f.git(&["merge", "-q", "-s", "ours", "-m", "ours merge", "a"])?;
    f.git(&["merge", "-q", "-m", "octopus merge", "b", "c"])?;
    for revs in [&["HEAD"][..], &["HEAD~1"], &["HEAD", "HEAD~1"]] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revs);
            parity(&f, &args)?;
        }
    }
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
    // Several revisions in one call print nothing when one of them refuses.
    for format in [&[][..], &["--oneline"], &["--name-only"], &["--name-status"]] {
        for revs in [&["HEAD"][..], &["HEAD~1", "HEAD"]] {
            let mut args = vec!["show"];
            args.extend_from_slice(format);
            args.extend_from_slice(revs);
            let out = brit(f.root(), &args)?;
            ensure!(out.status.code() == Some(1), "{args:?} refuses with status 1");
            ensure!(out.stdout.is_empty(), "{args:?} prints nothing");
            ensure!(String::from_utf8_lossy(&out.stderr).contains("which needs a combined diff; use git show"));
        }
    }
    // A first-parent summary and the header alone remain available.
    for format in [
        &["--stat"][..],
        &["--numstat"],
        &["-s"],
        &["--oneline", "--stat"],
        &["--oneline", "-s"],
        &["-s", "HEAD~1", "HEAD"],
    ] {
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
