//! `brit diff` between revisions, ranges, merge bases, the index and working files, with
//! `git diff --no-renames` as the byte-for-byte oracle.
//!
//! The census of Git fallbacks puts diff first: "brit diff has no range form", "no --stat for
//! commits", "not per-path scoped". Each form here is one an agent typed.
#![cfg(unix)]

use std::{
    cell::Cell,
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::Path,
    process::Output,
};

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

    fn git_in(&self, dir: &Path, args: &[&str]) -> Result<Output> {
        let tick = self.clock.get() + 1;
        self.clock.set(tick);
        let date = format!("{} +0200", 1_700_000_000 + tick * 60);
        Ok(gix_testtools::git_command(dir)
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
        let out = self.git_in(self.root(), args)?;
        ensure!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        Ok(out.stdout)
    }

    fn rev(&self, name: &str) -> Result<String> {
        Ok(String::from_utf8(self.git(&["rev-parse", name])?)?.trim().to_owned())
    }

    fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let path = self.root().join(path);
        fs::create_dir_all(path.parent().expect("inside the fixture"))?;
        fs::write(path, data)?;
        Ok(())
    }

    fn commit(&self, message: &str) -> Result<String> {
        self.git(&["add", "-A"])?;
        self.git(&["commit", "-q", "--allow-empty", "-m", message])?;
        self.rev("HEAD")
    }

    fn gitlink(&self, path: &str, commit: &str) -> Result<()> {
        self.git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{commit},{path}"),
        ])?;
        Ok(())
    }
}

fn brit_in(dir: &Path, args: &[&str]) -> Result<Output> {
    let mut command = std::process::Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, dir);
    Ok(command.current_dir(dir).args(args).output()?)
}

/// Git's arguments for a native `brit diff` invocation.
fn oracle_args<'a>(native: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["-c", "color.ui=false", "diff", "--no-ext-diff", "--no-renames"];
    for arg in native {
        args.push(arg);
        if *arg == "--stat" {
            args.push("--stat-width=80");
        }
    }
    args
}

/// Assert that `brit diff <native>` prints exactly what Git prints, with the same exit status.
fn parity_in(fixture: &Fixture, dir: &Path, native: &[&str]) -> Result<()> {
    let expected = fixture.git_in(dir, &oracle_args(native))?;
    let mut args = vec!["diff"];
    args.extend_from_slice(native);
    let actual = brit_in(dir, &args)?;
    ensure!(
        expected.status.code() == actual.status.code(),
        "exit status for {native:?}: git {:?} ({}), brit {:?} ({})",
        expected.status.code(),
        String::from_utf8_lossy(&expected.stderr),
        actual.status.code(),
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&actual.stdout),
        String::from_utf8_lossy(&expected.stdout),
        "brit diff {native:?}"
    );
    assert_eq!(actual.stdout, expected.stdout, "bytes of brit diff {native:?}");
    Ok(())
}

fn parity(fixture: &Fixture, native: &[&str]) -> Result<()> {
    parity_in(fixture, fixture.root(), native)
}

/// Assert that Brit refuses with a message containing `reason` and prints nothing on stdout.
fn refuses(dir: &Path, native: &[&str], reason: &str) -> Result<()> {
    let mut args = vec!["diff"];
    args.extend_from_slice(native);
    let out = brit_in(dir, &args)?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    ensure!(!out.status.success(), "{native:?} must be refused");
    ensure!(out.stdout.is_empty(), "{native:?} prints nothing on refusal");
    ensure!(stderr.contains(reason), "{native:?}: {stderr}");
    Ok(())
}

const FORMATS: &[&[&str]] = &[&[], &["--stat"], &["--name-only"], &["--name-status"], &["--numstat"]];

/// Every kind of change Git's patch distinguishes, between `base`, `change` (which also adds a
/// gitlink) and `moved` (which moves the gitlink).
fn rich() -> Result<(Fixture, String, String, String)> {
    let f = Fixture::new()?;
    let lines: String = (1..=10).map(|n| n.to_string() + "\n").collect();
    let big: String = (1..=400)
        .map(|n| ["line ", &n.to_string(), " of a large text file\n"].concat())
        .collect();
    // Hunk headers carry Git's default function context: the last earlier line starting with a
    // letter, `_` or `$`, cut to 80 bytes and right-trimmed.
    let functions = format!(
        "{}\n  a\n  b\n  c\n  d\n  e\n_underscore  \t\n  f\n  g\n  h\n  i\n  j\n$dollar\n  k\n  l\n  m\n  n\n",
        "header ".repeat(15)
    );
    for (path, data) in [
        ("text", lines.as_bytes()),
        ("deleted", b"gone\n"),
        ("mode", b"x\n"),
        ("f2d", b"file\n"),
        ("d2f/inner", b"inner\n"),
        ("no-newline-old", b"old"),
        ("no-newline-new", b"keep\n"),
        ("binary", b"a\0b"),
        ("emptied", b"full\n"),
        ("empty-deleted", b""),
        ("caf\u{e9} space.txt", b"old\n"),
        ("dir/a", b"a\n"),
        ("dir/b", b"b\n"),
        ("dir-file", b"x\n"),
        ("dir2/x", b"x\n"),
        ("big", big.as_bytes()),
        ("functions", functions.as_bytes()),
    ] {
        f.write(path, data)?;
    }
    symlink("target", f.root().join("link"))?;
    // Above this size Git reports a blob as binary without reading it.
    f.git(&["config", "core.bigFileThreshold", "4000"])?;
    let base = f.commit("base")?;

    f.write(
        "text",
        lines.replace("5\n", "five\n").replace("10\n", "10\n11\n").as_bytes(),
    )?;
    fs::remove_file(f.root().join("deleted"))?;
    fs::set_permissions(f.root().join("mode"), fs::Permissions::from_mode(0o755))?;
    fs::remove_file(f.root().join("f2d"))?;
    f.write("f2d/inner", b"now a directory\n")?;
    fs::remove_dir_all(f.root().join("d2f"))?;
    f.write("d2f", b"now a file\n")?;
    f.write("no-newline-old", b"old\nnew\n")?;
    f.write("no-newline-new", b"keep\nlast")?;
    f.write("binary", b"a\0c")?;
    f.write("emptied", b"")?;
    fs::remove_file(f.root().join("empty-deleted"))?;
    f.write("empty-added", b"")?;
    f.write("caf\u{e9} space.txt", b"new\n")?;
    f.write("new file", b"n\n")?;
    f.write("dir/a", b"a2\n")?;
    f.write("dir2/x", b"x2\n")?;
    f.write("dir-file", b"x2\n")?;
    f.write("big", big.replace("line 7 ", "line seven ").as_bytes())?;
    f.write(
        "functions",
        functions.replace("  h\n", "  H\n").replace("  m\n", "  M\n").as_bytes(),
    )?;
    fs::remove_file(f.root().join("link"))?;
    f.write("link", b"now a file\n")?;
    f.git(&["add", "-A"])?;
    f.gitlink("sub", &base)?;
    f.git(&["commit", "-q", "-m", "change"])?;
    let change = f.rev("HEAD")?;
    f.gitlink("sub", &change)?;
    f.git(&["commit", "-q", "-m", "move gitlink"])?;
    let moved = f.rev("HEAD")?;
    Ok((f, base, change, moved))
}

#[test]
fn every_output_form_matches_git_for_two_revisions_and_ranges() -> Result<()> {
    let (f, base, change, moved) = rich()?;
    let two_dot = format!("{base}..{moved}");
    let pairs: [Vec<&str>; 5] = [
        vec![&base, &change],
        vec![&change, &base],
        vec![&change, &moved],
        vec![&moved, &base],
        vec![&two_dot],
    ];
    for revisions in &pairs {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend(revisions.iter().copied());
            parity(&f, &args)?;
        }
    }
    // Non-ASCII names print raw when core.quotePath is off.
    f.git(&["config", "core.quotePath", "false"])?;
    for format in FORMATS {
        let mut args: Vec<&str> = format.to_vec();
        args.extend([base.as_str(), moved.as_str()]);
        parity(&f, &args)?;
    }
    Ok(())
}

#[test]
fn literal_paths_restrict_every_form() -> Result<()> {
    let (f, base, _change, moved) = rich()?;
    let filters: &[&[&str]] = &[
        &["text"],
        &["dir"],
        &["dir/"],
        &["dir2", "text"],
        &["f2d/"],
        &["f2d"],
        &["sub/"],
        &["sub"],
        &["nonexistent"],
        &["caf\u{e9} space.txt"],
    ];
    for filter in filters {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend([base.as_str(), moved.as_str(), "--"]);
            args.extend_from_slice(filter);
            parity(&f, &args)?;
        }
    }
    // Paths are relative to the current directory, like Git's.
    let dir = f.root().join("dir");
    parity_in(&f, &dir, &["--name-status", &base, &moved, "--", "a"])?;
    parity_in(&f, &dir, &[&base, &moved, "--", "."])?;
    refuses(f.root(), &[&base, &moved, "--", "di*"], "literal paths")?;
    Ok(())
}

#[test]
fn diff_tree_shares_the_revision_forms() -> Result<()> {
    let (f, base, _change, moved) = rich()?;
    for (native, oracle) in [
        (vec!["tree", "--stat", &base, &moved], vec!["--stat", &base, &moved]),
        (
            vec!["tree", "--name-only", &base, &moved],
            vec!["--name-only", &base, &moved],
        ),
    ] {
        let expected = f.git(&oracle_args(&oracle))?;
        let mut args = vec!["diff"];
        args.extend(native.iter().copied());
        let actual = brit_in(f.root(), &args)?;
        ensure!(actual.status.success(), "{}", String::from_utf8_lossy(&actual.stderr));
        assert_eq!(actual.stdout, expected, "brit diff {native:?}");
    }
    Ok(())
}

fn diverged() -> Result<Fixture> {
    let f = Fixture::new()?;
    f.write("shared", b"1\n2\n3\n")?;
    f.write("left", b"l\n")?;
    f.commit("base")?;
    f.git(&["branch", "topic"])?;
    f.write("shared", b"1\nmain\n3\n")?;
    f.write("main-only", b"m\n")?;
    f.commit("main side")?;
    f.git(&["switch", "-q", "topic"])?;
    f.write("topic-only", b"t\n")?;
    f.write("left", b"topic\n")?;
    f.commit("topic side")?;
    f.git(&["switch", "-q", "main"])?;
    Ok(f)
}

#[test]
fn two_dot_and_three_dot_differ_on_a_diverged_history() -> Result<()> {
    let f = diverged()?;
    for revisions in [
        &["main..topic"][..],
        &["main...topic"],
        &["topic...main"],
        &["main", "topic"],
        &["--merge-base", "main", "topic"],
        &["--merge-base", "topic", "main"],
        &["main.."],
        &["...topic"],
    ] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revisions);
            parity(&f, &args)?;
        }
    }
    let two = brit_in(f.root(), &["diff", "--name-only", "main..topic"])?.stdout;
    let three = brit_in(f.root(), &["diff", "--name-only", "main...topic"])?.stdout;
    ensure!(two != three, "the fixture tells two-dot from three-dot");
    Ok(())
}

/// The true merge base `b1` lies below the shallow boundary `s`, so Git (reading `s` as a root)
/// uses `b0`. A walk that follows `s`'s parents would wrongly pick `b1`.
#[test]
fn shallow_merge_bases_follow_the_boundary_like_git() -> Result<()> {
    let f = Fixture::new()?;
    f.write("b0", b"0\n")?;
    let b0 = f.commit("b0")?;
    f.write("b1", b"1\n")?;
    let b1 = f.commit("b1")?;
    f.write("main-file", b"m\n")?;
    f.commit("m1")?;
    f.git(&["switch", "-q", "-c", "topic", &b0])?;
    f.write("t1", b"t\n")?;
    f.commit("t1")?;
    f.git(&["switch", "-q", "-c", "side", &b1])?;
    f.write("s", b"s\n")?;
    let s = f.commit("s")?;
    f.git(&["switch", "-q", "topic"])?;
    f.git(&["merge", "-q", "--no-edit", "side"])?;
    f.git(&["switch", "-q", "main"])?;
    fs::write(f.root().join(".git/shallow"), format!("{s}\n"))?;
    let base = String::from_utf8(f.git(&["merge-base", "main", "topic"])?)?;
    ensure!(base.trim() == b0, "Git reads the boundary as a root");
    for revisions in [
        &["main...topic"][..],
        &["--merge-base", "main", "topic"],
        &["topic...main"],
    ] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revisions);
            parity(&f, &args)?;
        }
    }
    for (native, oracle) in [
        (
            vec!["tree", "--name-only", "main...topic"],
            vec!["--name-only", "main...topic"],
        ),
        (
            vec!["tree", "--name-only", "--merge-base", "main", "topic"],
            vec!["--name-only", "--merge-base", "main", "topic"],
        ),
        (vec!["tree", "--stat", "main...topic"], vec!["--stat", "main...topic"]),
    ] {
        let expected = f.git(&oracle_args(&oracle))?;
        let mut args = vec!["diff"];
        args.extend(native.iter().copied());
        let actual = brit_in(f.root(), &args)?;
        ensure!(actual.status.success(), "{}", String::from_utf8_lossy(&actual.stderr));
        assert_eq!(actual.stdout, expected, "brit diff {native:?}");
    }
    Ok(())
}

#[test]
fn several_merge_bases_and_none_are_handled_like_git() -> Result<()> {
    let f = Fixture::new()?;
    f.write("base", b"0\n")?;
    let base = f.commit("base")?;
    f.write("x-file", b"x1\n")?;
    let x1 = f.commit("x1")?;
    f.git(&["switch", "-q", "-c", "y", &base])?;
    f.write("y-file", b"y1\n")?;
    let y1 = f.commit("y1")?;
    f.git(&["merge", "-q", "--no-edit", &x1])?;
    f.write("y-file", b"y2\n")?;
    f.commit("y2")?;
    f.git(&["switch", "-q", "main"])?;
    f.git(&["merge", "-q", "--no-edit", &y1])?;
    f.write("x-file", b"x2\n")?;
    f.commit("x2")?;
    let bases = String::from_utf8(f.git(&["merge-base", "--all", "main", "y"])?)?;
    ensure!(bases.lines().count() == 2, "criss-cross fixture: {bases}");

    // `A...B` uses the first merge base and warns, as Git does.
    for format in FORMATS {
        let mut args: Vec<&str> = format.to_vec();
        args.push("main...y");
        parity(&f, &args)?;
    }
    let warned = brit_in(f.root(), &["diff", "--name-only", "main...y"])?;
    ensure!(String::from_utf8_lossy(&warned.stderr).contains("multiple merge bases"));
    // `--merge-base` refuses, as Git does, and names the bases.
    let failed = f.git_in(f.root(), &["diff", "--merge-base", "main", "y"])?;
    ensure!(!failed.status.success(), "Git refuses several merge bases");
    for args in [
        &["--merge-base", "main", "y"][..],
        &["tree", "--merge-base", "main", "y"],
    ] {
        refuses(f.root(), args, "multiple merge bases found")?;
        let out = brit_in(f.root(), &[&["diff"][..], args].concat())?;
        for id in bases.lines() {
            ensure!(String::from_utf8_lossy(&out.stderr).contains(id), "names {id}");
        }
    }

    // Unrelated histories: two trees still compare; merge-base forms refuse like Git.
    f.git(&["switch", "-q", "--orphan", "unrelated"])?;
    f.write("other", b"o\n")?;
    f.commit("unrelated root")?;
    parity(&f, &["--stat", "main", "unrelated"])?;
    for args in [&["main...unrelated"][..], &["--merge-base", "main", "unrelated"]] {
        ensure!(!f.git_in(f.root(), &oracle_args(args))?.status.success());
        refuses(f.root(), args, "no merge base")?;
    }
    Ok(())
}

#[test]
fn one_revision_compares_with_the_working_files_or_the_index() -> Result<()> {
    let f = diverged()?;
    f.write("shared", b"1\nworking\n3\n")?;
    f.write("staged", b"staged\n")?;
    f.git(&["add", "staged"])?;
    fs::remove_file(f.root().join("left"))?;
    f.write("untracked", b"never shown\n")?;
    for revisions in [
        &["topic"][..],
        &["--cached", "topic"],
        &["HEAD~1"],
        &["--cached", "HEAD"],
        &["--merge-base", "topic"],
        &[],
        &["--cached"],
        &["topic", "shared"],
        &["topic", "--", "shared"],
        &["shared"],
    ] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revisions);
            parity(&f, &args)?;
        }
    }
    refuses(f.root(), &["--cached", "main", "topic"], "--cached")?;
    refuses(f.root(), &["main", "topic", "HEAD"], "at most two revisions")?;
    // Unknown names refuse like Git, with the first line of Git's message.
    for (args, reason) in [
        (
            &["no-such-revision"][..],
            "ambiguous argument 'no-such-revision': unknown revision or path not in the working tree.",
        ),
        (&["no-such-revision", "--"], "bad revision 'no-such-revision'"),
        (&["no-such-revision", "--", "shared"], "bad revision 'no-such-revision'"),
        (
            &["no-such..main"],
            "ambiguous argument 'no-such..main': unknown revision or path not in the working tree.",
        ),
        (&["no-such..main", "--"], "bad revision 'no-such..main'"),
        (
            &["main", "no-such"],
            "ambiguous argument 'no-such': unknown revision or path not in the working tree.",
        ),
        (
            &["--cached", "no-such"],
            "ambiguous argument 'no-such': unknown revision or path not in the working tree.",
        ),
    ] {
        let git = f.git_in(f.root(), &oracle_args(args))?;
        ensure!(
            String::from_utf8_lossy(&git.stderr).contains(reason),
            "Git says {reason}"
        );
        refuses(f.root(), args, reason)?;
    }
    // A name that is both a revision and a path is ambiguous without `--`, as in Git; with `--`
    // it is a revision.
    f.write("topic", b"a file named like the branch\n")?;
    for args in [&["topic"][..], &["--name-only", "topic"]] {
        ensure!(!f.git_in(f.root(), &oracle_args(args))?.status.success());
        refuses(f.root(), args, "ambiguous argument 'topic': both revision and filename")?;
    }
    for format in FORMATS {
        let mut args: Vec<&str> = format.to_vec();
        args.extend(["topic", "--"]);
        parity(&f, &args)?;
    }
    parity(&f, &["--name-only", "topic", "--", "shared"])?;
    Ok(())
}

#[test]
fn a_linked_worktree_compares_its_own_index_and_files() -> Result<()> {
    let f = diverged()?;
    f.git(&["worktree", "add", "-q", "-b", "linked", "linked", "topic"])?;
    let linked = f.root().join("linked");
    fs::write(linked.join("left"), b"edited in the linked worktree\n")?;
    for revisions in [&["main"][..], &["--cached", "main"], &["main...HEAD"], &[]] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revisions);
            parity_in(&f, &linked, &args)?;
        }
    }
    Ok(())
}

#[test]
fn exit_code_and_quiet_report_differences_by_status() -> Result<()> {
    let f = diverged()?;
    for args in [
        &["--exit-code", "main", "main"][..],
        &["--exit-code", "main", "topic"],
        &["--quiet", "main", "topic"],
        &["--quiet", "main", "main"],
        &["--quiet"],
        &["--exit-code", "--stat", "main", "topic"],
        &["--exit-code", "--name-only", "main..topic", "--", "nonexistent"],
    ] {
        parity(&f, args)?;
    }
    f.write("shared", b"changed\n")?;
    parity(&f, &["--quiet"])?;
    parity(&f, &["--exit-code", "--numstat"])?;
    // A path that selects nothing is an empty result, not an error.
    for selection in [
        &["--", "nosuch"][..],
        &["--cached", "--", "nosuch"],
        &["HEAD", "--", "nosuch"],
    ] {
        for format in FORMATS.iter().chain([&["--exit-code"][..], &["--quiet"]].iter()) {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(selection);
            parity(&f, &args)?;
        }
    }
    Ok(())
}

/// A superproject with a checked-out submodule `sub` at the second of its two commits, and the
/// submodule's source repository, which must outlive the fixture's clones of it.
fn with_submodule() -> Result<(Fixture, tempfile::TempDir)> {
    let f = Fixture::new()?;
    let source = tempfile::tempdir()?;
    for args in [
        &["init", "-q", "--initial-branch=main"][..],
        &["config", "user.name", "Committer Name"],
    ] {
        ensure!(f.git_in(source.path(), args)?.status.success());
    }
    for content in ["one\n", "two\n"] {
        fs::write(source.path().join("file"), content)?;
        ensure!(f.git_in(source.path(), &["add", "file"])?.status.success());
        ensure!(f
            .git_in(source.path(), &["commit", "-q", "-m", content])?
            .status
            .success());
    }
    f.write("ordinary", b"o\n")?;
    f.commit("base")?;
    let url = source.path().to_str().expect("UTF-8 temporary path");
    f.git(&["-c", "protocol.file.allow=always", "submodule", "add", "-q", url, "sub"])?;
    f.commit("add submodule")?;
    Ok((f, source))
}

/// Every output form of `brit diff`, against the index, HEAD and with `--cached`, matches Git.
fn submodule_parity(f: &Fixture, state: &str) -> Result<()> {
    for revisions in [&[][..], &["HEAD"], &["--cached"], &["--", "ordinary"]] {
        for format in FORMATS.iter().chain([&["--exit-code"][..]].iter()) {
            let mut args: Vec<&str> = format.to_vec();
            args.extend_from_slice(revisions);
            parity(f, &args).map_err(|err| err.context(state.to_owned()))?;
        }
    }
    Ok(())
}

#[test]
fn gitlinks_print_git_short_submodule_form_against_the_working_files() -> Result<()> {
    let (f, _source) = with_submodule()?;
    let sub = f.root().join("sub");
    let in_sub = |args: &[&str]| -> Result<()> {
        ensure!(f.git_in(&sub, args)?.status.success(), "git {args:?} in the submodule");
        Ok(())
    };
    submodule_parity(&f, "clean")?;
    in_sub(&["checkout", "-q", "HEAD~1"])?;
    submodule_parity(&f, "submodule HEAD moved")?;
    fs::write(sub.join("file"), "edited\n")?;
    submodule_parity(&f, "moved and dirty")?;
    in_sub(&["checkout", "-q", "-f", "main"])?;
    fs::write(sub.join("file"), "edited\n")?;
    submodule_parity(&f, "dirty only")?;
    in_sub(&["checkout", "-q", "-f", "main"])?;
    in_sub(&["add", "-A"])?;
    fs::write(sub.join("untracked"), "u\n")?;
    submodule_parity(&f, "untracked only")?;
    fs::remove_file(sub.join("untracked"))?;
    fs::write(sub.join("file"), "staged in the submodule\n")?;
    in_sub(&["add", "file"])?;
    submodule_parity(&f, "staged in the submodule")?;
    in_sub(&["reset", "-q", "--hard"])?;
    in_sub(&["checkout", "-q", "HEAD~1"])?;
    f.git(&["add", "sub"])?;
    submodule_parity(&f, "staged gitlink move")?;
    f.git(&["reset", "-q"])?;
    in_sub(&["checkout", "-q", "main"])?;

    // An empty directory is an uninitialized submodule: unchanged, as in Git.
    let saved = f.root().join("saved-sub");
    fs::rename(&sub, &saved)?;
    fs::create_dir(&sub)?;
    submodule_parity(&f, "uninitialized")?;
    fs::remove_dir(&sub)?;
    submodule_parity(&f, "missing directory")?;
    fs::write(&sub, "a file replaced the submodule\n")?;
    submodule_parity(&f, "file replaced gitlink")?;
    fs::remove_file(&sub)?;
    fs::rename(&saved, &sub)?;
    f.git(&["rm", "-q", "--cached", "sub"])?;
    submodule_parity(&f, "gitlink removed from the index")?;
    f.git(&["reset", "-q"])?;
    Ok(())
}

#[test]
fn gitlink_additions_removals_and_type_changes_match_git_between_revisions() -> Result<()> {
    let (f, _source) = with_submodule()?;
    let added = f.rev("HEAD")?;
    let base = f.rev("HEAD~1")?;
    f.git(&["rm", "-q", "sub"])?;
    f.write("sub", b"now a file\n")?;
    let replaced = f.commit("replace submodule with a file")?;
    f.git(&["rm", "-q", "sub"])?;
    let removed = f.commit("remove")?;
    for pair in [
        [&base, &added],
        [&added, &base],
        [&added, &replaced],
        [&replaced, &added],
        [&added, &removed],
    ] {
        for format in FORMATS {
            let mut args: Vec<&str> = format.to_vec();
            args.extend(pair.iter().map(|rev| rev.as_str()));
            parity(&f, &args)?;
        }
    }
    Ok(())
}

#[test]
fn non_default_submodule_formats_and_ignore_settings_refuse() -> Result<()> {
    let (f, _source) = with_submodule()?;
    let sub = f.root().join("sub");
    ensure!(f.git_in(&sub, &["checkout", "-q", "HEAD~1"])?.status.success());
    // Git's option is not one of Brit's, so it refuses by name instead of being ignored.
    refuses(f.root(), &["--submodule=log"], "--submodule")?;
    for value in ["log", "diff"] {
        f.git(&["config", "diff.submodule", value])?;
        refuses(f.root(), &[], &format!("diff.submodule={value}"))?;
        refuses(f.root(), &["HEAD~1", "HEAD"], &format!("diff.submodule={value}"))?;
        // Git's summaries do not depend on it.
        parity(&f, &["--stat"])?;
    }
    f.git(&["config", "diff.submodule", "short"])?;
    parity(&f, &[])?;
    f.git(&["config", "diff.ignoreSubmodules", "all"])?;
    refuses(f.root(), &["--name-only"], "diff.ignoreSubmodules=all")?;
    f.git(&["config", "--unset", "diff.ignoreSubmodules"])?;
    f.git(&["config", "-f", ".gitmodules", "submodule.sub.ignore", "dirty"])?;
    refuses(f.root(), &[], "submodule.sub.ignore=dirty")?;
    f.git(&["config", "-f", ".gitmodules", "submodule.sub.ignore", "none"])?;
    parity(&f, &[])?;
    Ok(())
}
