//! Resource fixtures compare public output, not only private storage helpers.
use anyhow::{ensure, Result};
use cli_journey::support::{
    runner::{brit_bin, BritInvocation},
    test_repo::TestRepo,
};
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

fn large_file(path: &Path, size: usize) -> Result<()> {
    let mut file = fs::File::create(path)?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    file.write_all(&chunk[..size % chunk.len()])?;
    Ok(())
}

fn brit(root: &Path, args: &[&str], limit: &str) -> Result<String> {
    let result = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["-c", limit])
        .args(args.iter().copied())
        .run()?;
    ensure!(result.status.success(), "Brit {args:?}: {}", result.stderr);
    Ok(result.stdout)
}

#[test]
fn large_diff_and_stat_match_git_across_allocation_boundary() -> Result<()> {
    for size in [
        16 * 1024 * 1024 - 1,
        16 * 1024 * 1024,
        16 * 1024 * 1024 + 1,
        20 * 1024 * 1024,
        64 * 1024 * 1024,
        256 * 1024 * 1024,
    ] {
        let repo = TestRepo::new("memory-diff")?;
        let root = repo.path();
        let path = root.join("governance.txt");
        large_file(&path, size)?;
        git(root, &["add", "governance.txt"])?;
        git(
            root,
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
        let mut file = fs::OpenOptions::new().write(true).open(&path)?;
        file.seek(SeekFrom::Start(65536))?;
        file.write_all(b"changed")?;
        drop(file);
        for packed in [false, true] {
            if packed {
                git(root, &["repack", "-ad"])?;
            }
            let cat = brit(
                root,
                &["cat", "HEAD:governance.txt"],
                "gitoxide.objects.allocLimit=16777216",
            )?;
            ensure!(
                cat.len() == size && cat.as_bytes()[65536] == b'x',
                "cat must stream the committed bytes"
            );
            drop(cat);
            for stat in [false, true] {
                let args = if stat { vec!["diff", "--stat"] } else { vec!["diff"] };
                let expected = git(root, &args)?;
                for limit in [
                    "gitoxide.objects.allocLimit=16777216",
                    "gitoxide.objects.allocLimit=1073741824",
                ] {
                    ensure!(
                        brit(root, &args, limit)?.as_bytes() == expected,
                        "output differs: size={size} packed={packed} stat={stat}"
                    );
                }
            }
        }
        brit(
            root,
            &["add", "--", "governance.txt"],
            "gitoxide.objects.allocLimit=16777216",
        )?;
        for args in [&["diff", "--cached"][..], &["diff", "--cached", "--stat"][..]] {
            ensure!(
                brit(root, args, "gitoxide.objects.allocLimit=16777216")?.as_bytes() == git(root, args)?,
                "staged output differs"
            );
        }
    }
    Ok(())
}

#[test]
fn late_refusal_does_not_publish_an_earlier_patch() -> Result<()> {
    let repo = TestRepo::new("memory-atomic")?;
    let root = repo.path();
    fs::write(root.join("a"), b"before\n")?;
    fs::write(root.join("z"), b"before\n")?;
    fs::write(root.join(".gitattributes"), b"z diff=custom\n")?;
    git(root, &["add", "."])?;
    fs::write(root.join("a"), b"after\n")?;
    fs::write(root.join("z"), b"after\n")?;
    let result = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .arg("diff")
        .run()?;
    ensure!(!result.status.success(), "unsupported attributed patch must refuse");
    ensure!(
        result.stdout.is_empty(),
        "a later refusal must not leak the first patch"
    );
    Ok(())
}

#[test]
fn restore_large_selection_streams_and_validates_before_publication() -> Result<()> {
    let repo = TestRepo::new("memory-restore")?;
    let root = repo.path();
    for name in ["a", "b"] {
        large_file(&root.join(name), 64 * 1024 * 1024)?;
    }
    git(root, &["add", "a", "b"])?;
    let expected = git(root, &["ls-files", "--stage"])?;
    for name in ["a", "b"] {
        fs::write(root.join(name), b"dirty\n")?;
    }
    let rejected = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["restore", "--worktree", "--", "a", "absent"])
        .run()?;
    ensure!(!rejected.status.success());
    ensure!(
        fs::read(root.join("a"))? == b"dirty\n",
        "invalid selection must publish nothing"
    );
    brit(
        root,
        &["restore", "--worktree", "--", "a", "b"],
        "gitoxide.objects.allocLimit=16777216",
    )?;
    ensure!(git(root, &["diff", "--exit-code"])?.is_empty());
    ensure!(
        git(root, &["ls-files", "--stage"])? == expected,
        "restore must preserve the index"
    );
    Ok(())
}

#[test]
fn million_line_diff_keeps_existing_tie_breaking() -> Result<()> {
    // Numeric-leading lines avoid the documented omission of function-context headers.
    for unique in [false, true] {
        let repo = TestRepo::new("memory-lines")?;
        let root = repo.path();
        let path = root.join("lines");
        let mut file = std::io::BufWriter::new(fs::File::create(&path)?);
        for i in 0..1_000_000 {
            if unique {
                writeln!(file, "{i:07} line")?;
            } else {
                writeln!(file, "0 repeat")?;
            }
        }
        file.flush()?;
        drop(file);
        git(root, &["add", "lines"])?;
        let mut file = fs::OpenOptions::new().append(true).open(&path)?;
        writeln!(file, "appended")?;
        drop(file);
        for args in [&["diff"][..], &["diff", "--stat"][..]] {
            let expected = git(root, args)?;
            for limit in [
                "gitoxide.objects.allocLimit=1048576",
                "gitoxide.objects.allocLimit=1073741824",
            ] {
                let actual = brit(root, args, limit)?;
                ensure!(
                    actual.as_bytes() == expected,
                    "million-line output differs, unique={unique}, args={args:?}: expected {:?}, actual {actual:?}",
                    String::from_utf8_lossy(&expected)
                );
            }
        }
    }
    Ok(())
}

#[test]
fn three_sided_64_mib_file_merge_matches_git() -> Result<()> {
    let repo = TestRepo::new("memory-merge")?;
    let root = repo.path();
    let size = 64 * 1024 * 1024;
    large_file(&root.join("base"), size)?;
    for (name, offset, text) in [("ours", 0, &b"ours"[..]), ("theirs", size - 65536, &b"theirs"[..])] {
        fs::copy(root.join("base"), root.join(name))?;
        let mut file = fs::OpenOptions::new().write(true).open(root.join(name))?;
        file.seek(SeekFrom::Start(offset as u64))?;
        file.write_all(text)?;
    }
    let expected = git(root, &["merge-file", "-p", "ours", "base", "theirs"])?;
    ensure!(
        brit(
            root,
            &["merge", "file", "ours", "base", "theirs"],
            "gitoxide.objects.allocLimit=16777216"
        )?
        .as_bytes()
            == expected,
        "three-sided merge must preserve the existing algorithm's result"
    );
    Ok(())
}

#[test]
fn large_clean_conversion_preserves_index_eol_and_git_object_identity() -> Result<()> {
    let repo = TestRepo::new("memory-clean")?;
    let root = repo.path();
    fs::write(
        root.join(".gitattributes"),
        b"encoded text=auto eol=crlf ident working-tree-encoding=ISO-8859-1\n",
    )?;
    let mut file = fs::File::create(root.join("encoded"))?;
    let mut chunk = [b'x'; 65536];
    chunk[..17].copy_from_slice(b"$Id: expanded$\xe9\r\n");
    chunk[65534..].copy_from_slice(b"\r\n");
    for _ in 0..1024 {
        file.write_all(&chunk)?;
    }
    drop(file);
    let expected = String::from_utf8(git(root, &["hash-object", "--path=encoded", "encoded"])?)?;
    for _ in 0..2 {
        brit(
            root,
            &["add", "--", ".gitattributes", "encoded"],
            "gitoxide.objects.allocLimit=16777216",
        )?;
        let staged = String::from_utf8(git(root, &["ls-files", "--stage", "encoded"])?)?;
        ensure!(staged.contains(expected.trim()), "clean conversion differs from Git");
    }
    git(
        root,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.com",
            "commit",
            "-qm",
            "converted",
        ],
    )?;
    let mut file = fs::OpenOptions::new().write(true).open(root.join("encoded"))?;
    file.write_all(b"$")?;
    drop(file);
    brit(root, &["status", "--no-write"], "gitoxide.objects.allocLimit=16777216")?;
    Ok(())
}

#[test]
fn metadata_crosses_object_boundary_independently_of_blob_size() -> Result<()> {
    let repo = TestRepo::new("memory-index")?;
    let root = repo.path();
    large_index(root)?;
    let rendered = brit(
        root,
        &["index", "entries", "--no-attributes"],
        "gitoxide.objects.allocLimit=1073741824",
    )?;
    ensure!(
        rendered.lines().count() == 200_000,
        "all index metadata must be decoded"
    );
    drop(rendered);
    let refused = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args([
            "-c",
            "gitoxide.objects.allocLimit=16777216",
            "index",
            "entries",
            "--no-attributes",
        ])
        .run()?;
    ensure!(
        !refused.status.success(),
        "explicit metadata allocation policy must remain effective"
    );
    ensure!(
        refused.stderr.contains("Index decode requested") && refused.stderr.contains("16777216 bytes"),
        "metadata limit must identify requested bytes and configured limit: {}",
        refused.stderr
    );
    ensure!(refused.stdout.is_empty(), "decode refusal precedes output");
    Ok(())
}

/// Run explicitly on the delegated qualification worker; ordinary CI users cannot chown.
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn actual_reduced_trust_keeps_ownership_policy_and_streams_large_blobs() -> Result<()> {
    use std::os::unix::fs::{chown, MetadataExt};
    let repo = TestRepo::new("memory-reduced-trust")?;
    let root = repo.path();
    ensure!(
        fs::metadata(root)?.uid() == 0,
        "a privileged qualification worker is required"
    );
    large_file(&root.join("large"), 20 * 1024 * 1024)?;
    git(root, &["add", "large"])?;
    chown(root, Some(65534), None)?;
    chown(root.join(".git"), Some(65534), None)?;
    let executable = brit_bin().expect("built Brit");
    let trust = BritInvocation::new(&executable)
        .current_dir(root)
        .args(["free", "trust", ".", ".git"])
        .run()?;
    ensure!(
        trust.status.success() && trust.stdout.lines().filter(|line| line.starts_with("Reduced")).count() == 2,
        "ownership-derived trust must actually be Reduced: {} {}",
        trust.stdout,
        trust.stderr
    );
    let cat = BritInvocation::new(&executable)
        .current_dir(root)
        .args(["cat", ":large"])
        .run()?;
    ensure!(
        cat.status.success() && cat.stdout.len() == 20 * 1024 * 1024,
        "reduced-trust cat: {}",
        cat.stderr
    );
    let diff = BritInvocation::new(&executable).current_dir(root).arg("diff").run()?;
    ensure!(
        diff.status.success() && diff.stdout.is_empty(),
        "reduced-trust unchanged diff: {}",
        diff.stderr
    );
    let metadata_repo = TestRepo::new("memory-reduced-metadata")?;
    large_index(metadata_repo.path())?;
    chown(metadata_repo.path(), Some(65534), None)?;
    chown(metadata_repo.path().join(".git"), Some(65534), None)?;
    let control = BritInvocation::new(&executable)
        .current_dir(metadata_repo.path())
        .args(["index", "entries", "--no-attributes"])
        .run()?;
    ensure!(
        !control.status.success() && control.stderr.contains("16777216"),
        "actual reduced-trust metadata allocation limit must remain enforced: {}",
        control.stderr
    );
    Ok(())
}

fn large_index(root: &Path) -> Result<()> {
    fs::write(root.join("empty"), b"")?;
    let object = String::from_utf8(git(root, &["hash-object", "-w", "empty"])?)?;
    let input = root.join(".git/index-input");
    let mut file = std::io::BufWriter::new(fs::File::create(&input)?);
    let suffix = "x".repeat(100);
    for i in 0..200_000 {
        writeln!(file, "100644 {}\tmetadata/{i:06}-{suffix}", object.trim())?;
    }
    file.flush()?;
    drop(file);
    let output = gix_testtools::git_command(root)
        .args(["update-index", "--index-info"])
        .stdin(fs::File::open(&input)?)
        .output()?;
    ensure!(
        output.status.success(),
        "index fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_file(input)?;
    ensure!(fs::metadata(root.join(".git/index"))?.len() > 16 * 1024 * 1024);
    Ok(())
}

#[test]
fn archive_streams_large_converted_entries() -> Result<()> {
    for size in [64 * 1024 * 1024, 256 * 1024 * 1024] {
        let repo = TestRepo::new("memory-archive")?;
        let root = repo.path();
        large_file(&root.join("large"), size)?;
        fs::write(root.join(".gitattributes"), b"large text eol=crlf\n")?;
        git(root, &["add", "large", ".gitattributes"])?;
        brit(
            root,
            &["commit", "-m", "large archive"],
            "gitoxide.objects.allocLimit=16777216",
        )?;
        git(root, &["repack", "-ad"])?;
        let archive = root.join("output.tar");
        let outcome = BritInvocation::new(brit_bin().expect("built Brit"))
            .current_dir(root)
            .args(["-c", "gitoxide.objects.allocLimit=16777216", "archive"])
            .arg(archive.as_os_str())
            .run()?;
        ensure!(outcome.status.success(), "archive: {}", outcome.stderr);
        // Inspect the tar without retaining its contents or relying on a platform tar binary.
        let mut file = fs::File::open(archive)?;
        let mut header = [0; 512];
        let mut found = false;
        loop {
            file.read_exact(&mut header)?;
            if header == [0; 512] {
                break;
            }
            let name = header[..100].split(|byte| *byte == 0).next().expect("name field");
            let length = std::str::from_utf8(&header[124..136])?.trim_matches(['\0', ' ']);
            let length = u64::from_str_radix(length, 8)?;
            if name == b"large" {
                ensure!(length == size as u64 + size as u64 / 65536, "CRLF conversion length");
                let mut expected = vec![b'x'; 65537];
                expected[65535] = b'\r';
                expected[65536] = b'\n';
                let mut actual = vec![0; 65537];
                for _ in 0..size / 65536 {
                    file.read_exact(&mut actual)?;
                    ensure!(actual == expected, "archive conversion bytes");
                }
                found = true;
            } else {
                file.seek(SeekFrom::Current(length as i64))?;
            }
            file.seek(SeekFrom::Current(((512 - length % 512) % 512) as i64))?;
        }
        ensure!(found, "archive contains its large entry");
    }
    Ok(())
}

#[test]
fn large_local_clone_fetch_and_push_preserve_objects() -> Result<()> {
    for size in [64 * 1024 * 1024, 256 * 1024 * 1024] {
        let source = TestRepo::new("memory-transfer")?;
        let root = source.path();
        large_file(&root.join("large"), size)?;
        git(root, &["add", "large"])?;
        git(root, &["commit", "-qm", "large"])?;
        let destinations = tempfile::tempdir()?;
        let cloned = destinations.path().join("cloned");
        let outcome = BritInvocation::new(brit_bin().expect("built Brit"))
            .current_dir(root)
            .arg("clone")
            .arg(root.as_os_str())
            .arg(cloned.as_os_str())
            .run()?;
        ensure!(outcome.status.success(), "clone: {}", outcome.stderr);
        ensure!(git(&cloned, &["hash-object", "large"])? == git(root, &["rev-parse", "HEAD:large"])?);
        let mut file = fs::OpenOptions::new().append(true).open(root.join("large"))?;
        writeln!(file, "appended")?;
        drop(file);
        git(root, &["add", "large"])?;
        git(root, &["commit", "-qm", "append"])?;
        brit(&cloned, &["fetch"], "gitoxide.objects.allocLimit=1073741824")?;
        let expected = git(root, &["rev-parse", "HEAD"])?;
        ensure!(git(&cloned, &["rev-parse", "refs/remotes/origin/main"])? == expected);
        let bare = destinations.path().join("pushed.git");
        let initialized = gix_testtools::git_command(root)
            .args(["init", "--bare", "-q"])
            .arg(&bare)
            .output()?;
        ensure!(initialized.status.success(), "initialize bare destination");
        let pushed = BritInvocation::new(brit_bin().expect("built Brit"))
            .current_dir(root)
            .arg("push")
            .arg(bare.as_os_str())
            .arg("refs/heads/main:refs/heads/main")
            .run()?;
        ensure!(pushed.status.success(), "push: {}", pushed.stderr);
        ensure!(git(&bare, &["rev-parse", "refs/heads/main"])? == expected);
        git(&bare, &["fsck", "--full"])?;
    }
    Ok(())
}

#[test]
fn append_only_blame_reclaims_completed_comparisons() -> Result<()> {
    let repo = TestRepo::new("memory-blame")?;
    let root = repo.path();
    large_file(&root.join("history"), 20 * 1024 * 1024)?;
    git(root, &["add", "history"])?;
    git(root, &["commit", "-qm", "base"])?;
    let mut expected = Vec::new();
    for i in 0..64 {
        let mut file = fs::OpenOptions::new().append(true).open(root.join("history"))?;
        writeln!(file, "{i:06} appended history")?;
        drop(file);
        git(root, &["add", "history"])?;
        git(root, &["commit", "-qm", "append"])?;
        expected.push(String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?);
    }
    let observed = brit(
        root,
        &["blame", "-L", "321,384", "history"],
        "gitoxide.objects.allocLimit=1073741824",
    )?;
    let lines: Vec<_> = observed.lines().collect();
    ensure!(lines.len() == expected.len(), "all appended lines have blame");
    for (line, id) in lines.into_iter().zip(expected) {
        ensure!(line.starts_with(&id[..8]), "append attribution: {line} expected {id}");
    }
    Ok(())
}

#[test]
fn blame_explicit_policy_matches_buffered_history() -> Result<()> {
    let repo = TestRepo::new("memory-blame-policy")?;
    let root = repo.path();
    large_file(&root.join("history"), 20 * 1024 * 1024)?;
    git(root, &["add", "history"])?;
    git(root, &["commit", "-qm", "base"])?;
    for i in 0..2 {
        let mut file = fs::OpenOptions::new().append(true).open(root.join("history"))?;
        writeln!(file, "{i:06} appended history")?;
        drop(file);
        git(root, &["commit", "-qam", "append"])?;
    }
    for packed in [false, true] {
        if packed {
            git(root, &["repack", "-ad"])?;
        }
        let args = ["blame", "-L", "321,322", "history"];
        let buffered = brit(root, &args, "gitoxide.objects.allocLimit=1073741824")?;
        let bounded = brit(root, &args, "gitoxide.objects.allocLimit=16777216")?;
        ensure!(bounded == buffered, "allocation policy must preserve attribution");
    }
    git(root, &["mv", "history", "renamed"])?;
    let mut file = fs::OpenOptions::new().append(true).open(root.join("renamed"))?;
    writeln!(file, "000002 changed during rename")?;
    drop(file);
    git(root, &["commit", "-qam", "rename with an edit"])?;
    let args = ["blame", "-L", "321,323", "renamed"];
    let buffered = brit(root, &args, "gitoxide.objects.allocLimit=1073741824")?;
    let bounded = brit(root, &args, "gitoxide.objects.allocLimit=16777216")?;
    ensure!(
        bounded == buffered,
        "allocation policy must preserve rename attribution"
    );
    let mut file = fs::OpenOptions::new().append(true).open(root.join("renamed"))?;
    writeln!(file, "uncommitted line")?;
    drop(file);
    let args = ["blame", "-L", "321,324", "renamed"];
    let buffered = brit(root, &args, "gitoxide.objects.allocLimit=1073741824")?;
    let bounded = brit(root, &args, "gitoxide.objects.allocLimit=16777216")?;
    ensure!(
        bounded == buffered && bounded.lines().last().is_some_and(|line| line.starts_with("00000000")),
        "worktree attribution includes the uncommitted line"
    );
    let bare = tempfile::tempdir()?;
    git(
        root,
        &["clone", "--bare", ".", bare.path().to_str().expect("temporary path")],
    )?;
    let args = ["blame", "-L", "321,323", "renamed"];
    let buffered = brit(bare.path(), &args, "gitoxide.objects.allocLimit=1073741824")?;
    let bounded = brit(bare.path(), &args, "gitoxide.objects.allocLimit=16777216")?;
    ensure!(
        bounded == buffered && bounded.lines().count() == 3,
        "bare blame reads committed bytes"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn blame_actual_reduced_trust_streams_history() -> Result<()> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let repo = TestRepo::new("memory-blame-reduced")?;
    let root = repo.path();
    large_file(&root.join("history"), 20 * 1024 * 1024)?;
    git(root, &["add", "history"])?;
    git(root, &["commit", "-qm", "base"])?;
    let expected = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    git(root, &["repack", "-ad"])?;
    for path in [root.to_owned(), root.join(".git")] {
        let path = CString::new(path.as_os_str().as_bytes())?;
        // SAFETY: these NUL-terminated paths belong exclusively to this disposable fixture.
        ensure!(
            unsafe { libc::chown(path.as_ptr(), 65534, 65534) } == 0,
            "requires chown authority"
        );
    }
    let out = BritInvocation::new(brit_bin().expect("built Brit"))
        .current_dir(root)
        .args(["blame", "-L", "1,1", "history"])
        .run()?;
    ensure!(out.status.success(), "actual reduced-trust blame: {}", out.stderr);
    ensure!(
        out.stdout.starts_with(&expected[..8]),
        "attribution retains the original commit"
    );
    Ok(())
}

#[test]
fn blame_one_million_unique_short_lines() -> Result<()> {
    let repo = TestRepo::new("memory-blame-million")?;
    let root = repo.path();
    let mut file = std::io::BufWriter::new(fs::File::create(root.join("history"))?);
    for n in 0..1_000_000 {
        writeln!(file, "line-{n:08} unique")?;
    }
    file.flush()?;
    drop(file);
    git(root, &["add", "history"])?;
    git(root, &["commit", "-qm", "million lines"])?;
    let base = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    fs::OpenOptions::new()
        .append(true)
        .open(root.join("history"))?
        .write_all(b"last-line\n")?;
    git(root, &["commit", "-qam", "append"])?;
    let last = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let output = brit(
        root,
        &["blame", "-L", "1,1", "-L", "1000001,1000001", "history"],
        "gitoxide.objects.allocLimit=16777216",
    )?;
    let lines: Vec<_> = output.lines().collect();
    ensure!(
        lines.len() == 2 && lines[0].starts_with(&base[..8]) && lines[1].starts_with(&last[..8]),
        "disjoint ranges preserve attribution across a million unique lines"
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn archive_cancels_during_large_object_spooling() -> Result<()> {
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
    let repo = TestRepo::new("memory-archive-interrupt")?;
    let root = repo.path();
    large_file(&root.join("large"), 256 * 1024 * 1024)?;
    git(root, &["add", "large"])?;
    git(root, &["commit", "-qm", "large"])?;
    let index = fs::read(root.join(".git/index"))?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let scratch = tempfile::tempdir()?;
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command
        .current_dir(root)
        .env("TMPDIR", scratch.path())
        .env("GIT_CONFIG_GLOBAL", root.join(".git/absent-global"))
        .args(["archive", "interrupted.tar"])
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
        ensure!(
            child.0.try_wait()?.is_none(),
            "archive exited before spooling checkpoint"
        );
        ensure!(Instant::now() < deadline, "archive did not reach spooling checkpoint");
        std::thread::sleep(Duration::from_millis(1));
    }
    let interrupted = Instant::now();
    ensure!(Command::new("kill")
        .args(["-INT", &child.0.id().to_string()])
        .status()?
        .success());
    loop {
        if let Some(status) = child.0.try_wait()? {
            ensure!(!status.success(), "cancelled archive must report failure");
            break;
        }
        ensure!(
            interrupted.elapsed() < Duration::from_secs(1),
            "controlled archive cancellation exceeded one second"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(git(root, &["rev-parse", "HEAD"])? == head);
    ensure!(
        fs::read_dir(scratch.path())?.next().is_none(),
        "cancelled archive must reclaim its spools"
    );
    Ok(())
}

#[test]
fn mapped_single_lines_and_binary_files_keep_git_semantics() -> Result<()> {
    for binary in [false, true] {
        let repo = TestRepo::new("memory-long-line")?;
        let root = repo.path();
        let mut file = fs::File::create(root.join("content"))?;
        let chunk = [b'x'; 65536];
        for _ in 0..320 {
            file.write_all(&chunk)?;
        }
        if binary {
            file.rewind()?;
            file.write_all(&[0])?;
        }
        drop(file);
        git(root, &["add", "content"])?;
        git(root, &["commit", "-qm", "base"])?;
        ensure!(brit(root, &["diff"], "gitoxide.objects.allocLimit=16777216")?.is_empty());
        let mut file = fs::OpenOptions::new().write(true).open(root.join("content"))?;
        file.seek(SeekFrom::Start(10 * 1024 * 1024))?;
        file.write_all(b"changed")?;
        drop(file);
        let expected = git(root, &["diff", "--no-ext-diff", "--no-textconv"])?;
        for limit in [
            "gitoxide.objects.allocLimit=16777216",
            "gitoxide.objects.allocLimit=1073741824",
        ] {
            let actual = brit(root, &["diff"], limit)?;
            ensure!(
                actual.as_bytes() == expected,
                "single-line/binary patch differs from Git (binary={binary})"
            );
        }
    }
    Ok(())
}
