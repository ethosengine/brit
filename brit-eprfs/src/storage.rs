use std::path::{Path, PathBuf};

use async_trait::async_trait;
use bytes::Bytes;
use eprfs_core::{
    AttestationDraft, BlobCid, BlobHandle, BlobPresence, EprRecord, EprRef, EprfsError, EprfsStorage, FetchPolicy,
    Result,
};

/// EPRFS storage adapter backed by a local git object database.
///
/// This is a local proof adapter. It resolves `git-blob:<oid>` blob IDs emitted
/// by `project_tree()` into bytes from the repository's object database. It
/// intentionally does not publish attestations or write blobs.
pub struct GitObjectStorage {
    repo_path: PathBuf,
}

impl GitObjectStorage {
    pub fn open(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = repo_path.as_ref();
        gix::open(repo_path).map_err(|source| {
            EprfsError::Storage(format!(
                "failed to open git repository at {}: {source}",
                repo_path.display()
            ))
        })?;
        Ok(Self {
            repo_path: repo_path.to_path_buf(),
        })
    }

    fn parse_git_blob_cid(cid: &BlobCid) -> Result<gix::ObjectId> {
        let Some(hex) = cid.as_str().strip_prefix("git-blob:") else {
            return Err(EprfsError::Storage(format!(
                "unsupported git object blob id: {}",
                cid.as_str()
            )));
        };

        gix::ObjectId::from_hex(hex.as_bytes())
            .map_err(|source| EprfsError::Storage(format!("invalid git blob object id {hex}: {source}")))
    }

    fn find_blob(&self, cid: &BlobCid) -> Result<Bytes> {
        let oid = Self::parse_git_blob_cid(cid)?;

        let repo = gix::open(&self.repo_path).map_err(|source| {
            EprfsError::Storage(format!(
                "failed to open git repository at {}: {source}",
                self.repo_path.display()
            ))
        })?;
        let object = repo
            .find_object(oid)
            .map_err(|source| EprfsError::Storage(format!("git object {oid} not found: {source}")))?;

        if object.kind != gix::object::Kind::Blob {
            return Err(EprfsError::Storage(format!(
                "git object {oid} is {:?}, expected blob",
                object.kind
            )));
        }

        Ok(Bytes::copy_from_slice(&object.data))
    }
}

#[async_trait]
impl EprfsStorage for GitObjectStorage {
    async fn resolve_epr(&self, reference: &EprRef) -> Result<EprRecord> {
        Err(EprfsError::Storage(format!(
            "git object storage cannot resolve EPR records: {}",
            reference.as_str()
        )))
    }

    async fn has_blob(&self, cid: &BlobCid) -> Result<BlobPresence> {
        match self.find_blob(cid) {
            Ok(_) => Ok(BlobPresence::Local),
            Err(EprfsError::Storage(_)) => Ok(BlobPresence::Missing),
            Err(error) => Err(error),
        }
    }

    async fn fetch_blob(&self, cid: &BlobCid, _policy: FetchPolicy) -> Result<BlobHandle> {
        Ok(BlobHandle {
            cid: cid.clone(),
            bytes: self.find_blob(cid)?,
        })
    }

    async fn put_blob(&self, _bytes: Bytes) -> Result<BlobCid> {
        Err(EprfsError::Storage(
            "git object storage is read-only for eprfs blobs".into(),
        ))
    }

    async fn publish_attestation(&self, _draft: AttestationDraft) -> Result<EprRef> {
        Err(EprfsError::Storage(
            "git object storage cannot publish attestations".into(),
        ))
    }
}
