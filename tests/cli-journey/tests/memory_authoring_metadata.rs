//! Checkout and restoration must not buffer unrelated commit messages.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
    process::Command,
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

fn brit(root: &Path, args: &[&str], limit: Option<&str>) -> Result<Vec<u8>> {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command.current_dir(root);
    if let Some(limit) = limit {
        command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
    }
    let output = command.args(args).output()?;
    ensure!(
        output.status.success(),
        "Brit {args:?}, limit {limit:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn checkout_metadata(size: usize, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-checkout-metadata")?;
    let root = repo.path();
    let message = root.join(".git/large-message");
    let mut file = BufWriter::new(fs::File::create(&message)?);
    file.write_all(b"governance decision\n\n")?;
    let chunk = [b'x'; 65536];
    for _ in 0..size / chunk.len() {
        file.write_all(&chunk)?;
    }
    file.write_all(b"\n")?;
    file.flush()?;
    drop(file);
    for (branch, content) in [("main", "main contents\n"), ("target", "target contents\n")] {
        if branch == "target" {
            git(root, &["switch", "-qc", branch])?;
        }
        fs::write(root.join("selected"), content)?;
        git(root, &["add", "selected"])?;
        git(root, &["commit", "-q", "-F", message.to_str().expect("message path")])?;
    }
    git(root, &["switch", "-q", "main"])?;
    let refs = git(root, &["show-ref"])?;
    fs::write(root.join("unrelated"), b"preserved untracked content\n")?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let trust = brit(root, &["free", "trust", ".", ".git"], None)?;
            ensure!(
                String::from_utf8(trust)?
                    .lines()
                    .filter(|line| line.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("reduced trust qualification runs on Linux");
    }
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [Some("1024"), None, Some("16777216"), Some("1073741824")] {
            let original_index = fs::read(root.join(".git/index"))?;
            fs::write(root.join("selected"), b"working edit\n")?;
            brit(
                root,
                &["restore", "--worktree", "--source", "HEAD", "--", "selected"],
                limit,
            )?;
            ensure!(fs::read(root.join("selected"))? == b"main contents\n");
            ensure!(fs::read(root.join(".git/index"))? == original_index);
            fs::write(root.join("selected"), b"staged edit\n")?;
            git(root, &["add", "selected"])?;
            brit(root, &["restore", "--staged", "--", "selected"], limit)?;
            ensure!(git(root, &["show", ":selected"])? == b"main contents\n");
            ensure!(fs::read(root.join("selected"))? == b"staged edit\n");
            brit(root, &["restore", "--worktree", "--", "selected"], limit)?;
            // Durable switch is supported only on Linux; restore remains portable.
            #[cfg(target_os = "linux")]
            for (branch, contents) in [
                ("target", b"target contents\n".as_slice()),
                ("main", b"main contents\n"),
            ] {
                brit(root, &["switch", branch], limit)?;
                ensure!(fs::read(root.join("selected"))? == contents);
                ensure!(git(root, &["show", ":selected"])? == contents);
                ensure!(git(root, &["symbolic-ref", "--short", "HEAD"])? == format!("{branch}\n").as_bytes());
            }
            ensure!(git(root, &["show-ref"])? == refs);
            ensure!(fs::read(root.join("unrelated"))? == b"preserved untracked content\n");
        }
    }
    Ok(())
}

#[test]
fn checkout_preserves_large_commit_metadata() -> Result<()> {
    checkout_metadata(128 * 1024, false)
}

#[test]
#[ignore = "large metadata: qualification runs this explicitly"]
fn checkout_twenty_mib_commit_metadata() -> Result<()> {
    checkout_metadata(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn checkout_twenty_mib_commit_metadata_reduced_trust() -> Result<()> {
    checkout_metadata(20 * 1024 * 1024, true)
}

fn worktree_metadata(size: usize, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-linked-metadata")?;
    let root = repo.path();
    repo.commit_file("selected", "linked contents\n")?;
    let message = root.join(".git/large-message");
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
        &["commit", "--amend", "-q", "-F", message.to_str().expect("message path")],
    )?;
    git(
        root,
        &["tag", "-a", "large-tag", "-F", message.to_str().expect("message path")],
    )?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    // Keep explicit-output writer guards inside this disposable repository even
    // when qualification's TMPDIR belongs to a shared parent checkout.
    let destinations = tempfile::tempdir_in(root)?;
    let oracle = destinations.path().join("oracle");
    git(
        root,
        &[
            "worktree",
            "add",
            "--detach",
            oracle.to_str().expect("oracle path"),
            "large-tag",
        ],
    )?;
    let expected_index = git(&oracle, &["ls-files", "--stage"])?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let trust = brit(root, &["free", "trust", ".", ".git"], None)?;
            ensure!(
                String::from_utf8(trust)?
                    .lines()
                    .filter(|line| line.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("reduced trust qualification runs on Linux");
    }
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for (n, limit) in [Some("1024"), None, Some("16777216"), Some("1073741824")]
            .into_iter()
            .enumerate()
        {
            for mode in ["existing", "new", "detached"] {
                let name = format!("{mode}-{packed}-{n}");
                let target = destinations.path().join(&name);
                let path = target.to_str().expect("worktree path");
                let args = match mode {
                    "existing" => {
                        git(root, &["branch", &name])?;
                        vec!["worktree", "add", path, &name]
                    }
                    "new" => vec!["worktree", "add", "-b", &name, path, "large-tag"],
                    _ => vec!["worktree", "add", "--detach", path, "large-tag"],
                };
                brit(root, &args, limit)?;
                ensure!(git(&target, &["rev-parse", "HEAD"])? == head);
                ensure!(git(&target, &["ls-files", "--stage"])? == expected_index);
                ensure!(fs::read(target.join("selected"))? == fs::read(oracle.join("selected"))?);
                ensure!(git(&target, &["status", "--porcelain"])?.is_empty());
                brit(root, &["worktree", "remove", path], limit)?;
                ensure!(!target.exists());
                if mode != "detached" {
                    ensure!(
                        git(root, &["rev-parse", &name])? == head,
                        "removal must retain the branch"
                    );
                }
            }
            ensure!(git(root, &["rev-parse", "HEAD"])? == head);
            ensure!(fs::read(root.join(".git/index"))? == index);
        }
    }
    Ok(())
}

#[test]
fn worktree_preserves_large_commit_and_tag_metadata() -> Result<()> {
    worktree_metadata(128 * 1024, false)
}

#[test]
#[ignore = "large metadata: qualification runs this explicitly"]
fn worktree_twenty_mib_commit_and_tag_metadata() -> Result<()> {
    worktree_metadata(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn worktree_twenty_mib_commit_and_tag_metadata_reduced_trust() -> Result<()> {
    worktree_metadata(20 * 1024 * 1024, true)
}
