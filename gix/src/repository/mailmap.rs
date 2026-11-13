use crate::error::{ErrorExt, ResultExt, message};
use crate::{Error, Id, Result, bstr::ByteSlice, config::tree::Mailmap};
use gix_fs::FileOrSymlink;
use gix_object::FindStream;

impl crate::Repository {
    /// Similar to [`open_mailmap_into()`][crate::Repository::open_mailmap_into()], but ignores all errors and returns at worst
    /// an empty mailmap, e.g. if there is no mailmap or if there were errors loading them.
    ///
    /// This represents typical usage within git, which also works with what's there without considering a populated mailmap
    /// a reason to abort an operation, considering it optional.
    pub fn open_mailmap(&self) -> gix_mailmap::Snapshot {
        let mut out = gix_mailmap::Snapshot::default();
        self.open_mailmap_into(&mut out).ok();
        out
    }

    /// Try to merge mailmaps from the following locations into `target`:
    ///
    /// - read the `.mailmap` file without following symlinks from the working tree, if present;
    ///   symlinks are reported as errors without reading their targets
    /// - OR read `HEAD:.mailmap` if this repository is bare (i.e. has no working tree), if the `mailmap.blob` is not set.
    /// - read the mailmap as configured in `mailmap.blob`, if set.
    /// - read the file as configured by `mailmap.file`, following symlinks, if set.
    ///
    /// Only the first error will be reported, and as many source mailmaps will be merged into `target` as possible.
    /// Parsing errors will be ignored.
    pub fn open_mailmap_into(&self, target: &mut gix_mailmap::Snapshot) -> Result<()> {
        let mut err = None::<Error>;
        let mut blob_id = self.config.resolved.string(Mailmap::BLOB).and_then(|spec| {
            self.rev_parse_single(spec.as_bstr())
                .map_err(|e| err.get_or_insert(e.and_raise(message("The configured mailmap.blob could not be parsed"))))
                .map(Id::detach)
                .ok()
        });
        match self.workdir() {
            None => {
                blob_id = blob_id.or_else(|| {
                    self.head().ok().and_then(|mut head| {
                        let commit = head.peel_to_commit().ok()?;
                        let tree = commit.tree().ok()?;
                        tree.find_entry(".mailmap").map(|e| e.object_id())
                    })
                });
            }
            Some(root) => match gix_fs::open_read_only_no_follow(&root.join(".mailmap")) {
                Ok(FileOrSymlink::File(mut file)) => {
                    let mut buf = Vec::new();
                    std::io::copy(&mut file, &mut buf)
                        .map_err(|e| {
                            err.get_or_insert(e.and_raise(message("The worktree .mailmap file could not be read")))
                        })
                        .ok();
                    target.merge(gix_mailmap::parse_ignore_errors(&buf));
                }
                Ok(FileOrSymlink::Symlink) => {
                    err.get_or_insert(message("The worktree .mailmap file is a symlink").raise());
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    err.get_or_insert(e.and_raise(message("The worktree .mailmap file could not be read")));
                }
            },
        }

        if let Some(id) = blob_id {
            let result = (|| -> Result<()> {
                let budget = gix_utils::stream::Budget::new(&std::env::temp_dir()).or_error()?;
                let (_, spool) = self.find_stream(&id, &budget, &crate::interrupt::IS_INTERRUPTED)?;
                let data = spool.into_mapped().or_error()?;
                target.merge(gix_mailmap::parse_ignore_errors(data.as_slice()));
                Ok(())
            })();
            if let Err(e) = result {
                err.get_or_insert(e.and_raise(message!(
                    "Could not read mailmap object {id} in {}",
                    self.git_dir().display()
                )));
            }
        }

        let configured_path = self
            .config_snapshot()
            .trusted_path(Mailmap::FILE)
            .map_err(|e| err.get_or_insert(Error::from_error(e)))
            .ok()
            .flatten();

        if let Some(mut file) = configured_path.and_then(|path| {
            std::fs::File::open(path)
                .map_err(|e| {
                    err.get_or_insert(
                        e.and_raise(message("The mailmap file declared in `mailmap.file` could not be read")),
                    )
                })
                .ok()
        }) {
            let mut buf = Vec::new();
            std::io::copy(&mut file, &mut buf)
                .map_err(|e| {
                    err.get_or_insert(
                        e.and_raise(message("The mailmap file declared in `mailmap.file` could not be read")),
                    )
                })
                .ok();
            target.merge(gix_mailmap::parse_ignore_errors(&buf));
        }

        err.map_or(Ok(()), Err)
    }
}
