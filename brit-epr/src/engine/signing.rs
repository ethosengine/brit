//! Agent signing — ed25519 keypair management for attestation signatures.

use std::fs;
use std::path::{Path, PathBuf};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};

/// An agent's signing identity, loaded from or generated to a file.
pub struct AgentKey {
    signing_key: SigningKey,
    key_path: PathBuf,
}

impl AgentKey {
    /// Load an existing key or generate a new one at the given path.
    pub fn load_or_generate(key_path: &Path) -> Result<Self, AgentKeyError> {
        if key_path.exists() {
            Self::load(key_path)
        } else {
            Self::generate(key_path)
        }
    }

    /// Load from an existing 32-byte seed file.
    pub fn load(key_path: &Path) -> Result<Self, AgentKeyError> {
        let bytes = fs::read(key_path).map_err(AgentKeyError::Io)?;
        if bytes.len() != 32 {
            return Err(AgentKeyError::InvalidKeyLength(bytes.len()));
        }
        let seed: [u8; 32] = bytes.try_into().map_err(|_| AgentKeyError::InvalidKeyLength(0))?;
        let signing_key = SigningKey::from_bytes(&seed);
        Ok(Self { signing_key, key_path: key_path.to_path_buf() })
    }

    /// Generate a new keypair and write the 32-byte seed to disk.
    pub fn generate(key_path: &Path) -> Result<Self, AgentKeyError> {
        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::generate(&mut rng);
        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent).map_err(AgentKeyError::Io)?;
        }
        fs::write(key_path, signing_key.to_bytes()).map_err(AgentKeyError::Io)?;
        // Restrict key file to owner-only read/write (0600) — the seed is a
        // private key and must not be readable by other users on shared CI.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(key_path, fs::Permissions::from_mode(0o600))
                .map_err(AgentKeyError::Io)?;
        }
        Ok(Self { signing_key, key_path: key_path.to_path_buf() })
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
        .canonical_json()
        .map_err(|e| AgentKeyError::Io(std::io::Error::other(e)))?;
    verify_signature(&canonical, node.signature(), node.agent_id())
}

/// Verify a hex-encoded signature against a hex-encoded public key.
pub fn verify_signature(
    payload: &[u8],
    signature_hex: &str,
    pubkey_hex: &str,
) -> Result<bool, AgentKeyError> {
    let sig_bytes = hex::decode(signature_hex).map_err(|_| AgentKeyError::InvalidSignatureHex)?;
    let sig = ed25519_dalek::Signature::from_slice(&sig_bytes)
        .map_err(|_| AgentKeyError::InvalidSignatureHex)?;
    let pub_bytes = hex::decode(pubkey_hex).map_err(|_| AgentKeyError::InvalidPubkeyHex)?;
    let pubkey = VerifyingKey::from_bytes(
        &pub_bytes.try_into().map_err(|_| AgentKeyError::InvalidPubkeyHex)?,
    ).map_err(|_| AgentKeyError::InvalidPubkeyHex)?;
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
    use super::*;
    use tempfile::TempDir;

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
}
