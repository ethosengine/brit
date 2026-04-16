//! `LocalObjectStore` — stores ContentNodes as JSON files under
//! `.git/brit/objects/`, addressed by their BritCid.

use std::fs;
use std::path::PathBuf;
use crate::engine::cid::BritCid;
use crate::engine::content_node::ContentNode;

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
        let json = node.canonical_json().map_err(ObjectStoreError::Serialize)?;
        let cid = BritCid::compute(&json);
        fs::create_dir_all(&self.base_dir).map_err(ObjectStoreError::Io)?;
        let path = self.base_dir.join(cid.as_str());
        fs::write(&path, &json).map_err(ObjectStoreError::Io)?;
        Ok(cid)
    }

    /// Retrieve a ContentNode by CID.
    pub fn get<T: ContentNode>(&self, cid: &BritCid) -> Result<T, ObjectStoreError> {
        let path = self.base_dir.join(cid.as_str());
        let bytes = fs::read(&path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ObjectStoreError::NotFound(cid.clone())
            } else {
                ObjectStoreError::Io(e)
            }
        })?;
        serde_json::from_slice(&bytes).map_err(ObjectStoreError::Deserialize)
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
    Serialize(serde_json::Error),
    /// Deserialization failed.
    #[error("deserialization error: {0}")]
    Deserialize(serde_json::Error),
    /// Object not found.
    #[error("object not found: {0}")]
    NotFound(BritCid),
}
