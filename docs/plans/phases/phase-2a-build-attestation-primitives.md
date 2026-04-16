# Phase 2a — Build Attestation Primitives

**Status:** Design
**Date:** 2026-04-13
**Depends on:** Phase 0 (EPR trailer foundation), Phase 2 (ContentNode adapter)
**Independent of:** Phases 3–6 (libp2p, branch READMEs, DHT discovery, fork governance) — this phase is pure local, requires no networking
**Consumers:** rakia (see `elohim/rakia/docs/plans/build-attestation-integration.md`)

## Problem

Build and deployment state today live in executor memory (Jenkins JSON artifacts, `build-state.json`). When the executor fails mid-run, the state either:

- **Leapfrogs** — advances past unbuilt changes, poisoning future change detection
- **Gets lost** — falls back to a stale global baseline, triggering full rebuilds
- **Says nothing about deployment** — CI knows what it triggered, not what's actually running

The artifact itself has no way to answer "was I built?" / "am I deployed?" / "how trusted am I?" without consulting external systems that can disagree or disappear.

## Insight

These are protocol primitives, not CI infrastructure. An artifact's build and deployment state belongs in the same covenant structure that brit already applies to commits: content-addressed, peer-attested, carrying pillar coupling, survivable across executor death.

**The artifact becomes self-aware** through peer attestations in the git namespace plus the DHT. Any participant can compute what needs to build, deploy, or promote without asking Jenkins, without reading a fragile JSON file, without trusting a central registry.

Succession and stewardship are innate: if the primary steward is unavailable, the collective's succession order kicks in. Stewardship credit flows through the REA shefa pillar for every attestation produced. The bus factor dissolves.

## Scope of This Phase

Brit adds three ContentNode types and ref-management CLI commands. Pure local operations (schema + refs + git). No DHT, no P2P, no remote peers. The DHT publication path opens in a later phase.

**In scope:**
- `BuildAttestationContentNode` schema
- `DeployAttestationContentNode` schema
- `ValidationAttestationContentNode` schema (with check vocabulary)
- `brit build-ref` CLI (read/write/list for all three attestation types)
- Ref namespace design under `refs/notes/brit/`
- AppSchema registration (these are elohim-protocol-flagged types)

**Out of scope (future phases):**
- DHT publication of attestations (future phase, composes with Phase 5 DHT discovery)
- Cross-peer attestation reconciliation (future phase, composes with Phase 3 libp2p transport)
- Reach promotion rule DSL (future phase, tied to AppManifest work)
- Economic event emission on attestation (shefa integration, separate phase)

## Schemas

### `BuildAttestationContentNode`

Records that an agent produced an output artifact from a manifest's inputs.

| Field | Type | Description |
|---|---|---|
| `manifestCid` | CID | The BuildManifestContentNode this attestation is for |
| `stepName` | string | Qualified step name (e.g., `elohim-edge:cargo-build-storage`) |
| `inputsHash` | string | Content hash of all declared inputs at build time |
| `outputCid` | CID | Content-addressed output artifact |
| `agentId` | AgentPubKey | Peer that performed the build |
| `hardwareProfile` | object | CPU arch, OS, memory, relevant toolchain versions |
| `buildDurationMs` | number | Wall-clock build time |
| `builtAt` | timestamp | When the build completed |
| `success` | boolean | Did the build succeed |
| `signature` | bytes | Agent's signature over the full payload |

**Pillar coupling:**
- Lamad: `build-knowledge` — what was built, from what, how
- Shefa: `compute-expended` — the economic cost of producing it
- Qahal: `build-authority` — agent's right to attest this artifact

### `DeployAttestationContentNode`

Records that an agent confirms an artifact is live at an environment.

| Field | Type | Description |
|---|---|---|
| `artifactCid` | CID | The output CID being attested |
| `stepName` | string | Which step's artifact this is |
| `environmentLabel` | string | `alpha`, `staging`, `prod`, `self`, or custom |
| `endpoint` | string | URL or service address being verified |
| `healthCheckUrl` | string | Endpoint used to verify liveness |
| `healthStatus` | enum | `healthy`, `degraded`, `unreachable` |
| `deployedAt` | timestamp | When the artifact started serving here |
| `attestedAt` | timestamp | When this attestation was produced |
| `livenessTtlSec` | number | After this many seconds without re-attestation, the claim self-invalidates |
| `agentId` | AgentPubKey | Peer producing the attestation |
| `signature` | bytes | |

**Pillar coupling:**
- Lamad: `deployment-knowledge` — what is running where
- Shefa: `serving-compute` — the cost of hosting/serving
- Qahal: `environment-authority` — agent's right to attest this environment

### `ValidationAttestationContentNode`

Records that a validator (tool or agent) applied a named check to an artifact.

| Field | Type | Description |
|---|---|---|
| `artifactCid` | CID | What was validated |
| `checkName` | string | Registered check identifier (e.g., `sonarqube-scan@v10`, `trivy-cve@latest`, `nist-800-53`, `test-suite-vitest`, `code-review`) |
| `validatorId` | string | Tool identity or agent pubkey |
| `validatorVersion` | string | Version of the tool/agent |
| `result` | enum | `pass`, `fail`, `warn`, `skip` |
| `resultSummary` | string | Human-readable summary |
| `findingsCid` | CID \| null | Optional detailed report |
| `validatedAt` | timestamp | |
| `ttlSec` | number \| null | When validation goes stale (e.g., CVE DB refresh interval) |
| `signature` | bytes | |

**Check vocabulary is governed by the AppManifest.** A check is only recognized if its `checkName` is registered in the current manifest version. This lets the community evolve the vocabulary — add new scanners, retire outdated ones — without protocol changes.

**Pillar coupling:**
- Lamad: `validation-knowledge` — the findings
- Shefa: `verification-compute` — cost of running the check
- Qahal: `validation-authority` — community's recognition that this check counts

## Ref Namespace

All attestation refs live under `refs/notes/brit/` to stay within git's notes convention and survive clone/fetch:

| Ref | Contents |
|---|---|
| `refs/notes/brit/build/{stepName}` | JSON: `{commit: {attestationCid, outputCid, agentId, builtAt}}` — most recent build attestation per commit |
| `refs/notes/brit/deploy/{stepName}/{env}` | JSON: `{artifactCid, attestationCid, healthStatus, attestedAt, livenessTtlSec}` |
| `refs/notes/brit/validate/{stepName}/{checkName}` | JSON: `{artifactCid, attestationCid, result, validatedAt, ttlSec}` |
| `refs/notes/brit/reach/{stepName}` | JSON: `{artifactCid, computedReach, contributingAttestations: [...]}` — derived, rebuildable from above |

**The refs are a cache; the ContentNodes are truth.** When DHT publication lands (composes with Phase 5), attestations publish across the network; refs become projections. For this phase, the ContentNode is stored locally in `.git/brit/objects/` and the ref points to its CID — the same structure brit already uses for commit-level EPR trailers.

## CLI

`brit build-ref` command group:

```
brit build-ref build put    --step <name> --manifest <cid> --output <cid> [--success] [--hardware <json>]
brit build-ref build get    --step <name> [--commit <sha>]
brit build-ref build list   [--step <pattern>]

brit build-ref deploy put   --step <name> --env <label> --artifact <cid> --endpoint <url> --health <status> [--ttl <sec>]
brit build-ref deploy get   --step <name> --env <label>
brit build-ref deploy list  [--step <pattern>] [--env <label>]

brit build-ref validate put  --step <name> --check <name> --artifact <cid> --result <pass|fail|warn>
brit build-ref validate get  --step <name> --check <name>
brit build-ref validate list [--step <pattern>] [--check <pattern>]

brit build-ref reach compute --step <name>    # Derives reach from current attestations, writes ref
brit build-ref reach get     --step <name>
```

Every `put` command:
1. Constructs the ContentNode
2. Signs with the configured agent key
3. Writes to `.git/brit/objects/`
4. Updates the corresponding ref
5. Prints the new CID

## AppSchema Registration

These three types register into the `elohim-protocol` AppSchema alongside the existing BuildManifestContentNode. They are gated behind the `elohim-protocol` feature flag, preserving brit's engine/app separation.

```rust
// brit-epr/src/schemas/elohim.rs (feature = "elohim-protocol")
register_content_node!(BuildAttestationContentNode);
register_content_node!(DeployAttestationContentNode);
register_content_node!(ValidationAttestationContentNode);
```

Without the feature flag, `brit build-ref` is unavailable — brit falls back to pure git operation.

## Acceptance Criteria

- All three schemas round-trip through serialize/deserialize with ts-rs generation
- `brit build-ref build put/get/list` work on a fresh git repo
- Refs written by `brit build-ref` are visible via `git notes --ref refs/notes/brit/build/* list`
- Refs survive `git clone --bare` + `git fetch refs/notes/*`
- Engine compiles with `--no-default-features` (brit remains a valid git tool without the protocol feature)
- Reach computation is deterministic: same attestations → same reach level
- Check vocabulary registration is enforced: unregistered `checkName` values rejected

## Open Questions Deferred

1. **Reach promotion rule syntax** — how AppManifest declares "build + 3 diverse peers + sonarqube pass = community reach". Deferred to a later phase tied to AppManifest work.
2. **Liveness TTL defaults** — what's a sensible default for DeployAttestation TTL. Deferred to rakia's health-check pod design.
3. **Cross-schema attestation refs** — a ValidationAttestation from a future "code-quality" AppSchema referencing a BuildAttestation from "elohim-protocol". Deferred until a second AppSchema exists.

## Why This Phase First

Every higher phase needs these primitives:
- Rakia's change detection consumes build attestation refs
- Deployment verification consumes deploy attestation refs
- Reach-based UI surfaces consume validation attestations
- Succession and economic credit hang off attestation events

Shipping the primitives unlocks every downstream consumer to build in parallel.
