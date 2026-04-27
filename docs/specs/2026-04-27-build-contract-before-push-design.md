# Build/Compute Contract Before Push — Brit Design

**Date:** 2026-04-27
**Status:** Approved (brainstorm)
**Author:** Matthew Dowell + Claude Opus 4.7
**Supersedes:** none — extends `2026-04-12-brit-design.md` and `docs/plans/phases/phase-2a-build-attestation-primitives.md`
**Companion specs:** `elohim/rakia/docs/specs/2026-04-27-rakia-as-brit-attestation-executor-design.md`, `genesis/docs/superpowers/specs/2026-04-27-jenkins-as-brit-attestation-producer-design.md`

## TL;DR

Brit owns the surface that turns CI/CD prediction from a runtime-fragile, executor-coupled, drift-prone activity into a **content-addressed contract computed and validated on the developer's laptop before push**. The contract is a `BuildPlan` plus the attestation refs it expects to find or produce. Rakia is the runtime that executes the contract. Any executor (Jenkins, GitHub Actions, a peer's elohim-node) becomes a thin attestation producer. The 12 concerns a perfect CI/CD predictor must model collapse onto five brit primitives plus rakia executor logic. Phase 2A (BuildAttestation/DeployAttestation/ValidationAttestation primitives) is the substrate; this spec proposes Phase 2B (planner + verifier + pre-push gate) as the LLM-first / dev-desk surface that makes the substrate operational.

## 1. Problem

The Elohim monorepo's Jenkins orchestrator illustrates the failure mode the brit/rakia stack is meant to eliminate. A 2026-04-27 incident: a one-line manifest change (`storageClassName: openebs-hostpath`) triggered the entire build matrix — DNA pack + DNA Integration (~50min), elohim-edge build (~25min), elohim-sophia, elohim-app — when the only artifact that needed to change was a kubectl-apply against alpha. The diagnostic across orchestrator + downstream Jenkinsfiles surfaced three concurrent root causes:

1. **Baseline drift / accumulated commits.** The orchestrator's `__global__` baseline reached back across multiple prior sessions' EPR Rust work that landed but never produced a clean run. The graph correctly reported "lots of source changed since last green, build everything" — because the graph's notion of "fresh" is commit-history-keyed, not artifact-registry-keyed. Once a build legitimately succeeds and pushes its image to Harbor, the graph has no way to consult that fact.

2. **Deploy isn't a graph node.** "Deploy Edge Node — Alpha/Staging/Prod" lives as Jenkins stages inside the `elohim-edge` pipeline, not as graph nodes with their own `inputs` / `outputs`. The graph cannot say "manifest changed → deploy stale, build steps fresh." It can only say "elohim-edge needs to run," and the build pipeline does build-then-deploy as one indivisible unit.

3. **Downstream pipelines have no internal change detection.** Once the orchestrator dispatches a pipeline with `FORCE_BUILD: true` (which it does unconditionally), every stage in that pipeline runs. The orchestrator's correctness is necessary but not sufficient.

These are symptoms of one architectural absence: **no predictor knows what is already produced, by whom, for which inputs, and where it lives.** Each layer (orchestrator, pipeline, stage) reasons from local file diffs against ephemeral CI state. Nothing reasons from the artifact's perspective, because artifacts don't have a perspective. They are filenames in Harbor and tags in env files, with no provenance the predictor can walk.

The cost is not just wasted Jenkins minutes. It is the LLM-loop cost. An agentic developer trying to ship a manifest fix iterates by pushing → waiting 75 minutes → reading logs → forming hypothesis → patching → repeating. The right loop is: compute the contract locally in 5 seconds → verify it predicts the correct behavior → push only when the contract is right. The substrate that makes that loop possible is brit.

## 2. Insight

Building the build/compute contract **before push** is brit's surface, not rakia's. The reasons:

- **Brit owns the git-native attestation substrate.** `refs/notes/brit/...` carries `BuildAttestationContentNode` / `DeployAttestationContentNode` / `ValidationAttestationContentNode`. These survive `git clone` from any forge. Any participant — developer laptop, CI executor, peer's elohim-node — can read them with `brit build-ref ... get`.
- **Brit owns the local CLI surface.** The four framings in `docs/specs/2026-04-12-brit-design.md` §3.4 require LLM-first CLI patterned on git, with humans using elohim-app for review/consent. The build-contract CLI is a brit CLI, not a rakia daemon and not a Jenkins extension.
- **Brit's existing primitives are exactly what a contract carries.** `BuildAttestationContentNode` already records `manifestCid`, `stepName`, `inputsHash`, `outputCid`, `agentId`, `hardwareProfile`, `signature`. The contract is a graph of these — what should be produced, what already exists, what must be promoted to which environment.
- **The contract is what gets pushed.** A commit's trailers can carry a `Brit-BuildPlan-Cid:` reference to the contract the developer computed. CI verifies the contract before executing it. If the contract diverges (different toolchain, different manifest discovery), CI rejects with a loud error, and the developer fixes it locally. No 75-minute round trips for predictable failures.

This reframes Jenkins (and any other CI executor) as a thin attestation producer. Jenkins shells `brit build-ref build put` after each successful step. It does not own staleness detection, deployment tracking, or cross-pipeline coordination. Those move to brit (substrate) + rakia (executor logic).

## 3. The 12-concern matrix

A perfect CI/CD predictor must model these concerns. The matrix names the owner — which layer is responsible for the concern's primitive, the data, and the validation.

| # | Concern | Owner | Today's coverage | Required state |
|---|---|---|---|---|
| 1 | Source-input identity (file content hash) | brit | Partial (rakia_brit + brit-graph fingerprint exists; not yet per-step canonical) | Per-step canonical hash over sorted (path, content-sha) tuples |
| 2 | Build-process identity (Jenkinsfile, justfile, Dockerfile, executor script) | brit | Partial (build-graph.groovy hashes `buildProcess` refs at runtime in CI; not in brit's hash) | brit-graph fingerprint includes declared buildProcess refs |
| 3 | Toolchain identity (compiler/base-image/system-pkgs) | brit | None | `BuildAttestation.hardwareProfile` already reserves the field; brit-graph fingerprint extends to consume it |
| 4 | Upstream-artifact identity (what was produced by deps) | brit | Partial (manifest `depends:` exists as string ref to step name, propagates staleness) | Refactor `inputs.upstream` to `[{kind, repo}]` artifact-keyed; resolve at compose time; include resolved upstream `outputCid` in current step's `inputsHash` (Merkle DAG) |
| 5 | Registry presence (artifact-for-input-hash exists in registry) | rakia | None (no Harbor query in change detection) | rakia executor consults registry; brit attestation answers "was it produced," registry answers "does it still exist now" |
| 6 | Deploy as first-class graph node | rakia + manifest-author | None (deploy is a Jenkins stage inside the build pipeline) | Manifest schema declares deploy steps with `inputs.sources: [env-files, manifests]` and `inputs.upstream: [artifact-refs]`; `outputs.targetState` resolves to a DeployAttestation |
| 7 | Live target state (what is actually running where) | brit | None | DeployAttestation already specified in Phase 2A; health-check pods write attestations on schedule + on pod restart |
| 8 | Build-result history (input-hash → output-artifact mapping) | brit | None (Jenkins pipeline-baselines.json only carries commits, not artifact mappings) | BuildAttestationContentNode already specified in Phase 2A; `brit build-ref build get/list` query path |
| 9 | Cross-pipeline artifact dependencies | brit + manifest-author | Partial (manifest `depends:` exists; only as staleness hint, not as artifact-flow edge) | Same as concern 4 — promote to artifact-keyed first-class edge |
| 10 | Trigger provenance (webhook / manual / cascade / scheduled) | rakia (executor) | Yes (Jenkins captures BUILD_TRIGGER) | Carry into attestation `agentId` + builder context |
| 11 | Concurrency / abort semantics (race-free re-walks) | rakia | Partial (Jenkins `disableConcurrentBuilds: abortPrevious: true`; no partial-output recording) | rakia records partial outputs; re-walk after abort treats "in-flight" as a third stale state |
| 12 | Failure attribution (what input change made green→red) | brit | None (manual git log walk + Jenkins log archeology) | ValidationAttestationContentNode (Phase 2A) records check failures keyed to inputsHash; bisect-on-input-hash becomes a one-liner |

Six of the twelve are net-new capabilities. Five collapse onto Phase 2A primitives that are already designed but not yet shipped. One (concern 11) requires a rakia executor convention. None require a new substrate beyond `refs/notes/brit/`.

## 4. The "build/compute contract" as a first-class concept

A contract is the answer to: *given this commit, what will/should/must produce, skip, deploy?*

### 4.1 Contract structure

A `BuildPlanContentNode` (rakia owns the schema; brit reserves the slot — see brit-design.md §4 reserved types):

```json
{
  "schemaVersion": 1,
  "headCommit": "edfe5c57...",
  "baseline": { "ref": "refs/notes/brit/build-baselines/__global__", "commit": "ab70ecc0..." },
  "manifestCids": ["bafkrei...", "bafkrei..."],
  "steps": [
    {
      "qualifiedName": "elohim-edge:cargo-build-doorway",
      "verdict": "skip",
      "reason": "attestation-match",
      "inputsHash": "blake3:9f3c...",
      "satisfyingAttestationCid": "bafkrei...",
      "expectedOutputCid": "bafkrei..."
    },
    {
      "qualifiedName": "elohim-edge:deploy-alpha",
      "verdict": "build",
      "reason": "source: genesis/orchestrator/manifests/infra/alpha-mongodb.yaml matches inputs.sources",
      "inputsHash": "blake3:7e1a...",
      "satisfyingAttestationCid": null,
      "expectedOutputCid": null,
      "willConsumeUpstream": [
        { "kind": "container-image", "repo": "ethosengine/elohim-edge", "outputCid": "bafkrei..." }
      ]
    }
  ],
  "validation": {
    "manifestSchemaPassed": true,
    "graphCompositionPassed": true,
    "stageNameParityPassed": true,
    "globCoveragePassed": true,
    "registryPresenceVerified": false,
    "registryMode": "offline",
    "warnings": []
  },
  "computedBy": { "tool": "brit", "version": "0.4.2", "at": "2026-04-27T15:42:00Z" }
}
```

### 4.2 Contract lifecycle

1. **Compute** — `brit plan --since refs/notes/brit/build-baselines/__global__` produces the BuildPlan above. Operates on local repo state. Reads attestations from `refs/notes/brit/...` to mark steps SKIP. Pure local; no network unless `--registry` is passed.
2. **Verify** — `brit verify` runs the validation taxonomy (§5) against manifests, the plan, and the proposed change. Loud failure on schema/ref/glob/parity issues. Pre-push hook calls this.
3. **Optionally attach** — `brit commit --attach-plan` adds a `Brit-BuildPlan-Cid:` trailer to the commit, making the contract part of the covenant. The plan itself is stored in `.git/brit/objects/{cid}`; the trailer carries the CID. Optional because not every commit needs the overhead; a project policy can require it for branches that gate production.
4. **Push** — standard git push. The plan's CID travels with the commit (if attached) or is recomputable by any consumer that has the same manifests + attestations.
5. **CI executes** — rakia or any executor reads (or recomputes) the plan, executes only the BUILD steps, writes attestations on success.
6. **Re-verify post-execution** — CI's final step is `brit verify --post` which asserts that all expected attestations now exist and that the next plan computation against the new baseline shows the entire graph as fresh.

### 4.3 Why this works

- **Determinism.** Same inputs → same plan. `brit plan` on the developer's laptop and `brit plan` on the CI executor must produce byte-identical BuildPlan JSON (modulo `computedBy.at` timestamp, which is excluded from canonical hash). Any divergence is a bug — caught the first time it happens, not after weeks of mystery rebuilds.
- **Self-describing.** The plan carries its own validation results, the satisfying attestation CIDs, and the expected outputs. A reviewer reading the plan sees exactly what will happen, with cryptographic anchors for every claim. No "trust me, the orchestrator will do the right thing."
- **Compositional.** The plan is itself a ContentNode; it can be referenced from MergeProposal nodes ("this merge requires this plan to pass"), from QahalGovernanceContentNodes ("this branch requires plans signed by N stewards"), from shefa economic events ("compute-expended for this plan accrues to these contributors"). The protocol layer composes naturally.
- **Survivable.** The plan and its attestations live in git. If the developer's laptop dies, another developer pulls and gets the same view. If the Jenkins instance dies, the next CI executor (any executor) walks the same graph and produces the same plan.

## 5. Phase 2B — planner + verifier + pre-push gate

Phase 2A delivers the attestation primitives (read/write/list). Phase 2B delivers the operational surface that makes them load-bearing.

### 5.1 `brit plan` — attestation-aware planner

Today's `brit plan --since <ref>` outputs a `build-plan.schema.json` JSON computed from changed paths against manifests via `rakia_core::constellation::plan_from_changes`. The verdict per step is currently `affected | not_affected` based on glob match.

Phase 2B extends the verdict vocabulary:

| Verdict | Meaning | Reason format |
|---|---|---|
| `skip` | inputsHash matches an existing BuildAttestation | `attestation-match: <cid>` |
| `build` | inputsHash has no matching attestation OR an upstream is also `build` | `source: <file> matches <pattern>` / `upstream: <step>` / `attestation-missing` |
| `cold-start` | No attestation history exists for this step yet | `cold-start: source <file> in changeset` (preserves Phase 2A cold-start semantics) |
| `deploy` | Step is a deploy node; either `target_state.outputCid != upstream.outputCid` or `target_state` doesn't exist | `deploy-needed: target=alpha current=<cid> intended=<cid>` |

`brit plan` reads `refs/notes/brit/builds/<step>` for each step in the graph, finds the most recent attestation matching the step's currently-computed `inputsHash`, and consults `outputCid`. If found, SKIP. If not, BUILD.

For deploy steps, `brit plan` reads `refs/notes/brit/deploys/<step>/<env>` for the matching environment, compares `artifactCid` to the upstream BuildAttestation's `outputCid`, and decides DEPLOY or skip-deploy.

### 5.2 `brit verify` — holistic predictor

Today's `brit-verify` is a stub. Phase 2B fills it with the validation taxonomy. Single command, multiple checks, all loud-failure:

```bash
brit verify              # default: schema + graph + parity + glob + plan-staleness
brit verify --strict     # adds: registry-presence, attestation-coverage, deploy-currency
brit verify --plan PATH  # validates a specific BuildPlan JSON against the repo
```

Validation checks (all run by default unless noted):

| Check | Failure example | Mechanism |
|---|---|---|
| Schema (manifest) | Required field missing | JSON-schema validation against `manifest.schema.json` |
| Schema (plan) | Plan doesn't conform to `build-plan.schema.json` | JSON-schema validation |
| Pipeline-name uniqueness | Two manifests claim same `pipeline:` field | Set membership check during compose |
| Step-name uniqueness within pipeline | Two `steps.<name>` keys collide | Set membership check |
| Cross-pipeline producer existence | `inputs.upstream: [{kind, repo}]` resolves to no producing manifest | Reverse-index lookup across all manifests |
| Cross-pipeline producer uniqueness | Two manifests' `outputs.primary` claim same `(kind, repo)` | Reverse-index dupe detection |
| Cycle detection | Step depends on its own descendant | DFS in brit-graph |
| Stage-name parity | Manifest step name doesn't match a `stage('...')` in the named Jenkinsfile | Walk Jenkinsfile AST (or grep for stage() declarations); off-by-default for non-Jenkins executors |
| Glob coverage | Source file under a tracked tree matches no step's `inputs.sources` | Reverse glob match across all manifests; warn on orphan files |
| `buildProcess` ref existence | `buildProcess: ["foo.sh@build_dna"]` where function `build_dna` doesn't exist | Read file, check function presence |
| Parameter declarations | Caller passes a param the manifest doesn't declare | Cross-reference at lint time |
| Note schema parity | Existing notes don't match current schema version | Read recent notes, validate against current schema |
| **Plan staleness** | Plan's `headCommit` doesn't match repo HEAD; manifests have been edited since plan was computed | Hash-compare |
| **Registry presence** (`--strict`) | Attestation says `outputCid: X`, registry doesn't have it | Registry API call (deferred to rakia executor for the actual call; brit verify --strict shells `rakia registry has`) |
| **Attestation coverage** (`--strict`) | Plan SKIP for step S, but `brit build-ref build get --step S` returns no matching attestation | Re-walk attestations |
| **Deploy currency** (`--strict`) | `target_state.healthStatus` is unhealthy or `attestedAt` older than `livenessTtlSec` | Read DeployAttestation |

Exit code 0 on all-pass; non-zero with structured JSON error on any failure.

### 5.3 Pre-push hook integration

`.husky/pre-push` (or equivalent for non-husky setups) gains:

```bash
# Fast fail: catches structural issues
brit verify || exit 1

# Slower: computes plan against origin/dev to predict CI behavior
brit plan --since origin/dev --check
```

`brit plan --check` reads the plan and exits non-zero if the plan would BUILD more than a threshold N steps (configurable; default unlimited, opt-in for "warn me if I'm about to trigger a full rebuild"). This catches accidental wide-blast-radius commits before push.

Hook is bypassed with `HUSKY=0 git push` (existing pattern), explicit and rare.

### 5.4 Golden snapshot tests

A convention, not a feature. Project repos that want regression-protection commit golden plans:

```
tests/build-contracts/
├── manifest-only-change.golden.json
├── doorway-source-change.golden.json
├── dna-rs-change.golden.json
├── manifest-rename-fails-validation.golden.json
└── README.md
```

Each golden is the output of `brit plan` against a synthetic input set (specified via `--files`). CI runs all goldens on every PR via `brit plan --files=... --against=tests/build-contracts/<name>.golden.json`. Anyone changing graph semantics MUST regenerate the affected goldens — visible in PR diff, reviewable, no silent drift.

### 5.5 LLM-first cost model

The brit master design (§3.4 framing 1) names "LLM-first CLI" as a core constraint. Phase 2B operationalizes it as cost asymmetry:

| Loop | Cost | Use case |
|---|---|---|
| `brit plan` on dev laptop | ~5s, $0 | Iterate until plan is correct |
| `brit verify` on dev laptop | ~2s, $0 | Catch validation issues |
| `brit plan` in pre-push hook | ~5s, blocks push on structural failure | Last gate before remote |
| Jenkins/CI run | 5–75min, $$ | Execute the plan |

An agentic developer iterating against `brit plan` saves on the order of 100× the iteration cost vs. push-and-watch-Jenkins. This is also why `brit verify` must be exhaustive — every check that can be dev-side must be dev-side; CI is only for things that genuinely need network or registry access.

## 6. Storage / namespace decisions

- **Refs:** `refs/notes/brit/builds/<pipeline>:<step>` (per Phase 2A), keyed by commit SHA, value is the BuildAttestationContentNode JSON. Per-step ref (not a single shared ref) so concurrent CI writes from different pipelines don't conflict.
- **Local object store:** `.git/brit/objects/{cid}` (per Phase 2A), holds the canonical JSON for any ContentNode the agent has touched. BuildPlanContentNodes go here too.
- **CIDs:** blake3 over canonicalized JSON (per Phase 2A). Plans, attestations, manifests all use the same CID scheme.
- **Trailer:** `Brit-BuildPlan-Cid:` reserved (per brit-design.md §5 reserved keys list — adding here as a new reservation alongside `Built-By:`, `Brit-Schema:`).

## 7. Migration path

Phase 2A ships the primitives. Phase 2B (this spec's scope) ships the planner + verifier + pre-push gate. Migration of existing executors (Jenkins specifically — see companion Jenkins design spec) proceeds in three additive stages:

1. **Producer-only.** Each successful CI step shells `brit build-ref build put` after its existing logic. Notes accumulate. Plans become consultable. No behavior change in CI.
2. **Consumer.** rakia executor (or a Jenkins integration shim) reads notes via `brit plan` to mark steps SKIP. Existing change-detection logic stays as fallback for cold-start / verification.
3. **Authoritative.** Legacy change-detection (Jenkins's `build-graph.groovy`, `pipeline-baselines.json`, `build-state.json`) is removed. brit becomes the sole source of truth.

Each stage is independently shippable. Stage 1 is pure additive (nothing to break). Stage 2 is gated behind a per-pipeline flag for safe rollout. Stage 3 is the cleanup.

## 8. Open questions / explicit non-goals

**Open:**

- **Canonical JSON for CID stability.** Phase 2A uses `serde_json` with sort_keys; the BuildPlan/attestation schemas need a more rigorous canonicalization (DAG-CBOR per brit-design.md §4) eventually. Acceptable trade-off for Phase 2B to keep `serde_json` + sort_keys with a documented migration to DAG-CBOR.
- **Multi-attestation precedence.** If two agents have produced attestations for the same `(step, inputsHash)` with different `outputCid` values (different binary outputs from "identical" inputs — e.g., timestamp baked into the binary), what's the resolution? Recommend: `brit plan` SKIPs on first match; CI policy can require `--strict` which fails if precedence is ambiguous. Defer the policy DSL to a later phase.
- **Toolchain identity capture.** `BuildAttestation.hardwareProfile` exists but the contents are unspecified. Recommendation: container image digest (sha256:...) for containerized builds; `rustc --version`, `node --version`, etc. for native; full reproducibility nirvana via Nix derivation hash deferred indefinitely. Phase 2B writes whatever the executor declares; `brit verify --strict` can require a non-empty profile.
- **Cross-pipeline artifact-keyed deps schema.** `inputs.upstream: [{kind, repo}]` is the proposed shape. The `kind` enum needs to be defined (container-image, npm-tarball, hApp-bundle, static-site-bundle, ...). Suggest: open enum, advisory validation.

**Explicit non-goals:**

- DHT publication of plans/attestations — Phase 5+.
- Cross-peer attestation reconciliation / consensus — Phase 3+ (libp2p transport).
- Reach promotion rule DSL — separate phase tied to qahal governance.
- Economic event emission on attestation (shefa credit) — separate phase.
- Replacing rakia. This spec is brit's contribution; rakia owns the executor side and is detailed in its companion spec.

## 9. Cross-references

- **Master design:** `docs/specs/2026-04-12-brit-design.md`
- **Phase 2A (substrate this spec depends on):** `docs/plans/phases/phase-2a-build-attestation-primitives.md`
- **Brit-graph + rakia MVP:** `../../docs/superpowers/specs/2026-04-19-brit-graph-rakia-mvp-design.md` (parent repo)
- **Rakia harvest companion:** `elohim/rakia/docs/specs/2026-04-27-rakia-as-brit-attestation-executor-design.md`
- **Jenkins harvest companion:** `genesis/docs/superpowers/specs/2026-04-27-jenkins-as-brit-attestation-producer-design.md`
- **Originating incident:** orchestrator-build #727/#728 over-build (storageClass commit `edfe5c57`)
- **Diagnostic:** ci-pipeline agent report dispatched 2026-04-27

## 10. Done criteria

For Phase 2B to be considered "designed":

1. This spec is approved.
2. `brit plan` verdict vocabulary is implemented and documented.
3. `brit verify` validation taxonomy is implemented; each check has at least one failing-test fixture in `brit-cli/tests/`.
4. Pre-push hook integration documented in `docs/integrations/pre-push.md` (new), with examples for husky and bare git hooks.
5. Golden snapshot test convention documented in `docs/conventions/golden-plans.md` (new).
6. Three companion specs (this one, rakia, Jenkins) cross-reference cleanly with no contradictions.
