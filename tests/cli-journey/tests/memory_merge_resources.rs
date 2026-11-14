//! Object-backed merges preserve bytes and decisions at either allocation policy.
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

fn fixture(size: usize) -> Result<TestRepo> {
    let repo = TestRepo::new("memory-merge-resources")?;
    let root = repo.path();
    let mut file = fs::File::create(root.join("base"))?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    finish_fixture(repo, size)
}

fn finish_fixture(repo: TestRepo, size: usize) -> Result<TestRepo> {
    let root = repo.path();
    for (name, offset, text) in [("ours", 0, &b"ours"[..]), ("theirs", size - 65536, &b"theirs"[..])] {
        fs::copy(root.join("base"), root.join(name))?;
        let mut file = fs::OpenOptions::new().write(true).open(root.join(name))?;
        file.seek(SeekFrom::Start(offset as u64))?;
        file.write_all(text)?;
    }
    git(root, &["add", "base", "ours", "theirs"])?;
    git(root, &["commit", "-qm", "three sides"])?;
    Ok(repo)
}

fn compare_merge(size: usize) -> Result<()> {
    compare_fixture(fixture(size)?)
}

fn compare_fixture(repo: TestRepo) -> Result<()> {
    let root = repo.path();
    let expected = git(root, &["merge-file", "-p", "ours", "base", "theirs"])?;
    for packed in [false, true] {
        if packed {
            git(root, &["repack", "-ad"])?;
        }
        for limit in ["1073741824", "16777216"] {
            let output = BritInvocation::new(brit_bin().expect("built Brit"))
                .current_dir(root)
                .args([
                    "-c",
                    &format!("gitoxide.objects.allocLimit={limit}"),
                    "merge",
                    "file",
                    "HEAD:ours",
                    "HEAD:base",
                    "HEAD:theirs",
                ])
                .run()?;
            ensure!(
                output.status.success(),
                "object-backed merge at {limit}: {}",
                output.stderr
            );
            ensure!(output.stdout.as_bytes() == expected, "merge bytes must match Git");
        }
    }
    compare_tree_merges(root)?;
    Ok(())
}

fn compare_tree_merges(root: &Path) -> Result<()> {
    let mut commits = Vec::<String>::new();
    for side in ["base", "ours", "theirs"] {
        let blob = String::from_utf8(git(root, &["rev-parse", &format!("HEAD:{side}")])?)?;
        git(root, &["read-tree", "--empty"])?;
        git(
            root,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{},content", blob.trim()),
            ],
        )?;
        let tree = String::from_utf8(git(root, &["write-tree"])?)?;
        let mut args = vec!["commit-tree", tree.trim(), "-m", side];
        if let Some(base) = commits.first() {
            args.extend(["-p", base]);
        }
        let commit = String::from_utf8(git(root, &args)?)?;
        commits.push(commit.trim().to_owned());
    }
    let [base, ours, theirs] = &commits[..] else {
        unreachable!()
    };
    let expected = String::from_utf8(git(
        root,
        &["merge-tree", "--write-tree", "--no-messages", ours, theirs],
    )?)?;
    let expected = expected.split_whitespace().next().expect("Git merged tree");
    let head = fs::read(root.join(".git/HEAD"))?;
    let index = fs::read(root.join(".git/index"))?;
    for limit in ["1073741824", "16777216"] {
        for args in [
            vec!["merge", "tree", ours, base, theirs],
            vec!["merge", "commit", ours, theirs],
        ] {
            let output = BritInvocation::new(brit_bin().expect("built Brit"))
                .current_dir(root)
                .args(["-c", &format!("gitoxide.objects.allocLimit={limit}")])
                .args(args)
                .run()?;
            ensure!(output.status.success(), "tree/commit merge: {}", output.stderr);
            ensure!(
                output.stdout.split_whitespace().next() == Some(expected),
                "merged tree identity matches Git"
            );
        }
    }
    ensure!(fs::read(root.join(".git/HEAD"))? == head && fs::read(root.join(".git/index"))? == index);
    Ok(())
}
#[test]
fn twenty_mib_object_merge() -> Result<()> {
    compare_merge(20 * 1024 * 1024)
}
#[test]
fn three_sided_sixty_four_mib_object_merge() -> Result<()> {
    compare_merge(64 * 1024 * 1024)
}

#[cfg(unix)]
fn external_driver(size: usize) -> Result<()> {
    use std::process::{Command, Stdio};

    let repo = fixture(size)?;
    let root = repo.path();
    let expected = root.join(".git/expected-merge");
    let mut oracle = gix_testtools::git_command(root);
    let status = oracle
        .args(["merge-file", "-p", "ours", "base", "theirs"])
        .stdout(Stdio::from(fs::File::create(&expected)?))
        .status()?;
    ensure!(status.success(), "Git merge oracle");
    let expected_id = git(root, &["hash-object", expected.to_str().expect("fixture path")])?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let inputs = git(root, &["hash-object", "base", "ours", "theirs"])?;
    // Object-only merges read index attributes; info attributes also apply without a worktree root.
    fs::write(root.join(".git/info/attributes"), b"* merge=fixture\n")?;
    fs::write(
        root.join("driver.sh"),
        b"printf 'called\\n' >> .git/driver-ran\nexec git merge-file \"$1\" \"$2\" \"$3\"\n",
    )?;
    git(root, &["config", "merge.fixture.driver", "sh driver.sh %A %O %B"])?;
    let mut calls = 0;
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in ["1073741824", "16777216"] {
            eprintln!("external merge size={size}, packed={packed}, limit={limit}");
            let output_path = root.join(".git/actual-merge");
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            let output = command
                .current_dir(root)
                .args([
                    "-c",
                    &format!("gitoxide.objects.allocLimit={limit}"),
                    "merge",
                    "file",
                    "HEAD:ours",
                    "HEAD:base",
                    "HEAD:theirs",
                ])
                .stdout(Stdio::from(fs::File::create(&output_path)?))
                .output()?;
            ensure!(
                output.status.success(),
                "external merge: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            calls += 1;
            ensure!(
                fs::read_to_string(root.join(".git/driver-ran"))?.lines().count() == calls,
                "the configured external driver must actually run"
            );
            ensure!(
                git(root, &["hash-object", output_path.to_str().expect("fixture path")])? == expected_id,
                "external merge output matches Git"
            );
            ensure!(
                git(root, &["rev-parse", "HEAD"])? == head
                    && fs::read(root.join(".git/index"))? == index
                    && git(root, &["hash-object", "base", "ours", "theirs"])? == inputs,
                "external merge preserves repository and input files"
            );
        }
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn external_driver_twenty_mib() -> Result<()> {
    external_driver(20 * 1024 * 1024)
}

#[cfg(unix)]
#[test]
fn external_driver_sixty_four_mib() -> Result<()> {
    external_driver(64 * 1024 * 1024)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn actual_reduced_trust_object_merge() -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let repo = fixture(20 * 1024 * 1024)?;
    let root = repo.path();
    let expected = git(root, &["merge-file", "-p", "ours", "base", "theirs"])?;
    git(root, &["repack", "-ad"])?;
    let index = fs::read(root.join(".git/index"))?;
    for path in [root.to_owned(), root.join(".git")] {
        let path = CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: the NUL-terminated paths name this test's disposable fixture only.
        ensure!(
            unsafe { libc::chown(path.as_ptr(), 65534, 65534) } == 0,
            "requires chown authority"
        );
    }
    let output = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["merge", "file", "HEAD:ours", "HEAD:base", "HEAD:theirs"])
        .run()?;
    ensure!(output.status.success(), "reduced trust merge: {}", output.stderr);
    ensure!(output.stdout.as_bytes() == expected);
    ensure!(fs::read(root.join(".git/index"))? == index);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn merge_cancels_during_object_spooling() -> Result<()> {
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
    let repo = fixture(64 * 1024 * 1024)?;
    let root = repo.path();
    let index = fs::read(root.join(".git/index"))?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let scratch = tempfile::tempdir()?;
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        .env("TMPDIR", scratch.path())
        .env("GIT_CONFIG_GLOBAL", root.join(".git/absent-global"))
        .args(["merge", "file", "HEAD:ours", "HEAD:base", "HEAD:theirs"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = OwnedChild(command.spawn()?);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let mut spooling = false;
        for dir in fs::read_dir(scratch.path())? {
            let dir = dir?;
            if dir.file_type()?.is_dir() && dir.file_name().to_string_lossy().starts_with(".brit-checkout-") {
                for file in fs::read_dir(dir.path())? {
                    if file?.metadata()?.len() >= 1024 * 1024 {
                        spooling = true;
                    }
                }
            }
        }
        if spooling {
            break;
        }
        ensure!(child.0.try_wait()?.is_none(), "merge exited before spooling checkpoint");
        ensure!(Instant::now() < deadline, "merge did not reach spooling checkpoint");
        std::thread::sleep(Duration::from_millis(1));
    }
    let interrupted = Instant::now();
    ensure!(Command::new("kill")
        .args(["-INT", &child.0.id().to_string()])
        .status()?
        .success());
    loop {
        if let Some(status) = child.0.try_wait()? {
            ensure!(!status.success(), "cancelled merge must report failure");
            break;
        }
        ensure!(
            interrupted.elapsed() < Duration::from_secs(1),
            "controlled merge cancellation exceeded one second"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(git(root, &["rev-parse", "HEAD"])? == head);
    ensure!(
        fs::read_dir(scratch.path())?.next().is_none(),
        "cancelled merge must reclaim its spools"
    );
    Ok(())
}

#[test]
fn one_million_short_line_merges() -> Result<()> {
    for unique in [false, true] {
        let repo = TestRepo::new("memory-merge-million")?;
        let path = repo.path().join("base");
        let mut file = std::io::BufWriter::new(fs::File::create(&path)?);
        for line in 0..1_000_000 {
            writeln!(file, "line-{:08} body", if unique { line } else { 0 })?;
        }
        file.flush()?;
        drop(file);
        let size = usize::try_from(fs::metadata(path)?.len())?;
        compare_fixture(finish_fixture(repo, size)?)?;
    }
    Ok(())
}
