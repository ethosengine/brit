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

fn verify_metadata(root: &Path, output: &Path, limit: Option<&str>) -> Result<()> {
    for algorithm in ["less-time", "less-memory"] {
        for mode in [None, Some("--decode"), Some("--re-encode")] {
            let mut args = vec!["--threads", "1", "verify", "--algorithm", algorithm];
            args.extend(mode);
            brit(root, &args, output, limit)?;
        }
    }
    Ok(())
}

fn large_reflog(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-reflog")?;
    let root = repo.path();
    let first = repo.head_id()?;
    let second = repo.commit_file("tracked", "unchanged working content\n")?;
    let index = fs::read(root.join(".git/index"))?;
    let log = root.join(".git/logs/HEAD");
    let mut output = BufWriter::new(fs::File::create(&log)?);
    let expected_branches = root.join(".git/expected-branches");
    let mut branches = BufWriter::new(fs::File::create(&expected_branches)?);
    for n in 0..200_000 {
        let (old, new) = if n % 2 == 0 {
            (&second, &first)
        } else {
            (&first, &second)
        };
        writeln!(
            output,
            "{old} {new} Fixture <fixture@example.com> {} +0000\tcheckout: moving from main to main",
            1_600_000_000 + n
        )?;
        writeln!(branches, "{old} main")?;
    }
    output.flush()?;
    drop(output);
    branches.flush()?;
    drop(branches);
    ensure!(fs::metadata(&log)?.len() > 20 * 1024 * 1024);
    let saved = root.join(".git/expected-reflog");
    fs::copy(&log, &saved)?;
    reduced_trust(root, reduced)?;
    let actual = root.join(".git/actual-revision");
    let expected_json = root.join(".git/expected-branches.json");
    brit(
        root,
        &["--format", "json", "revision", "previous-branches"],
        &expected_json,
        None,
    )?;
    ensure!(fs::metadata(&expected_json)?.len() > 16 * 1024 * 1024);
    for limit in [None, Some("16777216"), Some("1")] {
        brit(root, &["revision", "previous-branches"], &actual, limit)?;
        same_bytes(&expected_branches, &actual)?;
        brit(
            root,
            &["--format", "json", "revision", "previous-branches"],
            &actual,
            limit,
        )?;
        same_bytes(&expected_json, &actual)?;
    }
    for spec in [
        "HEAD@{0}",
        "HEAD@{199999}",
        "HEAD@{2020-09-14 00:00:00 +0000}",
        "@{-1}",
        "@{-200000}",
    ] {
        let expected = git(root, &["rev-parse", "--verify", spec])?;
        for limit in [None, Some("16777216"), Some("1")] {
            brit(root, &["revision", "resolve", spec], &actual, limit)?;
            ensure!(fs::read(&actual)? == expected, "reflog query {spec} differs from Git");
        }
    }
    same_bytes(&log, &saved)?;
    ensure!(git(root, &["rev-parse", "HEAD"])? == format!("{second}\n").as_bytes());
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(fs::read(root.join("tracked"))? == b"unchanged working content\n");
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN"]
fn reflog_over_twenty_mib_preserves_revision_queries() -> Result<()> {
    large_reflog(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN and chown authority"]
fn actual_reduced_trust_large_reflog() -> Result<()> {
    large_reflog(true)
}

#[cfg(target_os = "linux")]
fn svg_history(reduced: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let repo = TestRepo::new("memory-svg-history")?;
    let root = repo.path();
    for n in 0..63 {
        git(root, &["commit", "--allow-empty", "-q", "-m", &format!("history {n}")])?;
    }
    let message = root.join(".git/message-input");
    let mut input = BufWriter::new(fs::File::create(&message)?);
    for _ in 0..320 {
        input.write_all(&[b'x'; 65536])?;
    }
    input.flush()?;
    drop(input);
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
    let head = repo.head_id()?;
    let index = fs::read(root.join(".git/index"))?;
    let ids = String::from_utf8(git(root, &["rev-list", "--abbrev-commit", "HEAD"])?)?;
    let helpers = root.join(".git/viewer-bin");
    fs::create_dir(&helpers)?;
    let helper = helpers.join("xdg-open");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf '%s\\n' \"$1\" > \"$BRIT_TEST_VIEWER_RECEIPT\"\n",
    )?;
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755))?;
    let receipt = root.join(".git/viewer-receipt");
    let svg = root.join(".git/history.svg");
    let expected = root.join(".git/expected.svg");
    reduced_trust(root, reduced)?;
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("1073741824"), Some("16777216"), Some("1")] {
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            command
                .current_dir(root)
                .env("PATH", &helpers)
                .env("BRIT_TEST_VIEWER_RECEIPT", &receipt);
            if let Some(limit) = limit {
                command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
            }
            if receipt.exists() {
                fs::remove_file(&receipt)?;
            }
            output_file(
                command,
                &["revision", "list", "--svg", svg.to_str().expect("fixture path"), "HEAD"],
                &root.join(".git/svg-stdout"),
            )?;
            ensure!(fs::read_to_string(&receipt)?.trim() == svg.to_str().expect("fixture path"));
            let rendered = fs::read_to_string(&svg)?;
            ensure!(rendered.contains("<svg"));
            for id in ids.lines() {
                ensure!(rendered.contains(id), "SVG omitted history node {id}");
            }
            if expected.exists() {
                same_bytes(&expected, &svg)?;
            } else {
                fs::copy(&svg, &expected)?;
            }
        }
    }
    ensure!(
        String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?.trim() == head
            && fs::read(root.join(".git/index"))? == index
    );
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN; isolated viewer handoff, no GUI"]
fn svg_history_with_twenty_mib_commit() -> Result<()> {
    svg_history(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN and chown authority"]
fn actual_reduced_trust_svg_history() -> Result<()> {
    svg_history(true)
}

fn large_commit_graph(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-commit-graph")?;
    let root = repo.path();
    let message = root.join(".git/message-input");
    let mut input = BufWriter::new(fs::File::create(&message)?);
    for _ in 0..320 {
        input.write_all(&[b'x'; 65536])?;
    }
    input.flush()?;
    drop(input);
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
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let mut expectations = Vec::new();
    for spec in ["HEAD", "HEAD^..HEAD"] {
        let mut expected = Vec::new();
        for row in String::from_utf8(git(
            root,
            &["rev-list", "--timestamp", "--parents", "--date-order", spec],
        )?)?
        .lines()
        {
            let mut fields = row.split_whitespace();
            let time = fields.next().expect("timestamp");
            let id = fields.next().expect("commit identity");
            let tree = String::from_utf8(git(root, &["rev-parse", &format!("{id}^{{tree}}")])?)?;
            let generation = String::from_utf8(git(root, &["rev-list", "--count", id])?)?;
            writeln!(
                expected,
                "{id} {time} {} {} {}",
                fields.count(),
                tree.trim(),
                generation.trim()
            )?;
        }
        expectations.push((spec, expected));
    }
    reduced_trust(root, reduced)?;
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        git(root, &["commit-graph", "write", "--reachable"])?;
        let graph = fs::read(root.join(".git/objects/info/commit-graph"))?;
        for limit in [None, Some("16777216"), Some("1")] {
            for (spec, expected) in &expectations {
                let actual = root.join(".git/actual-graph");
                brit(root, &["commit-graph", "list", "--long-hashes", spec], &actual, limit)?;
                ensure!(fs::read(actual)? == *expected, "commit-graph {spec} differs from Git");
            }
        }
        ensure!(fs::read(root.join(".git/objects/info/commit-graph"))? == graph);
        fs::write(root.join(".git/shallow"), &head)?;
        let visible = git(root, &["rev-list", "--date-order", "HEAD"])?;
        let mut baseline = None;
        for cache in ["core.commitGraph=true", "core.commitGraph=false"] {
            for limit in [None, Some("16777216"), Some("1")] {
                let actual = root.join(".git/actual-shallow");
                brit(
                    root,
                    &["-c", cache, "revision", "list", "--long-hashes", "HEAD"],
                    &actual,
                    limit,
                )?;
                let bytes = fs::read(actual)?;
                let mut ids = String::new();
                for row in std::str::from_utf8(&bytes)?.lines() {
                    ids.push_str(row.split_whitespace().next().expect("commit identity"));
                    ids.push('\n');
                }
                ensure!(ids.as_bytes() == visible, "shallow visible history differs from Git");
                ensure!(
                    *baseline.get_or_insert_with(|| bytes.clone()) == bytes,
                    "buffered/mapped or cached shallow output differs"
                );
            }
        }
        fs::remove_file(root.join(".git/shallow"))?;
    }
    ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN"]
fn commit_graph_with_twenty_mib_commit() -> Result<()> {
    large_commit_graph(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN and chown authority"]
fn actual_reduced_trust_large_commit_graph() -> Result<()> {
    large_commit_graph(true)
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN"]
fn commit_graph_does_not_hide_failed_shallow_boundary() -> Result<()> {
    for corrupt in [false, true] {
        let repo = TestRepo::new("memory-shallow-failure")?;
        let root = repo.path();
        let boundary = repo.commit_file("tracked", "boundary\n")?;
        let head = repo.commit_file("tracked", "tip\n")?;
        let index = fs::read(root.join(".git/index"))?;
        git(root, &["commit-graph", "write", "--reachable"])?;
        fs::write(root.join(".git/shallow"), format!("{boundary}\n"))?;
        let object = root.join(".git/objects").join(&boundary[..2]).join(&boundary[2..]);
        if corrupt {
            fs::write(object, b"invalid compressed object")?;
        } else {
            fs::remove_file(object)?;
        }
        for cache in ["core.commitGraph=true", "core.commitGraph=false"] {
            let actual = root.join(".git/failed-shallow-output");
            let error = brit(
                root,
                &["-c", cache, "revision", "list", "--long-hashes", "HEAD"],
                &actual,
                Some("1"),
            )
            .expect_err("failed boundary lookup must not become cached history");
            let diagnostic = format!("{error:#}");
            ensure!(
                diagnostic.contains("Could not read shallow boundary") && diagnostic.contains(&boundary),
                "{diagnostic}"
            );
        }
        ensure!(git(root, &["rev-parse", "HEAD"])? == format!("{head}\n").as_bytes());
        ensure!(fs::read(root.join(".git/index"))? == index && fs::read(root.join("tracked"))? == b"tip\n");
    }
    Ok(())
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
            brit(root, &["fsck", "HEAD"], &actual, limit)?;
            ensure!(
                fs::metadata(&actual)?.len() == 0,
                "complete history has no missing objects"
            );
            verify_metadata(root, &actual, limit)?;
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
            let actual = root.join(".git/actual-connectivity");
            brit(root, &["fsck", "HEAD"], &actual, limit)?;
            ensure!(
                fs::metadata(&actual)?.len() == 0,
                "complete tree has no missing objects"
            );
            verify_metadata(root, &actual, limit)?;
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

fn large_attributes_and_excludes(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-attributes")?;
    let root = repo.path();
    repo.commit_file("selected", "unchanged\n")?;
    for name in [".gitattributes", ".gitignore"] {
        let mut file = BufWriter::new(fs::File::create(root.join(name))?);
        for n in 0..200_000 {
            write!(file, "unused-{n:06}-{}", "x".repeat(96))?;
            if name == ".gitattributes" {
                write!(file, " custom=value")?;
            }
            writeln!(file)?;
        }
        if name == ".gitattributes" {
            writeln!(file, "selected text\nselected -text custom=last")?;
        } else {
            writeln!(file, "*.tmp\n!keep.tmp")?;
        }
        file.flush()?;
        ensure!(fs::metadata(root.join(name))?.len() > 20 * 1024 * 1024);
    }
    git(root, &["add", ".gitattributes", ".gitignore"])?;
    git(root, &["commit", "-qm", "large pattern metadata"])?;
    git(root, &["update-index", "--skip-worktree", ".gitignore"])?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let expected = root.join(".git/expected-excludes");
    output_file(
        oracle(root),
        &["check-ignore", "-nv", "discard.tmp", "keep.tmp", "plain"],
        &expected,
    )?;
    let attrs = git(root, &["check-attr", "--all", "selected"])?;
    ensure!(
        attrs == b"selected: text: unset\nselected: custom: last\n",
        "stock Git must establish the selected attribute values"
    );
    reduced_trust(root, reduced)?;
    let output = root.join(".git/pattern-output");
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for from_object in [false, true] {
            if from_object {
                fs::rename(root.join(".gitattributes"), root.join(".git/saved-attributes"))?;
                fs::rename(root.join(".gitignore"), root.join(".git/saved-ignore"))?;
                output_file(
                    oracle(root),
                    &["check-ignore", "-nv", "discard.tmp", "keep.tmp", "plain"],
                    &expected,
                )?;
            }
            for limit in [None, Some("1073741824"), Some("16777216"), Some("1024")] {
                brit(root, &["attributes", "query", "selected"], &output, limit)?;
                let attributes = fs::read_to_string(&output)?;
                let mut assignments: Vec<_> = attributes
                    .lines()
                    .map(|line| line.rsplit('\t').next().expect("assignment"))
                    .collect();
                assignments.sort_unstable();
                ensure!(
                    assignments == ["-text", "custom=last"],
                    "selected attribute values differ: {attributes}"
                );
                brit(
                    root,
                    &[
                        "exclude",
                        "query",
                        "--show-ignore-patterns",
                        "discard.tmp",
                        "keep.tmp",
                        "plain",
                    ],
                    &output,
                    limit,
                )?;
                let actual = fs::read_to_string(&output)?;
                let root_prefix = format!("{}/", root.display());
                let actual = actual
                    .lines()
                    .map(|line| {
                        let line = line.strip_prefix(&root_prefix).unwrap_or(line);
                        line.strip_prefix("./").unwrap_or(line)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
                    + "\n";
                ensure!(
                    fs::read_to_string(&expected)? == actual,
                    "exclude output differs: {actual:?}"
                );
            }
            if from_object {
                fs::rename(root.join(".git/saved-attributes"), root.join(".gitattributes"))?;
                fs::rename(root.join(".git/saved-ignore"), root.join(".gitignore"))?;
            }
        }
    }
    ensure!(git(root, &["rev-parse", "HEAD"])? == head);
    ensure!(fs::read(root.join(".git/index"))? == index);
    ensure!(fs::read(root.join("selected"))? == b"unchanged\n");
    ensure!(git(root, &["hash-object", ".gitignore"])? == git(root, &["rev-parse", "HEAD:.gitignore"])?);
    git(
        root,
        &["diff", "--exit-code", "--", ".gitattributes", ".gitignore", "selected"],
    )?;
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN and delegated cgroup"]
fn attributes_and_excludes_over_twenty_mib() -> Result<()> {
    large_attributes_and_excludes(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN, delegated cgroup and chown authority"]
fn actual_reduced_trust_large_attributes_and_excludes() -> Result<()> {
    large_attributes_and_excludes(true)
}

fn attribute_pathspec(size: usize, reduced: bool, faults: bool, status: bool) -> Result<()> {
    let repo = TestRepo::new("memory-attribute-pathspec")?;
    let root = repo.path();
    repo.commit_file("selected", "selected bytes\n")?;
    repo.commit_file("other", "other bytes\n")?;
    for (name, tail) in [
        (
            ".gitattributes",
            b"\nselected custom=yes\nuntracked custom=yes\nrename-* custom=yes\nother custom=no\n".as_slice(),
        ),
        (".gitignore", b"\nignored\n".as_slice()),
    ] {
        let mut patterns = BufWriter::new(fs::File::create(root.join(name))?);
        for _ in 0..size / 65536 {
            patterns.write_all(&[b'#'; 65536])?;
        }
        patterns.write_all(tail)?;
        patterns.flush()?;
    }
    git(root, &["add", ".gitattributes", ".gitignore"])?;
    git(root, &["commit", "-qm", "indexed attributes"])?;
    let attr_id = String::from_utf8(git(root, &["rev-parse", "HEAD:.gitattributes"])?)?
        .trim()
        .to_owned();
    git(root, &["update-index", "--skip-worktree", ".gitignore"])?;
    fs::remove_file(root.join(".gitattributes"))?;
    fs::remove_file(root.join(".gitignore"))?;
    fs::write(root.join("ignored"), "ignored bytes\n")?;
    fs::write(root.join("untracked"), "untracked bytes\n")?;
    ensure!(git(root, &["ls-files", "--", ":(attr:custom=yes)"])? == b"selected\n");
    let mut renamed = String::new();
    for n in 0..32 {
        use std::fmt::Write as _;
        writeln!(renamed, "line {n}")?;
    }
    if status {
        repo.commit_file("rename-source", &renamed)?;
        fs::remove_file(root.join("rename-source"))?;
        fs::write(root.join("rename-target"), format!("{renamed}added line\n"))?;
        fs::write(root.join("selected"), "selected staged bytes\n")?;
        git(root, &["add", "selected"])?;
        fs::write(root.join("selected"), "selected working bytes\n")?;
    }
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    reduced_trust(root, reduced)?;
    let commands: Vec<&[&str]> = if status {
        vec![
            &["status", "--no-write", ":(attr:custom=yes)"],
            &["status", "--no-write", "--untracked=no", ":(attr:custom=yes)"],
            &[
                "status",
                "--no-write",
                "--index-worktree-renames=50",
                ":(attr:custom=yes)",
            ],
            &["status", "--no-write"],
        ]
    } else {
        vec![
            &["attributes", "query", ":(attr:custom=yes)selected"],
            &["index", "entries", "--no-attributes", ":(attr:custom=yes)"],
            &["clean", "--pathspec-matches-result", ":(attr:custom=yes)"],
            &["clean", ":(attr:custom=yes)"],
        ]
    };
    let output = root.join(".git/pathspec-output");
    for packed in [false, true] {
        if packed {
            if faults {
                break;
            }
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for (n, args) in commands.iter().enumerate() {
            let expected = root.join(format!(".git/pathspec-expected-{n}"));
            brit(root, args, &expected, Some("1073741824"))?;
            let bytes = fs::read_to_string(&expected)?;
            ensure!(
                bytes.contains(if !status && n >= 2 { "untracked" } else { "selected" }) && !bytes.contains("other"),
                "expected selected result: {bytes:?}"
            );
            if status && n == 2 {
                ensure!(
                    bytes.contains("R rename-source → rename-target"),
                    "similarity rename: {bytes:?}"
                );
            }
            if status {
                ensure!(
                    bytes.contains(" M  selected\n") && bytes.contains("  M selected\n"),
                    "both status planes: {bytes:?}"
                );
            }
            for limit in [None, Some("16777216"), Some("1024")] {
                brit(root, args, &output, limit)?;
                if status {
                    // Staged and working-tree producers share a channel. Their existing
                    // interleaving varies even between two buffered runs; preserve each record.
                    let actual = fs::read_to_string(&output)?;
                    ensure!(actual.ends_with('\n'));
                    let mut expected_records = bytes.lines().collect::<Vec<_>>();
                    let mut actual_records = actual.lines().collect::<Vec<_>>();
                    expected_records.sort_unstable();
                    actual_records.sort_unstable();
                    ensure!(
                        expected_records == actual_records,
                        "status records differ: {bytes:?} / {actual:?}"
                    );
                } else {
                    same_bytes(&expected, &output)?;
                }
            }
        }
    }
    if faults {
        fs::write(
            root.join(".git/objects").join(&attr_id[..2]).join(&attr_id[2..]),
            b"corrupt object",
        )?;
        for args in commands {
            let mut command = Command::new(brit_bin().expect("built Brit"));
            gix_testtools::configure_git_environment(&mut command, root);
            command
                .current_dir(root)
                .args(["-c", "gitoxide.objects.allocLimit=1024"])
                .args(args);
            if args[0] == "clean" {
                command.arg("--execute");
            }
            let result = command.output()?;
            ensure!(
                !result.status.success(),
                "{args:?} must preserve a failed attribute read"
            );
            let diagnostic = String::from_utf8_lossy(&result.stderr);
            ensure!(
                diagnostic.contains(&attr_id) || diagnostic.contains(&attr_id[2..]),
                "{args:?} must identify the failed pattern object or loose-object path: {diagnostic}"
            );
        }
    }
    ensure!(git(root, &["rev-parse", "HEAD"])? == head);
    ensure!(fs::read(root.join(".git/index"))? == index);
    for (name, content) in [
        (
            "selected",
            if status {
                "selected working bytes\n"
            } else {
                "selected bytes\n"
            },
        ),
        ("other", "other bytes\n"),
        ("untracked", "untracked bytes\n"),
        ("ignored", "ignored bytes\n"),
    ] {
        ensure!(fs::read(root.join(name))? == content.as_bytes(), "preserve {name}");
    }
    if status {
        ensure!(!root.join("rename-source").exists());
        ensure!(fs::read_to_string(root.join("rename-target"))? == format!("{renamed}added line\n"));
    }
    Ok(())
}

#[test]
fn attribute_pathspec_preserves_selection_and_read_failures() -> Result<()> {
    attribute_pathspec(128 * 1024, false, true, false)
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN and delegated cgroup"]
fn attribute_pathspec_twenty_mib() -> Result<()> {
    attribute_pathspec(20 * 1024 * 1024, false, false, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN, delegated cgroup and chown authority"]
fn attribute_pathspec_twenty_mib_reduced_trust() -> Result<()> {
    attribute_pathspec(20 * 1024 * 1024, true, false, false)
}

#[test]
fn status_patterns_preserve_selection_and_read_failures() -> Result<()> {
    attribute_pathspec(128 * 1024, false, true, true)
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN and delegated cgroup"]
fn status_patterns_twenty_mib() -> Result<()> {
    attribute_pathspec(20 * 1024 * 1024, false, false, true)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN, delegated cgroup and chown authority"]
fn status_patterns_twenty_mib_reduced_trust() -> Result<()> {
    attribute_pathspec(20 * 1024 * 1024, true, false, true)
}

fn describe_metadata(size: usize, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-describe")?;
    let root = repo.path();
    let message = root.join(".git/describe-message");
    let mut file = BufWriter::new(fs::File::create(&message)?);
    file.write_all(b"governance decision\n\n")?;
    let chunk = [b'x'; 65536];
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    file.write_all(b"\n")?;
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
    git(
        root,
        &["tag", "-a", "large-tag", "-F", message.to_str().expect("fixture path")],
    )?;
    git(root, &["commit", "--allow-empty", "-qm", "small intermediate"])?;
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
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    reduced_trust(root, reduced)?;
    let expected = root.join(".git/describe-expected");
    let actual = root.join(".git/describe-actual");
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [Some("1073741824"), None, Some("16777216"), Some("1024")] {
            for revision in ["HEAD", "HEAD^", "HEAD~2"] {
                for annotated in [false, true] {
                    let mut args = vec!["describe", "--long", "--always"];
                    if !annotated {
                        args.push("--tags");
                    }
                    args.push(revision);
                    output_file(oracle(root), &args, &expected)?;
                    let mut args = vec!["commit", "describe", "--long", "--always"];
                    if annotated {
                        args.push("--annotated-tags");
                    }
                    args.push(revision);
                    brit(root, &args, &actual, limit)?;
                    same_bytes(&expected, &actual)?;
                }
            }
        }
        if !packed {
            let tag = String::from_utf8(git(root, &["rev-parse", "refs/tags/large-tag"])?)?;
            let tag = tag.trim();
            let path = root.join(".git/objects").join(&tag[..2]).join(&tag[2..]);
            let original = fs::read(&path)?;
            let permissions = fs::metadata(&path)?.permissions();
            let mut writable = permissions.clone();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                writable.set_mode(writable.mode() | 0o200);
            }
            #[cfg(not(unix))]
            writable.set_readonly(false);
            fs::set_permissions(&path, writable)?;
            fs::write(&path, b"corrupt retained tag")?;
            for annotated in [false, true] {
                let mut command = Command::new(brit_bin().expect("built Brit"));
                gix_testtools::configure_git_environment(&mut command, root);
                command.current_dir(root).args(["commit", "describe", "--always"]);
                if annotated {
                    command.arg("--annotated-tags");
                }
                let result = command.output()?;
                ensure!(
                    !result.status.success(),
                    "corrupt tag must not become an object-ID fallback"
                );
                ensure!(String::from_utf8_lossy(&result.stderr).contains("object"));
            }
            fs::write(&path, original)?;
            fs::set_permissions(path, permissions)?;
        }
    }
    let superproject = TestRepo::new("memory-describe-submodule")?;
    let parent = superproject.path();
    // The stock-Git upload-pack child checks the source gitdir's ownership.
    // This process-local exception does not change Brit's ownership-derived trust.
    let source_allow_list = format!("safe.directory={}", root.join(".git").display());
    git(
        parent,
        &[
            "-c",
            &source_allow_list,
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            "--",
            root.to_str().expect("fixture path"),
            "child",
        ],
    )?;
    let mut modules = fs::OpenOptions::new().append(true).open(parent.join(".gitmodules"))?;
    modules.write_all(b"\n#")?;
    for _ in 0..size / chunk.len() {
        modules.write_all(&chunk)?;
    }
    modules.write_all(b"\n")?;
    drop(modules);
    git(parent, &["add", ".gitmodules", "child"])?;
    git(parent, &["commit", "-qm", "nested metadata"])?;
    let parent_head = git(parent, &["rev-parse", "HEAD"])?;
    let parent_index = fs::read(parent.join(".git/index"))?;
    reduced_trust(parent, reduced)?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::chown;
            chown(parent.join("child"), Some(65534), None)?;
            chown(parent.join(".git/modules/child"), Some(65534), None)?;
            let trust = parent.join(".git/child-trust");
            brit(parent, &["free", "trust", "child", ".git/modules/child"], &trust, None)?;
            ensure!(
                fs::read_to_string(trust)?
                    .lines()
                    .filter(|line| line.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
    }
    let expected = parent.join(".git/submodule-expected");
    let actual = parent.join(".git/submodule-actual");
    brit(parent, &["submodule", "list"], &expected, Some("1073741824"))?;
    ensure!(fs::read_to_string(&expected)?.contains("(main)"));
    for indexed in [false, true] {
        if indexed {
            fs::remove_file(parent.join(".gitmodules"))?;
        }
        for limit in [None, Some("16777216"), Some("1024")] {
            brit(parent, &["submodule", "list"], &actual, limit)?;
            same_bytes(&expected, &actual)?;
        }
    }
    ensure!(git(parent, &["rev-parse", "HEAD"])? == parent_head);
    ensure!(fs::read(parent.join(".git/index"))? == parent_index);
    ensure!(git(root, &["rev-parse", "HEAD"])? == head);
    ensure!(fs::read(root.join(".git/index"))? == index);
    Ok(())
}

#[test]
fn describe_preserves_large_metadata_and_tag_selection() -> Result<()> {
    describe_metadata(128 * 1024, false)
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN and delegated cgroup"]
fn describe_twenty_mib_metadata() -> Result<()> {
    describe_metadata(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires BRIT_BIN, delegated cgroup and chown authority"]
fn describe_twenty_mib_metadata_reduced_trust() -> Result<()> {
    describe_metadata(20 * 1024 * 1024, true)
}
