//! Machine-readable output uses Git as the byte-for-byte oracle.
use std::{fs, path::Path, process::Command};

use anyhow::{ensure, Result};
use cli_journey::support::{parity::git_ok, runner::brit_bin};

fn git_stdout(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn brit(repo: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = Command::new(brit_bin().expect("required Brit binary"));
    gix_testtools::configure_git_environment(&mut cmd, repo);
    cmd.env(
        "GIT_CONFIG_GLOBAL",
        std::env::temp_dir().join("brit-journey-absent-global.gitconfig"),
    );
    Ok(cmd.current_dir(repo).args(args).output()?)
}

fn parity(repo: &Path, native: &[&str], oracle: &[&str]) -> Result<()> {
    let expected = git_stdout(repo, oracle)?;
    let actual = brit(repo, native)?;
    ensure!(
        actual.status.success(),
        "brit {native:?}: {}",
        String::from_utf8_lossy(&actual.stderr)
    );
    ensure!(
        actual.stdout == expected,
        "{native:?}\nBrit: {:?}\nGit: {:?}",
        String::from_utf8_lossy(&actual.stdout),
        String::from_utf8_lossy(&expected)
    );
    Ok(())
}

fn init(repo: &Path) -> Result<()> {
    git_ok(repo, &["init", "-q", "--initial-branch=main"])?;
    git_ok(repo, &["config", "user.name", "Committer"])?;
    git_ok(repo, &["config", "user.email", "committer@example.com"])?;
    Ok(())
}

fn commit(repo: &Path, msg: &str, date: &str) -> Result<()> {
    git_ok(repo, &["add", "-A"])?;
    git_ok(
        repo,
        &[
            "commit",
            "-qm",
            msg,
            "--author=Author <author@example.com>",
            "--date",
            date,
        ],
    )?;
    Ok(())
}

fn stages(repo: &Path) -> Result<()> {
    let output = git_stdout(repo, &["ls-files", "-s", "conflict", "stage-zero"])?;
    let text = String::from_utf8(output)?;
    let mut blobs = text
        .lines()
        .map(|line| line.split_ascii_whitespace().nth(1).expect("object ID"));
    let base = blobs.next().expect("conflict blob");
    let other = blobs.next().expect("stage zero blob");
    git_ok(repo, &["update-index", "--force-remove", "conflict"])?;
    for (path, stages) in [
        ("conflict", vec![(1, base), (2, other), (3, base)]),
        ("both-added", vec![(2, base), (3, other)]),
        ("deleted-by-us", vec![(1, base), (3, other)]),
        ("both-deleted", vec![(1, base)]),
        ("added-by-us", vec![(2, base)]),
        ("added-by-them", vec![(3, base)]),
        ("deleted-by-them", vec![(1, base), (2, other)]),
    ] {
        use std::{io::Write, process::Stdio};
        let mut child = gix_testtools::git_command(repo)
            .args(["update-index", "--index-info"])
            .stdin(Stdio::piped())
            .spawn()?;
        let mut input = child.stdin.take().expect("piped stdin");
        for (stage, blob) in stages {
            writeln!(input, "100644 {blob} {stage}\t{path}")?;
        }
        drop(input);
        ensure!(child.wait()?.success(), "write index stages for {path}");
    }
    Ok(())
}

#[test]
fn status_porcelain_matches_git_for_changes_conflicts_and_branch() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = dir.path();
    init(repo)?;
    for (path, data) in [
        ("conflict", "base\n"),
        ("stage-zero", "other\n"),
        ("deleted", "delete me\n"),
        ("rename from", "rename me\n"),
        (
            "z before edit and rename",
            "line one stays the same
line two stays the same
line three stays the same
line four stays the same
",
        ),
        ("modified", "old\n"),
        ("staged-modified", "previous\n"),
        ("staged-deleted", "remove me\n"),
        (".gitignore", "*.ignored\n"),
    ] {
        fs::write(repo.join(path), data)?;
    }
    commit(repo, "root", "1700000000 +0100")?;
    git_ok(repo, &["branch", "upstream"])?;
    git_ok(repo, &["branch", "--set-upstream-to=upstream"])?;
    git_ok(repo, &["commit", "--allow-empty", "-qm", "ahead"])?;
    let behind = git_ok(repo, &["commit-tree", "HEAD^{tree}", "-p", "HEAD^", "-m", "behind"])?;
    git_ok(repo, &["update-ref", "refs/heads/upstream", &behind])?;
    fs::rename(repo.join("rename from"), repo.join("rename to"))?;
    fs::rename(
        repo.join("z before edit and rename"),
        repo.join("a after edit and rename"),
    )?;
    use std::io::Write;
    writeln!(
        fs::OpenOptions::new()
            .append(true)
            .open(repo.join("a after edit and rename"))?,
        "new line"
    )?;
    fs::write(repo.join("added"), "add me\n")?;
    fs::write(repo.join("staged-modified"), "staged content\n")?;
    git_ok(repo, &["add", "-A"])?;
    git_ok(repo, &["rm", "staged-deleted"])?;
    stages(repo)?;
    fs::remove_file(repo.join("deleted"))?;
    fs::write(repo.join("modified"), "new\n")?;
    fs::write(repo.join("added"), "added and modified\n")?;
    fs::write(repo.join("intent-to-add"), "new\n")?;
    git_ok(repo, &["add", "-N", "intent-to-add"])?;
    let untracked = if cfg!(windows) {
        "untracked name"
    } else {
        "untracked\tname"
    };
    fs::write(repo.join(untracked), "untracked\n")?;
    fs::write(repo.join("test.ignored"), "ignored\n")?;
    fs::create_dir(repo.join("untracked dir"))?;
    fs::write(repo.join("untracked dir/file"), "collapsed\n")?;
    for format in ["--porcelain", "--porcelain=v1", "--porcelain=v2"] {
        for suffix in [
            &[][..],
            &["-z", "--ignored"],
            &["--branch", "--ignored"],
            &["--branch", "-z"],
        ] {
            let mut args = vec!["status", format];
            args.extend_from_slice(suffix);
            parity(repo, &args, &args)?;
        }
    }
    parity(repo, &["status", "-f", "porcelain-v2"], &["status", "--porcelain=v2"])?;
    parity(repo, &["status", "-z"], &["status", "-z"])?;
    for format in ["--porcelain", "--porcelain=v2"] {
        for suffix in [&[][..], &["-z"]] {
            let mut args = vec!["status", format, "--branch", "--ignored"];
            args.extend_from_slice(suffix);
            parity(&repo.join("untracked dir"), &args, &args)?;
        }
    }
    Ok(())
}

#[test]
fn show_staged_blobs_match_git_at_each_stage() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = dir.path();
    init(repo)?;
    fs::write(repo.join("conflict"), b"base\0\xff")?;
    fs::write(repo.join("stage-zero"), b"other\0\xfe")?;
    fs::create_dir(repo.join("subdir"))?;
    fs::write(repo.join("subdir/file..name"), b"staged nested data\0")?;
    commit(repo, "root", "1700000000 +0100")?;
    stages(repo)?;
    for spec in [
        ":0:stage-zero",
        ":stage-zero",
        ":1:conflict",
        ":2:conflict",
        ":3:conflict",
    ] {
        parity(repo, &["show", spec], &["show", spec])?;
    }
    for spec in [":subdir/file..name", ":0:./file..name", ":0:../stage-zero"] {
        parity(&repo.join("subdir"), &["show", spec], &["show", spec])?;
    }
    for (spec, path, stage) in [(":0:conflict", "conflict", "0"), (":3:stage-zero", "stage-zero", "3")] {
        let missing = brit(repo, &["show", spec])?;
        ensure!(
            !missing.status.success() && missing.stdout.is_empty(),
            "missing stage must refuse"
        );
        let stderr = String::from_utf8_lossy(&missing.stderr);
        ensure!(
            stderr.contains(path) && stderr.contains(&format!("stage {stage}")),
            "{stderr}"
        );
    }
    Ok(())
}

#[test]
fn log_machine_formats_and_stat_match_git() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = dir.path();
    init(repo)?;
    fs::write(repo.join("tracked"), "old\n")?;
    commit(repo, "root", "1700000000 +0130")?;
    git_ok(repo, &["tag", "-am", "release notes", "v1"])?;
    fs::write(repo.join("tracked"), "new\nmore\n")?;
    fs::write(repo.join("added"), "added\n")?;
    commit(repo, "subject\ncontinued\n\nbody\n\nmore body", "1700000100 -0230")?;
    git_ok(repo, &["branch", "topic"])?;
    let format = "%H|%h|%s|%b|%B|%an|%ae|%ad|%aI|%at|%cn|%ce|%cI|%ct|%P|%p|%d|%D%n%%";
    for (option, oracle) in [
        (format!("--format={format}"), format!("--format={format}")),
        (format!("--pretty=format:{format}"), format!("--pretty=format:{format}")),
    ] {
        for date in ["--date=iso", "--date=iso-strict", "--date=unix", "--date=short"] {
            parity(repo, &["log", &option, date], &["log", &oracle, date])?;
        }
        parity(repo, &["log", &option], &["log", &oracle])?;
        parity(repo, &["log", &option, "--stat"], &["log", &oracle, "--stat"])?;
    }
    for args in [
        &["log", "--stat"][..],
        &["log", "-n", "1", "--stat"],
        &["log", "--oneline", "--stat"],
    ] {
        parity(repo, args, args)?;
    }
    let unknown = brit(repo, &["log", "--format=%H %ZZ"])?;
    ensure!(
        !unknown.status.success() && unknown.stdout.is_empty(),
        "unknown placeholder must refuse"
    );
    ensure!(
        String::from_utf8_lossy(&unknown.stderr).contains("%Z"),
        "diagnostic names placeholder"
    );
    Ok(())
}

#[test]
fn status_porcelain_matches_git_for_submodules_and_branch_states() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = dir.path().join("parent");
    let module = dir.path().join("source");
    fs::create_dir(&repo)?;
    fs::create_dir(&module)?;
    init(&repo)?;
    init(&module)?;
    for args in [
        &["status", "--porcelain", "--branch"][..],
        &["status", "--porcelain=v2", "--branch"],
    ] {
        parity(&repo, args, args)?;
    }
    fs::write(module.join("tracked"), "original\n")?;
    commit(&module, "first", "1700000000 +0000")?;
    git_ok(
        &repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            module.to_str().expect("UTF-8 fixture"),
            "module",
        ],
    )?;
    commit(&repo, "root", "1700000001 +0000")?;
    let checkout = repo.join("module");
    git_ok(&repo, &["switch", "--detach"])?;
    for (path, content) in [("untracked", "new\n"), ("tracked", "changed\n")] {
        fs::write(checkout.join(path), content)?;
        for args in [
            &["status", "--porcelain", "--branch"][..],
            &["status", "--porcelain=v2", "--branch", "-z"],
        ] {
            parity(&repo, args, args)?;
        }
    }
    git_ok(&checkout, &["config", "user.name", "Committer"])?;
    git_ok(&checkout, &["config", "user.email", "committer@example.com"])?;
    commit(&checkout, "new submodule commit", "1700000002 +0000")?;
    let args = [
        &["status", "--porcelain"][..],
        &["status", "--porcelain=v2", "--branch"],
    ];
    for args in args {
        parity(&repo, args, args)?;
    }
    git_ok(&repo, &["add", "module"])?;
    for args in args {
        parity(&repo, args, args)?;
    }
    git_ok(
        &checkout,
        &["submodule", "add", module.to_str().expect("UTF-8 fixture"), "nested"],
    )?;
    commit(&checkout, "add nested submodule", "1700000003 +0000")?;
    git_ok(&repo, &["add", "module"])?;
    for (path, content) in [("untracked", "nested untracked\n"), ("tracked", "nested modified\n")] {
        fs::write(checkout.join("nested").join(path), content)?;
        for args in args {
            parity(&repo, args, args)?;
        }
    }
    Ok(())
}

#[test]
fn log_stat_matches_git_for_renames_and_path_limits() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let repo = dir.path();
    init(repo)?;
    fs::create_dir(repo.join("dir"))?;
    fs::write(repo.join("dir/first"), "rename me\n")?;
    fs::write(repo.join("another"), "delete me\n")?;
    commit(repo, "root", "1700000000 +0000")?;
    fs::remove_file(repo.join("another"))?;
    git_ok(repo, &["mv", "dir/first", "dir/second"])?;
    commit(repo, "rename and delete", "1700000001 +0000")?;
    git_ok(repo, &["commit", "--allow-empty", "-qm", "empty"])?;
    for args in [
        &["log", "--stat"][..],
        &["log", "--stat", "HEAD", "--", "dir/"],
        &["log", "-n", "1", "--stat", "HEAD", "--", "dir/second"],
        &["log", "--stat", "--format="],
        &["log", "--stat", "--pretty=format:%s"],
    ] {
        parity(repo, args, args)?;
    }
    git_ok(repo, &["config", "diff.renames", "false"])?;
    parity(repo, &["log", "--stat"], &["log", "--stat"])?;
    Ok(())
}
