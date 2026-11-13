use super::*;
use gix_error::{OptionExt, ResultExt, message};
use gix_utils::stream::{self, Spool};
use std::io::Seek;

impl Pipeline {
    /// Preserve merge conversion and large-file decisions while streaming payloads.
    #[expect(
        clippy::too_many_arguments,
        reason = "Preserves pipeline inputs with an explicit operation storage policy"
    )]
    pub fn convert_to_mergeable_stream(
        &mut self,
        id: &gix_hash::oid,
        mode: EntryKind,
        rela_path: &BStr,
        kind: ResourceKind,
        attributes: &mut dyn FnMut(&BStr, &mut gix_filter::attributes::search::Outcome),
        objects: &(impl gix_object::FindObjectOrHeader + gix_object::FindStream),
        convert: Mode,
        storage: &crate::Bounded<'_>,
    ) -> Result<(Option<Data>, Option<Spool>)> {
        stream::check(storage.interrupt).or_error()?;
        if !matches!(mode, EntryKind::Blob | EntryKind::BlobExecutable) {
            bail!(gix_error::validation(format!(
                "Entry at '{rela_path}' must be regular file or symlink, but was {mode:?}"
            )));
        }
        let bytes = match self.roots.by_kind(kind) {
            Some(root) => {
                let path = root.join(gix_path::from_bstr(rela_path)?);
                if self.options.large_file_threshold_bytes > 0 {
                    let Some(size) = none_if_missing(path.metadata().map(|metadata| metadata.len())).or_error()? else {
                        return Ok((None, None));
                    };
                    if size > self.options.large_file_threshold_bytes {
                        return Ok((Some(Data::TooLarge { size }), None));
                    }
                }
                let Some(mut file) = none_if_missing(std::fs::File::open(&path))
                    .or_raise(|| message!("Could not read merge resource '{}'", path.display()))?
                else {
                    return Ok((None, None));
                };
                let mut bytes = storage.budget.spool().or_error()?;
                stream::copy(&mut file, &mut bytes, storage.interrupt).or_error()?;
                bytes.rewind().or_error()?;
                self.filter.convert_to_git_stream(
                    bytes,
                    rela_path,
                    attributes,
                    &mut || {
                        if convert == Mode::Renormalize || id.is_null() {
                            Ok(None)
                        } else {
                            objects
                                .find_stream(id, &storage.budget, storage.interrupt)
                                .map(|(_, bytes)| Some(bytes))
                        }
                    },
                    &storage.budget,
                    storage.interrupt,
                )?
            }
            None => {
                if id.is_null() {
                    return Ok((None, None));
                }
                let header = objects
                    .try_header(id)?
                    .ok_or_raise(|| message!("An object with id {id} could not be found").not_found())?;
                if self.options.large_file_threshold_bytes > 0 && header.size > self.options.large_file_threshold_bytes
                {
                    return Ok((Some(Data::TooLarge { size: header.size }), None));
                }
                // The buffered pipeline trusts the supplied entry mode and accepts
                // raw bytes from any object kind. Storage must not add a type restriction.
                let (_, mut bytes) = objects.find_stream(id, &storage.budget, storage.interrupt)?;
                if convert == Mode::Renormalize {
                    bytes = match self.filter.convert_to_worktree_stream(
                        bytes,
                        rela_path,
                        attributes,
                        to_worktree::Options {
                            can_delay: Delay::Forbid,
                            unknown_encoding: to_worktree::UnknownEncoding::Fail,
                        },
                        &storage.budget,
                        storage.interrupt,
                    )? {
                        gix_filter::pipeline::stream::Outcome::Ready(bytes) => bytes,
                        gix_filter::pipeline::stream::Outcome::Delayed(_) => unreachable!("delay forbidden"),
                    };
                    bytes = self.filter.convert_to_git_stream(
                        bytes,
                        rela_path,
                        attributes,
                        &mut || Ok(None),
                        &storage.budget,
                        storage.interrupt,
                    )?;
                }
                bytes
            }
        };
        Ok((Some(Data::Buffer), Some(bytes)))
    }
}
