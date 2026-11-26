use std::{
    collections::BTreeMap,
    io::{self, Write},
    path::Path,
};

use gix::{
    ObjectId, Result,
    bstr::{BStr, BString, ByteSlice, ByteVec},
    diff::index::Change as Staged,
    error::{OptionExt, ResultExt, message},
    index::entry::{Mode, mode::Change as ModeChange},
    status::{Item, index_worktree},
};
use gix_status::index_as_worktree::{Change, EntryStatus};

use super::super::diff::{quote_patch_path, quote_path};

#[derive(Clone, Copy)]
struct File {
    mode: Mode,
    id: ObjectId,
}

#[derive(Default)]
struct Tracked {
    staged: u8,
    unstaged: u8,
    head: Option<File>,
    index: Option<File>,
    worktree_mode: Option<Mode>,
    submodule: Option<[u8; 4]>,
    rename: Option<Rename>,
    conflict: Option<Conflict>,
}

struct Rename {
    source: BString,
    kind: u8,
    score: u8,
}

struct Conflict {
    code: &'static str,
    stages: [Option<File>; 3],
}

pub(super) struct Status {
    v2: bool,
    tracked: BTreeMap<BString, Tracked>,
    untracked: Vec<BString>,
    ignored: Vec<BString>,
    worktree_renames: Vec<(BString, Rename, Mode)>,
}

impl Status {
    pub(super) fn new(v2: bool) -> Self {
        Self {
            v2,
            tracked: BTreeMap::new(),
            untracked: Vec::new(),
            ignored: Vec::new(),
            worktree_renames: Vec::new(),
        }
    }

    pub(super) fn record(&mut self, repo: &gix::Repository, item: Item) -> Result<()> {
        use index_worktree::Item as Worktree;
        match item {
            Item::TreeIndex(change) => self.staged(repo, change)?,
            Item::IndexWorktree(Worktree::Modification {
                entry,
                rela_path,
                status,
                ..
            }) => {
                let tracked = self.tracked.entry(rela_path).or_default();
                match status {
                    EntryStatus::NeedsUpdate(_) => {}
                    EntryStatus::IntentToAdd => {
                        tracked.unstaged = b'A';
                        tracked.worktree_mode = Some(entry.mode);
                    }
                    EntryStatus::Conflict { summary, entries } => {
                        tracked.conflict = Some(Conflict {
                            code: summary.as_str(),
                            stages: entries.map(|entry| {
                                entry.map(|entry| File {
                                    mode: entry.mode,
                                    id: entry.id,
                                })
                            }),
                        });
                    }
                    EntryStatus::Change(change) => {
                        let index = File {
                            mode: entry.mode,
                            id: entry.id,
                        };
                        let (code, mode) = match change {
                            Change::Removed => (b'D', Mode::empty()),
                            Change::Type { worktree_mode } => (b'T', worktree_mode),
                            Change::Modification {
                                executable_bit_changed, ..
                            } => {
                                let mode = if executable_bit_changed {
                                    ModeChange::ExecutableBit.apply(entry.mode)
                                } else {
                                    entry.mode
                                };
                                (b'M', mode)
                            }
                            Change::SubmoduleModification(status) => {
                                tracked.submodule = Some(submodule_flags(&status));
                                (b'M', entry.mode)
                            }
                        };
                        tracked.unstaged = code;
                        tracked.worktree_mode = Some(mode);
                        // An unstaged-only change has the same entry in HEAD and the index.
                        tracked.index = Some(index);
                    }
                }
            }
            Item::IndexWorktree(Worktree::DirectoryContents {
                mut entry,
                collapsed_directory_status: None,
            }) => {
                use gix::dir::entry::Status as DirectoryStatus;
                let paths = match entry.status {
                    DirectoryStatus::Untracked => &mut self.untracked,
                    DirectoryStatus::Ignored(_) => &mut self.ignored,
                    DirectoryStatus::Tracked | DirectoryStatus::Pruned => return Ok(()),
                };
                if entry.disk_kind.is_some_and(|kind| kind.is_dir()) {
                    entry.rela_path.push_byte(b'/');
                }
                paths.push(entry.rela_path);
            }
            Item::IndexWorktree(Worktree::Rewrite {
                source,
                dirwalk_entry,
                similarity,
                copy,
                ..
            }) => {
                let path = dirwalk_entry.rela_path;
                let mode = disk_mode(repo, path.as_bstr(), None)?;
                self.worktree_renames.push((
                    path,
                    Rename {
                        source: source.rela_path().into(),
                        kind: if copy { b'C' } else { b'R' },
                        score: similarity.map_or(100, |score| score.percentage()),
                    },
                    mode,
                ));
            }
            Item::IndexWorktree(Worktree::DirectoryContents { .. }) => {}
        }
        Ok(())
    }

    fn staged(&mut self, repo: &gix::Repository, change: Staged) -> Result<()> {
        let tracked = self.tracked.entry(change.location().into()).or_default();
        let file = |mode, id: &gix::hash::oid| {
            Some(File {
                mode,
                id: id.to_owned(),
            })
        };
        tracked.staged = match change {
            Staged::Addition { entry_mode, id, .. } => {
                tracked.index = file(entry_mode, &id);
                b'A'
            }
            Staged::Deletion { entry_mode, id, .. } => {
                tracked.head = file(entry_mode, &id);
                b'D'
            }
            Staged::Modification {
                previous_entry_mode,
                previous_id,
                entry_mode,
                id,
                ..
            } => {
                tracked.head = file(previous_entry_mode, &previous_id);
                tracked.index = file(entry_mode, &id);
                if previous_entry_mode.bits() & 0o170000 == entry_mode.bits() & 0o170000 {
                    b'M'
                } else {
                    b'T'
                }
            }
            Staged::Rewrite {
                source_location,
                source_index,
                source_entry_mode,
                source_id,
                location,
                index,
                entry_mode,
                id,
                copy,
            } => {
                tracked.head = file(source_entry_mode, &source_id);
                tracked.index = file(entry_mode, &id);
                let kind = if copy { b'C' } else { b'R' };
                let score = if self.v2 && source_id != id {
                    // The tree-index iterator gives us the selected pair but discards its score.
                    staged_similarity(
                        repo,
                        Staged::Deletion {
                            location: source_location.clone(),
                            index: source_index,
                            entry_mode: source_entry_mode,
                            id: source_id,
                        },
                        Staged::Addition {
                            location,
                            index,
                            entry_mode,
                            id,
                        },
                    )?
                } else {
                    100
                };
                tracked.rename = Some(Rename {
                    source: source_location.into_owned(),
                    kind,
                    score,
                });
                kind
            }
        };
        Ok(())
    }

    pub(super) fn write(
        mut self,
        repo: &gix::Repository,
        index: &gix::index::State,
        out: &mut dyn Write,
        nul: bool,
        branch: bool,
        ahead_behind: bool,
    ) -> Result<()> {
        if !self.worktree_renames.is_empty() {
            for (path, rename, mode) in self.worktree_renames {
                use gix::index::entry::Stage;
                let mut tracked = self.tracked.remove(&rename.source).unwrap_or_default();
                if let Some(entry) = index.entry_by_path_and_stage(rename.source.as_bstr(), Stage::Unconflicted) {
                    tracked.index = Some(File {
                        mode: entry.mode,
                        id: entry.id,
                    });
                }
                tracked.unstaged = rename.kind;
                tracked.worktree_mode = Some(mode);
                if tracked.rename.is_none() {
                    tracked.rename = Some(rename);
                }
                self.tracked.insert(path, tracked);
            }
        }
        let format = Format {
            v2: self.v2,
            end: if nul { b'\0' } else { b'\n' },
            quote_high: quote_path(repo)?,
            prefix: if self.v2
                && !nul
                && repo
                    .config_snapshot()
                    .try_boolean("status.relativePaths")?
                    .unwrap_or(true)
            {
                repo.prefix()?
            } else {
                None
            },
        };
        if branch {
            format.branch(repo, out, ahead_behind)?;
        }
        let null = ObjectId::null(repo.object_hash());
        for (path, tracked) in &self.tracked {
            if let Some(conflict) = &tracked.conflict {
                if !self.v2 {
                    format.conflict(repo, out, path.as_bstr(), conflict)?;
                }
            } else if tracked.staged != 0 || tracked.unstaged != 0 {
                format.tracked(out, path.as_bstr(), tracked, null)?;
            }
        }
        if self.v2 {
            for (path, tracked) in &self.tracked {
                if let Some(conflict) = &tracked.conflict {
                    format.conflict(repo, out, path.as_bstr(), conflict)?;
                }
            }
        }
        for (kind, mut paths) in [(b'?', self.untracked), (b'!', self.ignored)] {
            paths.sort_unstable();
            for path in paths {
                if !self.v2 {
                    out.write_all(&[kind]).or_error()?;
                }
                out.write_all(&[kind, b' ']).or_error()?;
                format.path(out, &path)?;
                out.write_all(&[format.end]).or_error()?;
            }
        }
        Ok(())
    }
}

fn submodule_flags(status: &gix::submodule::Status) -> [u8; 4] {
    let mut flags = *b"S...";
    if status.checked_out_head_id != status.index_id {
        flags[1] = b'C';
    }
    for change in status.changes.iter().flatten() {
        match change {
            Item::IndexWorktree(index_worktree::Item::DirectoryContents { entry, .. }) => {
                if entry.status == gix::dir::entry::Status::Untracked {
                    flags[3] = b'U';
                }
            }
            Item::IndexWorktree(index_worktree::Item::Modification {
                status: EntryStatus::NeedsUpdate(_),
                ..
            }) => {}
            Item::IndexWorktree(index_worktree::Item::Modification {
                status: EntryStatus::Change(Change::SubmoduleModification(child)),
                ..
            }) => {
                let [_, commit, modified, untracked] = submodule_flags(child);
                if commit == b'C' || modified == b'M' {
                    flags[2] = b'M';
                }
                if untracked == b'U' {
                    flags[3] = b'U';
                }
            }
            _ => flags[2] = b'M',
        }
    }
    flags
}

fn disk_mode(repo: &gix::Repository, path: &BStr, index_mode: Option<Mode>) -> Result<Mode> {
    let workdir = repo
        .workdir()
        .ok_or_raise(|| message("Status requires a working tree"))?;
    let path = workdir.join(gix::path::from_bstr(path)?);
    let metadata = match gix::index::fs::Metadata::from_path_no_follow(&path) {
        Ok(metadata) => metadata,
        Err(err) if matches!(err.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory) => {
            return Ok(Mode::empty());
        }
        Err(err) => return Err(err).or_error(),
    };
    let config = repo.config_snapshot();
    let filemode = config.try_boolean("core.filemode")?.unwrap_or(true);
    let symlinks = config.try_boolean("core.symlinks")?.unwrap_or(true);
    if metadata.is_symlink() {
        Ok(Mode::SYMLINK)
    } else if metadata.is_dir() && index_mode == Some(Mode::COMMIT) {
        Ok(Mode::COMMIT)
    } else if metadata.is_file() {
        Ok(match index_mode {
            Some(Mode::SYMLINK) if !symlinks => Mode::SYMLINK,
            Some(Mode::FILE_EXECUTABLE) if !filemode => Mode::FILE_EXECUTABLE,
            _ if filemode && metadata.is_executable() => Mode::FILE_EXECUTABLE,
            _ => Mode::FILE,
        })
    } else {
        Ok(Mode::empty())
    }
}

fn staged_similarity(repo: &gix::Repository, old: Staged, new: Staged) -> Result<u8> {
    use gix::diff::rewrites::{Tracker, tracker::Context};
    let mut tracker = Tracker::new(gix::diff::Rewrites {
        percentage: Some(0.0),
        ..Default::default()
    });
    let old_path = BString::from(old.location());
    let new_path = BString::from(new.location());
    tracker.try_push_change(old, old_path.as_bstr());
    tracker.try_push_change(new, new_path.as_bstr());
    let mut score = None;
    tracker.emit(
        |_, source| {
            if let Some(source) = source {
                score = source.similarity.map(|similarity| similarity.percentage());
            }
            std::ops::ControlFlow::Continue(())
        },
        &mut repo.diff_resource_cache_for_tree_diff()?,
        repo,
        |_| Ok::<_, io::Error>(()),
        Context::default(),
    )?;
    score.ok_or_raise(|| message("Could not recover the staged rename similarity"))
}

struct Format<'a> {
    v2: bool,
    end: u8,
    quote_high: bool,
    prefix: Option<&'a Path>,
}

impl Format<'_> {
    fn path(&self, out: &mut dyn Write, path: &[u8]) -> Result<()> {
        if self.end == 0 {
            return out.write_all(path).or_error();
        }
        let (relative, native);
        let path = if let Some(prefix) = self.prefix.filter(|prefix| !prefix.as_os_str().is_empty()) {
            native = gix::path::from_bstr(path.as_bstr())?;
            let adjusted = gix::path::relativize_with_prefix(&native, prefix);
            let mut adjusted = gix::path::to_unix_separators_on_windows(gix::path::into_bstr(adjusted)?);
            if path.ends_with(b"/") && !adjusted.ends_with(b"/") {
                adjusted.to_mut().push_byte(b'/');
            }
            relative = adjusted;
            relative.as_ref()
        } else {
            path
        };
        let quoted = quote_patch_path(path, self.quote_high);
        if !self.v2 && path.contains(&b' ') && quoted.as_slice() == path {
            out.write_all(b"\"").or_error()?;
            out.write_all(path).or_error()?;
            return out.write_all(b"\"").or_error();
        }
        out.write_all(&quoted).or_error()
    }

    fn code(&self, code: u8) -> u8 {
        if code == 0 {
            if self.v2 { b'.' } else { b' ' }
        } else {
            code
        }
    }

    fn tracked(&self, out: &mut dyn Write, path: &BStr, tracked: &Tracked, null: ObjectId) -> Result<()> {
        let xy = [self.code(tracked.staged), self.code(tracked.unstaged)];
        if self.v2 {
            let index = tracked.index;
            let head = if tracked.staged == 0 { index } else { tracked.head };
            let empty = File {
                mode: Mode::empty(),
                id: null,
            };
            let head = head.unwrap_or(empty);
            let index = index.unwrap_or(empty);
            let worktree_mode = tracked.worktree_mode.unwrap_or(index.mode).bits();
            let submodule = tracked.submodule.unwrap_or_else(|| {
                if index.mode.is_submodule() || head.mode.is_submodule() {
                    *b"S..."
                } else {
                    *b"N..."
                }
            });
            out.write_all(&[if tracked.rename.is_some() { b'2' } else { b'1' }, b' '])
                .or_error()?;
            out.write_all(&xy).or_error()?;
            write!(
                out,
                " {} {:06o} {:06o} {worktree_mode:06o} {} {} ",
                submodule.as_bstr(),
                head.mode.bits(),
                index.mode.bits(),
                head.id,
                index.id
            )
            .or_error()?;
            if let Some(rename) = &tracked.rename {
                write!(out, "{}{} ", char::from(rename.kind), rename.score).or_error()?;
            }
        } else {
            out.write_all(&xy).or_error()?;
            out.write_all(b" ").or_error()?;
        }
        match &tracked.rename {
            Some(rename) if !self.v2 && self.end != 0 => {
                self.path(out, &rename.source)?;
                out.write_all(b" -> ").or_error()?;
                self.path(out, path)?;
            }
            Some(rename) => {
                self.path(out, path)?;
                out.write_all(&[if self.end == 0 { 0 } else { b'\t' }]).or_error()?;
                self.path(out, &rename.source)?;
            }
            None => self.path(out, path)?,
        }
        out.write_all(&[self.end]).or_error()
    }

    fn conflict(&self, repo: &gix::Repository, out: &mut dyn Write, path: &BStr, conflict: &Conflict) -> Result<()> {
        if self.v2 {
            let stages = &conflict.stages;
            let index_mode = stages[1].or(stages[2]).or(stages[0]).map(|file| file.mode);
            let mode = disk_mode(repo, path, index_mode)?;
            let submodule = if mode.is_submodule() { "S..." } else { "N..." };
            write!(out, "u {} {submodule} ", conflict.code).or_error()?;
            for stage in stages {
                write!(out, "{:06o} ", stage.map_or(0, |file| file.mode.bits())).or_error()?;
            }
            write!(out, "{:06o} ", mode.bits()).or_error()?;
            let null = ObjectId::null(repo.object_hash());
            for stage in stages {
                write!(out, "{} ", stage.map_or(null, |file| file.id)).or_error()?;
            }
        } else {
            write!(out, "{} ", conflict.code).or_error()?;
        }
        self.path(out, path)?;
        out.write_all(&[self.end]).or_error()
    }

    fn branch(&self, repo: &gix::Repository, out: &mut dyn Write, ahead_behind: bool) -> Result<()> {
        let head = repo.head()?;
        let name = head.referent_name().map(|name| name.shorten());
        let tip = head.id();
        let upstream = head
            .clone()
            .try_into_referent()
            .as_ref()
            .map(super::upstream_name)
            .transpose()?
            .flatten();
        let upstream_id = if let Some(upstream) = &upstream {
            repo.try_find_reference(upstream.as_ref())?
                .map(|mut reference| reference.peel_to_id())
                .transpose()?
        } else {
            None
        };
        let exact = ahead_behind
            || repo
                .config_snapshot()
                .try_boolean("status.aheadBehind")?
                .unwrap_or(true);
        let counts = if let Some((tip, base)) = tip.zip(upstream_id).filter(|_| exact) {
            Some(repo.ahead_behind_counts(tip, base)?)
        } else {
            None
        };
        let end = char::from(self.end);
        if self.v2 {
            if let Some(tip) = tip {
                write!(out, "# branch.oid {tip}{end}").or_error()?;
            } else {
                write!(out, "# branch.oid (initial){end}").or_error()?;
            }
            out.write_all(b"# branch.head ").or_error()?;
            out.write_all(name.unwrap_or(b"(detached)".as_bstr())).or_error()?;
            out.write_all(&[self.end]).or_error()?;
            if let Some(upstream) = upstream {
                out.write_all(b"# branch.upstream ").or_error()?;
                out.write_all(upstream.shorten()).or_error()?;
                out.write_all(&[self.end]).or_error()?;
            }
            if let Some(counts) = counts {
                write!(out, "# branch.ab +{} -{}{end}", counts.ahead, counts.behind).or_error()?;
            } else if tip.zip(upstream_id).is_some_and(|(tip, base)| tip != base) {
                write!(out, "# branch.ab +? -?{end}").or_error()?;
            } else if upstream_id.is_some() {
                write!(out, "# branch.ab +0 -0{end}").or_error()?;
            }
        } else {
            out.write_all(b"## ").or_error()?;
            if tip.is_none() {
                out.write_all(b"No commits yet on ").or_error()?;
            }
            out.write_all(name.unwrap_or(b"HEAD (no branch)".as_bstr()))
                .or_error()?;
            if let Some(upstream) = upstream {
                out.write_all(b"...").or_error()?;
                out.write_all(upstream.shorten()).or_error()?;
                match counts.map(|counts| (counts.ahead, counts.behind)) {
                    Some((0, 0)) => {}
                    Some((ahead, 0)) => write!(out, " [ahead {ahead}]").or_error()?,
                    Some((0, behind)) => write!(out, " [behind {behind}]").or_error()?,
                    Some((ahead, behind)) => write!(out, " [ahead {ahead}, behind {behind}]").or_error()?,
                    None if upstream_id.is_none() => out.write_all(b" [gone]").or_error()?,
                    None if tip != upstream_id => out.write_all(b" [different]").or_error()?,
                    None => {}
                }
            }
            out.write_all(&[self.end]).or_error()?;
        }
        Ok(())
    }
}
