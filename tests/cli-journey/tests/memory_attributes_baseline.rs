//! Public baseline validation must retain helper failures, not report false parity.
#![cfg(unix)]

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::{BufWriter, Read, Write},
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Command, Stdio},
};

#[test]
fn baseline_helper_failures_are_not_success() -> Result<()> {
    let repo = TestRepo::new("memory-baseline-helper")?;
    let root = repo.path();
    repo.commit_file("selected", "unchanged\n")?;
    repo.commit_file(".gitattributes", "selected custom=value extra=present\n")?;
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    let paths = root.join(".git/baseline-paths");
    fs::write(&paths, "selected\n")?;
    let helpers = root.join(".git/helpers");
    fs::create_dir(&helpers)?;
    let real_git = std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .expect("stock Git oracle");
    let shim = helpers.join("git");
    fs::write(
        &shim,
        r#"#!/bin/sh
if test "$1" = check-attr; then
    printf '%s\n' "$$" >> "$BRIT_TEST_HELPER_PIDS"
    case "$BRIT_TEST_BASELINE_MODE" in
        fail) printf '%s\n' 'synthetic helper failure' >&2; exit 17 ;;
        malformed) printf '%s\n' 'malformed attribute record'; exit 0 ;;
        prefix) printf 'selected\000custom\000value\000'; exit 0 ;;
        empty) exit 0 ;;
    esac
fi
exec "$BRIT_TEST_REAL_GIT" "$@"
"#,
    )?;
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755))?;
    let path = std::env::join_paths(
        std::iter::once(helpers).chain(std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))),
    )?;
    for mode in ["ordinary", "fail", "malformed", "prefix", "empty"] {
        let pids = root.join(format!(".git/helper-pids-{mode}"));
        let mut command = Command::new(brit_bin().expect("built Brit"));
        gix_testtools::configure_git_environment(&mut command, root);
        let output = command
            .current_dir(root)
            .args(["attributes", "validate-baseline", "--no-ignore"])
            .env("PATH", &path)
            .env("BRIT_TEST_REAL_GIT", &real_git)
            .env("BRIT_TEST_HELPER_PIDS", &pids)
            .env("BRIT_TEST_BASELINE_MODE", mode)
            .stdin(Stdio::from(fs::File::open(&paths)?))
            .output()?;
        ensure!(pids.is_file(), "the baseline helper must have run");
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        if mode == "ordinary" {
            ensure!(output.status.success(), "ordinary baseline: {diagnostic}");
        } else {
            ensure!(!output.status.success(), "{mode} helper must refuse false parity");
            ensure!(
                diagnostic.contains(match mode {
                    "fail" => "17",
                    "prefix" | "empty" => "mismatch",
                    _ => "baseline",
                }),
                "{diagnostic}"
            );
        }
    }
    ensure!(repo.head_id()? == head);
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(fs::read(root.join("selected"))? == b"unchanged\n");
    Ok(())
}

fn oracle(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let result = gix_testtools::git_command(root)
        .args(["-c", &format!("safe.directory={}", root.display())])
        .args(args)
        .output()?;
    ensure!(
        result.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(result.stdout)
}

fn baseline_command(root: &Path) -> Command {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        // This explicit fixture allow-list lets the stock-Git child read the repository.
        // Brit's ownership-derived trust must still be Reduced, as verified below.
        .env("GIT_CONFIG_COUNT", "1")
        .env("GIT_CONFIG_KEY_0", "safe.directory")
        .env("GIT_CONFIG_VALUE_0", root);
    command
}

fn large_baseline(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-attributes-baseline")?;
    let root = repo.path();
    for (name, rules) in [
        (".gitattributes", b"\n* custom=value extra=present\n".as_slice()),
        (".gitignore", b"\nignored-*\n".as_slice()),
    ] {
        let mut file = BufWriter::new(fs::File::create(root.join(name))?);
        for _ in 0..320 {
            file.write_all(&[b'#'; 65536])?;
        }
        file.write_all(rules)?;
        file.flush()?;
    }
    oracle(root, &["add", ".gitattributes", ".gitignore"])?;
    oracle(root, &["commit", "-qm", "large indexed attributes"])?;
    let head = oracle(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let ignore_id = oracle(root, &["hash-object", ".gitignore"])?;
    fs::remove_file(root.join(".gitattributes"))?;
    let input_path = root.join(".git/baseline-input");
    let mut input = BufWriter::new(fs::File::create(&input_path)?);
    let suffix = "x".repeat(176);
    for n in 0..120_000 {
        let prefix = if n % 2 == 0 { "selected" } else { "ignored" };
        writeln!(input, "{prefix}-{n:06}-{suffix}")?;
    }
    input.flush()?;
    drop(input);
    ensure!(fs::metadata(&input_path)?.len() > 20 * 1024 * 1024);
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let result = baseline_command(root).args(["free", "trust", ".", ".git"]).output()?;
            ensure!(result.status.success());
            ensure!(
                String::from_utf8(result.stdout)?
                    .lines()
                    .filter(|l| l.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("ownership qualification runs on Linux");
    }
    for packed in [false, true] {
        if packed {
            oracle(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("1073741824"), Some("16777216"), Some("1024")] {
            let mut command = baseline_command(root);
            if let Some(limit) = limit {
                command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
            }
            let output_path = root.join(".git/baseline-output");
            let error_path = root.join(".git/baseline-error");
            let result = command
                .args(["attributes", "validate-baseline"])
                .stdin(Stdio::from(fs::File::open(&input_path)?))
                .stdout(Stdio::from(fs::File::create(&output_path)?))
                .stderr(Stdio::from(fs::File::create(&error_path)?))
                .status()?;
            let mut diagnostic = String::new();
            fs::File::open(error_path)?.take(8192).read_to_string(&mut diagnostic)?;
            ensure!(result.success(), "packed={packed}, limit={limit:?}: {diagnostic}");
            ensure!(fs::metadata(output_path)?.len() == 0);
        }
    }
    ensure!(oracle(root, &["rev-parse", "HEAD"])? == head);
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(oracle(root, &["hash-object", ".gitignore"])? == ignore_id);
    ensure!(!root.join(".gitattributes").exists());
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN and delegated cgroup"]
fn baseline_twenty_mib_patterns_and_many_paths() -> Result<()> {
    large_baseline(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN, delegated cgroup and chown authority"]
fn baseline_twenty_mib_patterns_and_many_paths_reduced_trust() -> Result<()> {
    large_baseline(true)
}

#[cfg(target_os = "linux")]
#[test]
fn pattern_queries_preserve_input_and_statistics_io_errors() -> Result<()> {
    let repo = TestRepo::new("memory-pattern-io")?;
    let root = repo.path();
    repo.commit_file("selected", "unchanged\n")?;
    repo.commit_file(".gitattributes", "selected custom=value\n")?;
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    let commands: [&[&str]; 3] = [
        &["attributes", "query"],
        &["exclude", "query"],
        &["attributes", "validate-baseline"],
    ];
    for args in commands {
        let mut command = baseline_command(root);
        let mut child = command
            .args(args)
            .stdin(Stdio::from(fs::File::open(root)?))
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while child.try_wait()?.is_none() {
            if std::time::Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("{args:?} did not return its input read error");
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = child.wait_with_output()?;
        ensure!(!output.status.success());
        ensure!(String::from_utf8_lossy(&output.stderr).contains("directory"));
    }
    for args in [
        ["attributes", "query", "--statistics", "selected"],
        ["exclude", "query", "--statistics", "selected"],
    ] {
        let status = baseline_command(root)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::from(fs::OpenOptions::new().write(true).open("/dev/full")?))
            .status()?;
        ensure!(!status.success(), "statistics output failure must be returned");
    }
    ensure!(repo.head_id()? == head);
    ensure!(fs::read(root.join(".git/index"))? == index);
    Ok(())
}

#[test]
fn bare_baseline_reports_the_existing_attribute_source_difference() -> Result<()> {
    let repo = TestRepo::new("memory-baseline-bare")?;
    let root = repo.path();
    repo.commit_file(".gitattributes", "selected custom=value\n")?;
    let bare = root.join(".git/bare");
    oracle(root, &["clone", "--bare", ".", bare.to_str().expect("fixture path")])?;
    let input = root.join(".git/bare-input");
    fs::write(&input, "selected\n")?;
    let head = fs::read(bare.join("HEAD"))?;
    let result = baseline_command(&bare)
        .args(["attributes", "validate-baseline"])
        .stdin(Stdio::from(fs::File::open(input)?))
        .output()?;
    ensure!(!result.status.success());
    ensure!(String::from_utf8_lossy(&result.stderr).contains("mismatch"));
    ensure!(fs::read(bare.join("HEAD"))? == head);
    Ok(())
}

#[test]
fn baseline_checks_unattributed_repeated_and_byte_paths() -> Result<()> {
    use std::os::unix::ffi::OsStringExt;
    let repo = TestRepo::new("memory-baseline-paths")?;
    let root = repo.path();
    repo.commit_file(".gitattributes", "* custom=value\nplain -custom\n")?;
    repo.commit_file(".gitignore", "ignored*\n")?;
    let mut paths = Vec::new();
    for path in [
        b"ordinary".as_slice(),
        b"ordinary",
        b"plain",
        b"a: path",
        b"tab\tpath",
        b"quote\"path",
        b"back\\slash",
        // macOS filesystems reject non-UTF-8 filenames before Brit runs.
        #[cfg(not(target_os = "macos"))]
        b"bytes\xff",
        #[cfg(target_os = "macos")]
        "bytes-é".as_bytes(),
        b"ordinary",
        #[cfg(not(target_os = "macos"))]
        b"ignored\xff",
        #[cfg(target_os = "macos")]
        "ignored-é".as_bytes(),
    ] {
        fs::write(root.join(std::ffi::OsString::from_vec(path.to_vec())), b"unchanged\n")?;
        paths.extend_from_slice(path);
        paths.push(b'\n');
    }
    let input = root.join(".git/paths");
    fs::write(&input, paths)?;
    for attributes in [true, false] {
        if !attributes {
            fs::write(root.join(".gitattributes"), b"")?;
        }
        let result = baseline_command(root)
            .args(["attributes", "validate-baseline"])
            .stdin(Stdio::from(fs::File::open(&input)?))
            .output()?;
        ensure!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn interrupted_baseline_reaps_its_helper_and_reclaims_spools() -> Result<()> {
    use std::{
        process::Child,
        time::{Duration, Instant},
    };
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let repo = TestRepo::new("memory-baseline-interrupt")?;
    let root = repo.path();
    repo.commit_file("selected", "unchanged\n")?;
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    let helpers = root.join(".git/helpers");
    fs::create_dir(&helpers)?;
    let shim = helpers.join("git");
    let real_git = std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))
        .map(|p| p.join("git"))
        .find(|p| p.is_file())
        .expect("stock Git oracle");
    // exec preserves the recorded PID, making child lifetime directly observable.
    // Installation-config discovery still uses the real Git before this checkpoint.
    fs::write(
        &shim,
        br#"#!/bin/sh
if test "$1" = check-attr; then
    printf '%s' "$$" > "$BRIT_TEST_HELPER_PID"
    exec sleep 30
fi
exec "$BRIT_TEST_REAL_GIT" "$@"
"#,
    )?;
    fs::set_permissions(&shim, fs::Permissions::from_mode(0o755))?;
    let path = std::env::join_paths(
        std::iter::once(helpers).chain(std::env::split_paths(&std::env::var_os("PATH").expect("PATH"))),
    )?;
    let pid_file = root.join(".git/helper-pid");
    let input = root.join(".git/paths");
    fs::write(&input, b"selected\n")?;
    let scratch = tempfile::tempdir()?;
    let errors = tempfile::tempfile()?;
    let mut child = OwnedChild(
        baseline_command(root)
            .args(["attributes", "validate-baseline", "--no-ignore"])
            .env("PATH", path)
            .env("BRIT_TEST_HELPER_PID", &pid_file)
            .env("BRIT_TEST_REAL_GIT", &real_git)
            .env("TMPDIR", scratch.path())
            .stdin(Stdio::from(fs::File::open(input)?))
            .stdout(Stdio::null())
            .stderr(Stdio::from(errors.try_clone()?))
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    let pid = loop {
        ensure!(
            child.0.try_wait()?.is_none(),
            "baseline exited before helper checkpoint"
        );
        if let Ok(pid) = fs::read_to_string(&pid_file) {
            if let Ok(pid) = pid.parse::<u32>() {
                break pid;
            }
        }
        ensure!(Instant::now() < deadline, "baseline helper checkpoint timed out");
        std::thread::sleep(Duration::from_millis(1));
    };
    let start = Instant::now();
    ensure!(Command::new("kill")
        .args(["-INT", &child.0.id().to_string()])
        .status()?
        .success());
    loop {
        if let Some(status) = child.0.try_wait()? {
            ensure!(!status.success());
            break;
        }
        ensure!(
            start.elapsed() < Duration::from_secs(1),
            "controlled interruption exceeded one second"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(!Path::new(&format!("/proc/{pid}")).exists(), "owned helper was reaped");
    ensure!(
        fs::read_dir(scratch.path())?.next().is_none(),
        "owned spools were reclaimed"
    );
    ensure!(repo.head_id()? == head && fs::read(root.join(".git/index"))? == index);
    Ok(())
}
