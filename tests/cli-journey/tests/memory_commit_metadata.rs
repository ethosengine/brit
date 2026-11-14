//! Large commit metadata must preserve authoring semantics under allocation policy.
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

fn commit_metadata(size: usize, reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-commit-authoring")?;
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
    for contents in ["parent contents\n", "HEAD contents\n"] {
        fs::write(root.join("selected"), contents)?;
        git(root, &["add", "selected"])?;
        git(root, &["commit", "-q", "-F", message.to_str().expect("message path")])?;
    }
    let base = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let base = base.trim();
    let config = fs::read(root.join(".git/config"))?;
    fs::write(root.join("unrelated"), b"preserved untracked\n")?;
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
            for args in [
                vec!["commit", "-m", "followup"],
                vec!["commit", "--amend", "--no-edit"],
                vec!["commit", "--amend", "-m", "replacement"],
                vec!["commit", "-F", message.to_str().expect("message path")],
            ] {
                let mut expected = Vec::new();
                for use_brit in [false, true] {
                    git(root, &["reset", "--hard", "-q", base])?;
                    fs::write(root.join("selected"), b"authored change\n")?;
                    git(root, &["add", "selected"])?;
                    if use_brit {
                        brit(root, &args, limit)?;
                        ensure!(
                            git(root, &["rev-parse", "HEAD"])? == expected,
                            "commit differs from Git: {args:?}"
                        );
                    } else {
                        git(root, &args)?;
                        expected = git(root, &["rev-parse", "HEAD"])?;
                    }
                    ensure!(git(root, &["show", ":selected"])? == b"authored change\n");
                    ensure!(fs::read(root.join("selected"))? == b"authored change\n");
                    ensure!(fs::read(root.join("unrelated"))? == b"preserved untracked\n");
                    ensure!(fs::read(root.join(".git/config"))? == config);
                }
            }
            git(root, &["reset", "--hard", "-q", base])?;
        }
    }
    Ok(())
}

#[test]
fn commit_preserves_large_metadata() -> Result<()> {
    commit_metadata(128 * 1024, false)
}

#[test]
#[ignore = "large metadata: qualification runs this explicitly"]
fn commit_twenty_mib_metadata() -> Result<()> {
    commit_metadata(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn commit_twenty_mib_metadata_reduced_trust() -> Result<()> {
    commit_metadata(20 * 1024 * 1024, true)
}
