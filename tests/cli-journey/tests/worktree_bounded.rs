//! Large-file checkout preserves bytes, conversions and rollback under bounded resources.
use anyhow::{ensure, Context, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let out = gix_testtools::git_command(repo).args(args).output()?;
    ensure!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}
fn commit(repo: &Path) -> Result<()> {
    git(
        repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-qm",
            "fixture",
        ],
    )?;
    Ok(())
}
#[cfg(unix)]
fn isolated_command(program: impl AsRef<std::ffi::OsStr>, root: &Path) -> std::process::Command {
    let mut command = std::process::Command::new(program);
    gix_testtools::configure_git_environment(&mut command, root);
    command.current_dir(root);
    command
}
fn write_large(path: &Path, size: usize) -> Result<()> {
    let mut file = fs::File::create(path)?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    file.write_all(&chunk[..size % chunk.len()])?;
    Ok(())
}
fn same_file(left: &Path, right: &Path) -> Result<()> {
    let description = format!("{} versus {}", left.display(), right.display());
    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    let mut a = [0; 65536];
    let mut b = [0; 65536];
    loop {
        let n = left.read(&mut a)?;
        let m = right.read(&mut b)?;
        ensure!(
            n == m && a[..n] == b[..m],
            "checkout bytes differ: {description}; first bytes {:?} versus {:?}",
            &a[..n.min(100)],
            &b[..m.min(100)]
        );
        if n == 0 {
            break;
        }
    }
    Ok(())
}
#[test]
fn large_loose_and_packed_worktrees_preserve_allocation_safeguards() -> Result<()> {
    for packed in [false, true] {
        for size in [
            16 * 1024 * 1024 - 1,
            16 * 1024 * 1024,
            16 * 1024 * 1024 + 1,
            20 * 1024 * 1024,
        ] {
            let repo = TestRepo::new("bounded-large")?;
            let root = repo.path();
            write_large(&root.join("governance-findings.jsonl"), size)?;
            git(root, &["add", "governance-findings.jsonl"])?;
            commit(root)?;
            if packed {
                git(root, &["repack", "-ad"])?;
            }
            let destinations = tempfile::tempdir()?;
            let target = destinations.path().join("bounded");
            let result = BritInvocation::new(brit_bin().expect("built binary"))
                .current_dir(root)
                .args([
                    "-c",
                    "gitoxide.objects.allocLimit=16777216",
                    "worktree",
                    "add",
                    "-b",
                    "bounded",
                ])
                .arg(target.as_os_str().to_owned())
                .run()?;
            ensure!(result.status.success(), "{}", result.stderr);
            same_file(
                &root.join("governance-findings.jsonl"),
                &target.join("governance-findings.jsonl"),
            )?;
            ensure!(git(&target, &["status", "--porcelain"])?.is_empty());
            ensure!(git(root, &["branch", "--show-current"])? != "bounded");
            ensure!(fs::read_dir(&target)?.all(|e| !e
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".brit-checkout-")));
        }
    }
    Ok(())
}
#[test]
fn conversions_and_single_file_filters_match_git() -> Result<()> {
    let repo = TestRepo::new("bounded-filters")?;
    let root = repo.path();
    fs::write(root.join(".gitattributes"), "text text eol=crlf\nencoded working-tree-encoding=ISO-8859-1\nfiltered filter=echo\noptional filter=optional\n")?;
    fs::write(root.join("text"), b"$Id$\nline\n")?;
    fs::write(root.join("encoded"), b"caf\xe9\n")?;
    write_large(&root.join("filtered"), 20 * 1024 * 1024)?;
    fs::write(root.join("optional"), b"retain this input")?;
    git(
        root,
        &["add", ".gitattributes", "text", "encoded", "filtered", "optional"],
    )?;
    commit(root)?;
    git(root, &["config", "filter.echo.smudge", "cat"])?;
    git(root, &["config", "filter.echo.required", "true"])?;
    git(root, &["config", "filter.optional.smudge", "cat; exit 1"])?;
    let dest = tempfile::tempdir()?;
    let actual = dest.path().join("actual");
    let expected = dest.path().join("expected");
    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            expected.to_str().expect("temporary path"),
            "HEAD",
        ],
    )?;
    let result = BritInvocation::new(brit_bin().expect("built binary"))
        .current_dir(root)
        .args(["worktree", "add", "--detach"])
        .arg(actual.as_os_str().to_owned())
        .run()?;
    ensure!(result.status.success(), "{}", result.stderr);
    for name in ["text", "encoded", "filtered", "optional"] {
        same_file(&actual.join(name), &expected.join(name))?;
    }
    Ok(())
}
#[test]
fn failed_filter_rolls_back_registration_and_retains_created_branch() -> Result<()> {
    let repo = TestRepo::new("bounded-rollback")?;
    let root = repo.path();
    fs::write(root.join(".gitattributes"), "file filter=broken\n")?;
    fs::write(root.join("file"), b"retained")?;
    git(root, &["add", ".gitattributes", "file"])?;
    commit(root)?;
    git(root, &["config", "filter.broken.smudge", "exit 1"])?;
    git(root, &["config", "filter.broken.required", "true"])?;
    let before = git(root, &["worktree", "list", "--porcelain"])?;
    let dest = tempfile::tempdir()?;
    let target = dest.path().join("failed");
    let binary = brit_bin().expect("built binary");
    let result = BritInvocation::new(&binary)
        .current_dir(root)
        .args(["worktree", "add", "-b", "retained"])
        .arg(target.as_os_str().to_owned())
        .run()?;
    ensure!(!result.status.success());
    ensure!(!target.exists());
    ensure!(git(root, &["worktree", "list", "--porcelain"])? == before);
    ensure!(git(root, &["rev-parse", "retained"])? == git(root, &["rev-parse", "HEAD"])?);
    git(root, &["config", "filter.broken.smudge", "cat"])?;
    let result = BritInvocation::new(binary)
        .current_dir(root)
        .args(["worktree", "add"])
        .arg(target.as_os_str().to_owned())
        .arg("retained")
        .run()?;
    ensure!(result.status.success(), "{}", result.stderr);
    same_file(&target.join("file"), &root.join("file"))?;
    Ok(())
}

#[test]
#[cfg(unix)]
fn empty_trees_modes_symlinks_gitlinks_and_foreign_destinations() -> Result<()> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let repo = TestRepo::new("bounded-entry-modes")?;
    let root = repo.path();
    let destinations = tempfile::tempdir()?;
    let binary = brit_bin().expect("built binary");
    let empty = destinations.path().join("empty");
    let result = BritInvocation::new(&binary)
        .current_dir(root)
        .args(["worktree", "add", "--detach"])
        .arg(empty.as_os_str().to_owned())
        .run()?;
    ensure!(result.status.success(), "{}", result.stderr);
    ensure!(git(&empty, &["status", "--porcelain"])?.is_empty());
    let base = git(root, &["rev-parse", "HEAD"])?;
    fs::write(root.join("script"), b"#!/bin/sh\nexit 0\n")?;
    symlink("script", root.join("link"))?;
    git(root, &["add", "script", "link"])?;
    git(root, &["update-index", "--chmod=+x", "script"])?;
    git(
        root,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{base},submodule"),
        ],
    )?;
    commit(root)?;
    let actual = destinations.path().join("modes");
    let result = BritInvocation::new(&binary)
        .current_dir(root)
        .args(["worktree", "add", "--detach"])
        .arg(actual.as_os_str().to_owned())
        .run()?;
    ensure!(result.status.success(), "{}", result.stderr);
    let oracle = destinations.path().join("git-modes");
    git(
        root,
        &["worktree", "add", "--detach", oracle.to_str().context("oracle path")?],
    )?;
    ensure!(
        fs::metadata(actual.join("script"))?.permissions().mode() & 0o777
            == fs::metadata(oracle.join("script"))?.permissions().mode() & 0o777,
        "checkout permissions must preserve Git's umask semantics"
    );
    ensure!(fs::read_link(actual.join("link"))? == Path::new("script"));
    ensure!(actual.join("submodule").is_dir());
    ensure!(git(&actual, &["status", "--porcelain"])?.is_empty());
    let foreign = destinations.path().join("foreign");
    fs::create_dir(&foreign)?;
    fs::write(foreign.join("sentinel"), b"preserve")?;
    let result = BritInvocation::new(binary)
        .current_dir(root)
        .args(["worktree", "add", "--detach"])
        .arg(foreign.as_os_str().to_owned())
        .run()?;
    ensure!(!result.status.success());
    ensure!(fs::read(foreign.join("sentinel"))? == b"preserve");
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn controlled_large_checkout_measurements() -> Result<()> {
    for mebibytes in [64, 256] {
        for conversion in ["plain", "crlf", "filter"] {
            let repo = TestRepo::new("bounded-measurement")?;
            let root = repo.path();
            write_large(&root.join("large"), mebibytes * 1024 * 1024)?;
            if conversion != "plain" {
                let attributes = if conversion == "crlf" {
                    "large text eol=crlf\n"
                } else {
                    "large filter=echo\n"
                };
                fs::write(root.join(".gitattributes"), attributes)?;
                git(root, &["add", ".gitattributes"])?;
            }
            git(root, &["add", "large"])?;
            commit(root)?;
            if conversion == "filter" {
                git(root, &["config", "filter.echo.smudge", "printf 'filtered\\n'; cat"])?;
                git(root, &["config", "filter.echo.required", "true"])?;
            }
            git(root, &["repack", "-ad"])?;
            let destinations = tempfile::tempdir()?;
            let target = destinations.path().join("bounded");
            let timing = destinations.path().join("time.txt");
            let binary = brit_bin().expect("built binary");
            let started = std::time::Instant::now();
            let output = isolated_command("/usr/bin/time", root)
                .args(["-f", "%M %e %x", "-o"])
                .arg(&timing)
                .arg(&binary)
                .current_dir(root)
                .args([
                    "-c",
                    "gitoxide.objects.allocLimit=16777216",
                    "worktree",
                    "add",
                    "--detach",
                ])
                .arg(&target)
                .output()
                .context("run /usr/bin/time for the controlled checkout; GNU time is required")?;
            ensure!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let timing = fs::read_to_string(timing)?;
            let peak_kib: u64 = timing.split_whitespace().next().expect("time maximum RSS").parse()?;
            eprintln!(
                "{}",
                serde_json::json!({"bytes": mebibytes * 1024 * 1024, "conversion": conversion, "executable": binary, "elapsedMs": started.elapsed().as_millis(), "peakRssKiB": peak_kib, "outcome": "success"})
            );
            ensure!(
                peak_kib < 64 * 1024,
                "controlled checkout RSS exceeded 64 MiB: {peak_kib} KiB"
            );
            if conversion != "plain" {
                let expected = destinations.path().join("git");
                git(
                    root,
                    &[
                        "worktree",
                        "add",
                        "--detach",
                        expected.to_str().expect("temporary path"),
                        "HEAD",
                    ],
                )?;
                same_file(&target.join("large"), &expected.join("large"))?;
            } else {
                same_file(&target.join("large"), &root.join("large"))?;
            }
        }
    }
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn interruption_during_blob_writing_rolls_back_owned_state() -> Result<()> {
    use std::{
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    let repo = TestRepo::new("bounded-interrupt")?;
    let root = repo.path();
    write_large(&root.join("large"), 256 * 1024 * 1024)?;
    git(root, &["add", "large"])?;
    commit(root)?;
    let index = fs::read(root.join(".git/index"))?;
    fs::write(root.join(".git/index.lock"), b"foreign lock")?;
    let destinations = tempfile::tempdir()?;
    let target = destinations.path().join("interrupted");
    let log = destinations.path().join("stderr");
    let mut child = isolated_command(brit_bin().expect("built binary"), root)
        .args(["worktree", "add", "-b", "interrupted"])
        .arg(&target)
        .stdout(Stdio::null())
        .stderr(fs::File::create(&log)?)
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if child.try_wait()?.is_some() {
            anyhow::bail!("checkout ended before interruption: {}", fs::read_to_string(&log)?);
        }
        let writing = fs::read_dir(&target)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(".brit-checkout-"))
            .any(|dir| {
                fs::read_dir(dir.path())
                    .into_iter()
                    .flatten()
                    .filter_map(Result::ok)
                    .any(|file| file.metadata().is_ok_and(|m| m.len() >= 1024 * 1024))
            });
        if writing {
            break;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("no observed blob write");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let started = Instant::now();
    ensure!(Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()?
        .success());
    while child.try_wait()?.is_none() {
        if started.elapsed() > Duration::from_secs(5) {
            child.kill()?;
            child.wait()?;
            anyhow::bail!("controlled cancellation exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(!child.wait()?.success());
    ensure!(!target.exists());
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(fs::read(root.join(".git/index.lock"))? == b"foreign lock");
    ensure!(git(root, &["rev-parse", "interrupted"])? == git(root, &["rev-parse", "HEAD"])?);
    eprintln!(
        "bounded checkout SIGINT observed after writing; return_ms={}",
        started.elapsed().as_millis()
    );
    Ok(())
}

#[test]
#[cfg(unix)]
fn operating_system_write_failure_rolls_back_without_touching_foreign_lock() -> Result<()> {
    let repo = TestRepo::new("bounded-write-failure")?;
    let root = repo.path();
    write_large(&root.join("large"), 20 * 1024 * 1024)?;
    git(root, &["add", "large"])?;
    commit(root)?;
    fs::write(root.join(".git/index.lock"), b"foreign lock")?;
    let destinations = tempfile::tempdir()?;
    let target = destinations.path().join("failed");
    let output = isolated_command("sh", root)
        .args([
            "-c",
            "trap '' XFSZ; ulimit -f 1024; exec \"$@\"",
            "bounded-write-fixture",
        ])
        .arg(brit_bin().expect("built binary"))
        .current_dir(root)
        .args(["worktree", "add", "-b", "retained"])
        .arg(&target)
        .output()?;
    ensure!(!output.status.success());
    ensure!(!target.exists());
    ensure!(fs::read(root.join(".git/index.lock"))? == b"foreign lock");
    ensure!(git(root, &["rev-parse", "retained"])? == git(root, &["rev-parse", "HEAD"])?);
    ensure!(!root.join(".git/worktrees/failed").exists());
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn optional_filter_cannot_hide_hard_quota_exhaustion() -> Result<()> {
    let repo = TestRepo::new("bounded-hard-quota")?;
    let root = repo.path();
    fs::write(root.join(".gitattributes"), "file filter=expand\n")?;
    fs::write(root.join("file"), b"preserve original")?;
    git(root, &["add", ".gitattributes", "file"])?;
    commit(root)?;
    git(
        root,
        &[
            "config",
            "filter.expand.smudge",
            "cat >/dev/null; head -c 1073741824 /dev/zero",
        ],
    )?;
    let before = git(root, &["worktree", "list", "--porcelain"])?;
    let index = fs::read(root.join(".git/index"))?;
    fs::write(root.join(".git/index.lock"), b"foreign lock")?;
    let destinations = tempfile::tempdir()?;
    let target = destinations.path().join("quota");
    let result = BritInvocation::new(brit_bin().expect("built binary"))
        .current_dir(root)
        .args(["worktree", "add", "-b", "retained"])
        .arg(target.as_os_str().to_owned())
        .run()?;
    ensure!(
        !result.status.success(),
        "quota must not fall back to unfiltered success"
    );
    ensure!(
        result.stderr.contains("hard 1 GiB aggregate scratch budget"),
        "{}",
        result.stderr
    );
    ensure!(!target.exists());
    ensure!(git(root, &["worktree", "list", "--porcelain"])? == before);
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(fs::read(root.join(".git/index.lock"))? == b"foreign lock");
    ensure!(git(root, &["rev-parse", "retained"])? == git(root, &["rev-parse", "HEAD"])?);
    Ok(())
}
