//! Signing and verification retain Git compatibility when commit bytes exceed policy.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use gix_testtools::signature;
use std::{fs, path::Path, process::Command};

fn git(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let out = gix_testtools::git_command(root)
        .args(["-c", &format!("safe.directory={}", root.display())])
        .args(args)
        .output()?;
    ensure!(
        out.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn signatures(size: usize, reduced: bool) -> Result<()> {
    ensure!(
        signature::program_available("ssh-keygen"),
        "SSH qualification requires ssh-keygen"
    );
    let repo = TestRepo::new("memory-commit-signatures")?;
    let root = repo.path();
    let (_keys, key) = signature::ssh_private_key().map_err(anyhow::Error::from_boxed)?;
    let allowed = signature::fixture("ssh-allowed-signers");
    let message = root.join(".git/message");
    fs::write(&message, vec![b'x'; size])?;
    git(
        root,
        &["commit", "--allow-empty", "-F", message.to_str().expect("message path")],
    )?;
    let head = git(root, &["rev-parse", "HEAD"])?;
    let tree = git(root, &["rev-parse", "HEAD^{tree}"])?;
    let config = fs::read(root.join(".git/config"))?;
    let settings = [
        "gpg.format=ssh".to_owned(),
        format!("user.signingKey={}", key.display()),
        format!("gpg.ssh.allowedSignersFile={}", allowed.display()),
    ];
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("reduced trust qualification runs on Linux");
    }
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("1024"), Some("16777216"), Some("1073741824")] {
            let run = |args: &[&str]| -> Result<std::process::Output> {
                let mut command = Command::new(brit_bin().expect("built Brit"));
                gix_testtools::configure_git_environment(&mut command, root);
                command.current_dir(root);
                // Explicit operator configuration does not trust the reduced repository's config.
                for setting in &settings {
                    command.args(["-c", setting]);
                }
                if let Some(limit) = limit {
                    command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
                }
                Ok(command.args(args).output()?)
            };
            let unsigned = run(&["commit", "verify", "HEAD"])?;
            ensure!(
                !unsigned.status.success() && String::from_utf8_lossy(&unsigned.stderr).contains("is not signed"),
                "wrong unsigned refusal: {}",
                String::from_utf8_lossy(&unsigned.stderr)
            );
            if reduced && !packed && limit.is_none() {
                let trust = run(&["free", "trust", ".", ".git"])?;
                ensure!(trust.status.success());
                ensure!(
                    String::from_utf8(trust.stdout)?
                        .lines()
                        .filter(|line| line.starts_with("Reduced"))
                        .count()
                        == 2
                );
            }
            let signed = run(&["commit", "sign", "HEAD"])?;
            ensure!(signed.status.success(), "{}", String::from_utf8_lossy(&signed.stderr));
            let signed = String::from_utf8(signed.stdout)?;
            let signed = signed.trim();
            git(root, &["-c", &settings[2], "verify-commit", signed])?;
            let verified = run(&["commit", "verify", signed])?;
            ensure!(
                verified.status.success(),
                "{}",
                String::from_utf8_lossy(&verified.stderr)
            );
            let again = run(&["commit", "sign", signed])?;
            ensure!(again.status.success() && String::from_utf8(again.stdout)?.trim() == signed);
            ensure!(
                git(root, &["show", "-s", "--format=%B", signed])?
                    == git(root, &["show", "-s", "--format=%B", "HEAD"])?
            );
            ensure!(git(root, &["rev-parse", &format!("{signed}^{{tree}}")])? == tree);
            git(root, &["update-ref", "refs/tags/signed", signed])?;
            ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/config"))? == config);
        }
    }
    Ok(())
}

#[test]
fn commit_signatures_exceed_buffer_policy() -> Result<()> {
    signatures(128 * 1024, false)
}

#[test]
#[ignore = "large metadata: qualification runs this explicitly"]
fn commit_signatures_twenty_mib() -> Result<()> {
    signatures(20 * 1024 * 1024, false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn commit_signatures_twenty_mib_reduced_trust() -> Result<()> {
    signatures(20 * 1024 * 1024, true)
}
