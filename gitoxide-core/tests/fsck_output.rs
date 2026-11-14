//! Connectivity output failures retain their I/O cause instead of panicking.
use anyhow::{Result, ensure};
use std::io;

#[test]
fn absent_empty_tree_is_reported() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let scratch = tempfile::tempdir()?;
    ensure!(
        gix_testtools::git_command(scratch.path())
            .args(["init", "-q"])
            .status()?
            .success()
    );
    let repo = gix::open(scratch.path())?;
    let tree = gix::ObjectId::empty_tree(repo.object_hash());
    let signature = gix::actor::SignatureRef {
        name: "Fixture".into(),
        email: "fixture@example.com".into(),
        time: "1600000000 +0000",
    };
    let commit = repo
        .commit_as(
            signature,
            signature,
            "refs/heads/main",
            "absent tree",
            tree,
            [] as [gix::ObjectId; 0],
        )?
        .detach();
    for limit in [1, 1024 * 1024 * 1024] {
        let repo = gix::open_opts(
            scratch.path(),
            gix::open::Options::isolated().cli_overrides([format!("gitoxide.objects.allocLimit={limit}")]),
        )?;
        let mut output = Vec::new();
        gitoxide_core::repository::fsck(repo, Some(commit.to_string()), &mut output)?;
        ensure!(
            output == format!("{tree}: tree\n").as_bytes(),
            "missing empty tree was hidden: {output:?}"
        );
    }
    Ok(())
}

#[test]
fn missing_object_output_preserves_io_failure() -> Result<()> {
    if gix_testtools::run_in_isolated_process().map_err(anyhow::Error::from_boxed)? {
        return Ok(());
    }
    let scratch = tempfile::tempdir()?;
    ensure!(
        gix_testtools::git_command(scratch.path())
            .args(["init", "-q"])
            .status()?
            .success()
    );
    let repo = gix::open(scratch.path())?;
    let missing = gix::ObjectId::null(repo.object_hash());
    let tree = repo
        .write_object(&gix::objs::Tree {
            entries: vec![gix::objs::tree::Entry {
                mode: gix::objs::tree::EntryKind::Blob.into(),
                filename: "missing".into(),
                oid: missing,
            }],
        })?
        .detach();
    let signature = gix::actor::SignatureRef {
        name: "Fixture".into(),
        email: "fixture@example.com".into(),
        time: "1600000000 +0000",
    };
    let commit = repo
        .commit_as(
            signature,
            signature,
            "refs/heads/main",
            "missing blob",
            tree,
            [] as [gix::ObjectId; 0],
        )?
        .detach();
    struct Fails;
    impl io::Write for Fails {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::StorageFull, "fixture output full"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for limit in [1, 1024 * 1024 * 1024] {
        let open = || {
            gix::open_opts(
                scratch.path(),
                gix::open::Options::isolated().cli_overrides([format!("gitoxide.objects.allocLimit={limit}")]),
            )
        };
        let mut output = Vec::new();
        gitoxide_core::repository::fsck(open()?, Some(commit.to_string()), &mut output)?;
        ensure!(output == format!("{missing}: blob\n").as_bytes());
        let error = gitoxide_core::repository::fsck(open()?, Some(commit.to_string()), Fails)
            .expect_err("output exhaustion must refuse");
        ensure!(
            error.iter_errors().any(|cause| cause
                .downcast_ref::<io::Error>()
                .is_some_and(|cause| cause.kind() == io::ErrorKind::StorageFull)),
            "{error:?}"
        );
    }
    Ok(())
}
