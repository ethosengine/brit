//! Session covenant clause — a signed Layer 2 blessing over a governance rule.
//!
//! brit *carries* clauses (builds, signs, addresses, indexes); it never *judges* them. Whether
//! the speaker has standing, whether a rule may be varied and whether a clause is still honoured
//! belong to the consuming governance plane. `party` is recorded verbatim. See
//! `docs/specs/2026-10-19-session-covenant-design.md`.

use serde::{Deserialize, Serialize};

use super::note::{decode_note, Decoded, NoteBody, VersionedNote};
use crate::engine::{cid::BritCid, content_node::ContentNode, signing::Signed};

/// What a clause says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CovenantClauseKind {
    /// The party affirms they are the steward of record for the session.
    AffirmsStewardOfRecord,
    /// The party declares the session's purpose and write set.
    DeclaresPurpose,
    /// The party varies (attenuates) a governance rule for the session.
    VariesRule,
    /// The party revokes an earlier clause.
    RevokesClause,
}

impl CovenantClauseKind {
    /// The camelCase wire name of the kind.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AffirmsStewardOfRecord => "affirmsStewardOfRecord",
            Self::DeclaresPurpose => "declaresPurpose",
            Self::VariesRule => "variesRule",
            Self::RevokesClause => "revokesClause",
        }
    }
}

impl std::str::FromStr for CovenantClauseKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "affirmsStewardOfRecord" => Ok(Self::AffirmsStewardOfRecord),
            "declaresPurpose" => Ok(Self::DeclaresPurpose),
            "variesRule" => Ok(Self::VariesRule),
            "revokesClause" => Ok(Self::RevokesClause),
            other => Err(format!(
                "unknown clause kind `{other}` (expected affirmsStewardOfRecord|declaresPurpose|variesRule|revokesClause)"
            )),
        }
    }
}

/// Payload of `affirmsStewardOfRecord`: empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AffirmsStewardPayload {}

/// Payload of `declaresPurpose`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaresPurposePayload {
    /// What the session is for.
    pub purpose: String,
    /// Globs the session intends to write.
    pub write_set: Vec<String>,
}

/// Payload of `variesRule`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VariesRulePayload {
    /// Name of the rule being varied.
    pub rule: String,
    /// Path of the `.epr-meta` manifest declaring the rule.
    pub manifest_path: String,
    /// CID of the exact Layer 1 manifest bytes being attenuated (string form).
    pub manifest_cid: String,
    /// Subset of the rule's paths the variance covers.
    pub paths: Vec<String>,
    /// A floor the rule itself declares; selected, never written.
    pub floor: String,
}

/// Payload of `revokesClause`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevokesClausePayload {
    /// CID (string form) of the clause being revoked.
    pub revokes: String,
}

/// Check that `payload` is a well-formed payload for `kind`; CID strings must parse.
pub fn validate_payload(kind: CovenantClauseKind, payload: &serde_json::Value) -> Result<(), String> {
    fn parse<T: serde::de::DeserializeOwned>(v: &serde_json::Value) -> Result<T, String> {
        serde_json::from_value(v.clone()).map_err(|e| e.to_string())
    }
    fn cid(label: &str, s: &str) -> Result<(), String> {
        s.parse::<BritCid>().map(|_| ()).map_err(|e| format!("{label}: {e}"))
    }
    match kind {
        CovenantClauseKind::AffirmsStewardOfRecord => parse::<AffirmsStewardPayload>(payload).map(|_| ()),
        CovenantClauseKind::DeclaresPurpose => parse::<DeclaresPurposePayload>(payload).map(|_| ()),
        CovenantClauseKind::VariesRule => {
            let p: VariesRulePayload = parse(payload)?;
            cid("manifestCid", &p.manifest_cid)
        }
        CovenantClauseKind::RevokesClause => {
            let p: RevokesClausePayload = parse(payload)?;
            cid("revokes", &p.revokes)
        }
    }
    .map_err(|e| format!("invalid payload for {}: {e}", kind.as_str()))
}

/// A signed record of one thing a party said about a session's covenant — the frozen legacy
/// (epoch 1) clause; its serializer is the signature preimage of every historical clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CovenantClauseContentNode {
    /// What the clause says.
    pub kind: CovenantClauseKind,
    /// The agent session the clause binds (opaque to brit).
    pub session: String,
    /// Who spoke, e.g. `human:matthew`; recorded, never judged.
    pub party: String,
    /// The party's own words, verbatim.
    pub quote: String,
    /// Kind-specific payload (see the `*Payload` types; checked by [`validate_payload`]).
    pub payload: serde_json::Value,
    /// ISO-8601 start of validity.
    pub valid_from: String,
    /// ISO-8601 end of validity; `None` means until the session ends.
    pub valid_until: Option<String>,
    /// An earlier clause this one restates.
    pub supersedes: Option<BritCid>,
    /// Public key (hex) of the device that recorded the clause.
    pub agent_id: String,
    /// Agent signature (hex ed25519) over the canonical bytes with this field empty.
    pub signature: String,
}

impl ContentNode for CovenantClauseContentNode {
    fn content_type(&self) -> &'static str {
        "brit.covenant-clause"
    }
}

impl Signed for CovenantClauseContentNode {
    fn signature(&self) -> &str {
        &self.signature
    }
    fn agent_id(&self) -> &str {
        &self.agent_id
    }
    fn without_signature(&self) -> Self {
        let mut c = self.clone();
        c.signature = String::new();
        c
    }
}

/// What a clause says, signed in the envelope format (`brit.covenant@2`): the legacy clause's
/// fields without `signature`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CovenantClauseBody {
    /// What the clause says.
    pub kind: CovenantClauseKind,
    /// The agent session the clause binds (opaque to brit).
    pub session: String,
    /// Who spoke, e.g. `human:matthew`; recorded, never judged.
    pub party: String,
    /// The party's own words, verbatim.
    pub quote: String,
    /// Kind-specific payload (see the `*Payload` types; checked by [`validate_payload`]).
    pub payload: serde_json::Value,
    /// ISO-8601 start of validity.
    pub valid_from: String,
    /// ISO-8601 end of validity; `None` means until the session ends.
    pub valid_until: Option<String>,
    /// An earlier clause this one restates.
    pub supersedes: Option<BritCid>,
    /// The recording device's raw ed25519 public key (hex); the envelope key must be `ed 01 ‖` it.
    pub agent_id: String,
}

impl NoteBody for CovenantClauseBody {
    type Legacy = CovenantClauseContentNode;
    const FORMAT: &'static str = "brit.covenant@2";
    const DOMAIN: &'static [u8] = b"brit:covenant@2:";

    fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn from_legacy(legacy: &CovenantClauseContentNode) -> Self {
        Self {
            kind: legacy.kind,
            session: legacy.session.clone(),
            party: legacy.party.clone(),
            quote: legacy.quote.clone(),
            payload: legacy.payload.clone(),
            valid_from: legacy.valid_from.clone(),
            valid_until: legacy.valid_until.clone(),
            supersedes: legacy.supersedes.clone(),
            agent_id: legacy.agent_id.clone(),
        }
    }
}

/// A clause in either format.
pub type CovenantClause = VersionedNote<CovenantClauseBody>;

impl From<&CovenantClauseContentNode> for CovenantClause {
    fn from(legacy: &CovenantClauseContentNode) -> Self {
        Self::Legacy(legacy.clone())
    }
}

impl From<&CovenantClause> for CovenantClause {
    fn from(clause: &CovenantClause) -> Self {
        clause.clone()
    }
}

/// The stored form of a clause: the blob at `refs/notes/brit/covenant/<session>/<cid>`.
///
/// `cid` is the CID of the signed clause's canonical bytes (the whole stored clause, in its own
/// format); a reader recomputes it from `clause` rather than trusting it. Records are read only
/// through [`decode_covenant_record`] and the verified reads.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CovenantRecord {
    /// CID (string form) of the signed clause.
    pub cid: String,
    /// The signed clause.
    pub clause: CovenantClause,
}

/// The stored record with its clause still as stored bytes, so the clause decodes from exactly
/// what was stored (duplicate or unknown keys are refused, never resolved).
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredRecord<'a> {
    cid: String,
    #[serde(borrow)]
    clause: &'a serde_json::value::RawValue,
}

/// Decode a stored record strictly: its `cid` string and its clause, dispatched on the clause's
/// epoch like any note. `Err` is why the bytes are not a record.
pub fn decode_covenant_record(bytes: &[u8]) -> Result<(String, Decoded<CovenantClauseBody>), String> {
    let StoredRecord { cid, clause } = serde_json::from_slice(bytes).map_err(|e| format!("does not decode: {e}"))?;
    Ok((cid, decode_note(clause.get().as_bytes())?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::signing::{verify_signed_node, AgentKey};

    fn signed_sample() -> CovenantClauseContentNode {
        let dir = tempfile::TempDir::new().unwrap();
        let key = AgentKey::load_or_generate(&dir.path().join("k")).unwrap();
        let manifest = BritCid::compute_raw(b"manifest bytes").to_string();
        let mut node = CovenantClauseContentNode {
            kind: CovenantClauseKind::VariesRule,
            session: "sess-1".into(),
            party: "human:matthew".into(),
            quote: "skip the ledger for this docs session".into(),
            payload: serde_json::json!({
                "rule": "ledger-required", "manifestPath": ".epr-meta/manifest.md",
                "manifestCid": manifest, "paths": ["docs/**"], "floor": "advisory"
            }),
            valid_from: "2026-10-19T10:00:00Z".into(),
            valid_until: None,
            supersedes: None,
            agent_id: key.agent_id(),
            signature: String::new(),
        };
        let bytes = node.canonical_bytes().unwrap();
        node.signature = key.sign(&bytes);
        node
    }

    #[test]
    fn cid_is_stable_across_serde_round_trips() {
        let node = signed_sample();
        let json: CovenantClauseContentNode = serde_json::from_str(&serde_json::to_string(&node).unwrap()).unwrap();
        let cbor: CovenantClauseContentNode = serde_ipld_dagcbor::from_slice(&node.canonical_bytes().unwrap()).unwrap();
        assert_eq!(node, json);
        assert_eq!(node, cbor);
        let cid = node.compute_cid().unwrap();
        assert_eq!(cid, json.compute_cid().unwrap());
        assert_eq!(cid, cbor.compute_cid().unwrap());
        assert_eq!(node.content_type(), "brit.covenant-clause");
    }

    #[test]
    fn signature_verifies_and_tampering_fails() {
        let node = signed_sample();
        assert!(verify_signed_node(&node).unwrap());

        let mut t = node.clone();
        t.quote.push('!');
        assert!(!verify_signed_node(&t).unwrap());
        let mut t = node.clone();
        t.party = "human:someone-else".into();
        assert!(!verify_signed_node(&t).unwrap());
        let mut t = node.clone();
        t.valid_until = Some("2030-01-01T00:00:00Z".into());
        assert!(!verify_signed_node(&t).unwrap());
        let mut t = node;
        t.kind = CovenantClauseKind::DeclaresPurpose;
        assert!(!verify_signed_node(&t).unwrap());
    }

    #[test]
    fn payload_validation_is_per_kind() {
        use CovenantClauseKind::*;
        assert!(validate_payload(AffirmsStewardOfRecord, &serde_json::json!({})).is_ok());
        assert!(validate_payload(AffirmsStewardOfRecord, &serde_json::json!({"x": 1})).is_err());
        assert!(validate_payload(
            DeclaresPurpose,
            &serde_json::json!({"purpose": "p", "writeSet": ["a/**"]})
        )
        .is_ok());
        assert!(validate_payload(DeclaresPurpose, &serde_json::json!({"purpose": "p"})).is_err());
        let c = BritCid::compute(b"x").to_string();
        assert!(validate_payload(RevokesClause, &serde_json::json!({ "revokes": c })).is_ok());
        assert!(validate_payload(RevokesClause, &serde_json::json!({"revokes": "not-a-cid"})).is_err());
        assert!(validate_payload(VariesRule, &serde_json::json!({"rule": "r"})).is_err());
    }

    #[test]
    fn clause_and_record_refuse_unknown_fields() {
        let node = signed_sample();
        let mut clause = serde_json::to_value(&node).unwrap();
        clause["admitted"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<CovenantClauseContentNode>(clause).is_err());
        let mut record = serde_json::to_value(CovenantRecord {
            cid: node.compute_cid().unwrap().to_string(),
            clause: (&node).into(),
        })
        .unwrap();
        let decode = |v: &serde_json::Value| decode_covenant_record(&serde_json::to_vec(v).unwrap());
        assert!(matches!(
            decode(&record),
            Ok((_, Decoded::Note(VersionedNote::Legacy(_))))
        ));
        record["standing"] = serde_json::Value::Bool(true);
        assert!(decode(&record).is_err());
        record.as_object_mut().unwrap().remove("standing");
        record["clause"]["admitted"] = serde_json::Value::Bool(true);
        assert!(decode(&record).is_err());
    }

    #[test]
    fn kind_wire_names_round_trip() {
        for k in [
            CovenantClauseKind::AffirmsStewardOfRecord,
            CovenantClauseKind::DeclaresPurpose,
            CovenantClauseKind::VariesRule,
            CovenantClauseKind::RevokesClause,
        ] {
            assert_eq!(k.as_str().parse::<CovenantClauseKind>().unwrap(), k);
            assert_eq!(serde_json::to_value(k).unwrap(), serde_json::json!(k.as_str()));
        }
    }
}
