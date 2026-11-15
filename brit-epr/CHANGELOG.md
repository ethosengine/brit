# Changelog — brit-epr

All notable changes to `brit-epr` (and the brit crates released with it) are recorded here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/); versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 — the address crossing (re-genesis 1)

Released together with `brit-graph` 0.2.0 (follows the `BritCid` type), `brit-build-ref` 0.1.6
and `brit-cli` 0.1.13.

### Changed (BREAKING)

- **One addressing kernel.** `BritCid` mints through `elohim-address` 0.1 instead of its own
  multihash code: `compute` (records, dag-cbor + sha2-256, `bafyrei…`) and `compute_raw`
  (citations only, raw + sha2-256, `bafkrei…`) are unchanged in output; the local constructor
  code is gone. `short_fingerprint` now returns `Option<String>` — `None` for any digest that is
  not sha2-256, because a `sha256:` label over a BLAKE3 digest would be a lie.
- **HELD is a typed outcome.** `refs::read_verified` and `refs::admit` return
  `Admission::{Verified(note), Held { reason }, Invalid { reason }}` instead of
  `Result<_, String>`. `Lookup` gains `Held { at, reason }`; `Scan` gains `held`. A held note —
  one framed in a note epoch or signature scheme this reader does not implement — is never
  verified, never selected and never a reuse candidate, and is reported apart from the invalid.
- **Versioned notes.** The ref manager carries `BuildNote`, `DeployNote`, `ValidationNote` and
  `CovenantClause` (each a `VersionedNote<Body>`): the frozen legacy node or an envelope note.
  `put_*` accept either (`impl Into<…>`, so a legacy node still converts). `CovenantRecord` is
  read only through `decode_covenant_record` / the verified reads; it no longer derives
  `Deserialize`.
- `AgentKeyError` gains `HeldScheme(u16)` and `HeldEpoch(u16)`.
- `rust-version` 1.82 → 1.83 (the kernel's floor).

### Added

- `BritCid::compute_bytes` — payload bytes under the epoch-0 byte profile (raw `0x55` +
  BLAKE3-256, `bafkr4i…`) — and `BritCid::parse_bytes`, the kernel's full-decode byte-surface
  parser.
- The envelope note format (`brit.build@2`, `brit.validate@2`, `brit.deploy@2`,
  `brit.covenant@2`, module `elohim::attestation::note`): `{epoch: 2, scheme, key, sig, body}`,
  signed over `elohim_epr`'s transcript v1 with the domain `brit:<kind>@2:`; the envelope key must
  equal `ed01 ‖ agentId`. Readers dispatch on `epoch`: absent/1 → frozen, 2 → envelope, other →
  held.
- `Signed::epoch()` / `scheme()` / `key()`, defaulting to the legacy frame (1, 0, `ed01 ‖ agentId`).
- `AgentKey::sign_raw` and `AgentKey::public_key_bytes`.
- Tests: `cid_vectors` (the kernel's vendored conformance vectors), `note_formats` (the frozen
  historical fixture, the `@2` wire vector, the downgrade refusal, held through the ref manager).

### Unchanged by design

- The frozen legacy format: its serializer, its `"signature": ""` preimage and
  `verify_signed_node` are byte-for-byte as before; historical notes are never re-encoded.
  `v1_note_bytes_are_frozen` pins a real historical note's preimage, signature and CID.

### brit-build-ref 0.1.6

- `build put`, `deploy put`, `validate put` and `covenant put` write the `@2` envelope format.
- `build put --manifest-path` and `meta seal` address file bytes with the byte profile
  (`bafkr4i…`).
- `build find` / `get` (and deploy, validate and covenant reads) print each verified note's view:
  a legacy note as stored; an envelope note as its body's fields plus `epoch`, `scheme`, `key`,
  `sig`. A held note is named on stderr as `held: <where> <reason>`, never as a row.

### Repository

- The rust-style checker archive is held under its byte CID
  (`.epr-meta/rust-style/objects/bafkr4i….tar.gz`, declaration `rust-style-source-holding@2`;
  sha256 stays as labelled evidence). The installer recomputes the CID with `b3sum`;
  `rust-style.sh` admits only `objects/bafkr4i[a-z2-7]{52}\.tar\.gz`.
