use super::entry;
use bstr::{BStr, BString};
use gix_error::{ErrorExt, Result, ResultExt, bail, corruption, message};
use gix_filter::pipeline::{convert::to_worktree, stream::Outcome};
use gix_utils::stream::{self, Budget, Spool};
use std::{
    collections::BTreeMap,
    io::{Read, Seek},
    path::Path,
    sync::{Arc, atomic::AtomicBool},
};

/// Check out into an empty destination, serializing blob work to bound peak live storage.
/// All temporary content uses `budget`; only completed paths release their reservation.
pub fn checkout<Find: gix_object::Find + gix_object::FindStream>(
    index: &mut gix_index::State,
    directory: &Path,
    objects: Find,
    options: super::Options,
    budget: &Arc<Budget>,
    interrupt: &AtomicBool,
) -> Result<super::Outcome> {
    let paths = index.take_path_backing();
    let result = checkout_inner(index, &paths, directory, objects, options, budget, interrupt);
    index.return_path_backing(paths);
    result
}
fn checkout_inner<Find: gix_object::Find + gix_object::FindStream>(
    index: &mut gix_index::State,
    paths: &gix_index::PathStorage,
    directory: &Path,
    objects: Find,
    mut options: super::Options,
    budget: &Arc<Budget>,
    interrupt: &AtomicBool,
) -> Result<super::Outcome> {
    let mut cache = gix_worktree::Stack::from_state_and_ignore_case(
        directory.to_owned(),
        options.fs.ignore_case,
        gix_worktree::stack::State::for_checkout(false, options.validate, std::mem::take(&mut options.attributes)),
        index,
        paths,
    );
    let mut delayed = BTreeMap::<(gix_filter::driver::Key, BString), usize>::new();
    let mut symlinks = Vec::new();
    let mut outcome = super::Outcome::default();
    for (position, (entry, path)) in index.entries_mut_with_paths_in(paths).enumerate() {
        stream::check(interrupt).or_error()?;
        if entry.flags.contains(gix_index::entry::Flags::SKIP_WORKTREE) {
            continue;
        }
        if entry.mode == gix_index::entry::Mode::SYMLINK {
            symlinks.push(position);
            continue;
        }
        let platform = cache
            .at_path(gix_path::from_bstr(path)?.as_ref(), Some(entry.mode), &objects)
            .or_error()?;
        if !matches!(
            entry.mode,
            gix_index::entry::Mode::FILE | gix_index::entry::Mode::FILE_EXECUTABLE
        ) {
            continue;
        }
        let (kind, input) = objects
            .find_stream(&entry.id, budget, interrupt)
            .or_raise(|| message!("Could not check out {path:?} ({})", entry.id))?;
        if kind != gix_object::Kind::Blob {
            bail!(corruption("Checkout entry is not a blob"));
        }
        let converted = options.filters.convert_to_worktree_stream(
            input,
            path,
            &mut |_, attrs| {
                platform.matching_attributes(attrs);
            },
            to_worktree::Options {
                can_delay: options.filter_process_delay,
                unknown_encoding: to_worktree::UnknownEncoding::Ignore,
            },
            budget,
            interrupt,
        )?;
        match converted {
            Outcome::Ready(spool) => {
                outcome.bytes_written += publish(spool, platform.path(), entry, options.fs.executable_bit)?;
                outcome.files_updated += 1;
            }
            Outcome::Delayed(key) => {
                delayed.insert((key, path.to_owned()), position);
            }
        }
    }
    let keys: std::collections::BTreeSet<_> = delayed.keys().map(|(key, _)| key.clone()).collect();
    for key in keys {
        loop {
            stream::check(interrupt).or_error()?;
            let paths_ready = options.filters.driver_state_mut().list_delayed_paths(&key)?;
            if paths_ready.is_empty() {
                break;
            }
            for path in paths_ready {
                let position = delayed
                    .remove(&(key.clone(), path.clone()))
                    .ok_or_else(|| corruption("Filter returned an unknown or duplicate delayed path").raise())?;
                let entry = &mut index.entries_mut()[position];
                let platform = cache
                    .at_path(
                        gix_path::from_bstr(BStr::new(&path))?.as_ref(),
                        Some(entry.mode),
                        &objects,
                    )
                    .or_error()?;
                let mut reader = options.filters.driver_state_mut().fetch_delayed(
                    &key,
                    path.as_ref(),
                    gix_filter::driver::Operation::Smudge,
                )?;
                let mut out = budget.spool().or_error()?;
                stream::copy(&mut reader, &mut out, interrupt).or_error()?;
                outcome.bytes_written += publish(out, platform.path(), entry, options.fs.executable_bit)?;
                outcome.files_updated += 1;
            }
        }
    }
    if !delayed.is_empty() {
        bail!(corruption("Filter left delayed checkout paths unprocessed"));
    }
    for position in symlinks {
        stream::check(interrupt).or_error()?;
        let entry = &mut index.entries_mut()[position];
        let path = entry.path_in(paths);
        let platform = cache
            .at_path(gix_path::from_bstr(path)?.as_ref(), Some(entry.mode), &objects)
            .or_error()?;
        let (kind, mut input) = objects
            .find_stream(&entry.id, budget, interrupt)
            .or_raise(|| message!("Could not check out {path:?} ({})", entry.id))?;
        if kind != gix_object::Kind::Blob {
            bail!(corruption("Symlink entry is not a blob"));
        }
        let size = input.len();
        if options.fs.symlink {
            if size > stream::CHUNK as u64 {
                bail!(corruption("Symlink target exceeds filesystem path capacity"));
            }
            let mut target = Vec::new();
            input.read_to_end(&mut target).or_error()?;
            let target = gix_path::to_native_path_on_windows(BStr::new(&target))?;
            gix_fs::symlink::create(target.as_ref(), platform.path()).or_error()?;
            entry.stat = gix_index::entry::Stat::from_fs(
                &gix_index::fs::Metadata::from_path_no_follow(platform.path()).or_error()?,
            )
            .or_error()?;
        } else {
            publish(input, platform.path(), entry, false)?;
        }
        outcome.bytes_written += size;
        outcome.files_updated += 1;
    }
    stream::check(interrupt).or_error()?;
    options
        .filters
        .driver_state_mut()
        .shutdown(gix_filter::driver::shutdown::Mode::WaitForProcesses)
        .or_error()?;
    Ok(outcome)
}
fn publish(mut spool: Spool, path: &Path, entry: &mut gix_index::Entry, executable: bool) -> Result<u64> {
    let size = spool.len();
    spool.rewind().or_error()?;
    let file = spool.publish(path).or_error()?;
    let change = if executable && entry.mode == gix_index::entry::Mode::FILE_EXECUTABLE {
        entry::ExecutableBitChange::Set
    } else {
        entry::ExecutableBitChange::NoChange
    };
    entry::finalize_entry(entry, file, size, change)?;
    Ok(size)
}
