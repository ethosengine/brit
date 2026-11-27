//! Local capture suppression and recorder gaps for EPR to drain.

use std::{
    env,
    ffi::OsStr,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use super::claims::{Claim, RecorderCause};

const SPOOL_LIMIT: usize = 1024;
const SPOOL_MAX_ENTRIES: usize = 256;
pub(super) const STDERR_LIMIT: usize = 256;

fn xdg_dir(variable: &str, default: &str) -> Option<PathBuf> {
    env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| gix::path::env::home_dir().map(|home| home.join(default)))
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

pub(super) fn success_file(session: &str, emitter: &str, claim: Claim, outcome: &str) -> Option<PathBuf> {
    let key = digest(&[
        session.as_bytes(),
        emitter.as_bytes(),
        claim.id().as_bytes(),
        if claim.is_command() || outcome == "passed" {
            outcome.as_bytes()
        } else {
            b""
        },
    ]);
    Some(markers_dir()?.join(key))
}

pub(super) fn claim_gap_notice(session: &str) -> bool {
    xdg_dir("XDG_CACHE_HOME", ".cache")
        .map(|base| base.join("feedback-notices").join(session))
        .is_some_and(|path| mark(&path).is_ok())
}

pub(super) fn mark(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path.parent().expect("a marker has a directory"))?;
    OpenOptions::new().write(true).create_new(true).open(path)?;
    Ok(())
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
    for (index, entry) in fs::read_dir(directory)?.take(SPOOL_MAX_ENTRIES).enumerate() {
        entry?;
        if index + 1 == SPOOL_MAX_ENTRIES {
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

#[derive(Deserialize)]
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
