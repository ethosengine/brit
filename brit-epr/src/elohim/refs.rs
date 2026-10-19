//! `BritRefManager` — read/write/list git refs under `refs/notes/brit/`.

use std::{
    path::{Path, PathBuf},
    process::Command,
};

/// Manages git refs under `refs/notes/brit/` for attestation indexing.
pub struct BritRefManager {
    repo_path: PathBuf,
}

impl BritRefManager {
    /// Create a new `BritRefManager` for the given repository root.
    ///
    /// Returns [`RefError::NotARepo`] if `repo_path` is not a git repository.
    pub fn new(repo_path: &Path) -> Result<Self, RefError> {
        if !repo_path.join(".git").exists() && !repo_path.join("HEAD").exists() {
            return Err(RefError::NotARepo(repo_path.display().to_string()));
        }
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    // --- Build refs (per-commit via git notes) ---

    /// Write a build attestation payload as a git note on `commit_rev`.
    pub fn put_build_ref(
        &self,
        step_name: &str,
        commit_rev: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = format!("refs/notes/brit/build/{}", Self::encode_segment(step_name));
        let commit_sha = self.resolve_rev(commit_rev)?;
        self.write_note(&ref_name, &commit_sha, payload)
    }

    /// Read the build attestation payload for `commit_rev`, or `None` if absent.
    pub fn get_build_ref(&self, step_name: &str, commit_rev: &str) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!("refs/notes/brit/build/{}", Self::encode_segment(step_name));
        let commit_sha = self.resolve_rev(commit_rev)?;
        self.read_note(&ref_name, &commit_sha)
    }

    /// List all build ref step names under `refs/notes/brit/build/`.
    pub fn list_build_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under("refs/notes/brit/build/")
    }

    /// Every build attestation under this step's notes ref whose `inputsHash` equals
    /// `inputs_hash`, as `(commit_sha, payload)` pairs in `git notes list` order.
    ///
    /// The notes are keyed by commit, but the question a gate asks is "has this exact input
    /// closure already been built?" — and that answer must not depend on which commit carried
    /// the closure. One `git notes list` plus one `git cat-file --batch` over the note objects;
    /// a step with no notes ref yet reads as empty, and a note that is not JSON is skipped
    /// because it is not an attestation.
    pub fn find_build_by_inputs(
        &self,
        step_name: &str,
        inputs_hash: &str,
    ) -> Result<Vec<(String, serde_json::Value)>, RefError> {
        let ref_name = format!("refs/notes/brit/build/{}", Self::encode_segment(step_name));
        let listing = Command::new("git")
            .args(["notes", "--ref", &ref_name, "list"])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !listing.status.success() {
            return Ok(Vec::new());
        }
        let text = String::from_utf8_lossy(&listing.stdout);
        // Each line is `<note object> <annotated commit>`.
        let pairs: Vec<(String, String)> = text
            .lines()
            .filter_map(|line| {
                let mut it = line.split_whitespace();
                Some((it.next()?.to_string(), it.next()?.to_string()))
            })
            .collect();
        if pairs.is_empty() {
            return Ok(Vec::new());
        }
        let bodies = self.cat_file_batch(pairs.iter().map(|(note, _)| note.clone()).collect())?;
        let mut found = Vec::new();
        for ((_note, commit), body) in pairs.into_iter().zip(bodies) {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(body.trim()) else {
                continue;
            };
            if value.get("inputsHash").and_then(serde_json::Value::as_str) == Some(inputs_hash) {
                found.push((commit, value));
            }
        }
        Ok(found)
    }

    // --- Deploy refs (standalone blob refs) ---

    /// Write a deploy attestation payload as a standalone blob ref.
    pub fn put_deploy_ref(&self, step_name: &str, env: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        let ref_name = format!(
            "refs/notes/brit/deploy/{}/{}",
            Self::encode_segment(step_name),
            Self::encode_segment(env)
        );
        self.write_ref_blob(&ref_name, payload)
    }

    /// Read the deploy attestation payload, or `None` if absent.
    pub fn get_deploy_ref(&self, step_name: &str, env: &str) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!(
            "refs/notes/brit/deploy/{}/{}",
            Self::encode_segment(step_name),
            Self::encode_segment(env)
        );
        self.read_ref_blob(&ref_name)
    }

    /// List all deploy ref names under `refs/notes/brit/deploy/`.
    pub fn list_deploy_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under("refs/notes/brit/deploy/")
    }

    // --- Validate refs ---

    /// Write a validation attestation payload as a standalone blob ref.
    pub fn put_validate_ref(
        &self,
        step_name: &str,
        check_name: &str,
        payload: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = format!(
            "refs/notes/brit/validate/{}/{}",
            Self::encode_segment(step_name),
            Self::encode_segment(check_name)
        );
        self.write_ref_blob(&ref_name, payload)
    }

    /// Read the validation attestation payload, or `None` if absent.
    pub fn get_validate_ref(&self, step_name: &str, check_name: &str) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!(
            "refs/notes/brit/validate/{}/{}",
            Self::encode_segment(step_name),
            Self::encode_segment(check_name)
        );
        self.read_ref_blob(&ref_name)
    }

    /// List all validate ref names under `refs/notes/brit/validate/`.
    pub fn list_validate_refs(&self, _pattern: Option<&str>) -> Result<Vec<String>, RefError> {
        self.list_refs_under("refs/notes/brit/validate/")
    }

    // --- Reach refs ---

    /// Write a reach payload as a standalone blob ref.
    pub fn put_reach_ref(&self, step_name: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        let ref_name = format!("refs/notes/brit/reach/{}", Self::encode_segment(step_name));
        self.write_ref_blob(&ref_name, payload)
    }

    /// Read the reach payload, or `None` if absent.
    pub fn get_reach_ref(&self, step_name: &str) -> Result<Option<serde_json::Value>, RefError> {
        let ref_name = format!("refs/notes/brit/reach/{}", Self::encode_segment(step_name));
        self.read_ref_blob(&ref_name)
    }

    // --- Covenant refs (append-only, one blob ref per clause) ---

    /// Record a covenant clause at `refs/notes/brit/covenant/<session>/<clause-cid>`.
    ///
    /// The blob is `{"cid": "<clause_cid_str>", "clause": <clause_json>}`. Append-only: a clause
    /// is addressed by its CID, so an existing ref with identical content is a no-op and one with
    /// different content is refused with [`RefError::Exists`]; nothing is ever overwritten.
    pub fn put_covenant_clause(
        &self,
        session: &str,
        clause_cid_str: &str,
        clause_json: &serde_json::Value,
    ) -> Result<(), RefError> {
        let ref_name = Self::covenant_ref(session, clause_cid_str);
        let record = serde_json::json!({ "cid": clause_cid_str, "clause": clause_json });
        if let Some(existing) = self.read_ref_blob(&ref_name)? {
            return if existing == record {
                Ok(())
            } else {
                Err(RefError::Exists(ref_name))
            };
        }
        self.write_ref_blob(&ref_name, &record)
    }

    /// Every clause recorded for `session` as `{cid, clause}` records, ordered by the clause's
    /// `validFrom` (ties broken by CID). An unknown session reads as empty.
    pub fn list_covenant_clauses(&self, session: &str) -> Result<Vec<serde_json::Value>, RefError> {
        let prefix = format!("refs/notes/brit/covenant/{}/", Self::encode_segment(session));
        self.covenant_records_under(&prefix)
    }

    /// The `{cid, clause}` record for a clause CID in any session, or `None`.
    pub fn get_covenant_clause(&self, cid: &str) -> Result<Option<serde_json::Value>, RefError> {
        let suffix = format!("/{}", Self::encode_segment(cid));
        let hit = self
            .for_each_ref("refs/notes/brit/covenant/")?
            .into_iter()
            .find(|(name, _)| name.ends_with(&suffix));
        let Some((name, _)) = hit else {
            return Ok(None);
        };
        self.read_ref_blob(&name)
    }

    fn covenant_ref(session: &str, cid: &str) -> String {
        format!(
            "refs/notes/brit/covenant/{}/{}",
            Self::encode_segment(session),
            Self::encode_segment(cid)
        )
    }

    fn covenant_records_under(&self, prefix: &str) -> Result<Vec<serde_json::Value>, RefError> {
        let refs = self.for_each_ref(prefix)?;
        if refs.is_empty() {
            return Ok(Vec::new());
        }
        let bodies = self.cat_file_batch(refs.into_iter().map(|(_, oid)| oid).collect())?;
        let mut rows: Vec<serde_json::Value> = bodies
            .iter()
            .filter_map(|b| serde_json::from_str::<serde_json::Value>(b.trim()).ok())
            .collect();
        let key = |v: &serde_json::Value| {
            (
                v["clause"]["validFrom"].as_str().unwrap_or("").to_string(),
                v["cid"].as_str().unwrap_or("").to_string(),
            )
        };
        rows.sort_by_key(key);
        Ok(rows)
    }

    /// `(refname, object id)` for every ref under `prefix`, refnames undecoded.
    fn for_each_ref(&self, prefix: &str) -> Result<Vec<(String, String)>, RefError> {
        let output = Command::new("git")
            .args(["for-each-ref", "--format=%(refname) %(objectname)", prefix])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|l| l.split_once(' '))
            .map(|(n, o)| (n.to_string(), o.to_string()))
            .collect())
    }

    // --- Internal helpers ---

    /// Encode a ref path segment, replacing characters git rejects in ref names.
    ///
    /// Per `git check-ref-format`, ref names cannot contain: `:`, `@{`, `?`,
    /// `*`, `[`, `\`, `^`, `~`, space, DEL, or control characters. They also
    /// cannot contain `..` or end with `.lock`. We percent-encode a superset
    /// of forbidden characters to be safe.
    fn encode_segment(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                ':' => out.push_str("%3A"),
                '@' => out.push_str("%40"),
                '?' => out.push_str("%3F"),
                '*' => out.push_str("%2A"),
                '[' => out.push_str("%5B"),
                '\\' => out.push_str("%5C"),
                '^' => out.push_str("%5E"),
                '~' => out.push_str("%7E"),
                ' ' => out.push_str("%20"),
                c if c.is_ascii_control() => {
                    use std::fmt::Write;
                    let _ = write!(out, "%{:02X}", c as u8);
                }
                _ => out.push(c),
            }
        }
        // Also handle ".." which git forbids in ref names
        out.replace("..", "%2E%2E")
    }

    /// Decode a percent-encoded ref path segment back to its original form.
    fn decode_segment(s: &str) -> String {
        s.replace("%3A", ":")
            .replace("%40", "@")
            .replace("%3F", "?")
            .replace("%2A", "*")
            .replace("%5B", "[")
            .replace("%5C", "\\")
            .replace("%5E", "^")
            .replace("%7E", "~")
            .replace("%20", " ")
            .replace("%2E%2E", "..")
    }

    fn resolve_rev(&self, rev: &str) -> Result<String, RefError> {
        let output = Command::new("git")
            .args(["rev-parse", rev])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::RevNotFound(rev.to_string()));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    fn write_note(&self, ref_name: &str, commit_sha: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        let json = serde_json::to_string(payload).map_err(RefError::Json)?;
        // `git notes add` creates a notes commit, which needs a committer identity.
        // brit is designed to run in CI (Jenkins) where ambient git identity is
        // often unset, so supply a deterministic one via `-c` rather than relying
        // on the environment.
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=brit",
                "-c",
                "user.email=brit@ethosengine.com",
                "notes",
                "--ref",
                ref_name,
                "add",
                "-f",
                "-m",
                &json,
                commit_sha,
            ])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::GitFailed(format!(
                "notes add to {ref_name}: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    fn read_note(&self, ref_name: &str, commit_sha: &str) -> Result<Option<serde_json::Value>, RefError> {
        let output = Command::new("git")
            .args(["notes", "--ref", ref_name, "show", commit_sha])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let value = serde_json::from_str(text.trim()).map_err(RefError::Json)?;
        Ok(Some(value))
    }

    fn write_ref_blob(&self, ref_name: &str, payload: &serde_json::Value) -> Result<(), RefError> {
        let json = serde_json::to_string(payload).map_err(RefError::Json)?;
        let mut child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.repo_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(RefError::GitCommand)?;
        {
            use std::io::Write;
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| RefError::GitFailed("stdin pipe unavailable".into()))?;
            stdin.write_all(json.as_bytes()).map_err(RefError::GitCommand)?;
        }
        let hash_output = child.wait_with_output().map_err(RefError::GitCommand)?;
        if !hash_output.status.success() {
            return Err(RefError::GitFailed("hash-object failed".into()));
        }
        let blob_sha = String::from_utf8_lossy(&hash_output.stdout).trim().to_string();

        let update_output = Command::new("git")
            .args(["update-ref", ref_name, &blob_sha])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !update_output.status.success() {
            return Err(RefError::GitFailed(format!(
                "update-ref {ref_name}: {}",
                String::from_utf8_lossy(&update_output.stderr)
            )));
        }
        Ok(())
    }

    fn read_ref_blob(&self, ref_name: &str) -> Result<Option<serde_json::Value>, RefError> {
        let output = Command::new("git")
            .args(["cat-file", "-p", ref_name])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Ok(None);
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let value = serde_json::from_str(text.trim()).map_err(RefError::Json)?;
        Ok(Some(value))
    }

    /// The contents of each object, in order, through one `git cat-file --batch`. The oids are
    /// written from a separate thread so a long listing cannot deadlock against a full stdout
    /// pipe. A missing object yields an empty string in its slot.
    fn cat_file_batch(&self, oids: Vec<String>) -> Result<Vec<String>, RefError> {
        use std::{io::Write, process::Stdio};

        let mut child = Command::new("git")
            .args(["cat-file", "--batch"])
            .current_dir(&self.repo_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(RefError::GitCommand)?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| RefError::GitFailed("stdin pipe unavailable".into()))?;
        let writer = std::thread::spawn(move || -> std::io::Result<()> {
            for oid in oids {
                writeln!(stdin, "{oid}")?;
            }
            Ok(())
        });
        let output = child.wait_with_output().map_err(RefError::GitCommand)?;
        writer
            .join()
            .map_err(|_| RefError::GitFailed("cat-file --batch: writer thread panicked".into()))?
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Err(RefError::GitFailed("cat-file --batch failed".into()));
        }
        // Each entry is `<oid> <type> <size>\n`, `size` bytes, then one `\n`; a missing object
        // prints `<oid> missing\n` and nothing else.
        let bytes = output.stdout;
        let mut pos = 0;
        let mut out = Vec::new();
        while pos < bytes.len() {
            let nl = bytes[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| RefError::GitFailed("cat-file --batch: truncated header".into()))?;
            let header = String::from_utf8_lossy(&bytes[pos..pos + nl]).to_string();
            pos += nl + 1;
            let mut fields = header.split_whitespace();
            let _oid = fields.next();
            match (fields.next(), fields.next()) {
                (Some(_kind), Some(size)) => {
                    let size: usize = size
                        .parse()
                        .map_err(|_| RefError::GitFailed(format!("cat-file --batch: bad size in `{header}`")))?;
                    let end = pos + size;
                    if end > bytes.len() {
                        return Err(RefError::GitFailed("cat-file --batch: truncated object".into()));
                    }
                    out.push(String::from_utf8_lossy(&bytes[pos..end]).to_string());
                    pos = end + 1;
                }
                _ => out.push(String::new()),
            }
        }
        Ok(out)
    }

    fn list_refs_under(&self, prefix: &str) -> Result<Vec<String>, RefError> {
        let output = Command::new("git")
            .args(["for-each-ref", "--format=%(refname)", prefix])
            .current_dir(&self.repo_path)
            .output()
            .map_err(RefError::GitCommand)?;
        if !output.status.success() {
            return Ok(Vec::new());
        }
        let text = String::from_utf8_lossy(&output.stdout);
        let names: Vec<String> = text
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| line.strip_prefix(prefix))
            .map(Self::decode_segment)
            .collect();
        Ok(names)
    }
}

/// Errors returned by [`BritRefManager`] operations.
#[derive(Debug, thiserror::Error)]
pub enum RefError {
    /// The given path is not a git repository.
    #[error("not a git repository: {0}")]
    NotARepo(String),
    /// A git revision could not be resolved.
    #[error("rev not found: {0}")]
    RevNotFound(String),
    /// Failed to spawn or communicate with a git subprocess.
    #[error("git command error: {0}")]
    GitCommand(std::io::Error),
    /// A git subprocess exited with a non-zero status.
    #[error("git command failed: {0}")]
    GitFailed(String),
    /// An append-only ref already exists with different content.
    #[error("ref already exists with different content: {0}")]
    Exists(String),
    /// JSON serialization or deserialization failed.
    #[error("JSON error: {0}")]
    Json(serde_json::Error),
}
