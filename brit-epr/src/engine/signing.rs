//! Agent signing — ed25519 keypair management for attestation signatures.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use rand::RngCore;

/// An agent's signing identity, loaded from or generated to a file.
pub struct AgentKey {
    signing_key: SigningKey,
    key_path: PathBuf,
}

impl AgentKey {
    /// Load an existing key or generate a new one at the given path.
    pub fn load_or_generate(key_path: &Path) -> Result<Self, AgentKeyError> {
        match fs::read(key_path) {
            Ok(bytes) => Self::from_seed(key_path, bytes),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::generate(key_path),
            Err(err) => Err(AgentKeyError::Io(err)),
        }
    }

    /// Load from an existing 32-byte seed file.
    pub fn load(key_path: &Path) -> Result<Self, AgentKeyError> {
        let bytes = fs::read(key_path).map_err(AgentKeyError::Io)?;
        Self::from_seed(key_path, bytes)
    }

    fn from_seed(key_path: &Path, bytes: Vec<u8>) -> Result<Self, AgentKeyError> {
        if bytes.len() != 32 {
            return Err(AgentKeyError::InvalidKeyLength(bytes.len()));
        }
        let seed: [u8; 32] = bytes.try_into().map_err(|_| AgentKeyError::InvalidKeyLength(0))?;
        let signing_key = SigningKey::from_bytes(&seed);
        Ok(Self {
            signing_key,
            key_path: key_path.to_path_buf(),
        })
    }

    /// Generate a new keypair and atomically publish its 32-byte seed.
    pub fn generate(key_path: &Path) -> Result<Self, AgentKeyError> {
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut rng);
        if let Some(parent) = key_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(AgentKeyError::Io)?;
        }
        let temp_path = create_temp_seed_file(key_path, &signing_key.to_bytes(), &mut rng)?;
        match fs::hard_link(&temp_path, key_path) {
            Ok(()) => {
                let sync_result = fsync_parent_dir(key_path);
                let _ = fs::remove_file(&temp_path);
                sync_result.map_err(AgentKeyError::Io)?;
                Ok(Self {
                    signing_key,
                    key_path: key_path.to_path_buf(),
                })
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temp_path);
                Self::load(key_path)
            }
            Err(err) => {
                let _ = fs::remove_file(&temp_path);
                Err(AgentKeyError::Io(err))
            }
        }
    }

    /// Sign arbitrary bytes. Returns the 64-byte ed25519 signature as hex.
    pub fn sign(&self, payload: &[u8]) -> String {
        let sig = self.signing_key.sign(payload);
        hex::encode(sig.to_bytes())
    }

    /// The agent's public key as a 64-character hex string.
    pub fn agent_id(&self) -> String {
        hex::encode(self.signing_key.verifying_key().to_bytes())
    }

    /// The verifying (public) key.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing_key.verifying_key()
    }

    /// Path where the key is stored.
    pub fn key_path(&self) -> &Path {
        &self.key_path
    }
}

/// Write and sync a private, exclusive sibling without making it the active key.
fn create_temp_seed_file(key_path: &Path, seed: &[u8], rng: &mut impl RngCore) -> Result<PathBuf, AgentKeyError> {
    let file_name = key_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "agent-key".to_string());
    let mut random = [0u8; 8];
    rng.fill_bytes(&mut random);
    let random = hex::encode(random);
    let temp_path = key_path.with_file_name(format!(".{file_name}.{}.{random}.tmp", std::process::id()));

    let result = (|| -> std::io::Result<()> {
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;
            OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temp_path)?
        };
        #[cfg(not(unix))]
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temp_path)?;

        file.write_all(seed)?;
        file.sync_all()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o600))?;
            file.sync_all()?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => Ok(temp_path),
        Err(err) => {
            let _ = fs::remove_file(&temp_path);
            Err(AgentKeyError::Io(err))
        }
    }
}

#[cfg(unix)]
fn fsync_parent_dir(path: &Path) -> std::io::Result<()> {
    let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) else {
        return Ok(());
    };
    match std::fs::File::open(parent)?.sync_all() {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::Unsupported => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(not(unix))]
fn fsync_parent_dir(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Trait for ContentNodes that carry a signature field.
///
/// Implementors provide access to the signature and agent_id, plus the
/// ability to produce an unsigned clone for verification.
pub trait Signed: crate::engine::content_node::ContentNode + Clone {
    /// The hex-encoded signature.
    fn signature(&self) -> &str;
    /// The hex-encoded agent public key.
    fn agent_id(&self) -> &str;
    /// Return a clone with the signature field set to empty string.
    fn without_signature(&self) -> Self;
}

/// Verify a signed ContentNode's signature.
///
/// Zeros the signature field, computes canonical JSON, and verifies against
/// the agent_id embedded in the node.
pub fn verify_signed_node<T: Signed>(node: &T) -> Result<bool, AgentKeyError> {
    let unsigned = node.without_signature();
    let canonical = unsigned
        .canonical_bytes()
        .map_err(|e| AgentKeyError::Io(std::io::Error::other(e)))?;
    verify_signature(&canonical, node.signature(), node.agent_id())
}

/// Verify a hex-encoded signature against a hex-encoded public key.
pub fn verify_signature(payload: &[u8], signature_hex: &str, pubkey_hex: &str) -> Result<bool, AgentKeyError> {
    let sig_bytes = hex::decode(signature_hex).map_err(|_| AgentKeyError::InvalidSignatureHex)?;
    let sig = ed25519_dalek::Signature::from_slice(&sig_bytes).map_err(|_| AgentKeyError::InvalidSignatureHex)?;
    let pub_bytes = hex::decode(pubkey_hex).map_err(|_| AgentKeyError::InvalidPubkeyHex)?;
    let pubkey = VerifyingKey::from_bytes(&pub_bytes.try_into().map_err(|_| AgentKeyError::InvalidPubkeyHex)?)
        .map_err(|_| AgentKeyError::InvalidPubkeyHex)?;
    Ok(pubkey.verify_strict(payload, &sig).is_ok())
}

/// Agent key errors.
#[derive(Debug, thiserror::Error)]
pub enum AgentKeyError {
    /// Filesystem error.
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    /// Key file has wrong length.
    #[error("expected 32-byte key seed, got {0} bytes")]
    InvalidKeyLength(usize),
    /// Signature hex is invalid.
    #[error("invalid signature hex")]
    InvalidSignatureHex,
    /// Public key hex is invalid.
    #[error("invalid public key hex")]
    InvalidPubkeyHex,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn generate_load_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("brit").join("agent-key");
        let key1 = AgentKey::generate(&path).unwrap();
        let key2 = AgentKey::load(&path).unwrap();
        assert_eq!(key1.agent_id(), key2.agent_id());
    }

    #[test]
    fn sign_and_verify() {
        let tmp = TempDir::new().unwrap();
        let key = AgentKey::generate(&tmp.path().join("key")).unwrap();
        let payload = b"attestation payload";
        let sig = key.sign(payload);
        assert!(verify_signature(payload, &sig, &key.agent_id()).unwrap());
    }

    #[test]
    fn wrong_payload_fails_verify() {
        let tmp = TempDir::new().unwrap();
        let key = AgentKey::generate(&tmp.path().join("key")).unwrap();
        let sig = key.sign(b"original");
        assert!(!verify_signature(b"tampered", &sig, &key.agent_id()).unwrap());
    }

    #[test]
    fn load_or_generate_creates_if_missing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("agent-key");
        assert!(!path.exists());
        let key = AgentKey::load_or_generate(&path).unwrap();
        assert!(path.exists());
        assert_eq!(key.agent_id().len(), 64);
    }

    #[test]
    fn concurrent_first_generation_returns_the_published_identity() {
        let tmp = TempDir::new().expect("temporary key directory");
        let path = Arc::new(tmp.path().join("brit").join("agent-key"));
        let barrier = Arc::new(Barrier::new(2));
        let threads: Vec<_> = (0..2)
            .map(|_| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    AgentKey::generate(&path)
                        .expect("racing generation succeeds")
                        .agent_id()
                })
            })
            .collect();
        let agent_ids: Vec<_> = threads
            .into_iter()
            .map(|thread| thread.join().expect("generation thread does not panic"))
            .collect();

        assert_eq!(agent_ids[0], agent_ids[1], "both racers return the published key");
        assert_eq!(
            AgentKey::load(&path).expect("published key loads").agent_id(),
            agent_ids[0],
            "the shared identity is the one stored on disk"
        );
        let siblings = fs::read_dir(path.parent().expect("key has a parent"))
            .expect("key directory is readable")
            .count();
        assert_eq!(siblings, 1, "temporary seed files are removed");
    }

    #[cfg(unix)]
    #[test]
    fn generated_key_is_owner_read_write_only() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().expect("temporary key directory");
        let path = tmp.path().join("brit").join("agent-key");
        AgentKey::generate(&path).expect("key generation succeeds");

        let mode = fs::metadata(path)
            .expect("key metadata is readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "the published seed is never group- or world-readable");
    }

    #[cfg(unix)]
    #[test]
    fn unpublished_seed_is_private_before_atomic_publication() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().expect("temporary key directory");
        let key_path = tmp.path().join("brit").join("agent-key");
        fs::create_dir_all(key_path.parent().expect("key has a parent")).expect("create key directory");
        let temp_path = create_temp_seed_file(&key_path, &[7; 32], &mut rand::thread_rng())
            .expect("exclusive temporary seed creation succeeds");

        assert!(
            !key_path.exists(),
            "the final key name is not visible before publication"
        );
        let mode = fs::metadata(temp_path)
            .expect("temporary seed metadata is readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "the seed is private even before publication");
    }
}
