//! Large metadata qualifies independently of blob contents and output capture buffers.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::{BufWriter, Read, Write},
    path::Path,
    process::{Command, Stdio},
};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = oracle(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn oracle(root: &Path) -> Command {
    let mut command = gix_testtools::git_command(root);
    // Only the stock-Git oracle opts into this disposable fixture. Brit must derive trust from ownership.
    command.args(["-c", &format!("safe.directory={}", root.display())]);
    command
}

fn reduced_trust(root: &Path, reduced: bool) -> Result<()> {
    if !reduced {
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::{chown, MetadataExt};
        ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
        chown(root, Some(65534), None)?;
        chown(root.join(".git"), Some(65534), None)?;
        let trust = root.join(".git/observed-trust");
        brit(root, &["free", "trust", ".", ".git"], &trust, None)?;
        ensure!(
            fs::read_to_string(trust)?
                .lines()
                .filter(|line| line.starts_with("Reduced"))
                .count()
                == 2,
            "both worktree and repository must have actual reduced trust"
        );
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    anyhow::bail!("ownership qualification runs on Linux")
}

fn output_file(mut command: Command, args: &[&str], destination: &Path) -> Result<()> {
    let output = command
        .args(args)
        .stdout(Stdio::from(fs::File::create(destination)?))
        .output()?;
    ensure!(
        output.status.success() && !String::from_utf8_lossy(&output.stderr).contains("Error while loading mailmap"),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn brit(root: &Path, args: &[&str], destination: &Path, limit: Option<&str>) -> Result<()> {
    eprintln!("qualifying {args:?}, allocation policy {limit:?}");
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command.current_dir(root);
    if let Some(limit) = limit {
        command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
    }
    output_file(command, args, destination)
}

fn same_bytes(left: &Path, right: &Path) -> Result<()> {
    ensure!(
        fs::metadata(left)?.len() == fs::metadata(right)?.len(),
        "output lengths differ"
    );
    let mut left = fs::File::open(left)?;
    let mut right = fs::File::open(right)?;
    let mut a = [0; 65536];
    let mut b = [0; 65536];
    loop {
        let n = left.read(&mut a)?;
        if n == 0 {
            return Ok(());
        }
        right.read_exact(&mut b[..n])?;
        ensure!(a[..n] == b[..n], "output bytes differ");
    }
}

fn large_commit(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-commit-metadata")?;
    let root = repo.path();
    let message = root.join(".git/message-input");
    let mut file = BufWriter::new(fs::File::create(&message)?);
    file.write_all(b"large governance decision\n\n")?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..320 {
        file.write_all(&chunk)?;
    }
    file.flush()?;
    drop(file);
    git(
        root,
        &[
            "commit",
            "--allow-empty",
            "-q",
            "-F",
            message.to_str().expect("fixture path"),
        ],
    )?;
    ensure!(
        String::from_utf8(git(root, &["cat-file", "-s", "HEAD"])?)?
            .trim()
            .parse::<usize>()?
            > 20 * 1024 * 1024
    );
    let head = git(root, &["rev-parse", "HEAD"])?;
    let parent = String::from_utf8(git(root, &["rev-parse", "HEAD^"])?)?;
    let expressions = [
        "HEAD^",
        "HEAD~1",
        "HEAD^{}",
        "HEAD^{object}",
        "HEAD^{commit}",
        "HEAD^{/large governance decision}",
        ":/large governance decision",
    ]
    .into_iter()
    .map(|expression| Ok((expression, git(root, &["rev-parse", expression])?)))
    .collect::<Result<Vec<_>>>()?;
    let index = fs::read(root.join(".git/index"))?;
    let expected = root.join(".git/expected-log");
    output_file(
        oracle(root),
        &["log", "-n", "1", "--date-order", "--abbrev=8", "--format=%h %s"],
        &expected,
    )?;
    let expected_revisions = root.join(".git/expected-revisions");
    let mut revisions = BufWriter::new(fs::File::create(&expected_revisions)?);
    for row in String::from_utf8(git(
        root,
        &["rev-list", "--timestamp", "--parents", "--date-order", "HEAD"],
    )?)?
    .lines()
    {
        let mut fields = row.split_whitespace();
        let time = fields.next().expect("Git commit timestamp");
        let id = fields.next().expect("Git commit identity");
        writeln!(revisions, "{id} {time} {}", fields.count())?;
    }
    revisions.flush()?;
    drop(revisions);
    reduced_trust(root, reduced)?;
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("16777216")] {
            let actual = root.join(".git/actual-log");
            brit(root, &["log", "-n", "1"], &actual, limit)?;
            same_bytes(&expected, &actual)?;
            brit(root, &["revision", "list", "--long-hashes", "HEAD"], &actual, limit)?;
            same_bytes(&expected_revisions, &actual)?;
            brit(root, &["merge-base", "HEAD", parent.trim()], &actual, limit)?;
            ensure!(
                fs::read_to_string(&actual)? == parent,
                "merge-base changed the common ancestor"
            );
            for (expression, expected) in &expressions {
                brit(root, &["revision", "resolve", expression], &actual, limit)?;
                ensure!(fs::read(&actual)? == *expected, "revision expression {expression}");
            }
            brit(root, &["revision", "resolve", "HEAD"], &actual, limit)?;
            ensure!(
                fs::read(&actual)? == head,
                "reference resolution must not read the commit body"
            );
            ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
        }
    }
    git(root, &["symbolic-ref", "refs/heads/alias", "refs/heads/main"])?;
    git(root, &["symbolic-ref", "HEAD", "refs/heads/alias"])?;
    let actual = root.join(".git/actual-symbolic-log");
    brit(root, &["log", "-n", "1"], &actual, Some("16777216"))?;
    same_bytes(&expected, &actual)?;
    brit(root, &["revision", "resolve", "HEAD"], &actual, Some("16777216"))?;
    ensure!(
        fs::read(&actual)? == head,
        "symbolic reference resolution changed the target"
    );
    ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
    // A malformed HEAD naming a large blob must be rejected from its header,
    // without misreporting an allocation limit while trying to read the body.
    let blob = git(root, &["hash-object", "-w", message.to_str().expect("fixture path")])?;
    let saved_head = fs::read(root.join(".git/HEAD"))?;
    fs::write(root.join(".git/HEAD"), &blob)?;
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    let rejected = command
        .current_dir(root)
        .args(["-c", "gitoxide.objects.allocLimit=16777216", "log", "-n", "1"])
        .output()?;
    let error = String::from_utf8_lossy(&rejected.stderr);
    ensure!(
        !rejected.status.success()
            && error.contains("does not point to a commit")
            && error.contains(std::str::from_utf8(&blob)?.trim())
            && !error.contains("allocation limit"),
        "header refusal: {error}"
    );
    ensure!(fs::read(root.join(".git/HEAD"))? == blob && fs::read(root.join(".git/index"))? == index);
    fs::write(root.join(".git/HEAD"), saved_head)?;
    Ok(())
}

#[test]
fn commit_metadata_over_twenty_mib_preserves_log_title() -> Result<()> {
    large_commit(false)
}

#[cfg(unix)] // std::env::temp_dir uses TMPDIR on Unix; Windows needs its own environment fixture.
#[test]
fn small_revision_queries_only_require_scratch_when_mapping() -> Result<()> {
    let repo = TestRepo::new("memory-revision-scratch")?;
    let root = repo.path();
    let missing = root.join("missing-scratch-directory");
    for args in [
        vec!["revision", "list", "--long-hashes", "HEAD"],
        vec!["merge-base", "HEAD", "HEAD"],
    ] {
        let mut command = Command::new(brit_bin().expect("built Brit"));
        gix_testtools::configure_git_environment(&mut command, root);
        let output = command.current_dir(root).env("TMPDIR", &missing).args(&args).output()?;
        ensure!(
            output.status.success(),
            "buffered {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    let output = command
        .current_dir(root)
        .env("TMPDIR", &missing)
        .args(["-c", "gitoxide.objects.allocLimit=1", "revision", "list", "HEAD"])
        .output()?;
    ensure!(
        !output.status.success()
            && String::from_utf8_lossy(&output.stderr).contains("Could not create scratch for object"),
        "mapping must report the real scratch setup failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(!missing.exists());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn actual_reduced_trust_large_commit_metadata() -> Result<()> {
    large_commit(true)
}

fn large_tree(reduced: bool) -> Result<()> {
    const ENTRIES: u64 = 240_000;
    let repo = TestRepo::new("memory-tree-metadata")?;
    let root = repo.path();
    let input = root.join(".git/tree-input");
    fs::write(&input, b"x")?;
    let blob = String::from_utf8(git(
        root,
        &["hash-object", "-w", input.to_str().expect("fixture path")],
    )?)?;
    let decode_id = |hex: &str| -> Result<Vec<u8>> {
        hex.trim()
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
            .collect()
    };
    let blob_id = decode_id(&blob)?;
    let mut file = BufWriter::new(fs::File::create(&input)?);
    let suffix = "metadata-entry-".repeat(5);
    for n in 0..ENTRIES {
        write!(file, "100644 {n:06}-{suffix}\0")?;
        file.write_all(&blob_id)?;
    }
    file.flush()?;
    drop(file);
    ensure!(fs::metadata(&input)?.len() > 20 * 1024 * 1024);
    let wide = String::from_utf8(git(
        root,
        &["hash-object", "-w", "-t", "tree", input.to_str().expect("fixture path")],
    )?)?;
    let mut parent = b"40000 branch\0".to_vec();
    parent.extend(decode_id(&wide)?);
    fs::write(&input, parent)?;
    let tree = String::from_utf8(git(
        root,
        &["hash-object", "-w", "-t", "tree", input.to_str().expect("fixture path")],
    )?)?;
    let commit = String::from_utf8(git(root, &["commit-tree", tree.trim(), "-m", "large tree metadata"])?)?;
    git(root, &["update-ref", "refs/heads/main", commit.trim()])?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    reduced_trust(root, reduced)?;
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("16777216")] {
            for (spec, recursive) in [(wide.trim(), false), ("HEAD", true)] {
                for extended in [false, true] {
                    let expected = root.join(".git/expected-tree");
                    let format = if extended {
                        "--format=BLOB %(objectname) %(objectsize) %(path)"
                    } else {
                        "--format=BLOB %(objectname) %(path)"
                    };
                    output_file(oracle(root), &["ls-tree", "-r", format, spec], &expected)?;
                    let actual = root.join(".git/actual-tree");
                    let mut args = vec!["tree", "entries", spec];
                    if recursive {
                        args.push("--recursive");
                    }
                    if extended {
                        args.push("--extended");
                    }
                    brit(root, &args, &actual, limit)?;
                    same_bytes(&expected, &actual)?;
                }
            }
            let actual = root.join(".git/tree-info");
            brit(root, &["tree", "info", "--extended"], &actual, limit)?;
            let info: serde_json::Value = serde_json::from_slice(&fs::read(actual)?)?;
            ensure!(info["num_trees"] == 1 && info["num_blobs"] == ENTRIES && info["bytes"] == ENTRIES);
            for spec in [
                "HEAD:branch".to_owned(),
                format!("HEAD:branch/{:06}-{suffix}", ENTRIES - 1),
            ] {
                let expected = root.join(".git/expected-revision");
                output_file(oracle(root), &["rev-parse", &spec], &expected)?;
                let actual = root.join(".git/actual-revision");
                brit(root, &["revision", "resolve", &spec], &actual, limit)?;
                same_bytes(&expected, &actual)?;
            }
            ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
        }
    }
    Ok(())
}

#[test]
fn tree_metadata_over_twenty_mib_preserves_entries_and_statistics() -> Result<()> {
    large_tree(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn actual_reduced_trust_large_tree_metadata() -> Result<()> {
    large_tree(true)
}

#[test]
fn config_over_sixteen_mib_preserves_values_and_formatting() -> Result<()> {
    config(false)
}

fn config(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-config")?;
    let root = repo.path();
    let source = root.join(".git/config");
    let mut file = BufWriter::new(fs::OpenOptions::new().append(true).open(&source)?);
    writeln!(file, "\n[probe]")?;
    let value = "configuration-value-".repeat(4);
    for n in 0..200_000 {
        writeln!(file, "key{n:06} = {value}")?;
    }
    file.flush()?;
    drop(file);
    ensure!(
        fs::metadata(&source)?.len() > 16 * 1024 * 1024,
        "metadata fixture exceeds former boundary"
    );
    let expected = root.join(".git/expected-values");
    output_file(oracle(root), &["config", "--get-regexp", "^probe\\."], &expected)?;
    reduced_trust(root, reduced)?;
    for limit in [None, Some("16777216")] {
        let shown = root.join(".git/shown-config");
        brit(root, &["config", "probe"], &shown, limit)?;
        let values = root.join(".git/shown-values");
        output_file(
            oracle(root),
            &[
                "config",
                "--file",
                shown.to_str().expect("fixture path"),
                "--get-regexp",
                "^probe\\.",
            ],
            &values,
        )?;
        same_bytes(&expected, &values)?;
        let formatted = root.join(".git/formatted-config");
        brit(
            root,
            &["config", "fmt", source.to_str().expect("fixture path")],
            &formatted,
            limit,
        )?;
        output_file(
            oracle(root),
            &[
                "config",
                "--file",
                formatted.to_str().expect("fixture path"),
                "--get-regexp",
                "^probe\\.",
            ],
            &values,
        )?;
        same_bytes(&expected, &values)?;
        let twice = root.join(".git/formatted-twice");
        brit(
            root,
            &["config", "fmt", formatted.to_str().expect("fixture path")],
            &twice,
            limit,
        )?;
        same_bytes(&formatted, &twice)?;
        brit(root, &["config", "list"], &root.join(".git/config-files"), limit)?;
    }
    Ok(())
}

#[test]
fn packed_refs_over_sixteen_mib_preserve_branch_and_tag_order() -> Result<()> {
    packed_refs(false)
}

fn packed_refs(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-refs")?;
    let root = repo.path();
    fs::write(root.join("small"), b"small\n")?;
    git(root, &["add", "small"])?;
    git(root, &["commit", "-qm", "base"])?;
    let id = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let source = root.join(".git/packed-refs");
    let mut file = BufWriter::new(fs::File::create(&source)?);
    writeln!(file, "# pack-refs with: peeled fully-peeled sorted")?;
    let suffix = "x".repeat(160);
    for namespace in ["heads", "tags"] {
        for n in 0..40_000 {
            writeln!(file, "{} refs/{namespace}/n{n:06}-{suffix}", id.trim())?;
        }
    }
    file.flush()?;
    drop(file);
    ensure!(
        fs::metadata(&source)?.len() > 16 * 1024 * 1024,
        "ref metadata exceeds former boundary"
    );
    let before = git(root, &["hash-object", source.to_str().expect("fixture path")])?;
    reduced_trust(root, reduced)?;
    for (command, oracle) in [
        (
            "branch",
            vec![
                "for-each-ref",
                "--format=%(refname:lstrip=2)",
                "--sort=refname",
                "refs/heads",
            ],
        ),
        ("tag", vec!["tag", "--list", "--sort=version:refname"]),
    ] {
        let expected = root.join(format!(".git/expected-{command}"));
        output_file(self::oracle(root), &oracle, &expected)?;
        for limit in [None, Some("16777216")] {
            let actual = root.join(format!(".git/actual-{command}"));
            brit(root, &[command, "list"], &actual, limit)?;
            same_bytes(&expected, &actual)?;
        }
    }
    assert_eq!(
        git(root, &["hash-object", source.to_str().expect("fixture path")])?,
        before
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn actual_reduced_trust_large_config_and_refs() -> Result<()> {
    config(true)?;
    packed_refs(true)
}

#[test]
fn mailmap_over_sixteen_mib_from_worktree_loose_and_packed_objects() -> Result<()> {
    mailmap(false)
}

fn mailmap(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-mailmap")?;
    let root = repo.path();
    let source = root.join(".mailmap");
    let mut file = BufWriter::new(fs::File::create(&source)?);
    let expected = root.join(".git/expected-entries");
    let mut entries = BufWriter::new(fs::File::create(&expected)?);
    let suffix = "x".repeat(40);
    entries.write_all(b"[\n")?;
    for n in 0..200_000 {
        let name = format!("New{n:06}");
        let email = format!("new{n:06}@{suffix}.example");
        let old_name = format!("Old{n:06}");
        let old_email = format!("old{n:06}@example.com");
        writeln!(file, "{name} <{email}> {old_name} <{old_email}>")?;
        if n != 0 {
            entries.write_all(b",\n")?;
        }
        write!(entries, "  {{\n    \"new_name\": \"{name}\",\n    \"new_email\": \"{email}\",\n    \"old_name\": \"{old_name}\",\n    \"old_email\": \"{old_email}\"\n  }}")?;
    }
    entries.write_all(b"\n]")?;
    entries.flush()?;
    drop(entries);
    file.flush()?;
    drop(file);
    ensure!(
        fs::metadata(&source)?.len() > 16 * 1024 * 1024,
        "large mailmap metadata"
    );
    git(root, &["add", ".mailmap"])?;
    git(root, &["commit", "-qm", "mailmap metadata"])?;
    let contacts = [
        "Old000000 <old000000@example.com>",
        "Old100000 <old100000@example.com>",
        "Old199999 <old199999@example.com>",
        "Unknown <unknown@example.com>",
    ];
    let mut args = vec!["check-mailmap"];
    args.extend(contacts);
    let resolved = root.join(".git/expected-contacts");
    output_file(oracle(root), &args, &resolved)?;
    reduced_trust(root, reduced)?;
    for storage in ["worktree", "loose", "packed"] {
        eprintln!("qualifying large mailmap: {storage}, reduced={reduced}");
        if storage == "loose" {
            git(root, &["config", "mailmap.blob", "HEAD:.mailmap"])?;
            fs::remove_file(&source)?;
        } else if storage == "packed" {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("16777216")] {
            let actual = root.join(".git/actual-entries");
            brit(root, &["mailmap", "entries"], &actual, limit)?;
            same_bytes(&expected, &actual)?;
            let mut args = vec!["mailmap", "check"];
            args.extend(contacts);
            brit(root, &args, &actual, limit)?;
            same_bytes(&resolved, &actual)?;
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn actual_reduced_trust_large_mailmap() -> Result<()> {
    mailmap(true)
}

#[test]
fn annotated_tag_metadata_is_not_lost_above_allocation_policy() -> Result<()> {
    annotated_tag(false)
}

fn annotated_tag(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-tag-object")?;
    let root = repo.path();
    let source = root.join(".git/tag-input");
    let mut file = BufWriter::new(fs::File::create(&source)?);
    writeln!(
        file,
        "object {}\ntype commit\ntag payload-name\ntagger Fixture <fixture@example.com> 1600000000 +0000\n",
        repo.head_id()?
    )?;
    let chunk = [b'x'; 65536];
    for _ in 0..320 {
        file.write_all(&chunk)?;
    }
    file.write_all(
        b"\n-----BEGIN PGP SIGNATURE-----\nfixture armor, not a verified signature\n-----END PGP SIGNATURE-----\n",
    )?;
    file.flush()?;
    drop(file);
    let id = String::from_utf8(git(
        root,
        &["hash-object", "-t", "tag", "-w", source.to_str().expect("path")],
    )?)?;
    git(root, &["update-ref", "refs/tags/release", id.trim()])?;
    git(root, &["symbolic-ref", "refs/heads/tag-alias", "refs/tags/release"])?;
    let commit = git(root, &["rev-parse", "HEAD"])?;
    assert_eq!(
        git(root, &["for-each-ref", "--format=%(objecttype) %(tag)", "refs/tags"])?,
        b"tag payload-name\n"
    );
    let blob = String::from_utf8(git(root, &["hash-object", "-w", source.to_str().expect("path")])?)?;
    git(root, &["update-ref", "refs/tags/blob", blob.trim()])?;
    reduced_trust(root, reduced)?;
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
            git(root, &["pack-refs", "--all"])?;
        }
        for limit in [None, Some("16777216")] {
            let actual = root.join(".git/actual-tags");
            brit(root, &["tag", "list"], &actual, limit)?;
            assert_eq!(fs::read(&actual)?, b"blob\nrelease [tag name: payload-name, signed]\n");
            brit(root, &["revision", "resolve", "tag-alias"], &actual, limit)?;
            ensure!(fs::read(&actual)? == commit, "large tag peeling changed the target");
            for expression in ["release^{}", "release^{commit}", "release~0"] {
                brit(root, &["revision", "resolve", expression], &actual, limit)?;
                ensure!(fs::read(&actual)? == commit, "large tag expression {expression}");
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly with --ignored"]
fn actual_reduced_trust_large_annotated_tag() -> Result<()> {
    annotated_tag(true)
}
