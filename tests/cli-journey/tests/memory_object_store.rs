//! Object-store qualification keeps fixture construction and every child in the worker's cgroup.
use anyhow::{ensure, Context, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
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

fn brit(root: &Path, args: &[&str]) -> Result<()> {
    let output = brit_output(root, args)?;
    ensure!(
        output.status.success(),
        "Brit {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn brit_output(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    eprintln!("qualifying {args:?}");
    let mut command = std::process::Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    Ok(command.current_dir(root).args(args).output()?)
}

fn pack_in(directory: &Path) -> Result<std::path::PathBuf> {
    fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .find(|path| path.extension().is_some_and(|extension| extension == "pack"))
        .context("generated pack")
}

fn object_store(size: usize, explicit_limit: bool, ref_deltas: bool) -> Result<()> {
    let repo = TestRepo::new("memory-object-store")?;
    let root = repo.path();
    git(root, &["config", "user.name", "Fixture"])?;
    git(root, &["config", "user.email", "fixture@example.com"])?;
    let mut file = fs::File::create(root.join("large"))?;
    let chunk = [b'x'; 65536];
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    git(root, &["add", "large"])?;
    git(root, &["commit", "-qm", "large object"])?;
    let mut expected_size = size;
    if size == 64 * 1024 * 1024 {
        let change = b"\nappend\n";
        fs::OpenOptions::new()
            .append(true)
            .open(root.join("large"))?
            .write_all(change)?;
        git(root, &["commit", "-qam", "append history"])?;
        expected_size += change.len();
    }
    let expected = git(root, &["rev-parse", "HEAD:large"])?;
    for packed in [false, true] {
        if packed {
            eprintln!("preparing packed fixture");
            git(
                root,
                &[
                    "config",
                    "repack.useDeltaBaseOffset",
                    if ref_deltas { "false" } else { "true" },
                ],
            )?;
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
            if size == 64 * 1024 * 1024 {
                let pack = pack_in(&root.join(".git/objects/pack"))?;
                let entries = String::from_utf8(git(
                    root,
                    &[
                        "verify-pack",
                        "-v",
                        pack.with_extension("idx").to_str().context("index path")?,
                    ],
                )?)?;
                let offset = entries
                    .lines()
                    .find_map(|line| {
                        let fields: Vec<_> = line.split_whitespace().collect();
                        (fields.len() == 7).then(|| fields[4].parse::<u64>().ok()).flatten()
                    })
                    .context("fixture must contain a delta")?;
                let mut input = fs::File::open(pack)?;
                input.seek(SeekFrom::Start(offset))?;
                let mut byte = [0];
                input.read_exact(&mut byte)?;
                ensure!(
                    (byte[0] >> 4) & 7 == if ref_deltas { 7 } else { 6 },
                    "fixture uses the requested delta representation"
                );
            }
        }
        for limit in if explicit_limit {
            &["16777216"][..]
        } else {
            &["1073741824"][..]
        } {
            git(root, &["config", "gitoxide.objects.allocLimit", limit])?;
            eprintln!("size={size}, packed={packed}, allocation limit={limit}");
            let head = repo.head_id()?;
            let index = fs::read(root.join(".git/index"))?;
            for algorithm in ["less-time", "less-memory"] {
                for mode in [None, Some("--decode"), Some("--re-encode")] {
                    let mut args = vec!["verify", "--algorithm", algorithm];
                    args.extend(mode);
                    if packed && explicit_limit {
                        // This is a refusal/recovery contract, not successful packed verification.
                        // Bounded packed verification remains an open readiness station.
                        let output = brit_output(root, &args)?;
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        ensure!(
                            !output.status.success(),
                            "the configured packed-buffer limit must remain effective"
                        );
                        ensure!(
                            stderr.contains(&format!("requested {size} bytes"))
                                && stderr.contains("configured allocation limit is 16777216 bytes")
                                && stderr.contains("pack index, path="),
                            "accurate allocation diagnostic: {stderr}"
                        );
                        ensure!(output.stdout.is_empty(), "refusal must not report success output");
                    } else {
                        brit(root, &args)?;
                    }
                }
            }
            ensure!(
                repo.head_id()? == head && fs::read(root.join(".git/index"))? == index,
                "verification preserves repository state"
            );
            for args in [
                &["fsck"][..],
                &["odb", "entries"],
                &["odb", "info"],
                &["odb", "stats", "--extra-header-lookup"],
            ] {
                brit(root, args)?;
            }
        }
    }
    if explicit_limit {
        return Ok(());
    }
    let original = pack_in(&root.join(".git/objects/pack"))?;
    let original_arg = original.to_str().context("pack path")?;
    for algorithm in ["less-time", "less-memory"] {
        brit(
            root,
            &["free", "pack", "verify", "--algorithm", algorithm, original_arg],
        )?;
    }
    let generated = tempfile::tempdir()?;
    brit(
        root,
        &[
            "free",
            "pack",
            "create",
            "-r",
            ".",
            "-o",
            generated.path().to_str().context("output path")?,
            "HEAD",
        ],
    )?;
    let pack = pack_in(generated.path())?;
    let indexed = tempfile::tempdir()?;
    brit(
        root,
        &[
            "free",
            "pack",
            "index",
            "create",
            "--pack-path",
            pack.to_str().context("pack path")?,
            indexed.path().to_str().context("index path")?,
        ],
    )?;
    let indexed_pack = pack_in(indexed.path())?;
    git(
        root,
        &[
            "verify-pack",
            indexed_pack.with_extension("idx").to_str().context("index path")?,
        ],
    )?;
    let exploded = TestRepo::new("memory-exploded")?;
    brit(
        root,
        &[
            "free",
            "pack",
            "explode",
            "--verify",
            indexed_pack.to_str().context("pack path")?,
            exploded.path().join(".git/objects").to_str().context("objects path")?,
        ],
    )?;
    let id = std::str::from_utf8(&expected)?.trim();
    ensure!(
        String::from_utf8(git(exploded.path(), &["cat-file", "-s", id])?)?.trim() == expected_size.to_string(),
        "the exploded object retains its Git identity and size"
    );
    git(exploded.path(), &["fsck", "--full"])?;
    Ok(())
}

#[test]
fn object_store_twenty_mib() -> Result<()> {
    object_store(20 * 1024 * 1024, false, false)
}

#[test]
fn object_store_sixty_four_mib() -> Result<()> {
    object_store(64 * 1024 * 1024, false, false)
}

#[test]
fn object_store_two_hundred_fifty_six_mib() -> Result<()> {
    object_store(256 * 1024 * 1024, false, false)
}

#[test]
fn explicit_limit_streams_loose_blobs_and_diagnoses_packed_refusal() -> Result<()> {
    object_store(20 * 1024 * 1024, true, false)
}

#[test]
fn object_store_sixty_four_mib_ref_deltas() -> Result<()> {
    object_store(64 * 1024 * 1024, false, true)
}
