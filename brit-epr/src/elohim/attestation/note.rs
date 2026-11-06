//! Versioned brit notes: the frozen legacy format (epoch 1) beside the envelope format (epoch 2).
//!
//! # Epoch 1 — frozen
//!
//! Every note brit wrote before the address crossing: the typed node itself, carrying
//! `agentId` (raw 32-byte ed25519 key, hex) and `signature` (hex ed25519 over the node's
//! canonical DAG-CBOR with `"signature": ""` present). It has no `epoch` field on the wire.
//! Its path is [`crate::engine::signing::verify_signed_node`], unchanged byte-for-byte;
//! historical notes are never re-encoded or rewritten.
//!
//! # Epoch 2 — the envelope format (`brit.<kind>@2`)
//!
//! ```text
//! { "epoch": 2, "scheme": u16, "key": [multikey bytes], "sig": [signature bytes], "body": { … } }
//! ```
//!
//! `body` is the kind's attestation without a signature field (it still carries `agentId` /
//! `validatorId`, the legacy identity). The signed bytes are `elohim_epr`'s transcript v1:
//!
//! ```text
//! u16be(len(domain)) ‖ domain ‖ u16be(epoch = 2) ‖ u16be(scheme) ‖ u16be(len(key)) ‖ key
//!   ‖ dag-cbor(body)
//! ```
//!
//! with the domain `brit:<kind>@2:` (`brit:build@2:`, `brit:validate@2:`, `brit:deploy@2:`,
//! `brit:covenant@2:`). The envelope key is a must-match witness: it must equal
//! `ed 01 ‖ agentId` — the signer the body names, established independently of the envelope.
//!
//! # Dispatch
//!
//! A reader looks only at the `epoch` key: absent or `1` → the frozen path (a legacy note never
//! carries `epoch`, so an explicit `1` fails its strict decode); `2` → the envelope path; any
//! other `u16` → [`HeldReason::Epoch`]. Under epoch 2 a scheme this build does not implement →
//! [`HeldReason::Scheme`]. Held is not invalid: such a note is neither authentic nor forged
//! here, and confers no effect.

use std::fmt;

use elohim_epr::signature::{signing_transcript, Multikey, SignatureCheck, SignatureEnvelope, SignatureScheme};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::engine::{
    cid::BritCid,
    content_node::{CborError, ContentNode},
    signing::{
        ed25519_multikey_of_agent_id, verify_signed_node, AgentKey, AgentKeyError, Signed, ED25519_SCHEME,
        ENVELOPE_NOTE_EPOCH, LEGACY_NOTE_EPOCH,
    },
};

/// One kind's envelope-format body, tied to its frozen legacy node.
pub trait NoteBody: Serialize + DeserializeOwned + Clone + PartialEq + fmt::Debug {
    /// The kind's frozen epoch-1 node.
    type Legacy: Signed + PartialEq + fmt::Debug;
    /// The note format name, e.g. `brit.build@2` (the stored note's content type).
    const FORMAT: &'static str;
    /// The signature domain, e.g. `brit:build@2:`.
    const DOMAIN: &'static [u8];
    /// The legacy identity the body names: hex of the raw 32-byte ed25519 key.
    fn agent_id(&self) -> &str;
    /// The body a legacy node says, without its signature.
    fn from_legacy(legacy: &Self::Legacy) -> Self;
}

/// Why a note is held: a frame this reader does not implement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HeldReason {
    /// A note epoch other than 1 (frozen) or 2 (envelope).
    Epoch(u16),
    /// An epoch-2 note under a signature scheme this build does not implement.
    Scheme(u16),
}

impl fmt::Display for HeldReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Epoch(epoch) => write!(f, "note epoch {epoch} is not implemented by this reader"),
            Self::Scheme(scheme) => write!(f, "signature scheme {scheme} is not implemented by this reader"),
        }
    }
}

/// Why an envelope note could not be signed.
#[derive(Debug, thiserror::Error)]
pub enum NoteSignError {
    /// The body does not encode to DAG-CBOR.
    #[error(transparent)]
    Encode(#[from] CborError),
    /// The body names a signer other than the key signing it.
    #[error("body names agent {body:?} but the signing key is {key:?}")]
    SignerMismatch {
        /// The body's `agentId`.
        body: String,
        /// The signing key's agent id.
        key: String,
    },
}

/// A note in the envelope format (epoch 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvelopeNote<B> {
    /// The note epoch; [`ENVELOPE_NOTE_EPOCH`] for every note this build writes.
    pub epoch: u16,
    /// The signature scheme code (`0` = Ed25519).
    pub scheme: u16,
    /// The signer's key as canonical multikey bytes.
    #[serde(with = "serde_bytes")]
    pub key: Vec<u8>,
    /// The signature bytes.
    #[serde(with = "serde_bytes")]
    pub sig: Vec<u8>,
    /// The attestation.
    pub body: B,
}

impl<B: NoteBody> ContentNode for EnvelopeNote<B> {
    fn content_type(&self) -> &'static str {
        B::FORMAT
    }
}

/// The canonical bytes a body is signed as: its DAG-CBOR.
fn body_bytes<B: Serialize>(body: &B) -> Result<Vec<u8>, CborError> {
    serde_ipld_dagcbor::to_vec(body).map_err(|e| CborError::new(e.to_string()))
}

impl<B: NoteBody> EnvelopeNote<B> {
    /// Sign `body` with `key` under scheme 0 (Ed25519) at epoch 2. The body must name the
    /// signing key as its `agentId`.
    pub fn sign(body: B, key: &AgentKey) -> Result<Self, NoteSignError> {
        if body.agent_id() != key.agent_id() {
            return Err(NoteSignError::SignerMismatch {
                body: body.agent_id().to_string(),
                key: key.agent_id(),
            });
        }
        let multikey = Multikey::ed25519(&key.public_key_bytes());
        let transcript = signing_transcript(
            B::DOMAIN,
            ENVELOPE_NOTE_EPOCH,
            ED25519_SCHEME,
            multikey.as_bytes(),
            &body_bytes(&body)?,
        )
        .expect("a registered brit domain and a 34-byte Ed25519 multikey frame in transcript v1");
        Ok(Self {
            epoch: ENVELOPE_NOTE_EPOCH,
            scheme: ED25519_SCHEME,
            key: multikey.as_bytes().to_vec(),
            sig: key.sign_raw(&transcript).to_vec(),
            body,
        })
    }

    /// The signature as an `elohim_epr` envelope.
    pub fn envelope(&self) -> SignatureEnvelope {
        SignatureEnvelope {
            scheme: self.scheme,
            key: self.key.clone(),
            sig: self.sig.clone(),
        }
    }

    /// Verify against the signer the body names. `Err(HeldEpoch)` for an epoch other than 2,
    /// `Err(HeldScheme)` for a scheme this build does not implement, `Ok(false)` when the
    /// signature, the key material or the key-equals-`agentId` witness does not hold.
    pub fn verify(&self) -> Result<bool, AgentKeyError> {
        if self.epoch != ENVELOPE_NOTE_EPOCH {
            return Err(AgentKeyError::HeldEpoch(self.epoch));
        }
        if SignatureScheme::from_code(self.scheme).is_none() {
            return Err(AgentKeyError::HeldScheme(self.scheme));
        }
        let expected = ed25519_multikey_of_agent_id(self.body.agent_id())
            .and_then(|bytes| Multikey::from_bytes(bytes).ok())
            .ok_or(AgentKeyError::InvalidPubkeyHex)?;
        let body = body_bytes(&self.body).map_err(|e| AgentKeyError::Io(std::io::Error::other(e)))?;
        match self.envelope().check_for(&expected, B::DOMAIN, self.epoch, &body) {
            SignatureCheck::Verified => Ok(true),
            SignatureCheck::Invalid => Ok(false),
            SignatureCheck::Held { scheme } => Err(AgentKeyError::HeldScheme(scheme)),
        }
    }
}

/// A note of one kind in either format.
#[derive(Debug, Clone, PartialEq)]
pub enum VersionedNote<B: NoteBody> {
    /// The frozen legacy format (epoch 1).
    Legacy(B::Legacy),
    /// The envelope format (epoch 2).
    Envelope(EnvelopeNote<B>),
}

impl<B: NoteBody> VersionedNote<B> {
    /// What the note attests, whichever format carries it.
    pub fn body(&self) -> B {
        match self {
            Self::Legacy(legacy) => B::from_legacy(legacy),
            Self::Envelope(note) => note.body.clone(),
        }
    }

    /// The note epoch: 1 for the frozen format, the stored epoch otherwise.
    pub fn epoch(&self) -> u16 {
        match self {
            Self::Legacy(legacy) => legacy.epoch(),
            Self::Envelope(note) => note.epoch,
        }
    }

    /// The signature scheme code.
    pub fn scheme(&self) -> u16 {
        match self {
            Self::Legacy(legacy) => legacy.scheme(),
            Self::Envelope(note) => note.scheme,
        }
    }

    /// The signer's key as multikey bytes (`ed 01 ‖ agentId` for the frozen format).
    pub fn key(&self) -> Vec<u8> {
        match self {
            Self::Legacy(legacy) => legacy.key(),
            Self::Envelope(note) => note.key.clone(),
        }
    }

    /// The signer the note names (`agentId`, or `validatorId` for a validation note).
    pub fn agent_id(&self) -> String {
        match self {
            Self::Legacy(legacy) => legacy.agent_id().to_string(),
            Self::Envelope(note) => note.body.agent_id().to_string(),
        }
    }

    /// Verify through the note's own format: the frozen legacy path, or the envelope path.
    pub fn verify(&self) -> Result<bool, AgentKeyError> {
        match self {
            Self::Legacy(legacy) => verify_signed_node(legacy),
            Self::Envelope(note) => note.verify(),
        }
    }

    /// The CID of the stored note's canonical DAG-CBOR (the whole signed note).
    pub fn compute_cid(&self) -> Result<BritCid, CborError> {
        match self {
            Self::Legacy(legacy) => legacy.compute_cid(),
            Self::Envelope(note) => note.compute_cid(),
        }
    }

    /// The flat view tools print: a legacy note as it is stored; an envelope note as its body's
    /// fields plus `epoch`, `scheme`, `key` and `sig` (byte arrays, as stored).
    pub fn view(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            Self::Legacy(legacy) => serde_json::to_value(legacy),
            Self::Envelope(note) => {
                let mut view = serde_json::to_value(&note.body)?;
                if let serde_json::Value::Object(map) = &mut view {
                    map.insert("epoch".into(), note.epoch.into());
                    map.insert("scheme".into(), note.scheme.into());
                    map.insert("key".into(), note.key.clone().into());
                    map.insert("sig".into(), note.sig.clone().into());
                }
                Ok(view)
            }
        }
    }
}

impl<B: NoteBody> Serialize for VersionedNote<B> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Legacy(legacy) => legacy.serialize(serializer),
            Self::Envelope(note) => note.serialize(serializer),
        }
    }
}

/// What the bytes of a stored note decode to.
#[derive(Debug, Clone, PartialEq)]
pub enum Decoded<B: NoteBody> {
    /// A note in a format this reader implements (not yet verified).
    Note(VersionedNote<B>),
    /// A note in an epoch this reader does not implement.
    Held(HeldReason),
}

/// The `epoch` key alone; every other key is ignored here and checked by the strict decode.
#[derive(Deserialize)]
struct EpochPeek {
    #[serde(default)]
    epoch: Option<serde_json::Value>,
}

/// Decode a stored note's JSON strictly, dispatching on its `epoch` key (see the module docs).
/// The bytes are parsed as stored: never repaired, never read through a lossy conversion. `Err`
/// is why the bytes are not a note.
pub fn decode_note<B: NoteBody>(bytes: &[u8]) -> Result<Decoded<B>, String> {
    let peek: EpochPeek = serde_json::from_slice(bytes).map_err(|e| format!("does not decode: {e}"))?;
    let epoch = match peek.epoch {
        None => LEGACY_NOTE_EPOCH,
        Some(value) => value
            .as_u64()
            .and_then(|n| u16::try_from(n).ok())
            .ok_or_else(|| format!("does not decode: epoch {value} is not a u16"))?,
    };
    match epoch {
        LEGACY_NOTE_EPOCH => serde_json::from_slice::<B::Legacy>(bytes)
            .map(|node| Decoded::Note(VersionedNote::Legacy(node)))
            .map_err(|e| format!("does not decode: {e}")),
        ENVELOPE_NOTE_EPOCH => serde_json::from_slice::<EnvelopeNote<B>>(bytes)
            .map(|note| Decoded::Note(VersionedNote::Envelope(note)))
            .map_err(|e| format!("does not decode: {e}")),
        other => Ok(Decoded::Held(HeldReason::Epoch(other))),
    }
}
