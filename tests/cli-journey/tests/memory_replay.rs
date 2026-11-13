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

fn replay(size: usize, reduced_trust: bool, large_neighbor: bool) -> Result<()> {
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
    if large_neighbor {
        fs::copy(root.join("large"), root.join("neighbor"))?;
    } else {
        fs::write(root.join("neighbor"), b"base neighbor\n")?;
    }
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
        let write_neighbor = |bytes: &[u8]| -> Result<()> {
            fs::OpenOptions::new()
                .write(true)
                .truncate(!large_neighbor)
                .open(root.join("neighbor"))?
                .write_all(bytes)?;
            Ok(())
        };
        write_neighbor(b"staged neighbor\n")?;
        git(root, &["add", "neighbor"])?;
        let staged_neighbor = git(root, &["rev-parse", ":neighbor"])?;
        write_neighbor(b"unstaged neighbor\n")?;
        let working_neighbor = git(root, &["hash-object", "neighbor"])?;
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
        ensure!(git(root, &["rev-parse", ":neighbor"])? == staged_neighbor);
        ensure!(git(root, &["hash-object", "neighbor"])? == working_neighbor);
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
    replay(20 * 1024 * 1024, false, false)
}
#[test]
fn sixty_four_mib_replay() -> Result<()> {
    replay(64 * 1024 * 1024, false, false)
}

#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn actual_reduced_trust_replay() -> Result<()> {
    replay(20 * 1024 * 1024, true, false)
}

fn conflict_stop(size: usize, resolve: bool) -> Result<()> {
    let repo = TestRepo::new("memory-replay-conflict")?;
    let root = repo.path();
    eprintln!("owned conflict fixture: {}", root.display());
    let mut file = fs::File::create(root.join("large"))?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    fs::write(root.join("neighbor"), b"base\n")?;
    git(root, &["add", "large", "neighbor"])?;
    git(root, &["commit", "-qm", "base"])?;
    git(root, &["switch", "-c", "upstream"])?;
    fs::OpenOptions::new()
        .write(true)
        .open(root.join("large"))?
        .write_all(b"upstream")?;
    git(root, &["commit", "-qam", "upstream"])?;
    git(root, &["switch", "main"])?;
    fs::OpenOptions::new()
        .write(true)
        .open(root.join("large"))?
        .write_all(b"topic---")?;
    git(root, &["commit", "-qam", "topic"])?;
    fs::write(root.join("neighbor"), b"staged\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged\n")?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let original = git(root, &["hash-object", "large"])?;
    let result = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["-c", "gitoxide.objects.allocLimit=16777216", "rebase", "upstream"])
        .run()?;
    ensure!(!result.status.success(), "conflicting replay must stop");
    ensure!(
        !git(root, &["ls-files", "-u"])?.is_empty(),
        "must reach conflict publication: {}",
        result.stderr
    );
    if resolve {
        let mut file = fs::File::create(root.join("large"))?;
        for _ in 0..size / chunk.len() {
            file.write_all(&chunk)?;
        }
        file.seek(SeekFrom::Start(0))?;
        file.write_all(b"resolved")?;
        drop(file);
        let resolved = git(root, &["hash-object", "large"])?;
        let result = BritInvocation::new(brit_bin().expect("built Brit"))
            .current_dir(root)
            .args([
                "-c",
                "gitoxide.objects.allocLimit=16777216",
                "rebase",
                "--stage",
                "--",
                "large",
            ])
            .run()?;
        ensure!(result.status.success(), "stage resolution: {}", result.stderr);
        let result = BritInvocation::new(brit_bin().expect("built Brit"))
            .current_dir(root)
            .args(["-c", "gitoxide.objects.allocLimit=16777216", "rebase", "--continue"])
            .run()?;
        ensure!(result.status.success(), "continue: {}", result.stderr);
        assert_eq!(git(root, &["rev-parse", "HEAD:large"])?, resolved);
        assert_eq!(
            git(root, &["rev-parse", "HEAD^"])?,
            git(root, &["rev-parse", "upstream"])?
        );
        assert_eq!(git(root, &["show", ":neighbor"])?, b"staged\n");
        assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged\n");
        assert!(!root.join(".git/brit-rebase").exists());
        assert!(!root.join(".git/brit-authoring").exists());
        git(root, &["fsck", "--full", "--no-dangling"])?;
        return Ok(());
    }
    let result = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["-c", "gitoxide.objects.allocLimit=16777216", "rebase", "--abort"])
        .run()?;
    ensure!(result.status.success(), "abort: {}", result.stderr);
    assert_eq!(git(root, &["rev-parse", "HEAD"])?, head);
    assert_eq!(fs::read(root.join(".git/index"))?, index);
    assert_eq!(git(root, &["hash-object", "large"])?, original);
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged\n");
    assert!(!root.join(".git/brit-rebase").exists());
    assert!(!root.join(".git/brit-authoring").exists());
    git(root, &["fsck", "--full", "--no-dangling"])?;
    Ok(())
}

#[test]
fn twenty_mib_conflict_abort_preserves_original_files_and_index() -> Result<()> {
    conflict_stop(20 * 1024 * 1024, false)
}

#[test]
fn sixty_four_mib_conflict_abort_preserves_original_files_and_index() -> Result<()> {
    conflict_stop(64 * 1024 * 1024, false)
}

#[test]
fn sixty_four_mib_preserved_staged_and_dirty_neighbor() -> Result<()> {
    replay(64 * 1024 * 1024, false, true)
}

#[test]
fn sixty_four_mib_conflict_resolution_preserves_neighbor() -> Result<()> {
    conflict_stop(64 * 1024 * 1024, true)
}

fn edit_amend(size: usize, expand: bool, abort: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let repo = TestRepo::new("memory-replay-edit")?;
    let root = repo.path();
    eprintln!("owned edit fixture: {}", root.display());
    let mut file = fs::File::create(root.join("large"))?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    drop(file);
    fs::write(root.join("neighbor"), b"base\n")?;
    fs::write(root.join("selected"), b"base selected\n")?;
    git(root, &["add", "large", "neighbor", "selected"])?;
    git(root, &["commit", "-qm", "base"])?;
    git(root, &["switch", "-c", "upstream"])?;
    fs::write(root.join("upstream"), b"upstream\n")?;
    git(root, &["add", "upstream"])?;
    git(root, &["commit", "-qm", "upstream"])?;
    git(root, &["switch", "main"])?;
    if expand {
        fs::write(root.join("selected"), b"topic selected\n")?;
    } else {
        fs::OpenOptions::new()
            .write(true)
            .open(root.join("large"))?
            .write_all(b"topic---")?;
    }
    git(root, &["commit", "-qam", "topic"])?;
    let original = git(root, &["rev-parse", "HEAD"])?;
    let original_blob = git(root, &["hash-object", "large"])?;
    fs::write(root.join("neighbor"), b"staged\n")?;
    git(root, &["add", "neighbor"])?;
    fs::write(root.join("neighbor"), b"unstaged\n")?;
    let editor = root.join(".git/sequence-editor");
    fs::write(&editor, b"#!/bin/sh\nsed -i 's/^pick /edit /' \"$1\"\n")?;
    fs::set_permissions(&editor, fs::Permissions::from_mode(0o700))?;
    let invoke = |args: &[&str]| -> Result<()> {
        let output = BritInvocation::new(brit_bin().expect("built Brit"))
            .current_dir(root)
            .args(["-c", "gitoxide.objects.allocLimit=16777216"])
            .args(args)
            .env("GIT_SEQUENCE_EDITOR", &editor)
            .env("GIT_EDITOR", ":")
            .run()?;
        ensure!(output.status.success(), "{args:?}: {} {}", output.stdout, output.stderr);
        Ok(())
    };
    invoke(&["rebase", "--interactive", "upstream"])?;
    ensure!(root.join(".git/brit-authoring").is_dir(), "must reach the edit stop");
    fs::OpenOptions::new()
        .write(true)
        .open(root.join("large"))?
        .write_all(b"authored")?;
    let authored = git(root, &["hash-object", "large"])?;
    if expand {
        invoke(&["rebase", "--stage", "--expand-scope", "--", "large"])?;
    } else {
        invoke(&["rebase", "--stage", "--", "large"])?;
    }
    invoke(&["rebase", "--amend", "--no-edit"])?;
    if abort {
        invoke(&["rebase", "--abort"])?;
        assert_eq!(git(root, &["rev-parse", "HEAD"])?, original);
        assert_eq!(git(root, &["hash-object", "large"])?, original_blob);
        // Abort's independent retained copy keeps authored objects after pruning.
        git(root, &["reflog", "expire", "--expire=now", "--all"])?;
        git(root, &["gc", "--prune=now"])?;
        let copies = root.join(".git/brit-rebase-retained");
        let mut retained_authored = false;
        for operation in fs::read_dir(copies)? {
            for copy in fs::read_dir(operation?.path())? {
                let copy = copy?.path();
                if !copy.join("COPY_COMPLETE").is_file() {
                    continue;
                }
                let envelope: serde_json::Value = serde_json::from_slice(&fs::read(copy.join("manifest.json"))?)?;
                let payload: Vec<u8> = serde_json::from_value(envelope["payload"].clone())?;
                let manifest: serde_json::Value = serde_json::from_slice(&payload)?;
                for (ordinal, entry) in manifest["entries"].as_array().expect("entries").iter().enumerate() {
                    if entry["kind"]["Object"]["id"].as_str() == Some(std::str::from_utf8(&authored)?.trim()) {
                        let payload = copy.join(format!("{ordinal:08}"));
                        assert_eq!(
                            git(root, &["hash-object", payload.to_str().expect("fixture path")])?,
                            authored
                        );
                        retained_authored = true;
                    }
                }
            }
        }
        ensure!(
            retained_authored,
            "expanded authored object survives pruning in an independent copy"
        );
    } else {
        invoke(&["rebase", "--continue"])?;
        assert_eq!(git(root, &["rev-parse", "HEAD:large"])?, authored);
        assert_eq!(
            git(root, &["rev-parse", "HEAD^"])?,
            git(root, &["rev-parse", "upstream"])?
        );
    }
    assert_eq!(git(root, &["show", ":neighbor"])?, b"staged\n");
    assert_eq!(fs::read(root.join("neighbor"))?, b"unstaged\n");
    assert!(!root.join(".git/brit-rebase").exists());
    assert!(!root.join(".git/brit-authoring").exists());
    git(root, &["fsck", "--full", "--no-dangling"])?;
    Ok(())
}

#[test]
fn sixty_four_mib_edit_amend_preserves_authored_object_custody() -> Result<()> {
    edit_amend(64 * 1024 * 1024, false, false)
}

#[test]
fn twenty_mib_edit_scope_expansion_preserves_original_and_authored_bytes() -> Result<()> {
    edit_amend(20 * 1024 * 1024, true, false)?;
    edit_amend(20 * 1024 * 1024, true, true)
}
