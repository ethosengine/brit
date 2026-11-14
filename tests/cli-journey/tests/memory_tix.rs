//! Tix owns separate history readers; ordinary log qualification does not cover them.
use anyhow::{ensure, Result};
use cli_journey::support::{runner::brit_bin, test_repo::TestRepo};
use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
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

fn run(root: &Path, args: &[&str], limit: Option<&str>, output: &Path) -> Result<()> {
    let mut command = Command::new(brit_bin().expect("built Brit"));
    gix_testtools::configure_git_environment(&mut command, root);
    command.current_dir(root);
    if let Some(limit) = limit {
        command.args(["-c", &format!("gitoxide.objects.allocLimit={limit}")]);
    }
    eprintln!("qualifying Tix {args:?}, policy {limit:?}");
    let result = command
        .args(args)
        .stdout(Stdio::from(fs::File::create(output)?))
        .output()?;
    ensure!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    Ok(())
}

fn history(reduced: bool) -> Result<()> {
    let repo = TestRepo::new("memory-tix-history")?;
    let root = repo.path();
    let hidden = String::from_utf8(git(root, &["rev-parse", "HEAD"])?)?;
    let message = root.join(".git/message-input");
    let mut file = fs::File::create(&message)?;
    file.write_all(b"large governance history\n\n")?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    for _ in 0..320 {
        file.write_all(&chunk)?;
    }
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
    let head = git(root, &["rev-parse", "HEAD"])?;
    let index = fs::read(root.join(".git/index"))?;
    let setup_output = root.join(".git/setup-tix");
    for args in [
        ["tix", "enrich", "commit", "todo", "HEAD"],
        ["tix", "enrich", "tree", "checks-pass", "HEAD"],
    ] {
        run(root, &args, Some("1073741824"), &setup_output)?;
    }
    let refs = git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])?;
    let queries = [
        vec!["tix", "show", "--no-auto-hide", "--hide", hidden.trim(), "HEAD"],
        vec!["tix", "ref-tree", "--no-auto-hide", "--hide", hidden.trim(), "HEAD"],
        vec!["tix", "worktrunk", "show"],
        vec!["tix", "op", "log"],
    ];
    let mut expected = Vec::new();
    for args in &queries {
        let output = root.join(".git/expected-tix");
        run(root, args, Some("1073741824"), &output)?;
        expected.push(fs::read(output)?);
    }
    ensure!(String::from_utf8_lossy(&expected[0]).contains("large governance history"));
    ensure!(String::from_utf8_lossy(&expected[0]).contains("🚧"));
    ensure!(String::from_utf8_lossy(&expected[0]).contains("✔"));
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{chown, MetadataExt};
            ensure!(fs::metadata(root)?.uid() == 0, "qualification requires chown authority");
            chown(root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            let trust = root.join(".git/trust");
            run(root, &["free", "trust", ".", ".git"], None, &trust)?;
            ensure!(
                fs::read_to_string(trust)?
                    .lines()
                    .filter(|line| line.starts_with("Reduced"))
                    .count()
                    == 2
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("actual ownership qualification requires Linux");
    }
    for packed in [false, true] {
        if packed {
            git(root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for limit in [None, Some("16777216")] {
            for (args, expected) in queries.iter().zip(&expected) {
                let actual = root.join(".git/actual-tix");
                run(root, args, limit, &actual)?;
                ensure!(
                    fs::read(actual)? == *expected,
                    "allocation policy must not alter the displayed history"
                );
                ensure!(git(root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
                ensure!(git(root, &["for-each-ref", "--format=%(refname) %(objectname)"])? == refs);
            }
        }
    }
    Ok(())
}

#[test]
fn tix_history_with_twenty_mib_commit_metadata() -> Result<()> {
    history(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires chown authority: qualification runs this explicitly"]
fn tix_history_with_actual_reduced_trust() -> Result<()> {
    history(true)
}
