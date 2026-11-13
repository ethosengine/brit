//! Object-store qualification keeps fixture construction and every child in the worker's cgroup.
use anyhow::{ensure, Context, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = gix_testtools::git_command(root)
        .args(["-c", &format!("safe.directory={}", root.display())])
        .args(args)
        .output()?;
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
    if args.first() == Some(&"free") {
        // Standalone pack files do not declare their hash format in their header.
        let hash = git(root, &["rev-parse", "--show-object-format"])?;
        command.args(["--object-hash", std::str::from_utf8(&hash)?.trim()]);
    }
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

fn object_store(size: usize, explicit_limit: bool, ref_deltas: bool, reduced: bool) -> Result<()> {
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
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let trust = brit_output(root, &["free", "trust", ".", ".git"])?;
            ensure!(
                trust.status.success()
                    && String::from_utf8_lossy(&trust.stdout)
                        .lines()
                        .filter(|line| line.starts_with("Reduced"))
                        .count()
                        == 2,
                "fixture must have actual reduced trust"
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("ownership qualification runs on Linux");
    }
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
        if reduced && packed {
            git(root, &["config", "--unset", "gitoxide.objects.allocLimit"])?;
        }
        let limits: &[Option<&str>] = if reduced {
            &[None, Some("16777216")]
        } else if explicit_limit {
            &[Some("16777216")]
        } else {
            &[Some("1073741824")]
        };
        for limit in limits {
            if let Some(limit) = limit {
                git(root, &["config", "gitoxide.objects.allocLimit", limit])?;
            }
            eprintln!("size={size}, packed={packed}, allocation limit={limit:?}, reduced={reduced}");
            let head = git(root, &["rev-parse", "HEAD"])?;
            let index = fs::read(root.join(".git/index"))?;
            for algorithm in ["less-time", "less-memory"] {
                for mode in [None, Some("--decode"), Some("--re-encode")] {
                    let mut args = vec!["verify", "--algorithm", algorithm];
                    args.extend(mode);
                    brit(root, &args)?;
                }
            }
            ensure!(
                git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index,
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
    if explicit_limit || reduced {
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
    // Keep explicit destinations inside this fixture's writer boundary. A plain
    // TMPDIR beneath a shared checkout would discover and guard that parent repo.
    let generated = tempfile::tempdir_in(root.join(".git"))?;
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
    let indexed = tempfile::tempdir_in(root.join(".git"))?;
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
    object_store(20 * 1024 * 1024, false, false, false)
}

#[test]
fn object_store_sixty_four_mib() -> Result<()> {
    object_store(64 * 1024 * 1024, false, false, false)
}

#[test]
fn object_store_two_hundred_fifty_six_mib() -> Result<()> {
    object_store(256 * 1024 * 1024, false, false, false)
}

#[test]
fn explicit_limit_streams_loose_and_packed_blobs() -> Result<()> {
    object_store(20 * 1024 * 1024, true, false, false)
}

#[test]
fn object_store_sixty_four_mib_ref_deltas() -> Result<()> {
    object_store(64 * 1024 * 1024, false, true, false)
}

#[test]
fn explicit_limit_streams_sixty_four_mib_ofs_and_ref_deltas() -> Result<()> {
    for refs in [false, true] {
        object_store(64 * 1024 * 1024, true, refs, false)?;
    }
    Ok(())
}

#[test]
fn explicit_limit_streams_two_hundred_fifty_six_mib() -> Result<()> {
    object_store(256 * 1024 * 1024, true, false, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn actual_reduced_trust_streams_packed_objects_and_deltas() -> Result<()> {
    object_store(20 * 1024 * 1024, false, false, true)?;
    for refs in [false, true] {
        object_store(64 * 1024 * 1024, false, refs, true)?;
    }
    Ok(())
}
