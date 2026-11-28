//! Local capture suppression and recorder gaps for EPR to drain.

use std::{
    env,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use super::claims::{Claim, RecorderCause};

const SPOOL_LIMIT: usize = 1024;
pub(super) const SPOOL_MAX_ENTRIES: usize = 256;
const SPOOL_TEMP_MAX_AGE: Duration = Duration::from_hours(1);
const MARKER_MAX_ENTRIES: usize = 4096;
const MARKER_MAX_AGE: Duration = Duration::from_hours(7 * 24);
pub(super) const STDERR_LIMIT: usize = 256;

fn xdg_dir(variable: &str, default: &str) -> Option<PathBuf> {
    let xdg = env::var_os(variable);
    let home = env::var_os("HOME");
    let windows = cfg!(windows);
    let platform_home = windows.then(env::home_dir).flatten();
    resolve_xdg_dir(
        xdg.as_deref().map(Path::new),
        home.as_deref().map(Path::new),
        windows,
        platform_home.as_deref(),
        default,
    )
}

fn resolve_xdg_dir(
    xdg: Option<&Path>,
    home: Option<&Path>,
    windows: bool,
    platform_home: Option<&Path>,
    default: &str,
) -> Option<PathBuf> {
    xdg.filter(|path| path.is_absolute())
        .map(Path::to_path_buf)
        .or_else(|| {
            home.filter(|path| path.is_absolute())
                .or_else(|| platform_home.filter(|path| windows && path.is_absolute()))
                .map(|home| home.join(default))
        })
        .map(|base| base.join("brit"))
}

pub(super) struct Recorder(Option<Executable>);

impl Recorder {
    pub fn resolve() -> Self {
        let path = env::var_os("PATH").unwrap_or_else(|| {
            // execvp searches the OS utilities path when PATH is unset on Unix.
            if cfg!(windows) { "" } else { "/bin:/usr/bin" }.into()
        });
        let mut candidates = recorder_candidates(&path);
        Self(candidates.find_map(|path| {
            if !fs::metadata(&path).is_ok_and(|metadata| metadata.is_file()) {
                return None;
            }
            #[cfg(unix)]
            if rustix::fs::access(&path, rustix::fs::Access::EXEC_OK).is_err() {
                return None;
            }
            Executable::at(&path).ok()
        }))
    }

    pub fn found(&self) -> bool {
        self.0.is_some()
    }

    pub fn spool_file(&self, session: &str) -> Option<PathBuf> {
        let session = session.as_bytes();
        let key = match &self.0 {
            Some(executable) => executable.key(session),
            None => digest(&[session, b"missing"]),
        };
        let directory = xdg_dir("XDG_STATE_HOME", ".local/state")?.join("feedback-spool");
        Some(directory.join(format!("{}.json", &key[..32])))
    }
}

pub(super) fn full_spool_marker(spool_file: &Path) -> Option<PathBuf> {
    Some(markers_dir()?.join(format!("spool-full-{}", spool_file.file_stem()?.to_str()?)))
}

#[cfg(not(windows))]
fn recorder_candidates(path: &OsStr) -> impl Iterator<Item = PathBuf> + '_ {
    env::split_paths(path).map(|part| part.join("epr"))
}

#[cfg(windows)]
fn recorder_candidates(path: &OsStr) -> impl Iterator<Item = PathBuf> + '_ {
    use windows_sys::Win32::System::SystemInformation::{GetSystemDirectoryW, GetWindowsDirectoryW};

    // Command searches the application, system and Windows directories before the inherited PATH.
    let app = env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    app.into_iter()
        .chain(windows_directory(GetSystemDirectoryW))
        .chain(windows_directory(GetWindowsDirectoryW))
        .chain(env::split_paths(path))
        .map(|part| part.join("epr.exe"))
}

#[cfg(windows)]
fn windows_directory(read_dir: unsafe extern "system" fn(*mut u16, u32) -> u32) -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};

    let mut path = vec![0; 260];
    // SAFETY: The Win32 directory functions take the writable buffer and its capacity in UTF-16 units.
    let len = unsafe { read_dir(path.as_mut_ptr(), path.len() as u32) } as usize;
    let len = if len >= path.len() {
        path.resize(len, 0);
        // SAFETY: The buffer was resized to the capacity requested by Win32.
        unsafe { read_dir(path.as_mut_ptr(), path.len() as u32) as usize }
    } else {
        len
    };
    (len != 0 && len < path.len()).then(|| PathBuf::from(OsString::from_wide(&path[..len])))
}

struct Executable {
    path: PathBuf,
    size: u64,
    mtime: Option<i128>,
}

impl Executable {
    fn at(path: &Path) -> io::Result<Self> {
        let path = path.canonicalize()?;
        let metadata = fs::metadata(&path)?;
        Ok(Self {
            path,
            size: metadata.len(),
            mtime: metadata.modified().ok().map(unix_nanos),
        })
    }

    fn key(&self, scope: &[u8]) -> String {
        let mtime = self.mtime.map(i128::to_le_bytes);
        digest(&[
            scope,
            self.path.as_os_str().as_encoded_bytes(),
            &self.size.to_le_bytes(),
            mtime.as_ref().map_or(&[][..], |bytes| bytes.as_slice()),
        ])
    }
}

fn unix_nanos(time: SystemTime) -> i128 {
    let nanos = |duration: std::time::Duration| {
        i128::from(duration.as_secs()) * 1_000_000_000 + i128::from(duration.subsec_nanos())
    };
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => nanos(duration),
        Err(error) => -nanos(error.duration()),
    }
}

fn digest(parts: &[&[u8]]) -> String {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha256);
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher
        .try_finalize()
        .expect("SHA-256 has no collision detector")
        .to_string()
}

fn markers_dir() -> Option<PathBuf> {
    Some(xdg_dir("XDG_CACHE_HOME", ".cache")?.join("feedback-markers"))
}

fn notices_dir() -> Option<PathBuf> {
    Some(xdg_dir("XDG_CACHE_HOME", ".cache")?.join("feedback-notices"))
}

pub(super) fn success_file(session: &str, emitter: &str, claim: Claim, outcome: &str) -> Option<PathBuf> {
    let key = digest(&[
        session.as_bytes(),
        emitter.as_bytes(),
        claim.id().as_bytes(),
        outcome.as_bytes(),
    ]);
    Some(markers_dir()?.join(key))
}

pub(super) fn claim_notice(name: &str) -> bool {
    notices_dir()
        .map(|directory| directory.join(name))
        // A full notice directory skips the marker, so the notice repeats rather than going silent.
        .is_some_and(|path| mark(&path).is_ok())
}

pub(super) fn mark(path: &Path) -> io::Result<()> {
    create_marker(path, |_| {
        OpenOptions::new().write(true).create_new(true).open(path)?;
        Ok(())
    })
}

pub(super) fn mark_full_spool(path: &Path, cause: RecorderCause) -> io::Result<()> {
    create_marker(path, |directory| {
        // Publish the cause atomically so a later visible observation can report it.
        let mut temp = tempfile::NamedTempFile::new_in(directory)?;
        serde_json::to_writer(&mut temp, &StoredGap { cause })?;
        temp.persist(path)?;
        Ok(())
    })
}

fn create_marker(path: &Path, write: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<()> {
    let parent = path.parent().expect("a marker has a directory");
    fs::create_dir_all(parent)?;
    // Both cache directories carry the same bound; anything else is written unbounded.
    let Some(directory) = [markers_dir(), notices_dir()]
        .into_iter()
        .flatten()
        .find(|directory| parent.starts_with(directory))
    else {
        return write(parent);
    };
    let _lock = lock_file(&directory.with_extension("lock"))?;
    if !marker_directory_has_space(&directory)? {
        let cutoff = SystemTime::now() - MARKER_MAX_AGE;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension() != Some(OsStr::new("lock"))
                && entry.metadata().is_ok_and(|metadata| {
                    metadata.is_file() && metadata.modified().is_ok_and(|modified| modified < cutoff)
                })
            {
                let _ = fs::remove_file(path);
            }
        }
        if !marker_directory_has_space(&directory)? {
            return Ok(());
        }
    }
    write(parent)
}

fn marker_directory_has_space(directory: &Path) -> io::Result<bool> {
    let count = fs::read_dir(directory)?
        .take(MARKER_MAX_ENTRIES)
        .try_fold(0, |count, entry| entry.map(|_| count + 1))?;
    Ok(count < MARKER_MAX_ENTRIES)
}

pub(super) fn lock_file(path: &Path) -> io::Result<File> {
    fs::create_dir_all(path.parent().expect("a lock file has a directory"))?;
    let lock = OpenOptions::new().write(true).create(true).truncate(false).open(path)?;
    lock.lock()?;
    Ok(lock)
}

pub(super) fn emitter() -> Option<String> {
    if let Some(record) = env::var_os("EPR_BIN_RECORD") {
        return record.into_string().ok();
    }
    let sha = env!("BRIT_GIT_SHA");
    let version = env!("CARGO_PKG_VERSION");
    if sha == "unavailable" {
        let executable = env::current_exe().ok()?;
        let hash = cached_executable_sha(&executable, &markers_dir()?).ok()?;
        return Some(format!("unrecorded:{version}+exe.{hash}"));
    }
    Some(format!("unrecorded:{version}+{}", sha.get(..12).unwrap_or(sha)))
}

fn cached_executable_sha(path: &Path, markers: &Path) -> io::Result<String> {
    let executable = Executable::at(path)?;
    let key = executable.key(b"executable-sha256");
    let cached = markers.join(format!("exe-{key}"));
    let read_cache = || {
        let sha = fs::read_to_string(&cached).ok()?;
        (sha.len() == 12 && sha.bytes().all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))).then_some(sha)
    };
    if let Some(sha) = read_cache() {
        return Ok(sha);
    }
    let _lock = lock_file(&markers.join("executable-sha256.lock"))?;
    if let Some(sha) = read_cache() {
        return Ok(sha);
    }
    let mut executable = File::open(&executable.path)?;
    let mut digest = gix::hash::io::Write::new(io::sink(), gix::hash::Kind::Sha256);
    io::copy(&mut executable, &mut digest)?;
    let sha = digest
        .hash
        .try_finalize()
        .expect("SHA-256 has no collision detector")
        .to_string();
    let sha = &sha[..12];
    let mut temp = tempfile::NamedTempFile::new_in(markers)?;
    temp.write_all(sha.as_bytes())?;
    temp.persist(cached)?;
    Ok(sha.to_owned())
}

#[derive(Serialize)]
struct SpoolRecord<'a> {
    v: u8,
    claim: &'static str,
    cause: &'static str,
    at: String,
    emitter: &'a str,
    session: String,
    recorder: RecorderRecord<'a>,
    stderr: Option<&'a str>,
}

#[derive(Serialize)]
struct RecorderRecord<'a> {
    path: Option<std::borrow::Cow<'a, str>>,
    size: Option<u64>,
    mtime: Option<i64>,
}

pub(super) fn write_gap(
    path: &Path,
    recorder: &Recorder,
    cause: RecorderCause,
    emitter: &str,
    session: &str,
    stderr: Option<&str>,
) -> io::Result<GapWrite> {
    let directory = path.parent().expect("a spool file has a directory");
    fs::create_dir_all(directory)?;
    // Keep the lock outside the spool so it never counts as a pending notice.
    let _lock = lock_file(&directory.with_extension("lock"))?;
    if path.is_file() {
        return Ok(GapWrite::Stored);
    }
    // A clock within an hour of the epoch has nothing old enough to prune.
    let cutoff = SystemTime::now().checked_sub(SPOOL_TEMP_MAX_AGE).unwrap_or(UNIX_EPOCH);
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.as_encoded_bytes();
        if name.starts_with(b".tmp")
            && !name.ends_with(b".json")
            && entry
                .metadata()
                .is_ok_and(|metadata| metadata.is_file() && metadata.modified().is_ok_and(|modified| modified < cutoff))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
    let mut count = 0;
    for entry in fs::read_dir(directory)? {
        if !entry?.file_name().as_encoded_bytes().ends_with(b".json") {
            continue;
        }
        count += 1;
        if count == SPOOL_MAX_ENTRIES {
            return Ok(GapWrite::Full);
        }
    }

    let now = i64::try_from(unix_nanos(SystemTime::now()).div_euclid(1_000_000_000)).ok();
    let at = now
        .and_then(|seconds| chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, 0))
        .ok_or_else(|| io::Error::other("system time is outside the RFC 3339 range"))?
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let executable = recorder.0.as_ref();
    let mut session_hash = gix::hash::hasher(gix::hash::Kind::Sha256);
    session_hash.update(session.as_bytes());
    let session_hash = session_hash.try_finalize().expect("SHA-256 has no collision detector");
    let mut row = SpoolRecord {
        v: 1,
        claim: cause.claim().id(),
        cause: cause.as_str(),
        at,
        emitter,
        session: session_hash.to_string(),
        recorder: RecorderRecord {
            path: executable.map(|exe| exe.path.to_string_lossy()),
            size: executable.map(|exe| exe.size),
            mtime: executable
                .and_then(|exe| exe.mtime)
                .and_then(|ns| i64::try_from(ns.div_euclid(1_000_000_000)).ok()),
        },
        stderr: stderr.map(|line| &line[..line.floor_char_boundary(STDERR_LIMIT)]),
    };
    let bytes = loop {
        let bytes = serde_json::to_vec(&row)?;
        if bytes.len() <= SPOOL_LIMIT {
            break bytes;
        }
        if let Some(line) = row.stderr {
            // JSON escapes may use multiple bytes for a single input byte.
            row.stderr = line.char_indices().next_back().map(|(end, _)| &line[..end]);
        } else if row.recorder.path.take().is_none() {
            return Err(io::Error::other("recorder gap exceeds the spool limit"));
        }
    };
    let mut temp = tempfile::NamedTempFile::new_in(directory)?;
    temp.write_all(&bytes)?;
    match temp.persist_noclobber(path) {
        Ok(_) => Ok(GapWrite::Stored),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(GapWrite::Stored),
        Err(error) => Err(error.error),
    }
}

pub(super) enum GapWrite {
    Stored,
    Full,
}

#[derive(Deserialize, Serialize)]
pub(super) struct StoredGap {
    pub cause: RecorderCause,
}

pub(super) fn read_gap(path: &Path) -> Option<StoredGap> {
    serde_json::from_reader(File::open(path).ok()?.take(SPOOL_LIMIT as u64)).ok()
}

#[cfg(test)]
mod tests {
    use gix_testtools::TestResult;

    use super::*;

    #[test]
    fn xdg_directory_prefers_absolute_xdg_then_absolute_home() {
        let root = Path::new(if cfg!(windows) { r"C:\test" } else { "/test" });
        let xdg = root.join("xdg");
        let home = root.join("home");
        let default = ".local/state";
        for (variable, expected) in [
            (Some(xdg.as_path()), xdg.join("brit")),
            (Some(Path::new("relative-xdg")), home.join(default).join("brit")),
            (None, home.join(default).join("brit")),
        ] {
            assert_eq!(
                resolve_xdg_dir(variable, Some(&home), false, None, default),
                Some(expected),
                "resolve the state directory for XDG_STATE_HOME={variable:?}"
            );
        }
    }

    #[test]
    fn xdg_directory_uses_only_an_absolute_windows_fallback_without_absolute_home() {
        let profile = Path::new(if cfg!(windows) { r"C:\profile" } else { "/profile" });
        let default = ".cache";
        for home in [Some(Path::new("relative-home")), Some(Path::new("")), None] {
            assert_eq!(
                resolve_xdg_dir(None, home, false, Some(profile), default),
                None,
                "no Unix cache directory for HOME={home:?}"
            );
            assert_eq!(
                resolve_xdg_dir(None, home, true, Some(profile), default),
                Some(profile.join(default).join("brit")),
                "use the Windows profile for HOME={home:?}"
            );
        }
        assert_eq!(
            resolve_xdg_dir(None, None, true, Some(Path::new("relative-profile")), default),
            None,
            "the Windows fallback also has to be absolute"
        );
    }

    #[test]
    fn spool_capacity_counts_only_json_entries() -> TestResult {
        let dir = tempfile::tempdir()?;
        let spool = dir.path().join("feedback-spool");
        fs::create_dir(&spool)?;
        for index in 0..SPOOL_MAX_ENTRIES - 1 {
            fs::write(spool.join(format!("{index}.json")), b"{}")?;
        }
        for index in 0..10 {
            File::create(spool.join(format!(".tmp{index}")))?;
        }
        let write = |name| {
            write_gap(
                &spool.join(name),
                &Recorder(None),
                RecorderCause::NotFound,
                "test",
                "first",
                None,
            )
        };
        assert!(
            matches!(write("last.json")?, GapWrite::Stored),
            "temporaries leave the last slot free"
        );
        assert!(spool.join("last.json").is_file(), "the last record fills the spool");
        assert!(
            matches!(write("extra.json")?, GapWrite::Full),
            "256 records fill the spool"
        );
        Ok(())
    }

    #[test]
    fn spool_prunes_only_old_temporary_files() -> TestResult {
        let dir = tempfile::tempdir()?;
        let spool = dir.path().join("feedback-spool");
        fs::create_dir(&spool)?;
        let old_mtime = SystemTime::now() - Duration::from_hours(2);
        for name in [".tmp-old", ".tmp-keep.json", "keep.txt"] {
            File::create(spool.join(name))?.set_modified(old_mtime)?;
        }
        File::create(spool.join(".tmp-fresh"))?;
        #[cfg(unix)]
        std::os::unix::fs::symlink("keep.txt", spool.join(".tmp-link"))?;

        write_gap(
            &spool.join("new.json"),
            &Recorder(None),
            RecorderCause::NotFound,
            "test",
            "first",
            None,
        )?;

        assert!(!spool.join(".tmp-old").exists(), "stale temporary files are removed");
        for name in [".tmp-fresh", ".tmp-keep.json", "keep.txt"] {
            assert!(spool.join(name).is_file(), "{name} survives cleanup");
        }
        #[cfg(unix)]
        assert!(spool.join(".tmp-link").is_symlink(), "cleanup preserves symlinks");
        Ok(())
    }

    #[test]
    fn non_command_markers_distinguish_non_passing_outcomes() -> TestResult {
        if gix_testtools::run_in_isolated_process()? {
            return Ok(());
        }
        let cache = tempfile::tempdir()?;
        let _env = gix_testtools::Env::new().set("XDG_CACHE_HOME", cache.path().to_str().expect("fixture path"));
        let claim = super::super::claims::UNKNOWN_CAPABILITY;
        assert_ne!(
            success_file("session", "emitter", claim, "failed").expect("failure marker path"),
            success_file("session", "emitter", claim, "refused").expect("refusal marker path"),
            "each outcome has its own marker"
        );
        Ok(())
    }

    #[test]
    fn full_marker_directory_prunes_old_regular_files() -> TestResult {
        check_full_marker_directory(markers_dir, true)
    }

    #[test]
    fn full_marker_directory_keeps_fresh_entries_and_skips_new_marker() -> TestResult {
        check_full_marker_directory(markers_dir, false)
    }

    #[test]
    fn full_notice_directory_follows_the_marker_bound() -> TestResult {
        check_full_marker_directory(notices_dir, true)?;
        check_full_marker_directory(notices_dir, false)
    }

    fn check_full_marker_directory(dir: fn() -> Option<PathBuf>, old: bool) -> TestResult {
        if gix_testtools::run_in_isolated_process()? {
            return Ok(());
        }
        let cache = tempfile::tempdir()?;
        let _env = gix_testtools::Env::new().set("XDG_CACHE_HOME", cache.path().to_str().expect("fixture path"));
        let directory = dir().expect("fixture marker directory");
        fs::create_dir_all(&directory)?;
        let old_mtime = SystemTime::now() - Duration::from_hours(8 * 24);
        let lock = directory.join("cure.lock");
        let lock_file = File::create(&lock)?;
        if old {
            lock_file.set_modified(old_mtime)?;
        }
        drop(lock_file);
        for index in 1..4096 {
            let file = File::create(directory.join(format!("marker-{index}")))?;
            if old {
                file.set_modified(old_mtime)?;
            }
        }
        let marker = directory.join("new-marker");
        mark(&marker)?;
        assert_eq!(marker.is_file(), old, "only pruning makes space for a new marker");
        assert!(lock.is_file(), "the lock must survive pruning");
        assert_eq!(
            fs::read_dir(directory)?.collect::<io::Result<Vec<_>>>()?.len(),
            if old { 2 } else { 4096 },
            "old markers are cleaned up and fresh entries are kept"
        );
        Ok(())
    }

    #[test]
    fn executable_hash_is_cached_by_canonical_path_size_and_mtime() -> TestResult {
        let dir = tempfile::tempdir()?;
        let executable = dir.path().join("britannic");
        let markers = dir.path().join("feedback-markers");
        fs::write(&executable, b"abc")?;
        let original_mtime = fs::metadata(&executable)?.modified()?;
        assert_eq!(cached_executable_sha(&executable, &markers)?, "ba7816bf8f01");

        // Equal size and restored mtime exercise a cache hit even if file contents differ.
        fs::write(&executable, b"def")?;
        let file = OpenOptions::new().write(true).open(&executable)?;
        file.set_modified(original_mtime)?;
        let same_path = dir.path().join(".").join("britannic");
        assert_eq!(cached_executable_sha(&same_path, &markers)?, "ba7816bf8f01");

        file.set_modified(original_mtime - std::time::Duration::from_secs(60))?;
        assert_eq!(cached_executable_sha(&executable, &markers)?, "cb8379ac2098");
        Ok(())
    }
}
