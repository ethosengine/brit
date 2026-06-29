//! `LocalObjectStore` — stores ContentNodes as JSON files under
//! `.git/brit/objects/`, addressed by their BritCid.

use std::{fs, path::PathBuf};

use crate::engine::{cid::BritCid, content_node::ContentNode};

/// Filesystem-backed content-addressed store.
pub struct LocalObjectStore {
    base_dir: PathBuf,
}

impl LocalObjectStore {
    /// Create a store rooted at the given directory.
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Create a store for a git repo by locating `.git/brit/objects/`.
    pub fn for_git_dir(git_dir: &std::path::Path) -> Self {
        Self::new(git_dir.join("brit").join("objects"))
    }

    /// Store a ContentNode. Returns its CID. Idempotent.
    pub fn put<T: ContentNode>(&self, node: &T) -> Result<BritCid, ObjectStoreError> {
        let bytes = node
            .canonical_bytes()
            .map_err(|e| ObjectStoreError::Serialize(e.to_string()))?;
        let cid = BritCid::compute(&bytes);
        fs::create_dir_all(&self.base_dir).map_err(ObjectStoreError::Io)?;
        let name = cid.to_string();
        let path = self.base_dir.join(&name);
        // Atomic write: temp file + rename prevents partial writes on crash.
        let tmp_path = self.base_dir.join(format!("{name}.tmp"));
        fs::write(&tmp_path, &bytes).map_err(ObjectStoreError::Io)?;
        fs::rename(&tmp_path, &path).map_err(ObjectStoreError::Io)?;
        Ok(cid)
    }

    /// Retrieve a ContentNode by CID.
    pub fn get<T: ContentNode>(&self, cid: &BritCid) -> Result<T, ObjectStoreError> {
        let path = self.base_dir.join(cid.to_string());
        let bytes = fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound(cid.clone())
            } else {
                ObjectStoreError::Io(e)
            }
        })?;
        serde_ipld_dagcbor::from_slice(&bytes).map_err(|e| ObjectStoreError::Deserialize(e.to_string()))
    }

    /// List all stored CIDs.
    pub fn list(&self) -> Result<Vec<BritCid>, ObjectStoreError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }
        let mut cids = Vec::new();
        for entry in fs::read_dir(&self.base_dir).map_err(ObjectStoreError::Io)? {
            let entry = entry.map_err(ObjectStoreError::Io)?;
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(cid) = name.parse::<BritCid>() {
                    cids.push(cid);
                }
            }
        }
        Ok(cids)
    }
}

/// Errors from the local object store.
#[derive(Debug, thiserror::Error)]
pub enum ObjectStoreError {
    /// Filesystem error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// Serialization failed.
    #[error("serialization error: {0}")]
    Serialize(String),
    /// Deserialization failed.
    #[error("deserialization error: {0}")]
    Deserialize(String),
    /// Object not found.
    #[error("object not found: {0}")]
    NotFound(BritCid),
}
