//! Large, structurally valid cache metadata; no claim that synthetic IDs name stored objects.
use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{BufWriter, Read, Write},
    path::Path,
    process::Command,
};

const COMMITS: u32 = 400_000;

fn graph(path: &Path, hash: gix::hash::Kind) -> Result<()> {
    let hash_len = hash.len_in_bytes();
    let mut out = BufWriter::new(fs::File::create(path)?);
    out.write_all(b"CGPH")?;
    out.write_all(&[1, if hash_len == 20 { 1 } else { 2 }, 3, 0])?;
    let ids = 56 + 1024;
    let data = ids + u64::from(COMMITS) * hash_len as u64;
    let end = data + u64::from(COMMITS) * (hash_len as u64 + 16);
    for (name, offset) in [(b"OIDF", 56u64), (b"OIDL", ids), (b"CDAT", data), (b"\0\0\0\0", end)] {
        out.write_all(name)?;
        out.write_all(&offset.to_be_bytes())?;
    }
    // Sorted synthetic IDs have a zero first byte, so every cumulative fanout is COMMITS.
    for _ in 0..256 {
        out.write_all(&COMMITS.to_be_bytes())?;
    }
    for n in 1..=COMMITS {
        let mut id = [0u8; 32];
        id[hash_len - 4..hash_len].copy_from_slice(&n.to_be_bytes());
        out.write_all(&id[..hash_len])?;
    }
    let mut tree = [0u8; 32];
    tree[0] = 1;
    for n in 0..COMMITS {
        out.write_all(&tree[..hash_len])?;
        out.write_all(&n.checked_sub(1).unwrap_or(0x7000_0000).to_be_bytes())?;
        out.write_all(&0x7000_0000u32.to_be_bytes())?;
        out.write_all(&((n + 1) << 2).to_be_bytes())?;
        out.write_all(&1_600_000_000u32.to_be_bytes())?;
    }
    out.flush()?;
    drop(out);
    let mut input = fs::File::open(path)?;
    let mut digest = gix::hash::hasher(hash);
    let mut buffer = [0u8; 65536];
    loop {
        let n = input.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        digest.update(&buffer[..n]);
    }
    fs::OpenOptions::new()
        .append(true)
        .open(path)?
        .write_all(digest.try_finalize()?.as_slice())?;
    ensure!(fs::metadata(path)?.len() > 20 * 1024 * 1024);
    Ok(())
}

fn qualify(reduced: bool) -> Result<()> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path();
    ensure!(
        gix_testtools::git_command(root)
            .args(["init", "-q", "--initial-branch=main"])
            .status()?
            .success()
    );
    let repo = gix::open_opts(root, gix::open::Options::isolated())?;
    let path = repo.git_dir().join("objects/info/commit-graph");
    graph(&path, repo.object_hash())?;
    let digest = gix::commitgraph::File::at(path.clone())?.checksum().to_owned();
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{MetadataExt, chown};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(repo.git_dir(), Some(65534), None)?;
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("ownership qualification requires Linux");
        let mut command = Command::new(std::env::var_os("BRIT_BIN").context("BRIT_BIN is required")?);
        gix_testtools::configure_git_environment(&mut command, root);
        let trust = command
            .current_dir(root)
            .args(["free", "trust", ".", ".git"])
            .output()?;
        ensure!(trust.status.success());
        ensure!(
            String::from_utf8(trust.stdout)?
                .lines()
                .filter(|line| line.starts_with("Reduced"))
                .count()
                == 2
        );
    }
    for free in [false, true] {
        for limit in ["1073741824", "16777216", "1"] {
            let mut command = Command::new(std::env::var_os("BRIT_BIN").context("BRIT_BIN is required")?);
            gix_testtools::configure_git_environment(&mut command, root);
            command.current_dir(root).args([
                "-c",
                &format!("gitoxide.objects.allocLimit={limit}"),
                "--format",
                "json",
            ]);
            if free {
                command.arg("free");
            }
            command.args(["commit-graph", "verify", "--statistics"]);
            if free {
                command.arg(&path);
            }
            let output = command.output()?;
            ensure!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
            let statistics: serde_json::Value = serde_json::from_slice(&output.stdout)?;
            ensure!(statistics["num_commits"] == COMMITS && statistics["longest_path_length"] == COMMITS - 1);
            ensure!(statistics["parent_counts"]["0"] == 1 && statistics["parent_counts"]["1"] == COMMITS - 1);
        }
    }
    ensure!(gix::commitgraph::File::at(path)?.verify_checksum()? == digest);
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN"]
fn commit_graph_metadata_over_twenty_mib() -> Result<()> {
    qualify(false)
}

#[test]
#[cfg(target_os = "linux")]
#[ignore = "resource qualification: requires BRIT_BIN and chown authority"]
fn actual_reduced_trust_commit_graph_metadata() -> Result<()> {
    qualify(true)
}
