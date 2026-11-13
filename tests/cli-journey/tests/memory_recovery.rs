//! Large authoring journals must preserve repository state within the qualification ceiling.
#![cfg(target_os = "linux")]

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{fs, io::Write, path::Path};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn switch_large_file(size: usize) -> Result<()> {
    let repo = TestRepo::new("memory-switch")?;
    let root = repo.path();
    git(root, &["config", "user.name", "Fixture"])?;
    git(root, &["config", "user.email", "fixture@example.com"])?;
    let selected = root.join("selected");
    let chunk = [b'x'; 65536];
    let mut file = fs::File::create(&selected)?;
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    git(root, &["add", "selected"])?;
    git(root, &["commit", "-qm", "large base"])?;
    git(root, &["branch", "destination"])?;
    let branch = git(root, &["symbolic-ref", "--short", "HEAD"])?;
    git(root, &["switch", "destination"])?;
    let mut file = fs::OpenOptions::new().append(true).open(&selected)?;
    file.write_all(b"\nchanged\n")?;
    drop(file);
    git(root, &["commit", "-qam", "large destination"])?;
    let expected = git(root, &["ls-files", "--stage"])?;
    for limit in [
        "gitoxide.objects.allocLimit=1073741824",
        "gitoxide.objects.allocLimit=16777216",
    ] {
        git(root, &["switch", std::str::from_utf8(&branch)?.trim()])?;
        if limit == "gitoxide.objects.allocLimit=16777216" {
            git(root, &["repack", "-ad"])?;
        }
        let mut command = std::process::Command::new(brit_bin().expect("built Brit"));
        gix_testtools::configure_git_environment(&mut command, root);
        let output = command
            .current_dir(root)
            .args(["-c", limit, "switch", "destination"])
            .output()?;
        ensure!(
            output.status.success(),
            "switch: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        ensure!(git(root, &["ls-files", "--stage"])? == expected, "index matches target");
        ensure!(
            git(root, &["status", "--porcelain"])?.is_empty(),
            "checkout matches target"
        );
        ensure!(!root.join(".git/brit-switch").exists(), "completed journal reclaimed");
    }
    Ok(())
}

#[test]
fn switch_twenty_mib_file() -> Result<()> {
    switch_large_file(20 * 1024 * 1024)
}

#[test]
fn switch_sixty_four_mib_file() -> Result<()> {
    switch_large_file(64 * 1024 * 1024)
}

#[test]
fn switch_two_hundred_fifty_six_mib_file() -> Result<()> {
    switch_large_file(256 * 1024 * 1024)
}
