//! Snapshot resource fixtures verify identities without retaining a second blob in the harness.
use anyhow::{ensure, Context, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};
use std::{fs, io::Write, path::Path};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = gix_testtools::git_command(root).args(args).output()?;
    ensure!(
        out.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn snapshot(root: &Path, args: &[&str]) -> Result<serde_json::Value> {
    let out = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .arg("snapshot")
        .args(args.iter().copied())
        .run()?;
    ensure!(out.status.success(), "snapshot {args:?}: {}", out.stderr);
    Ok(serde_json::from_str(&out.stdout)?)
}

fn source_fixture(size: usize) -> Result<TestRepo> {
    let source = TestRepo::new("memory-snapshot-source")?;
    let file = source.path().join("large");
    let mut output = fs::File::create(&file)?;
    let chunk = [b'x'; 65536];
    for _ in 0..size / chunk.len() {
        output.write_all(&chunk)?;
    }
    drop(output);
    git(source.path(), &["add", "large"])?;
    git(
        source.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-qm",
            "large snapshot",
        ],
    )?;
    git(source.path(), &["repack", "-ad"])?;
    Ok(source)
}

fn roundtrip(size: usize, limit: Option<&str>) -> Result<()> {
    roundtrip_source(source_fixture(size)?, limit)
}

fn roundtrip_source(source: TestRepo, limit: Option<&str>) -> Result<()> {
    if let Some(limit) = limit {
        git(source.path(), &["config", "gitoxide.objects.allocLimit", limit])?;
    }
    let expected = git(source.path(), &["rev-parse", "HEAD:large"])?;
    let artifacts = tempfile::tempdir()?;
    let store = artifacts.path().join("store");
    let store_arg = store.to_str().context("fixture path")?;
    let sealed = snapshot(source.path(), &["seal", "--store", store_arg])?;
    let root = sealed["root"].as_str().context("snapshot root")?;
    let tree = sealed["gitTree"].as_str().context("Git tree")?;
    drop(source);
    snapshot(artifacts.path(), &["verify", "--root", root, "--store", store_arg])?;
    let destination = TestRepo::new("memory-snapshot-destination")?;
    let head = destination.head_id()?;
    let exported = snapshot(
        destination.path(),
        &[
            "export-git",
            "--root",
            root,
            "--store",
            store_arg,
            "--repo",
            ".",
            "--expected-tree",
            tree,
        ],
    )?;
    ensure!(exported["gitTree"].as_str() == Some(tree));
    ensure!(destination.head_id()? == head, "export preserves refs");
    ensure!(git(destination.path(), &["rev-parse", &format!("{tree}:large")])? == expected);
    #[cfg(target_os = "linux")]
    {
        let restored = artifacts.path().join("restored");
        snapshot(
            artifacts.path(),
            &[
                "restore",
                "--root",
                root,
                "--store",
                store_arg,
                "--destination",
                restored.to_str().context("fixture path")?,
            ],
        )?;
        ensure!(
            git(&restored, &["hash-object", "large"])? == expected,
            "restored bytes match Git identity"
        );
        git(&restored, &["init"])?;
        git(&restored, &["add", "."])?;
        ensure!(
            String::from_utf8(git(&restored, &["write-tree"])?)?.trim() == tree,
            "the complete restored tree matches Git, including modes"
        );
    }
    Ok(())
}

#[test]
fn snapshot_sixty_four_mib_roundtrip() -> Result<()> {
    roundtrip(64 * 1024 * 1024, None)
}

#[test]
fn snapshot_twenty_mib_explicit_allocation_limit() -> Result<()> {
    roundtrip(20 * 1024 * 1024, Some("16777216"))
}

#[test]
fn snapshot_many_files_exceed_the_old_threshold_in_aggregate() -> Result<()> {
    let source = source_fixture(1024 * 1024)?;
    for n in 0..32u8 {
        let name = format!("small-{n:02}");
        let mut file = fs::File::create(source.path().join(&name))?;
        let chunk = [n; 65536];
        for _ in 0..16 {
            file.write_all(&chunk)?;
        }
        drop(file);
        git(source.path(), &["add", &name])?;
    }
    git(
        source.path(),
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-qm",
            "aggregate snapshot",
        ],
    )?;
    roundtrip_source(source, Some("16777216"))
}

#[test]
fn snapshot_preserves_the_existing_native_content_ceiling() -> Result<()> {
    // DirectoryStorage's published contract already restricts each object to 64 MiB.
    // This is an existing content capability boundary, not the process memory ceiling.
    let source = source_fixture(256 * 1024 * 1024)?;
    let head = source.head_id()?;
    let index = fs::read(source.path().join(".git/index"))?;
    let artifacts = tempfile::tempdir()?;
    let store = artifacts.path().join("store");
    let out = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(source.path())
        .args(["snapshot", "seal", "--store", store.to_str().context("fixture path")?])
        .run()?;
    ensure!(
        !out.status.success() && out.stderr.contains("content object exceeds byte limit"),
        "existing native content limit: {}",
        out.stderr
    );
    ensure!(out.stdout.is_empty(), "refusal does not advertise a snapshot root");
    ensure!(source.head_id()? == head && fs::read(source.path().join(".git/index"))? == index);
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn snapshot_actual_reduced_trust_streams_twenty_mib() -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let source = source_fixture(20 * 1024 * 1024)?;
    for path in [source.path().to_owned(), source.path().join(".git")] {
        let raw = CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: both paths are owned by this disposable fixture; CString is NUL-terminated.
        ensure!(
            unsafe { libc::chown(raw.as_ptr(), 65534, 65534) } == 0,
            "privileged qualification requires chown"
        );
    }
    let artifacts = tempfile::tempdir()?;
    let store = artifacts.path().join("store");
    snapshot(
        source.path(),
        &["seal", "--store", store.to_str().context("fixture path")?],
    )?;
    Ok(())
}
