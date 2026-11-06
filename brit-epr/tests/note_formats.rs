//! The two note formats: the frozen legacy format (epoch 1) and the envelope format (epoch 2,
//! `brit.<kind>@2`), and the reader's third outcome, HELD.
//!
//! The frozen fixture is a real historical note: the build note brit wrote on monorepo commit
//! `1edce85de842` under `refs/notes/brit/build/gate%3Aelohim-epr` (blob `57ff1343fc1f`), stored
//! byte-for-byte in `fixtures/frozen-v1/build-note.json`. Its signature preimage,
//! `fixtures/frozen-v1/build-note.preimage.cbor`, was derived outside brit (a hand-written
//! DAG-CBOR encoder over the note with `"signature": ""`); the ed25519 signature verifying over
//! those bytes is what proves they are the preimage.
#![cfg(feature = "elohim-protocol")]

use std::{path::Path, process::Command};

use brit_epr::{
    elohim::{
        attestation::{
            build::{BuildAttestationContentNode, BuildBody, BuildNote},
            note::{decode_note, Decoded, EnvelopeNote, HeldReason, NoteBody, VersionedNote},
        },
        refs::{admit, read_verified, Admission, BritRefManager, Lookup},
    },
    engine::{
        cid::BritCid,
        content_node::ContentNode,
        signing::{verify_signature, verify_signed_node, AgentKey, AgentKeyError, Signed},
    },
};
use serde_json::{json, Value};
use tempfile::TempDir;

const FROZEN_NOTE: &[u8] = include_bytes!("fixtures/frozen-v1/build-note.json");
const FROZEN_PREIMAGE: &[u8] = include_bytes!("fixtures/frozen-v1/build-note.preimage.cbor");
const FROZEN_STEP: &str = "gate:elohim-epr";
const FROZEN_AGENT: &str = "1fc315045b0ca83bc916e6d2881838df4d86852607cf6d9ede4f65e778f69077";
const FROZEN_SIGNATURE: &str = "558b1580b69d1d5e6e65bd2e85dd45d1e08e8df49bb17f81f0e9e95b506ed6d7\
2c628493c964b5a687a21ca38049cc92e45c6ae8bc0d7d10e3d2b8889d529601";
/// CID of the signed historical node's canonical DAG-CBOR (derived outside brit, too).
const FROZEN_CID: &str = "bafyreif4xv5zdd6pl7rmxpzbtigyvtabthf22jen5y2vtaalsa4oo64phi";

/// A scheme code no build implements: the test-only hybrid (Ed25519 + a second component).
const HYBRID_SCHEME: u16 = 0xfe01;

fn frozen_node() -> BuildAttestationContentNode {
    serde_json::from_slice(FROZEN_NOTE).expect("the historical note decodes")
}

fn bind_frozen_step(b: &BuildBody) -> Result<(), String> {
    if b.step_name == FROZEN_STEP {
        Ok(())
    } else {
        Err(format!("stepName {:?}", b.step_name))
    }
}

fn read(bytes: &[u8]) -> Admission<BuildNote> {
    read_verified(bytes, bind_frozen_step)
}

fn seeded_key(dir: &TempDir, name: &str, seed: u8) -> AgentKey {
    let path = dir.path().join(name);
    std::fs::write(&path, [seed; 32]).expect("seed file");
    AgentKey::load(&path).expect("seeded key loads")
}

fn body(agent_id: &str) -> BuildBody {
    BuildBody {
        manifest_cid: BritCid::compute_bytes(b"manifest"),
        step_name: FROZEN_STEP.to_string(),
        inputs_hash: "closure-1".to_string(),
        output_cid: BritCid::compute(&[0xa0]),
        agent_id: agent_id.to_string(),
        hardware_profile: json!({ "arch": "x86_64" }),
        build_duration_ms: 1200,
        built_at: "2026-11-06T00:00:00Z".to_string(),
        success: true,
    }
}

fn json_bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).expect("JSON encodes")
}

fn is_invalid<T: std::fmt::Debug>(admission: &Admission<T>) -> bool {
    matches!(admission, Admission::Invalid { .. })
}

/// `u16be(len(domain)) ‖ domain ‖ u16be(epoch) ‖ u16be(scheme) ‖ u16be(len(key)) ‖ key ‖ body`,
/// written out by hand so the wire framing is pinned independently of `elohim_epr`.
fn transcript(domain: &[u8], epoch: u16, scheme: u16, key: &[u8], body: &[u8]) -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(&u16::try_from(domain.len()).unwrap().to_be_bytes());
    t.extend_from_slice(domain);
    t.extend_from_slice(&epoch.to_be_bytes());
    t.extend_from_slice(&scheme.to_be_bytes());
    t.extend_from_slice(&u16::try_from(key.len()).unwrap().to_be_bytes());
    t.extend_from_slice(key);
    t.extend_from_slice(body);
    t
}

// ─── the frozen legacy format ────────────────────────────────────────────────

#[test]
fn v1_note_bytes_are_frozen() {
    // The fixture preimage IS the preimage: the historical signature verifies over it directly.
    assert!(verify_signature(FROZEN_PREIMAGE, FROZEN_SIGNATURE, FROZEN_AGENT).unwrap());

    let node = frozen_node();
    assert_eq!(node.signature, FROZEN_SIGNATURE);
    assert_eq!(node.agent_id, FROZEN_AGENT);
    // Any change to the legacy serializer — a field, its name, its order, an attribute — moves
    // these bytes and fails here.
    let preimage = node.without_signature().canonical_bytes().unwrap();
    assert_eq!(preimage, FROZEN_PREIMAGE, "the frozen preimage serializer moved");
    // `"signature": ""` is in the preimage: text(9) "signature", then text(0).
    assert!(
        preimage.windows(11).any(|w| w == b"isignature`"),
        "the empty signature is part of what was signed"
    );
    assert_eq!(node.compute_cid().unwrap().to_string(), FROZEN_CID);
    assert!(verify_signed_node(&node).unwrap());

    // The JSON a note is stored as is unchanged too: re-encoding the decoded node reproduces the
    // stored bytes (compact JSON, then the newline `git notes add -m` always wrote).
    let mut stored = serde_json::to_vec(&serde_json::to_value(&node).unwrap()).unwrap();
    stored.push(b'\n');
    assert_eq!(stored, FROZEN_NOTE);
}

#[test]
fn historical_v1_note_verifies_through_the_frozen_path() {
    let Admission::Verified(VersionedNote::Legacy(node)) = read(FROZEN_NOTE) else {
        panic!("the historical note is a verified legacy note");
    };
    assert_eq!(node, frozen_node());
    let note = BuildNote::Legacy(node);
    assert_eq!(note.epoch(), 1);
    assert_eq!(note.scheme(), 0);
    let mut key = vec![0xed, 0x01];
    key.extend_from_slice(&hex::decode(FROZEN_AGENT).unwrap());
    assert_eq!(
        note.key(),
        key,
        "the legacy identity is the raw agentId, as the multikey ed01 ‖ key"
    );
    // The view tools print is the stored note itself.
    assert_eq!(
        note.view().unwrap(),
        serde_json::from_slice::<Value>(FROZEN_NOTE).unwrap()
    );
}

// ─── the envelope format ─────────────────────────────────────────────────────

#[test]
fn envelope_note_signs_the_brit_build_at_2_transcript() {
    let dir = TempDir::new().unwrap();
    let key = seeded_key(&dir, "k", 42);
    let note = EnvelopeNote::sign(body(&key.agent_id()), &key).unwrap();
    assert_eq!((note.epoch, note.scheme), (2, 0));
    let mut multikey = vec![0xed, 0x01];
    multikey.extend_from_slice(&key.public_key_bytes());
    assert_eq!(note.key, multikey, "the envelope key is ed01 ‖ agentId");
    assert_eq!(note.sig.len(), 64);
    assert_eq!(note.content_type(), "brit.build@2");

    let body_cbor = serde_ipld_dagcbor::to_vec(&note.body).unwrap();
    let t = transcript(b"brit:build@2:", 2, 0, &note.key, &body_cbor);
    assert!(verify_signature(&t, &hex::encode(&note.sig), &key.agent_id()).unwrap());
    assert!(note.verify().unwrap());

    // Stored, read back, verified through the envelope path.
    let bytes = serde_json::to_vec(&BuildNote::Envelope(note.clone())).unwrap();
    let stored: Value = serde_json::from_slice(&bytes).unwrap();
    let mut keys: Vec<&str> = stored.as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["body", "epoch", "key", "scheme", "sig"]);
    assert_eq!(read(&bytes), Admission::Verified(BuildNote::Envelope(note)));
}

#[test]
fn envelope_note_wire_vector_is_stable() {
    // Seed [42; 32], the fixed body above: ed25519 is deterministic, so the stored bytes are.
    let dir = TempDir::new().unwrap();
    let key = seeded_key(&dir, "k", 42);
    let note = EnvelopeNote::sign(body(&key.agent_id()), &key).unwrap();
    assert_eq!(
        key.agent_id(),
        "197f6b23e16c8532c6abc838facd5ea789be0c76b2920334039bfa8b3d368d61"
    );
    assert_eq!(
        hex::encode(&note.sig),
        include_str!("fixtures/envelope-v2/build-note.sig.hex").trim()
    );
    // Stored as `put_build_ref` stores every note: its JSON value, compact (keys sorted).
    let stored = serde_json::to_vec(&serde_json::to_value(BuildNote::Envelope(note.clone())).unwrap()).unwrap();
    assert_eq!(
        stored,
        include_bytes!("fixtures/envelope-v2/build-note.json").trim_ascii_end(),
        "the @2 wire bytes moved"
    );
    assert_eq!(
        note.compute_cid().unwrap().to_string(),
        include_str!("fixtures/envelope-v2/build-note.cid").trim()
    );
}

#[test]
fn envelope_signature_binds_the_kind_domain() {
    let dir = TempDir::new().unwrap();
    let key = seeded_key(&dir, "k", 42);
    let mut note = EnvelopeNote::sign(body(&key.agent_id()), &key).unwrap();
    let body_cbor = serde_ipld_dagcbor::to_vec(&note.body).unwrap();
    // The same body, key, epoch and scheme signed under another kind's domain.
    note.sig = key
        .sign_raw(&transcript(b"brit:validate@2:", 2, 0, &note.key, &body_cbor))
        .to_vec();
    assert!(!note.verify().unwrap());
    assert!(is_invalid(&read(
        &serde_json::to_vec(&BuildNote::Envelope(note)).unwrap()
    )));
}

#[test]
fn envelope_key_must_equal_agent_id() {
    let dir = TempDir::new().unwrap();
    let named = seeded_key(&dir, "named", 1);
    let signer = seeded_key(&dir, "signer", 2);
    // Signed correctly by `signer`, with `signer`'s key in the envelope, but naming `named`.
    let b = body(&named.agent_id());
    let mut multikey = vec![0xed, 0x01];
    multikey.extend_from_slice(&signer.public_key_bytes());
    let t = transcript(
        b"brit:build@2:",
        2,
        0,
        &multikey,
        &serde_ipld_dagcbor::to_vec(&b).unwrap(),
    );
    let note = EnvelopeNote {
        epoch: 2,
        scheme: 0,
        key: multikey,
        sig: signer.sign_raw(&t).to_vec(),
        body: b,
    };
    assert!(!note.verify().unwrap(), "the key witness does not hold");
    assert!(is_invalid(&read(
        &serde_json::to_vec(&BuildNote::Envelope(note)).unwrap()
    )));
    assert!(matches!(
        EnvelopeNote::sign(body(&named.agent_id()), &signer),
        Err(brit_epr::elohim::attestation::note::NoteSignError::SignerMismatch { .. })
    ));
}

// ─── HELD ────────────────────────────────────────────────────────────────────

#[test]
fn note_with_unknown_scheme_is_held_not_invalid() {
    let dir = TempDir::new().unwrap();
    let key = seeded_key(&dir, "k", 42);
    let mut note = EnvelopeNote::sign(body(&key.agent_id()), &key).unwrap();
    note.scheme = 7;
    assert!(matches!(note.verify(), Err(AgentKeyError::HeldScheme(7))));
    let bytes = serde_json::to_vec(&BuildNote::Envelope(note.clone())).unwrap();
    assert_eq!(
        read(&bytes),
        Admission::Held {
            reason: HeldReason::Scheme(7)
        }
    );
    // Held is decided before binding: a held note never reaches the bind check.
    assert_eq!(
        admit(BuildNote::Envelope(note), |_: &BuildBody| -> Result<(), String> {
            panic!("a held note is not bound")
        }),
        Admission::Held {
            reason: HeldReason::Scheme(7)
        }
    );
}

#[test]
fn note_with_unknown_epoch_is_held() {
    let dir = TempDir::new().unwrap();
    let key = seeded_key(&dir, "k", 42);
    let note = EnvelopeNote::sign(body(&key.agent_id()), &key).unwrap();
    let stored = serde_json::to_value(&note).unwrap();

    for epoch in [0u16, 3, 9, u16::MAX] {
        let mut v = stored.clone();
        v["epoch"] = json!(epoch);
        assert_eq!(
            read(&json_bytes(&v)),
            Admission::Held {
                reason: HeldReason::Epoch(epoch)
            },
            "epoch {epoch}"
        );
    }
    // An epoch whose shape is unknown is still held, never decoded: its body is not read.
    assert_eq!(
        decode_note::<BuildBody>(&json_bytes(&json!({ "epoch": 3, "anything": [1, 2, 3] }))),
        Ok(Decoded::Held(HeldReason::Epoch(3)))
    );
    // A struct's own epoch check holds too, whoever built it.
    let mut odd = note;
    odd.epoch = 5;
    assert!(matches!(odd.verify(), Err(AgentKeyError::HeldEpoch(5))));

    // Not epochs: these are malformed, so invalid.
    for bad in [json!(1), json!(-1), json!(70_000), json!("2"), json!(2.5), Value::Null] {
        let mut v = stored.clone();
        v["epoch"] = bad.clone();
        assert!(is_invalid(&read(&json_bytes(&v))), "epoch {bad}");
    }
}

/// Design-gate signature item 1: a record under a scheme this reader does not implement is held;
/// stripped down toward the legacy shape — `scheme` removed or zeroed, `key` removed, the Ed25519
/// component moved into the frozen format's `signature` — it is refused by `read_verified`, never
/// admitted as an Ed25519 note. The historical fixture still verifies beside it.
#[test]
fn hybrid_note_downgraded_toward_legacy_is_refused() {
    let dir = TempDir::new().unwrap();
    let key = seeded_key(&dir, "k", 42);
    let b = body(&key.agent_id());
    let body_cbor = serde_ipld_dagcbor::to_vec(&b).unwrap();

    // A test-only composite: multikey codec 0x1300 ‖ ed25519 key ‖ second-component key, and a
    // signature whose first 64 bytes are a genuine Ed25519 signature over the hybrid transcript.
    let mut composite_key = vec![0x80, 0x26];
    composite_key.extend_from_slice(&key.public_key_bytes());
    composite_key.extend_from_slice(&[0x5a; 32]);
    let ed_component = key.sign_raw(&transcript(
        b"brit:build@2:",
        2,
        HYBRID_SCHEME,
        &composite_key,
        &body_cbor,
    ));
    let mut composite_sig = ed_component.to_vec();
    composite_sig.extend_from_slice(&[0xa5; 64]);
    let hybrid = EnvelopeNote {
        epoch: 2,
        scheme: HYBRID_SCHEME,
        key: composite_key.clone(),
        sig: composite_sig,
        body: b.clone(),
    };
    let stored = serde_json::to_value(&hybrid).unwrap();
    assert_eq!(
        read(&json_bytes(&stored)),
        Admission::Held {
            reason: HeldReason::Scheme(HYBRID_SCHEME)
        },
        "the hybrid record is held"
    );

    let mut ed_key = vec![0xed, 0x01];
    ed_key.extend_from_slice(&key.public_key_bytes());
    let ed_sig = json!(ed_component.to_vec());
    let mut attempts: Vec<(&str, Value)> = Vec::new();

    let mut v = stored.clone();
    v["scheme"] = json!(0);
    attempts.push(("scheme zeroed, composite key kept", v));

    let mut v = stored.clone();
    v["scheme"] = json!(0);
    v["key"] = json!(ed_key);
    v["sig"] = ed_sig.clone();
    attempts.push(("scheme zeroed, ed25519 key and component", v));

    let mut v = stored.clone();
    v.as_object_mut().unwrap().remove("scheme");
    v["key"] = json!(ed_key);
    v["sig"] = ed_sig.clone();
    attempts.push(("scheme removed", v));

    let mut v = stored.clone();
    v["scheme"] = json!(0);
    v.as_object_mut().unwrap().remove("key");
    v["sig"] = ed_sig.clone();
    attempts.push(("key removed", v));

    // The frozen shape: the body's fields, `signature` = the Ed25519 component, no envelope.
    let mut legacy = serde_json::to_value(&b).unwrap();
    legacy["signature"] = json!(hex::encode(ed_component));
    attempts.push(("ed25519 component moved into the frozen signature", legacy.clone()));

    let mut v = legacy.clone();
    v["scheme"] = json!(0);
    attempts.push(("frozen shape with scheme 0", v));

    let mut v = legacy;
    v["scheme"] = json!(HYBRID_SCHEME);
    v["key"] = json!(composite_key);
    attempts.push(("frozen shape carrying the hybrid scheme and key", v));

    for (what, v) in attempts {
        let admission = read(&json_bytes(&v));
        assert!(is_invalid(&admission), "{what}: {admission:?}");
    }

    // The historical note verifies on the same reader.
    assert!(matches!(
        read(FROZEN_NOTE),
        Admission::Verified(VersionedNote::Legacy(_))
    ));
}

// ─── HELD through the ref manager ────────────────────────────────────────────

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(["-c", "user.email=t@t", "-c", "user.name=t"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn note_raw(dir: &Path, notes_ref: &str, commit: &str, bytes: &[u8]) {
    let path = dir.join(".note-bytes");
    std::fs::write(&path, bytes).unwrap();
    let blob = git(dir, &["hash-object", "-w", path.to_str().unwrap()]);
    git(dir, &["notes", "--ref", notes_ref, "add", "-f", "-C", &blob, commit]);
}

#[test]
fn held_notes_are_never_reuse_candidates() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();
    git(repo, &["init", "-q", "--initial-branch=main"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "a"]);
    let a = git(repo, &["rev-parse", "HEAD"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "b"]);
    let held_scheme = git(repo, &["rev-parse", "HEAD"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "c"]);
    let held_epoch = git(repo, &["rev-parse", "HEAD"]);

    let keys = TempDir::new().unwrap();
    let key = seeded_key(&keys, "k", 42);
    let note = EnvelopeNote::sign(body(&key.agent_id()), &key).unwrap();
    let refs = BritRefManager::new(repo).unwrap();
    refs.put_build_ref(FROZEN_STEP, &a, BuildNote::Envelope(note.clone()))
        .unwrap();

    // The same closure claimed by a note under an unimplemented scheme, and one under an
    // unimplemented epoch. A writer refuses to publish either; they arrive as foreign bytes.
    let notes_ref = "refs/notes/brit/build/gate%3Aelohim-epr";
    let mut v = serde_json::to_value(&note).unwrap();
    v["scheme"] = json!(HYBRID_SCHEME);
    note_raw(repo, notes_ref, &held_scheme, &json_bytes(&v));
    let mut v = serde_json::to_value(&note).unwrap();
    v["epoch"] = json!(3);
    note_raw(repo, notes_ref, &held_epoch, &json_bytes(&v));
    let mut held = serde_json::from_value::<EnvelopeNote<BuildBody>>(serde_json::to_value(&note).unwrap()).unwrap();
    held.scheme = HYBRID_SCHEME;
    assert!(
        refs.put_build_ref(FROZEN_STEP, &held_scheme, BuildNote::Envelope(held))
            .is_err(),
        "a held note is never written"
    );

    let scan = refs.find_build_by_inputs(FROZEN_STEP, "closure-1").unwrap();
    assert_eq!(scan.verified, vec![(a.clone(), BuildNote::Envelope(note))]);
    assert!(
        scan.unverifiable.is_empty(),
        "held is not invalid: {:?}",
        scan.unverifiable
    );
    let mut held = scan.held.clone();
    held.sort();
    let mut expected = vec![
        (held_scheme.clone(), HeldReason::Scheme(HYBRID_SCHEME)),
        (held_epoch.clone(), HeldReason::Epoch(3)),
    ];
    expected.sort();
    assert_eq!(held, expected);

    assert_eq!(
        refs.get_build_ref(FROZEN_STEP, &held_scheme).unwrap(),
        Lookup::Held {
            at: held_scheme,
            reason: HeldReason::Scheme(HYBRID_SCHEME)
        }
    );
    assert!(matches!(
        refs.get_build_ref(FROZEN_STEP, &a).unwrap(),
        Lookup::Verified(VersionedNote::Envelope(_))
    ));
}

#[test]
fn legacy_and_envelope_notes_answer_the_same_closure_side_by_side() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path();
    git(repo, &["init", "-q", "--initial-branch=main"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "a"]);
    let a = git(repo, &["rev-parse", "HEAD"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "b"]);
    let b = git(repo, &["rev-parse", "HEAD"]);

    let keys = TempDir::new().unwrap();
    let key = seeded_key(&keys, "k", 42);
    let refs = BritRefManager::new(repo).unwrap();
    // A historical note is carried exactly as stored.
    note_raw(repo, "refs/notes/brit/build/gate%3Aelohim-epr", &a, FROZEN_NOTE);
    let mut new_body = body(&key.agent_id());
    new_body.inputs_hash = frozen_node().inputs_hash;
    let note = EnvelopeNote::sign(new_body, &key).unwrap();
    refs.put_build_ref(FROZEN_STEP, &b, BuildNote::Envelope(note)).unwrap();

    let scan = refs
        .find_build_by_inputs(FROZEN_STEP, &frozen_node().inputs_hash)
        .unwrap();
    assert!(scan.unverifiable.is_empty() && scan.held.is_empty(), "{scan:?}");
    let epochs: Vec<(String, u16)> = scan.verified.iter().map(|(c, n)| (c.clone(), n.epoch())).collect();
    assert!(
        epochs.contains(&(a.clone(), 1)) && epochs.contains(&(b, 2)),
        "{epochs:?}"
    );
    // Reading never rewrites: the historical note is still the very blob brit wrote in 2026-11.
    let listing = git(
        repo,
        &["notes", "--ref", "refs/notes/brit/build/gate%3Aelohim-epr", "list"],
    );
    assert!(
        listing
            .lines()
            .any(|l| l == format!("57ff1343fc1f0b46c4104f40e37f21597a50aedc {a}")),
        "{listing}"
    );
    assert_eq!(BuildBody::FORMAT, "brit.build@2");
}
