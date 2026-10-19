# Session covenant clauses — a Layer 2 blessing over governance

Date: 2026-10-19 · Status: design, rung 1 being built on `feat/session-covenant`
Extends: `2026-06-29-canonical-epr-meta-git-bridge-design.md` §7 (two-layer notarization) and the
master `2026-04-12-brit-design.md` ("Brit doesn't own governance").

## Problem

An `.epr-meta` rule binds every write that matches its path, whoever writes and for whatever
purpose. That is right by default: the writer's self-assessment is often exactly what the rule
cannot trust. But some sessions have a declared purpose that discharges a rule's concern, and there
the full obligation is waste. The person who can vary it is the steward of record, speaking once
per session, and the variance has to:

- be bounded to a scope and a time;
- be attributable to that person;
- be readable by every agent the session spawns;
- survive as evidence, because a variance may one day carry consequences.

The bridge spec already names the shape: **Layer 1** is static identity (an `EprMeta`'s CID, which
is authority-free), and **Layer 2** is a runtime blessing, a signed edge outside the content bytes
that may only restate or attenuate authority. A session-covenant clause is a Layer 2 edge whose
subject is a governance rule.

## Boundary

brit **carries** clauses: it builds them canonically, signs them, addresses them by CID, and
indexes and lists them. brit does **not judge** them. Whether the speaker has standing, whether a
rule may be varied, whether a clause is still honoured, and what its revocation means are all
decided by the consuming governance plane (the Elohim workspace's `epr`, later Mishpat). brit's
own manifest (`.epr-meta/manifest.md`) and the master spec require this split. `party` is
therefore recorded verbatim and never interpreted here.

## Record: `CovenantClauseContentNode` (content type `brit.covenant-clause`)

Behind the `elohim-protocol` feature, beside the build/validation attestations
(`brit-epr/src/elohim/attestation/covenant.rs`). camelCase serde, like its siblings.

| field | type | meaning |
|---|---|---|
| `kind` | enum `affirmsStewardOfRecord \| declaresPurpose \| variesRule \| revokesClause` | what the clause says |
| `session` | string | the agent session the clause binds (opaque to brit) |
| `party` | string | who spoke, e.g. `human:matthew`; recorded, never judged |
| `quote` | string | the party's own words, verbatim |
| `payload` | kind-specific object (below) | |
| `validFrom` | ISO-8601 | |
| `validUntil` | ISO-8601 or null | null = until the session ends (the consumer decides what "ends" means) |
| `supersedes` | BritCid or null | an earlier clause this one restates |
| `agentId` | string | public key (hex) of the device that recorded it |
| `signature` | bytes | ed25519 over `canonical_bytes()` with the signature empty |

Payloads:

- `affirmsStewardOfRecord`: `{}`.
- `declaresPurpose`: `{ purpose: string, writeSet: [glob] }`.
- `variesRule`: `{ rule: string, manifestPath: string, manifestCid: BritCid, paths: [glob], floor: string }`.
  `manifestCid` names the exact Layer 1 bytes being attenuated. If the rule's bytes change, the
  clause is about a manifest that no longer exists; the consumer treats it as not held. `floor` is
  copied from the rule's own declaration. The clause selects a floor the rule offers and never
  writes one.
- `revokesClause`: `{ revokes: BritCid }`.

Attenuation-only is a consumer obligation that brit's shape supports: no field can widen a rule.
The only things a clause can name are a floor the rule declares and a subset of its paths.

## Index: append-only refs

`refs/notes/brit/covenant/<session>/<clause-cid>`, written with the existing blob-ref pattern
(`hash-object -w` + `update-ref`). There is one ref per clause and nothing is overwritten. A
revocation is a new clause. Segments are percent-encoded with `encode_segment`.

- **Ref blob:** `{ "cid": "<bafy… string>", "clause": <node JSON> }`. The CID is stored as a string
  so consumers need not decode `BritCid`'s byte-array JSON form.
- **Canonical bytes:** dag-cbor in `LocalObjectStore`, as for every other node.
- **Listing a session:** one `for-each-ref` on the session prefix, then `cat_file_batch` over the
  blobs. This stays well under a second for the handful of clauses a session holds.
- **Transport:** like the other `refs/notes/brit/*`, these refs do not travel by default. Pushing
  them is an explicit refspec today (`refs/notes/brit/covenant/*`). A default push is future work.

## CLI: `brit-build-ref covenant`

```
brit-build-ref [--repo R] covenant put  --session S --kind K --party P --quote Q
                                        [--payload-json JSON] [--valid-until T] [--supersedes CID]
                                        [--output-json]
brit-build-ref [--repo R] covenant list --session S [--output-json]
brit-build-ref [--repo R] covenant show --cid CID   [--output-json]
```

`put` signs with the repo's agent key and prints the clause CID, or the full `{cid, clause}` when
given `--output-json`. `list` prints a JSON array of `{cid, clause}` in `validFrom` order. Exit
status is non-zero on malformed input. An empty session lists `[]` with exit status 0.

**Agent key location:** resolve `<git-common-dir>/brit/agent-key`, not `<repo>/.git/brit/agent-key`.
In a linked worktree `.git` is a file, so the old path breaks there. This fix applies to the
existing `build put` as well.

## Graduation

- **Rung 1 (this):** the device key signs, and the consumer attributes the human through its own
  standing records.
- **Rung 2:** the human's own key co-signs, giving a second signature slot.
- **Rung 3:** a `variesRule` clause is notarized as a Mishpat `Commitment` (action `varies-rule`,
  coordinator-only), and brit keeps carrying the clause CID it cites. Commits that land work under
  a clause can cite it today with the existing qahal trailers (`Qahal: steward | ref=<cid>`,
  `Qahal-Node: <cid>`).

## Tests

- **CID stability:** CID stable across serialize → deserialize → re-encode.
- **Signing:** the signature verifies with `verify_signed_node`; tampering with any field fails it.
- **Append-only:** two `put`s for one session give two refs, and neither replaces the other.
- **Listing:** `list` returns both in order; an unknown session gives `[]`.
- **CLI journey:** `covenant put | list | show`, including from a linked worktree (the agent-key fix).
- **Feature flag:** the generic engine still builds with `elohim-protocol` off.
