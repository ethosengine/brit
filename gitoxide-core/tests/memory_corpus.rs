//! Default-feature corpus commands must record successful bounded tasks, not merely exit successfully.
#![cfg(feature = "corpus")]

use anyhow::{Context, Result, ensure};
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
        .env("GIT_AUTHOR_NAME", "Fixture")
        .env("GIT_COMMITTER_NAME", "Fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com")
        .env("GIT_AUTHOR_DATE", "2020-09-09T09:06:03+00:00")
        .env("GIT_COMMITTER_DATE", "2020-09-09T09:06:03+00:00")
        .output()?;
    ensure!(
        output.status.success(),
        "Git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output.stdout)
}

fn brit(root: &Path, args: &[&str]) -> Result<()> {
    let mut command =
        Command::new(std::env::var_os("BRIT_BIN").context("BRIT_BIN must name a default-feature Brit build")?);
    gix_testtools::configure_git_environment(&mut command, root);
    let output = command.current_dir(root).args(args).output()?;
    ensure!(
        output.status.success(),
        "Brit {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn corpus(reduced: bool) -> Result<()> {
    let fixture = tempfile::tempdir()?;
    let corpus = fixture.path().join("corpus");
    let root = corpus.join("repo");
    fs::create_dir_all(&root)?;
    git(&root, &["init", "-q", "--initial-branch=main"])?;
    let mut chunk = [b'x'; 65536];
    chunk[65535] = b'\n';
    let mut file = BufWriter::new(fs::File::create(root.join("large"))?);
    for _ in 0..1024 {
        file.write_all(&chunk)?;
    }
    file.flush()?;
    drop(file);
    let message = root.join(".git/message-input");
    let mut file = BufWriter::new(fs::File::create(&message)?);
    file.write_all(b"large corpus metadata\n\n")?;
    for _ in 0..320 {
        file.write_all(&chunk)?;
    }
    file.flush()?;
    drop(file);
    git(&root, &["add", "--", "large"])?;
    git(&root, &["commit", "-q", "-F", message.to_str().expect("fixture path")])?;
    let head = git(&root, &["rev-parse", "HEAD"])?;
    let blob = git(&root, &["rev-parse", "HEAD:large"])?;
    let index = fs::read(root.join(".git/index"))?;
    if reduced {
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::{MetadataExt, chown};
            ensure!(
                fs::metadata(&root)?.uid() == 0,
                "qualification requires chown authority"
            );
            chown(&root, Some(65534), None)?;
            chown(root.join(".git"), Some(65534), None)?;
            ensure!(
                gix::open_opts(&root, gix::open::Options::isolated())?.allocation_limit() == Some(16777216),
                "isolated corpus readers must actually have the reduced-trust allocation policy"
            );
        }
        #[cfg(not(target_os = "linux"))]
        anyhow::bail!("ownership qualification runs on Linux");
    }
    let limits: &[Option<&str>] = if reduced { &[None] } else { &[None, Some("16777216")] };
    for packed in [false, true] {
        if packed {
            git(&root, &["-c", "pack.threads=1", "repack", "-ad"])?;
        }
        for (n, limit) in limits.iter().enumerate() {
            if let Some(limit) = limit {
                git(&root, &["config", "gitoxide.objects.allocLimit", limit])?;
            }
            // Keep output ownership inside this fixture, rather than guarding an enclosing checkout.
            let db = root.join(".git").join(format!("corpus-{packed}-{n}.sqlite"));
            let mut args = vec![
                "--threads",
                "1",
                "corpus",
                "--db",
                db.to_str().expect("path"),
                "--path",
                corpus.to_str().expect("path"),
            ];
            args.push("refresh");
            brit(&root, &args)?;
            *args.last_mut().expect("subcommand") = "run";
            brit(&root, &args)?;
            let con = rusqlite::Connection::open(&db)?;
            let count: usize = con.query_row("SELECT count(*) FROM run", [], |row| row.get(0))?;
            ensure!(count == 4, "all four default corpus tasks must run, got {count}");
            let errors = con.prepare("SELECT task.short_name, run.error FROM run JOIN task ON task.id=run.task WHERE run.error IS NOT NULL OR run.duration IS NULL")?
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            ensure!(errors.is_empty(), "corpus task failures: {errors:?}");
            ensure!(git(&root, &["rev-parse", "HEAD"])? == head && fs::read(root.join(".git/index"))? == index);
            ensure!(git(&root, &["hash-object", "large"])? == blob);
            if limit.is_some() {
                git(&root, &["config", "--unset", "gitoxide.objects.allocLimit"])?;
            }
        }
    }
    Ok(())
}

#[test]
#[ignore = "resource qualification: requires BRIT_BIN built with default max features"]
fn sixty_four_mib_corpus_with_large_commit_metadata() -> Result<()> {
    corpus(false)
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "resource qualification: requires default-feature BRIT_BIN and chown authority"]
fn actual_reduced_trust_corpus() -> Result<()> {
    corpus(true)
}
