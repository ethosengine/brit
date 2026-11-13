//! Public comparison resources retain conversion semantics across allocation policies.
use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::Path,
};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn invoke(root: &Path, args: &[&str], limit: &str) -> Result<String> {
    let output = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["-c", limit])
        .args(args.iter().copied())
        .run()?;
    ensure!(output.status.success(), "Brit {args:?}: {}", output.stderr);
    Ok(output.stdout)
}

fn fixture(size: usize) -> Result<TestRepo> {
    let repo = TestRepo::new("memory-diff-resources")?;
    let mut file = fs::File::create(repo.path().join("large"))?;
    let mut chunk = [b'1'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    git(repo.path(), &["add", "large"])?;
    git(
        repo.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-qm",
            "base",
        ],
    )?;
    let mut file = fs::OpenOptions::new().write(true).open(repo.path().join("large"))?;
    file.seek(SeekFrom::Start(65536))?;
    file.write_all(b"changed")?;
    Ok(repo)
}

fn compare_resources(size: usize) -> Result<()> {
    let repo = fixture(size)?;
    let root = repo.path();
    let buffered = "gitoxide.objects.allocLimit=1073741824";
    let streamed = "gitoxide.objects.allocLimit=16777216";
    let args = ["diff", "file", "HEAD:large", "large"];
    let expected = invoke(root, &args, buffered)?;
    let oracle = git(root, &["diff", "--no-ext-diff", "--no-textconv", "--", "large"])?;
    let hunk = oracle
        .windows(3)
        .position(|bytes| bytes == b"@@ ")
        .expect("Git patch hunk");
    ensure!(
        expected.as_bytes() == &oracle[hunk..],
        "existing file-diff hunk compatibility"
    );
    ensure!(invoke(root, &args, streamed)? == expected);
    git(root, &["add", "large"])?;
    git(root, &["repack", "-ad"])?;
    let args = ["diff", "file", "HEAD:large", ":large"];
    ensure!(invoke(root, &args, streamed)? == expected);
    for format in ["diff", "diff-or-git"] {
        let args = [
            "revision",
            "resolve",
            "--cat-file",
            "--blob-format",
            format,
            "HEAD:large",
        ];
        ensure!(invoke(root, &args, streamed)? == invoke(root, &args, buffered)?);
    }
    Ok(())
}

#[test]
fn twenty_mib_file_comparison_and_diff_format_cat() -> Result<()> {
    compare_resources(20 * 1024 * 1024)
}
#[test]
fn sixty_four_mib_file_comparison_and_diff_format_cat() -> Result<()> {
    compare_resources(64 * 1024 * 1024)
}
#[test]
fn two_hundred_fifty_six_mib_file_comparison_and_diff_format_cat() -> Result<()> {
    compare_resources(256 * 1024 * 1024)
}

#[cfg(unix)]
#[test]
fn text_converter_resources_and_expansion_refusal_preserve_repository_state() -> Result<()> {
    let repo = fixture(20 * 1024 * 1024)?;
    let root = repo.path();
    fs::write(root.join(".gitattributes"), b"large diff=convert\n")?;
    git(root, &["add", ".gitattributes"])?;
    git(root, &["config", "diff.convert.textconv", "head -n 1"])?;
    let args = ["diff", "file", "HEAD:large", "large"];
    let limit = "gitoxide.objects.allocLimit=16777216";
    ensure!(
        invoke(root, &args, limit)?.is_empty(),
        "the converter hides the changed second line"
    );
    ensure!(git(root, &["diff", "--textconv", "--", "large"])?.is_empty());
    let converted = invoke(
        root,
        &[
            "revision",
            "resolve",
            "--cat-file",
            "--blob-format",
            "diff",
            "HEAD:large",
        ],
        limit,
    )?;
    ensure!(converted.len() == 65536 && converted.ends_with('\n'));
    fs::write(root.join(".git/expand"), b"head -c 20971520 /dev/zero\n")?;
    git(root, &["config", "diff.convert.textconv", "sh .git/expand"])?;
    let expanded = invoke(
        root,
        &[
            "revision",
            "resolve",
            "--cat-file",
            "--blob-format",
            "diff",
            "HEAD:large",
        ],
        limit,
    )?;
    ensure!(
        expanded.len() == 20 * 1024 * 1024 && expanded.bytes().all(|byte| byte == 0),
        "derived converter output above the allocation policy must not become binary"
    );
    drop(expanded);
    let index = fs::read(root.join(".git/index"))?;
    let head = repo.head_id()?;
    git(root, &["config", "diff.convert.textconv", "yes"])?;
    let output = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["-c", limit])
        .args(args)
        .run()?;
    ensure!(
        !output.status.success() && output.stderr.contains("hard 1 GiB aggregate scratch budget"),
        "{}",
        output.stderr
    );
    ensure!(
        output.stdout.is_empty(),
        "converter refusal does not publish a partial patch"
    );
    ensure!(fs::read(root.join(".git/index"))? == index && repo.head_id()? == head);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn interrupting_a_running_text_converter_reclaims_owned_resources() -> Result<()> {
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    struct OwnedChild(Child);
    impl Drop for OwnedChild {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let repo = fixture(20 * 1024 * 1024)?;
    let root = repo.path();
    fs::write(root.join(".gitattributes"), b"large diff=convert\n")?;
    git(root, &["add", ".gitattributes"])?;
    fs::write(
        root.join(".git/converter"),
        b"printf ready > .git/converter-started\nsleep 30\n",
    )?;
    git(root, &["config", "diff.convert.textconv", "sh .git/converter"])?;
    let index = fs::read(root.join(".git/index"))?;
    let head = repo.head_id()?;
    let scratch = tempfile::tempdir()?;
    let output = tempfile::tempfile()?;
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    let mut child = OwnedChild(
        command
            .current_dir(root)
            .env("TMPDIR", scratch.path())
            .args([
                "-c",
                "gitoxide.objects.allocLimit=16777216",
                "diff",
                "file",
                "HEAD:large",
                "large",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::from(output.try_clone()?))
            .stderr(Stdio::null())
            .spawn()?,
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    while !root.join(".git/converter-started").exists() {
        ensure!(child.0.try_wait()?.is_none(), "converter exited before the checkpoint");
        ensure!(Instant::now() < deadline, "converter did not reach the checkpoint");
        std::thread::sleep(Duration::from_millis(1));
    }
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
            "controlled converter interruption exceeded one second"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(
        output.metadata()?.len() == 0,
        "interruption does not publish a partial patch"
    );
    ensure!(fs::read(root.join(".git/index"))? == index && repo.head_id()? == head);
    ensure!(
        fs::read_dir(scratch.path())?.next().is_none(),
        "owned spools are reclaimed"
    );
    Ok(())
}
