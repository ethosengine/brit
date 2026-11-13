//! The existing conversion choices with operation-owned payload storage.
use super::*;
use gix_error::OptionExt;
use gix_object::FindStream;
use gix_utils::stream::{self, Budget, Spool};
use std::{
    io::Seek,
    sync::{Arc, atomic::AtomicBool},
};

impl Pipeline {
    /// Convert a resource using the same drivers, binary rules and filter order as
    /// `convert_to_diffable`, retaining payloads in charged operation scratch.
    #[expect(
        clippy::too_many_arguments,
        reason = "Preserves pipeline resource inputs with explicit operation quota and cancellation"
    )]
    pub fn convert_to_diffable_stream(
        &mut self,
        id: &gix_hash::oid,
        mode: EntryKind,
        rela_path: &BStr,
        kind: ResourceKind,
        attributes: &mut dyn FnMut(&BStr, &mut gix_filter::attributes::search::Outcome),
        objects: &dyn gix_object::FindObjectOrHeader,
        streams: &dyn FindStream,
        convert: Mode,
        budget: &Arc<Budget>,
        interrupt: &AtomicBool,
    ) -> Result<(Outcome, Option<Spool>)> {
        stream::check(interrupt).or_error()?;
        let is_symlink = match mode {
            EntryKind::Link => true,
            EntryKind::Blob | EntryKind::BlobExecutable => false,
            _ => bail!("Entry at {rela_path:?} must be regular file or symlink, but was {mode:?}".validation()),
        };
        attributes(rela_path, &mut self.attrs);
        let attr = self
            .attrs
            .iter_selected()
            .next()
            .expect("pre-initialized diff attribute");
        let driver_index = attr.assignment.state.as_bstr().and_then(|name| {
            self.drivers
                .binary_search_by(|driver| driver.name.as_bstr().cmp(name))
                .ok()
        });
        let driver = driver_index.map(|index| self.drivers[index].clone());
        let mut is_binary = if let Some(driver) = &driver {
            driver
                .is_binary
                .map(|value| value && driver.binary_to_text_command.is_none())
        } else {
            attr.assignment.state.is_unset().then_some(true)
        };
        let outcome = |data| Outcome { driver_index, data };
        let mut derived = false;
        let mut bytes = match self.roots.by_kind(kind) {
            Some(root) => {
                let path = root.join(gix_path::from_bstr(rela_path)?);
                if is_symlink {
                    if !self.options.fs.symlink {
                        bail!(
                            "Entry at {rela_path:?} is declared as symlink but symlinks are disabled via core.symlinks"
                                .validation()
                        );
                    }
                    let Some(target) = none_if_missing(std::fs::read_link(&path)).or_error()? else {
                        return Ok((outcome(None), None));
                    };
                    let mut bytes = budget.spool().or_error()?;
                    bytes.write_all(gix_path::into_bstr(target)?.as_ref()).or_error()?;
                    bytes.rewind().or_error()?;
                    return Ok((outcome(Some(Data::Buffer { is_derived: false })), Some(bytes)));
                }
                let need_size_only = is_binary == Some(true);
                let size = (need_size_only
                    || (is_binary != Some(false) && self.options.large_file_threshold_bytes > 0))
                    .then(|| none_if_missing(path.metadata().map(|metadata| metadata.len())).or_error())
                    .transpose()?;
                match size {
                    Some(None) => return Ok((outcome(None), None)),
                    Some(Some(size)) if size > self.options.large_file_threshold_bytes || need_size_only => {
                        return Ok((outcome(Some(Data::Binary { size })), None));
                    }
                    _ => {}
                }
                let command = driver
                    .as_ref()
                    .filter(|_| convert.to_worktree())
                    .map(|driver| driver.prepare_binary_to_text_cmd(&path))
                    .transpose()?
                    .flatten();
                if let Some(mut command) = command {
                    if self.options.large_file_threshold_bytes == 0
                        && none_if_missing(std::fs::symlink_metadata(&path)).or_error()?.is_none()
                    {
                        return Ok((outcome(None), None));
                    }
                    derived = true;
                    run_command(rela_path, &mut command, budget, interrupt)?
                } else {
                    let Some(mut file) = none_if_missing(std::fs::File::open(&path)).or_error()? else {
                        return Ok((outcome(None), None));
                    };
                    let mut bytes = budget.spool().or_error()?;
                    stream::copy(&mut file, &mut bytes, interrupt).or_error()?;
                    bytes.rewind().or_error()?;
                    if convert.to_git() {
                        bytes = self.worktree_filter.convert_to_git_stream(
                            bytes,
                            rela_path,
                            attributes,
                            &mut || {
                                if id.is_null() {
                                    Ok(None)
                                } else {
                                    streams.find_stream(id, budget, interrupt).map(|(_, bytes)| Some(bytes))
                                }
                            },
                            budget,
                            interrupt,
                        )?;
                    }
                    bytes
                }
            }
            None => {
                if id.is_null() {
                    return Ok((outcome(None), None));
                }
                let header = objects
                    .try_header(id)?
                    .ok_or_raise(|| message!("Object {id} is missing").not_found())?;
                if is_binary.is_none()
                    && self.options.large_file_threshold_bytes > 0
                    && header.size > self.options.large_file_threshold_bytes
                {
                    is_binary = Some(true);
                }
                if is_binary == Some(true) {
                    return Ok((outcome(Some(Data::Binary { size: header.size })), None));
                }
                let (_, mut bytes) = streams.find_stream(id, budget, interrupt)?;
                let textconv = driver
                    .as_ref()
                    .is_some_and(|driver| driver.binary_to_text_command.is_some());
                if (matches!(mode, EntryKind::Blob | EntryKind::BlobExecutable)
                    && convert == Mode::ToWorktreeAndBinaryToText)
                    || (convert == Mode::ToGitUnlessBinaryToTextIsPresent && textconv)
                {
                    bytes = match self.worktree_filter.convert_to_worktree_stream(
                        bytes,
                        rela_path,
                        attributes,
                        to_worktree::Options {
                            can_delay: Delay::Forbid,
                            unknown_encoding: to_worktree::UnknownEncoding::Fail,
                        },
                        budget,
                        interrupt,
                    )? {
                        gix_filter::pipeline::stream::Outcome::Ready(bytes) => bytes,
                        gix_filter::pipeline::stream::Outcome::Delayed(_) => unreachable!("delay forbidden"),
                    };
                    if textconv {
                        let input = bytes.into_named_input();
                        let mut command = driver
                            .as_ref()
                            .expect("textconv driver")
                            .prepare_binary_to_text_cmd(input.path())?
                            .expect("textconv command");
                        bytes = run_command(rela_path, &mut command, budget, interrupt)?;
                        derived = true;
                    }
                }
                // The buffered pipeline suppresses binary detection for a configured
                // textconv even in ToGit mode, where no conversion is executed.
                if textconv {
                    is_binary = Some(false);
                }
                bytes
            }
        };
        let mut prefix = [0u8; 8000];
        let mut read = 0;
        while read < prefix.len() {
            let count = bytes.read(&mut prefix[read..]).or_error()?;
            if count == 0 {
                break;
            }
            read += count;
        }
        bytes.rewind().or_error()?;
        if !derived && is_binary.unwrap_or_else(|| is_binary_buf(&prefix[..read])) {
            Ok((outcome(Some(Data::Binary { size: bytes.len() })), None))
        } else {
            Ok((outcome(Some(Data::Buffer { is_derived: derived })), Some(bytes)))
        }
    }
}

fn run_command(path: &BStr, command: &mut Command, budget: &Arc<Budget>, interrupt: &AtomicBool) -> Result<Spool> {
    let output = stream::command_output(command, budget, interrupt)
        .or_raise(|| message!("Binary-to-text conversion for {path:?} failed").with_program(command.get_program()))?;
    if !output.status.success() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let lengths = (output.stdout.len(), output.stderr.len());
        output
            .stdout
            .take(stream::CHUNK as u64)
            .read_to_end(&mut stdout)
            .or_error()?;
        output
            .stderr
            .take(stream::CHUNK as u64)
            .read_to_end(&mut stderr)
            .or_error()?;
        bail!(message!("Binary-to-text conversion for {path:?} failed; output sizes {lengths:?}, diagnostic prefixes limited to {} bytes", stream::CHUNK)
            .with_command_output(command, std::process::Output { status: output.status, stdout, stderr }));
    }
    Ok(output.stdout)
}
