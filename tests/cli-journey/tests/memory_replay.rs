//! Replay journals preserve large-file edits and neighboring dirty work under qualification.
#![cfg(target_os = "linux")]

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

fn replay(size: usize, reduced_trust: bool) -> Result<()> {
    let repo = TestRepo::new("memory-replay")?;
    let root = repo.path();
    eprintln!("owned replay fixture: {}", root.display());
    let mut file = fs::File::create(root.join("large"))?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    fs::write(root.join("neighbor"), b"base neighbor\n")?;
    git(root, &["add", "large", "neighbor"])?;
    git(root, &["commit", "-qm", "base"])?;
    git(root, &["switch", "-c", "upstream"])?;
    let mut file = fs::OpenOptions::new().write(true).open(root.join("large"))?;
    file.write_all(b"upstream")?;
    drop(file);
    git(root, &["commit", "-qam", "upstream"])?;
    git(root, &["switch", "main"])?;
    let mut file = fs::OpenOptions::new().write(true).open(root.join("large"))?;
    file.seek(SeekFrom::Start((size - 65536) as u64))?;
    file.write_all(b"topic")?;
    drop(file);
    git(root, &["commit", "-qam", "topic"])?;
    let original = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let expected = String::from_utf8(git(
        root,
        &["merge-tree", "--write-tree", "--no-messages", "upstream", "main"],
    )?)?;
    let expected = expected.split_whitespace().next().expect("Git merged tree");
    let limits: &[&str] = if reduced_trust {
        &[""]
    } else {
        &["1073741824", "16777216"]
    };
    for &limit in limits {
        git(root, &["reset", "--hard", original.trim()])?;
        fs::write(root.join("neighbor"), b"staged neighbor\n")?;
        git(root, &["add", "neighbor"])?;
        fs::write(root.join("neighbor"), b"unstaged neighbor\n")?;
        if limit == "16777216" || reduced_trust {
            git(root, &["repack", "-ad"])?;
        }
        use std::{
            ffi::CString,
            os::unix::{ffi::OsStrExt, fs::MetadataExt},
        };
        let root_path = CString::new(root.as_os_str().as_bytes())?;
        let original_uid = fs::metadata(root)?.uid();
        if reduced_trust {
            // Discovery takes the minimum trust of the worktree and metadata owners.
            // Keep private metadata owned by the acting user, as replay recovery requires.
            // SAFETY: this path names only the disposable fixture's worktree root.
            ensure!(
                unsafe { libc::chown(root_path.as_ptr(), 65534, u32::MAX) } == 0,
                "requires chown authority"
            );
        }
        let mut invocation = BritInvocation::new(brit_bin().expect("built Brit")).current_dir(root);
        if !limit.is_empty() {
            invocation = invocation.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
        }
        let output = invocation
            .args(["rebase", "upstream"])
            .env("GIT_EDITOR", ":")
            .env("GIT_SEQUENCE_EDITOR", ":")
            .run()?;
        if reduced_trust {
            // Restore fixture ownership before invoking the independent Git oracle.
            // SAFETY: restore the observed owner of the same disposable path.
            ensure!(unsafe { libc::chown(root_path.as_ptr(), original_uid, u32::MAX) } == 0);
        }
        ensure!(
            output.status.success(),
            "rebase {size} bytes at {limit}: {}",
            output.stderr
        );
        ensure!(String::from_utf8(git(root, &["rev-parse", "HEAD^{tree}"])?)?.trim() == expected);
        ensure!(git(root, &["rev-parse", "HEAD^"])? == git(root, &["rev-parse", "upstream"])?);
        ensure!(git(root, &["show", ":neighbor"])? == b"staged neighbor\n");
        ensure!(fs::read(root.join("neighbor"))? == b"unstaged neighbor\n");
        ensure!(
            !root.join(".git/brit-rebase").exists(),
            "completed replay journal reclaimed"
        );
        git(root, &["fsck", "--full"])?;
    }
    Ok(())
}

#[test]
fn twenty_mib_replay() -> Result<()> {
    replay(20 * 1024 * 1024, false)
}
#[test]
fn sixty_four_mib_replay() -> Result<()> {
    replay(64 * 1024 * 1024, false)
}

#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn actual_reduced_trust_replay() -> Result<()> {
    replay(20 * 1024 * 1024, true)
}
