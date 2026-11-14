//! Tree traversal must report output and object errors instead of silently succeeding.
use anyhow::{Result, ensure};
use std::io;

#[test]
fn recursive_and_direct_output_preserve_io_failure() -> Result<()> {
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
    let blob_id = repo.write_blob(b"body")?.detach();
    let tree_id = repo
        .write_object(&gix::objs::Tree {
            entries: (0..1000)
                .map(|n| gix::objs::tree::Entry {
                    mode: gix::objs::tree::EntryKind::Blob.into(),
                    filename: format!("entry-{n:04}").into(),
                    oid: blob_id,
                })
                .collect(),
        })?
        .detach();
    struct FailsAfter(usize);
    impl io::Write for FailsAfter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0 == 0 {
                return Err(io::Error::new(io::ErrorKind::StorageFull, "fixture output full"));
            }
            let n = self.0.min(bytes.len());
            self.0 -= n;
            Ok(n)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    for recursive in [false, true] {
        let mut complete = Vec::new();
        gitoxide_core::repository::tree::entries(
            gix::open(scratch.path())?,
            Some(&tree_id.to_string()),
            recursive,
            false,
            gitoxide_core::OutputFormat::Human,
            &mut complete,
        )?;
        for allowed in [0, 9000, complete.len() - 1] {
            let error = gitoxide_core::repository::tree::entries(
                gix::open(scratch.path())?,
                Some(&tree_id.to_string()),
                recursive,
                false,
                gitoxide_core::OutputFormat::Human,
                FailsAfter(allowed),
            )
            .expect_err("output exhaustion must not succeed");
            ensure!(
                error.iter_errors().any(|cause| cause
                    .downcast_ref::<io::Error>()
                    .is_some_and(|cause| cause.kind() == io::ErrorKind::StorageFull)),
                "{error:?}"
            );
        }
    }
    Ok(())
}
