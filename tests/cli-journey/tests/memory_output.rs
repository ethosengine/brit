//! Resource and filesystem failures must reach the public command's exit status.
#![cfg(target_os = "linux")]

use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

fn git(root: &Path, args: &[&str]) -> Result<()> {
    let output = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

#[test]
fn remote_and_pack_output_failures_are_not_success() -> Result<()> {
    let source = TestRepo::new("memory-output-source")?;
    source.commit_file("selected", "unchanged\n")?;
    git(source.path(), &["-c", "pack.threads=1", "repack", "-ad"])?;
    git(source.path(), &["multi-pack-index", "write"])?;
    let pack_dir = source.path().join(".git/objects/pack");
    let mut paths = fs::read_dir(&pack_dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.sort();
    let pack = paths
        .iter()
        .find(|p| p.extension().is_some_and(|ext| ext == "pack"))
        .expect("one pack");
    let index = paths
        .iter()
        .find(|p| p.extension().is_some_and(|ext| ext == "idx"))
        .expect("one index");
    let multi = pack_dir.join("multi-pack-index");
    let local = TestRepo::new("memory-output-client")?;
    let root = local.path();
    let url = source.path().to_str().expect("fixture path");
    git(root, &["remote", "add", "origin", url])?;
    let head = local.head_id()?;
    let object_hash = if head.len() == 64 { "sha256" } else { "sha1" };
    let index_before = fs::read(root.join(".git/index"))?;
    let config_before = fs::read(root.join(".git/config"))?;
    let commands = [
        vec!["remote", "refs"],
        vec!["remote", "ref-map"],
        vec!["free", "remote", "refs", url],
        vec!["free", "pack", "receive", url],
        vec![
            "free",
            "pack",
            "verify",
            "--statistics",
            index.to_str().expect("index path"),
        ],
        vec![
            "free",
            "pack",
            "verify",
            "--statistics",
            multi.to_str().expect("multi-index path"),
        ],
    ];
    let mut hidden_failures = Vec::new();
    for args in commands {
        for full in [false, true] {
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            command
                .current_dir(root)
                .args(["--object-hash", object_hash])
                .args(&args)
                .stdin(Stdio::null());
            command.stdout(if full {
                Stdio::from(fs::OpenOptions::new().write(true).open("/dev/full")?)
            } else {
                Stdio::piped()
            });
            let result = command.output()?;
            if full && result.status.success() {
                hidden_failures.push(args.join(" "));
            } else if full {
                let diagnostic = String::from_utf8_lossy(&result.stderr);
                ensure!(
                    diagnostic.contains("space") || diagnostic.contains("StorageFull"),
                    "{args:?}: {diagnostic}"
                );
            } else {
                ensure!(
                    result.status.success(),
                    "valid command {args:?}: {}",
                    String::from_utf8_lossy(&result.stderr)
                );
                ensure!(!result.stdout.is_empty(), "{args:?} must exercise output");
            }
        }
    }
    // A bare pack checksum verification has no statistics to print. A full output
    // device is not an error when the command never writes to it.
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    let result = command
        .current_dir(root)
        .args(["--object-hash", object_hash])
        .args(["free", "pack", "verify", "--statistics"])
        .arg(pack)
        .output()?;
    ensure!(result.status.success() && result.stdout.is_empty());
    ensure!(local.head_id()? == head && fs::read(root.join(".git/index"))? == index_before);
    ensure!(fs::read(root.join(".git/config"))? == config_before);
    ensure!(
        hidden_failures.is_empty(),
        "output failures reported success: {hidden_failures:#?}"
    );
    Ok(())
}
