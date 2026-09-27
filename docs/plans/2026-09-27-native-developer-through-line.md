---
title: Native developer through-line
status: active
date: 2026-09-27
habits:
  - dev-system-equilibrium
  - runtime-upgrade-propagation
  - blob-durability
---

# Native developer through-line

Approved scope: one physical `brit` executable connects intent, scoped source,
attributable checks, reproducible output, authorized release and verified peer
retrieval. hApp and brit are mandatory consumers of an artifact-neutral core.
This plan supersedes the recommendations in the preserved, untracked
2026-09-26-local-developer-cutover draft; it does not alter that draft.

Baseline: parent 4e71582f, brit 5ac98367, rakia 720c132. In particular, brit
already implements push. Ordinary clones do not transport custom evidence notes.
The operator's approved native slice supersedes the older blanket pause in this
directory's roadmap; it does not require exhaustive gitoxide parity first.

## Ownership

| Owner | Owns | Does not own |
|---|---|---|
| brit-cli (`brit`) | Developer orchestration and recoverable scoped operations | Another governance ledger |
| gitoxide/gix | Git objects, index, refs and interoperability | Elohim authority |
| epr flow | Intent, commitment, attribution, review, acceptance, gates | Artifact transport |
| eprfs core/local | Immutable trees and verified materialization | Git semantics or election |
| rakia | Build inputs, adapters, release compatibility | Blob store or new scheduler |
| content governance | Standing, lineage, delegation, revocation, election | Authority inferred from HTTP |
| storage | Existing byte/evidence transport and retention | Election based on possession |

## Ordered acceptance chain

All stations start incomplete. Local checks, CI, publication, binary releases,
and live household evidence are separate results; unrun is never green.

1. **Acceptance and package identity** (brit-cli; no dependencies): make the
   existing CLI journey require an actual binary; add unified help, Git dispatch,
   build dispatch, legacy-name and error tests. Extract parsed-argument dispatch
   from gitoxide, retain its library/`ein`, and make brit-cli build `brit`.
   No private Git-engine executable is allowed. A deprecated `rakia` link may
   name the same executable. Close with fresh executable tests and feature
   matrix, not a rename alone.
2. **Standalone delivery** (publisher/release CI; depends on 1): publish the
   versioned transitive fork dependency closure in order, prove clean Nexus
   consumer resolution without siblings, and deliver a checksummed one-binary
   archive with exact source SHA/features. No remote publication until gates
   pass. Root gitoxide must not silently resolve to the unforked public package.
3. **Complete shared tree** (eprfs-core/local and brit Git adapter; depends on 1):
   versioned canonical DAG-CBOR nodes/tag-42 links, raw leaves, byte names,
   modes, symlink target bytes, explicit submodule boundaries and build closure.
   Preserve EprMeta v1 bytes. Verify before staging writes; promote atomically.
   Golden CIDs, recursive restore, corrupt fetch, interruption and exact Git tree
   round-trip tests close this station. Refuse host-inexpressible projections.
4. **Scoped accountable work** (brit-cli/epr; depends on 1,3): record/prove/publish
   reuse existing flow and device identity; bind full source/tree OIDs and CIDs,
   checks and target context. Isolated two-worktree test preserves unrelated
   staged/dirty files and submodule pins, reviews exact staged changes, enforces
   independent acceptance, survives interruption, refuses stale/forged evidence,
   integrates by fast-forward and remains consumable by stock Git. Evidence
   transport is explicit. A self-signature is not participant standing.
5. **Artifact-neutral reproducibility** (rakia/adapters; depends on 3,4): add a
   native-tool class without weakening hApp role validation; bind declared input,
   toolchain and output roots. hApp packing and Cargo compilation are adapters.
   Two isolated builds produce identical roots for each declared platform.
   Wrong-platform and unknown-class tests refuse safely. No new scheduler.
6. **Portable authority** (protocol verifier/coordinator/client; depends on 5):
   share carried-head validation rules; independently provision a channel root
   anchor. Validate record binding, lineage, standing, delegation, revocation
   and observed election order without a preinstalled conductor. Same adversarial
   vectors must pass coordinator/client parity. Incomplete evidence is not an
   authorized result; offline results never claim globally latest.
7. **Peer delivery** (eprfs storage adapter/household; depends on 2-6): publish
   once to A, fetch through never-directly-uploaded B with A/Harbor/Jenkins
   unavailable. Repeat for hApp and brit twice. Verify before persisting; bounded
   retry; execution/install is a separate explicit step. Record exact IDs and
   receipts in the existing habit atoms, not a new readiness register.

## Verification and rollout

Use focused locked Cargo tests with empty native RUSTFLAGS and an explicit
external target directory. No whole-Elohim build. Commands from brit:

```sh
cargo test --locked -p brit-cli
cargo test --locked -p brit-epr -p brit-build-ref
cargo test --locked -p cli-journey --test brit
```

Run eprfs and rakia workspace tests separately as their stations land. Integrate
new scenarios into existing manifest-owned gates and CLI/a2o harnesses. Linux is
the live pilot; macOS/Windows qualify semantics and explicit host limitations.

Rollout: disposable fixtures, then two developers, then selected Elohim work.
Keep Git and current forge gates until native enforcement passes. Rollback
restores the previous verified executable/entry point without deleting history
or evidence. Release rollback uses the existing authorized election policy.

## P2P boundary

No new DHT entry type or transport. Immutable tree bytes are content-addressed
payloads under existing release identity; local projections are reconstructable.
Private draft/journal data remains private; publish outcome evidence, never
thoughts or credentials. CID integrity, signature authenticity, standing,
observed election and availability remain different claims.

Use two elected channel heads, not a head per file/commit: approximately 730
release versions/year at one daily release per channel, plus evidence. Measure
retention/verification cost before broadening. This does not establish native
source-history authority, browser resolver consolidation or JIT UI.

## Execution evidence

Implementation in progress. No station is declared complete by this document.
The existing untracked draft and unrelated working-tree changes are preserved.

### Local increment: executable composition and integrity floor

Implemented locally, not published or accepted as the full daily-driver cutover:

- `brit-cli` owns the single linked `brit` executable; root `gitoxide` retains
  its library and `ein`. Git dispatch consumes parsed arguments in-process.
- `brit build` exposes graph/affected/plan/fingerprint/baseline. A legacy
  `rakia` hardlink invokes the same executable with a warning; no compatibility
  link was installed over the user's tools. Git-only root options on `build`
  fail rather than silently acting on the wrong repository. Completions include
  the new namespace.
- CLI journey fixtures use shared Git environment isolation. Missing `brit`
  fails instead of skipping; the old rakia journey exercises `brit build`.
- Eprfs checks fetched handle identity and bytes before each file/symlink write;
  drift checks honor the expected raw/legacy DAG-CBOR codec. It does not yet
  implement the shared canonical tree or full-tree safe restore.
- Eleven new governance manifests plus the amended epr CLI manifest describe
  ownership and narrow advisory triggers. Evaluator/resolver suites passed
  42/36 checks; all twelve manifests passed health checks and independent review.
  Codex automatic hook projection remains absent on this machine.

Independent local execution (Rust 1.93.1, native RUSTFLAGS empty):

| Command | Result |
|---|---|
| `cargo test --locked -p brit-cli` | 13 passed |
| `BRIT_BIN=/tmp/brit-merge-target/debug/brit cargo test --locked -p cli-journey --test brit --test rakia --test inherited_git_environment` | 16 Git + 8 build + 1 isolation passed |
| `cargo test --locked -p eprfs-local --test materialization_integrity` (eprfs workspace) | 5 passed |

The implementing agent additionally ran the default root build, small/max-pure/
lean-async feature checks, root granular feature checks, and focused eprfs core/
local tests (25 + 8 + 5), no-default tests, formatting and eprfs strict Clippy.
All passed. Auxiliary brit-build-ref/brit-verify journey suites can still skip
their absent executables; they are NOT included in this qualification.

Targets: `/tmp/brit-merge-target` (reused) and `/tmp/eprfs-gate-target` (336 MB).
No full Elohim build, user-tool replacement, commit, push or publication occurred.
The wrapper declares Rust 1.88 because it links the root engine; actual MSRV
execution is unrun. The lockfile deliberately downgrades pre-existing kstring
2.0.5 (requires Rust 1.96) to 2.0.2 for the installed 1.93.1 toolchain, besides
the new dependency edges. Eprfs lock changes only add its async-trait test edge.

Build locally with `cargo build --locked -p brit-cli --bin brit` and an external
`CARGO_TARGET_DIR`. Invoke `brit status` or `brit build graph discover --repo <path>`.
This is not yet a standalone Nexus installation recipe.

Local binary receipt: `/tmp/brit-merge-target/debug/brit`, SHA-256
`cbba45491bb74de6e1849fc0b4baf48beb59849a5624c6f178b3c3c0cd9079be`,
frontend preset `max`, source HEAD `5ac98367cc6bbb6536c545cc9cc4242fbed49a8b`.
The binary explicitly labels working-tree bytes **unverified**; HEAD is not a
source attestation for these uncommitted changes. The build script watches
resolved Git HEAD/ref paths, including submodules/worktrees, rather than only
the `.git` pointer file.

Strict CLI Clippy on default Rust 1.93.1 stopped on upstream `gix-ref` lint
expectations and unknown newer lint names (exit 101). Repeating the focused
`cargo +1.98 clippy --locked -p brit-cli --all-targets -- -D warnings` with the
already-installed matching toolchain passed (exit 0), with only the upstream
removed-lint-allowance warnings. No upstream Rust source was changed to silence
the older compiler. After these checks, local disk has 29 GB free.

**Delivery HOLD:** the linked normal dependency closure contains 67 local
packages; the original publisher listed 19 and omitted root `gitoxide` and
required fork dependencies. Existing-version skips do not deliver the new executable.
Inherited release/install workflow sections still target `ein`/`gix` and need
station 2 migration. No fresh registry consumer, release archive, current remote
CI run, portable authority proof or household peer-delivery proof was produced.
Independent review accepts the bounded local source increment, not delivery.

### Station 2 staged publisher, local qualification only

The publisher now derives the local normal/optional/build dependency closure
from locked Cargo metadata for `brit-cli` **and** the previously published
`brit-build-ref` root. This is 68 packages and 368 local edges (61 optional,
zero build). The initial 331 missing `registry = "elohim"` tags have been added;
locked metadata now reports zero missing edges. `brit-epr` and `brit-graph`
are included transitively. `brit-verify` is deliberately excluded because its
manifest declares `publish = false`. The current no-upload index audit is:

```text
env PYTHONDONTWRITEBYTECODE=1 RUSTFLAGS='' CARGO_TARGET_DIR=/tmp/brit-merge-target CARGO_BUILD_JOBS=2 bash scripts/ci/cargo-publish-brit.sh --audit-index
EXIT=0 — 18/68 current versions present; 50/68 absent
```

The publisher validates the full dependency graph before upload, then packages,
compares, publishes and verifies one crate at a time in generated topological
order. It is intentionally **non-atomic and resumable**: a later failure may
leave earlier immutable versions published. A present Nexus archive is fetched
and checked against its index SHA-256 before comparison. Byte-identical archives
skip; otherwise the file map must match in content, full permission bits and
directory entries after
excluding only Cargo's `.cargo_vcs_info.json` source-location receipt. Cargo.lock,
both manifests, build scripts and source files remain compared. Unsafe archive
paths, duplicates and symlinks refuse. A failed upload/409 is accepted only if
the verified existing payload is equivalent. Auth/server/network errors are not
absence. The script requires an explicit external Cargo target and `--locked`.
The isolated fake-registry contract test command
`env PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/ci -p 'test_publish_brit.py'`
passed 15/15 (exit 0); shell syntax passed. This is local guardrail evidence,
not an upload or clean-consumer proof.

Read-only Nexus index inspection initially found 19/68 old current versions.
`brit-cli` was bumped to unpublished `0.1.2`, leaving 18/68 present and 50/68
absent. The old `brit-cli 0.1.1` index entry has no `gitoxide` dependency, so
its immutable version cannot represent the linked binary. Other present
versions can be reused only when their verified payloads compare equivalent;
otherwise they need targeted bumps. The registry tags changed manifest bytes,
but `deps.registry = null` in a sparse-index record means *same registry*, not
crates.io, so that field alone is not evidence of wrong fallback. The source
Cargo manifest and a fresh consumer resolve the actual boundary.

**Bootstrap resolved locally; delivery HOLD remains:** Cargo package generates
Cargo.lock against the target registry, so an all-archives-first strategy
cannot prepare packages whose dependencies are unpublished. The staged
algorithm waits for each predecessor to appear before packaging its successor.
Seven packages now carry targeted versions: `brit-cli 0.1.2`, `brit-epr 0.1.2`,
`brit-graph 0.1.2`, `brit-build-ref 0.1.2`, `gix-hash 0.26.3`,
`gix-hashtable 0.16.1`, and `gix-object 0.64.2`. All 368 local edges in the
68-package publish closure carry the Nexus registry tag. A real read-only
probe used `cargo package --locked --registry elohim --no-verify --allow-dirty`
with the existing Nexus read credential on all 18 current-version packages.
Twelve compared `skip-equivalent`, including leaf `gix-error`/`gix-trace` and
nonleaf `gix-path`; all remote archives were checksum-verified first. Six
refused as immutable collisions: `brit-epr 0.1.1` (governance files and
Cargo.lock), `brit-graph 0.1.1`, `brit-build-ref 0.1.1`, `gix-hash 0.26.2`,
`gix-hashtable 0.16.0`, and `gix-object 0.64.1` (the latter five differ in
packaged Cargo.lock only). The six versions and their incoming dependency
minimums were bumped. The fixed-point re-audit reports **12/68 present and
56/68 absent**, and all twelve still-present versions package with locked
Cargo and compare `skip-equivalent` against checksum-verified Nexus archives.
No further existing-version collision is known. A clean publisher
run and sibling-free `cargo install --registry elohim --locked` are still
unrun. No crate was uploaded. Do not claim station 2 or remote CI green from
these isolated tests.

Story-graph maintainer candidate: chain `standalone delivery` / between
`installed version exists` → `consumer usable` / missing node
`the immutable published payload matches the intended source and registry
closure, and the consumer resolves it from Nexus without sibling checkouts` /
probe `compare exact index/archive digest, inspect resolved registry sources,
then install and run brit from a clean Cargo home` / current state
`local fake-registry contract passes; no remote publication or clean consumer`.

### Station 2 binary archive guardrails (local evidence only)

`.github/workflows/brit-release.yml` owns `brit-v<brit-cli-version>` tags;
the inherited `release.yml` is now guarded to the upstream repository. The
initial Brit matrix is Linux x86_64, macOS x86_64/aarch64 and Windows x86_64,
using `max-pure`. Cross-target and universal archives remain unqualified.
Each archive contains one `brit` executable (Windows: `brit.exe`), licenses,
README and build information binding the source commit, clean source check,
lockfile hash, feature preset, target and binary hash. These are CI delivery
metadata, not native authority evidence.

The release workflow runs focused CLI/journey tests on the exact tagged source
before creating a draft, smoke-checks each host executable, and downloads and
verifies uploaded archive checksums before making the release public. Native
Cargo output stays outside the checkout. The new archive/publisher contract job
is required by the CI aggregate and Nexus publication. Source install call sites
now select `--path brit-cli --bin brit`.

Local checks: the primary and independent reviewer ran the publisher suite
(10/10), and the archive fixture passed tar and ZIP shape/checksum checks plus
wrong-SHA, dirty-source and receipt-field-injection refusals. ZIP packaging on
Linux does not qualify the Windows executable. Shell syntax, workflow YAML
parsing and CI blocking-job census passed; `actionlint` is unavailable locally.
No GitHub run, real release artifact or new installed consumer was measured.

The existing `just run-release-workflow`/`roll-release` recipes are inherited
upstream tooling, not Brit release commands. After source qualification and
explicit release authorization, a Brit tag can be dispatched with
`gh workflow run brit-release.yml --repo ethosengine/brit --ref brit-v<VERSION>`;
this is a mutating release operation, not a validation-only command. It was not
run. Binary archives and Cargo publication have independent delivery gates;
station 2 as a whole remains incomplete until both are qualified.

The parent was fast-forwarded to `38070427b` from
`origin/habit-deltas/2026-09-27` at the operator's request. Only the #1918
`push-delivers-within-budget` delta and its generated habit projection changed;
`habits-project.py --check` passed. Its RED status remains unchanged.

### Discovered intermediate station: fetched is not verified

Chain: complete shared tree / peer delivery.
Between: storage returns a blob handle -> local projection writes its bytes.
Missing node: the returned CID equals the requested CID AND the bytes verify
under that requested CID's codec, before touching the destination.
Probe: `cargo test --locked -p eprfs-local --test materialization_integrity`
from the eprfs workspace, with the external native Cargo target configured.
Current state: local regression suite passes; this is not a household or full-tree
restore measurement. Tests were added after implementation, not a red-first run.

Story-harvest candidate for the existing workspace-to-fleet release feature:

```gherkin
@wip @regression
Scenario: A retrieved artifact cannot overwrite a destination before verification
  Given Matthew has an existing local artifact
  And its replacement is pinned by a content address
  When a peer returns a mismatched handle or corrupted replacement bytes
  Then the replacement is refused
  And the existing artifact remains unchanged
```

Constraint: possession and fetch success do not prove requested content identity.
The local tests cover raw codec 0x55, legacy DAG-CBOR codec 0x71, and refusal of
unsupported codec 0x70, including native symlink preservation. Wiring the a2o
scenario remains pending; no new story or habit is declared green from this
candidate. Path-containment races, complete-tree atomic promotion and release
authority remain independent acceptance nodes.

### CI diagnostic checkpoint (2026-09-27)

The first post-integration GitHub run at `e8f` ([run 36330734596](https://github.com/ethosengine/brit/actions/runs/36330734596)) was **red**, not a delivery receipt. Its journey job 108652023733 found a changed Git error prefix at `file-v-any-no-output-wanted-ref-p1`; lint job 108652023899 found rustdoc E0004 in `src/shared.rs:68` for the additive `small,prodash-render-tui` feature union; ARM job 108652023901 found an EXDEV hard-link failure in the legacy-name fixture; and macOS fast tests job 108652023772 found a `/private/var` versus `/var` fixture-path mismatch. Earlier invalid job-level `${{ runner.temp }}` workflow expressions were repaired; official actionlint 1.7.12 subsequently accepted both CI and Brit release workflows locally (exit 0). This does not establish a later GitHub run as green.

Focused repairs preserve the old Git `Error:` diagnostic, its complete `anyhow` cause chain, and Git exit status 1; build errors retain their `error:` rendering and argument exit status 2. The legacy-name test now copies the same executable bytes into its isolated fixture, including Windows `.exe`, without assuming a shared filesystem. The inherited-Git-environment fixture compares canonical paths, and the small-build TUI arm explicitly refuses `--progress` rather than compiling an impossible match. Local evidence: the `small,prodash-render-tui` rustdoc command with `-D warnings` exited 0; final legacy-name and inherited-environment tests each passed 1/1; `cargo test --locked -p brit-cli` passed 15/15 (3 unit, 2 smoke, 10 unified); Rust 1.98 `clippy --locked -p brit-cli --all-targets -- -D warnings`, format check, and diff check exited 0. The full `gitoxide` max-feature shell journey exited 0 under a PTY with CI registry environment, but its intentional partial-clone and auxiliary `brit-verify` skips do not qualify those tools. These are local checks, not a successful replacement CI run.

The publisher fake-registry suite passed 16/16 and the clean-consumer verifier suite passed 8/8 locally. Neither remote Nexus publication, a clean installed consumer, nor a public binary release was exercised; station 2 remains open on those independent gates.

Standalone governance has a separate station-4 prerequisite. Brit has its own root `.epr-meta` anchor, `AGENTS.md`, and `CLAUDE.md`, but no `.epr-meta/elohim/packages`, checked-in `.husky/pre-push`, or local `core.hooksPath`. The current parent `epr doctor` requires that package root and configured hook (`elohim/eprfs/epr-cli/src/doctor.rs:37-43,130-166`); `epr ready` includes those doctor findings (`ready.rs:13-19`), while `epr setup` refuses an absent checked-in hook (`setup.rs:15-20`). Thus `epr explain` can inspect Brit's plan now, but standalone `epr ready --target origin/main` is not yet a green portable reach gate. Brit's anchor explicitly ends parent cascade and rejects copied parent tooling; the parent already attests Brit's pinned `Tests pass` GitHub check through `build-manifest.json`. Station 4 must supply a scoped portable integration contract, not transplant the monorepo's package and hook machinery just to silence doctor.

### Station 2 Nexus index-alias repair checkpoint (2026-09-27)

Main CI [run 36336625030](https://github.com/ethosengine/brit/actions/runs/36336625030) passed tests but its publish job stopped after 24 verified uploads. The next `cargo package --locked --registry elohim --no-verify -p gix-blame` rejected the newly published `gix-diff 0.67.1` index row as invalid. Its archive checksum matched the Nexus index, but Nexus represented the optional renamed `imara-diff` dependency as `gix-imara-diff` without the index `package` mapping while the `blob` feature still named `dep:imara-diff`. The published `0.67.1` is immutable and remains unusable; this repair neither edits nor yanks it.

The bounded repair gives the dependency its canonical Cargo key, retains the Rust `imara_diff` source path in library, integration and bench targets with explicit aliases, and raises only the published `gix-diff` version to `0.67.2`. The other five non-dev aliases in `gix-merge` and `gitoxide-core` use canonical keys before their first publication; the affected unpublished gix-diff consumers require `^0.67.2`. A targeted offline lock refresh changed only the `gix-diff` package version. The publisher now compares non-dev dependency identity, optionality, explicit feature names and `dep:` optional-feature links against the actual index row before treating a matching archive as a verified skip and after upload. Cargo metadata's implicit optional-dependency feature may legitimately be absent from the index; exemption requires its exact `name -> [dep:name]` shape, the matching optional index dependency, and confirmation that source `Cargo.toml` does not explicitly declare it. This is a dependency-name/feature-link predicate, **not** full Cargo schema validation or a clean-consumer receipt. A malformed row fails closed; 404 remains distinct from auth, network and server errors; checksum/semantic-payload comparisons remain separate.

Local evidence: publisher fake-registry suite 22/22, including lost optional/required aliases, dropped explicit features or links, malformed feature values, a valid omitted implicit feature, matching-archive refusal and failed post-upload verification; `cargo test --locked -p gix-diff -p gix-merge --tests --features sha1` passed 93/93 and 26/26 integration tests plus 5/5 library tests in each crate; `cargo check --locked -p gix-diff -p gix-merge --all-targets --features sha1` compiled their bench targets; `cargo test --locked -p gitoxide-core --lib --no-default-features --features archive,async-client` passed 4/4 and the matching `cargo check --all-targets` exited 0. A read-only `cargo package --locked --registry elohim --no-verify --allow-dirty -p gix-diff` produced a 24-file `0.67.2` archive whose normalized manifest has `[dependencies.gix-imara-diff]` and `blob = ["dep:gix-imara-diff", …]`. `--allow-dirty` was inspection-only; the production publisher still requires clean source. No repair upload, clean consumer, or complete publication run is claimed.

P2P concern gate for this publisher predicate: C0 is a packaging/index projection, not release authority. C1–C3 are inapplicable: no election, ordering authority, or state loop is added. C4 is answered by explicit invalid-row refusal and preservation of 404 versus auth/network/server distinctions. C5 checks archive digest and dependency metadata but does not confer native authority. C6a is bounded by the existing 8 MiB index and 128 MiB archive limits, with a finite dependency scan; C6b uses the verified-equivalent skip as the idempotence boundary. C7's `verified` means archive identity plus this index contract only; clean Cargo consumer resolution is a separate station. C8 emits errors and CI output, not a metric. C9 is inapplicable: no identity is created. C10 is covered by positive and negative optional/required alias and feature-link fixtures. C11 retains existing bounded polling, adding no queue/backpressure mechanism. C12–C13 are inapplicable: no authorization or stakes are added. C14 emits a refusal log, not a durable native witness. There is no DHT entity, head, route, transport or native readiness claim. Brit's standalone Python publisher tests are the named verification authority; the parent's Rust-only seam-registry schema cannot validly register this Python predicate, so no standalone census wiring is claimed.

### Experimental source-install receipt (2026-09-27)

At Brit `8ef5375948f0c7ea9aa804b683f2ce6b55b9fd8b`, three independent `cargo install --locked --debug --path` commands installed `brit-cli` (`brit`), `brit-build-ref`, and `brit-verify` into the previously unused `/home/matthew/.local/share/brit-cutover/0.1.2-8ef537594-debug/bin/`. They used native Rust/Cargo 1.93.1, `RUSTUP_AUTO_INSTALL=0`, `RUSTFLAGS=''`, two jobs and `/tmp/brit-merge-target`; each exited 0. SHA-256: `brit` `f4e5a79ac2e0919150156d483340ad75863bae2cf66ef23031169190f39ef958`; `brit-build-ref` `b0a4e9850c0fdba88c41f08a819af5577b6186dc2ca4b5dc750465c94dcd4894`; `brit-verify` `d682aec2494207afa27f342bab3a8315d2ba3a4b742f29e29b2d34fb298ac5d8`. The installed `brit --version --verbose` reports `0.1.2`, source HEAD `8ef537594…`, frontend preset `max`, and correctly labels working-tree bytes **unverified**. Top-level and `build --help` exited 0. `brit-verify --help` exits 3 because its positional revision parser treats `--help` as a revision; that is not an executable-build failure, and real commit-verification journeys passed.

With explicit `BRIT_BIN`, `BRIT_BUILD_REF_BIN` and `BRIT_VERIFY_BIN` pointing to those installed paths, `cargo test --locked -p cli-journey --test brit --test rakia --test inherited_git_environment --test brit_build_ref --test brit_verify` passed 16 + 8 + 1 + 11 + 3 = **39/39**, with no skips. The parent `node --test genesis/orchestrator/brit-helper.test.mjs`, run with an isolated process environment and `BRIT_BUILD_REF_TEST_BIN` pointing to the installed tool, passed **11/11**, no skips; its real notes fixture preserved a failed build as `success: false`. These are local installed-source checks, not a clean registry-consumer or public-release receipt.

Separately, source-matched `epr` from parent `a373f19d35d942f662267149a091cf718db9b287` was installed into the same new prefix with `cargo install --locked --debug --path elohim/eprfs/epr-cli --bin epr`, native Rust 1.93.1, one job and `/tmp/eprfs-gate-target` (exit 0); SHA-256 `b3739e2ee1f0e83322895aedb1aec4cad0d8957021d1f6cbc476b3fd6deb2e04`. This exact binary's `epr check` exited 0 for eight current worktree paths, and `epr ready --target origin/dev` exited 0 for 21 committed paths with the governance floor coherent. Codex agent/hook projection notices and deferred `--deep` Rust quality gates remain; this is not full pre-push/CI readiness or authority to publish. No existing executable, hook, installation or PATH was replaced. An operator may opt into this experimental set in one shell with `export PATH="/home/matthew/.local/share/brit-cutover/0.1.2-8ef537594-debug/bin:$PATH"`; that command was **not** applied during qualification.

### Station 2 canonical package-source checkpoint (2026-09-27)

At `8ef537594`, [CI run 36340317267](https://github.com/ethosengine/brit/actions/runs/36340317267) passed tests but publication stopped while packaging unpublished `gix 0.87.1`: Cargo rejected `gix-hash` for different source paths across build targets. A local `cargo package --locked --registry elohim --no-verify --allow-dirty -p gix` reproduced exit 101. Aligning only its dev version still failed; adding the Nexus registry tag exposed the same split for `gix-pack`. Giving both explicitly versioned dev occurrences the same registry as their normal occurrences (and aligning `gix-hash` to `^0.26.3`) made the locked read-only package command succeed: 212 files, normalized normal and dev dependency entries all routed to the Nexus index. `gix 0.87.1` was not published in that failed run; no immutable published payload was changed by this source edit.

The publisher's static preflight now reads source manifests to catch a duplicate local dependency whose explicitly versioned normal/build/dev/target occurrences resolve to different path or registry sources. Path-only dev dependencies remain exempt because Cargo omits them from published manifests; a literal `version = "*"` is **not** exempt. Workspace-inherited duplicate local edges fail closed pending explicit support. Cargo's own package normalization remains the final authority. Red-first fake-registry tests proved this guard refuses before any package/upload; the full publisher suite passed 24/24 after the repair. Actual locked offline closure preflight passed 68/68, `cargo test --locked -p gix --lib` passed 4/4, and `git diff --check` passed. This is local source/package evidence, not a new CI or installed-consumer receipt. No crate was uploaded by the repair probe.

Story-graph missing node: chain `station 2 standalone delivery` / between `workspace tests pass` → `clean published consumer resolves` / assertion `each dependency occurrence retained in a packaged crate has one canonical source, including explicitly versioned dev and target-specific occurrences` / probe `publisher preflight contract test plus locked cargo package --no-verify` / current state `local green, remote publication retry pending`. This narrows a packaging precondition; it creates no native authority or separate readiness ledger.

### Station 2 dual-source consumer-lock checkpoint (2026-09-27)

At Brit `098cae112`, [CI run 36353151202](https://github.com/ethosengine/brit/actions/runs/36353151202) passed the test matrix and completed the 68-package Nexus publication closure. Its isolated consumer then compiled and installed `brit-cli 0.1.2`, but stopped at the lock audit before executable smokes: `gix-trace 0.1.21 resolved outside elohim`. The published `brit-cli 0.1.2` archive has index checksum `a5c9c46e73d6392bb4912f365f08a7874f5c8dbd478d5d6d03810fc3bdad04c0`. Its `Cargo.lock` legitimately contains both Nexus and crates.io rows for the same `gix-trace 0.1.21` package name/version: private `gix 0.87.1` points explicitly to the Nexus row, while the separate public `gix 0.81.0` points to crates.io. The same dual-source shape occurs for `gix-utils` and `gix-validate`. The old pair-only audit falsely rejected any public row, even when the required private row and edge were present.

The consumer verifier now treats `(name, version, source)` as a package identity. It requires each expected private fork identity and resolves every declared local normal/optional/build edge, including `brit-cli → gitoxide`, against the full lock identities; a public same-version copy is allowed only when those private identities and edges still hold. Missing private copies, public fallback with an unused private copy, ambiguous shorthand and duplicate full identities refuse. The regression skeleton is: Given public and private dependencies share a name/version, when the installed lock is audited, then fork edges select the private copy while independent public edges remain valid. The runnable probe is `python3 -m unittest discover -s scripts/ci -p 'test_verify_brit_consumer.py'` (14/14 local, including a red-first dual-source fixture). A read-only audit of the actual checksum-verified published archive passed all 66 private dependency identities and 367 local edges. This is a local verifier correction, not a new CI consumer-smoke receipt or release archive. No package payload, version or registry record changed.

Story-graph missing node: chain `station 2 standalone delivery` / between `Nexus closure published` → `clean installed consumer verified` / assertion `the installed lock binds every expected fork edge to its exact private source while permitting parallel public packages with the same name/version` / probe `consumer lock audit on checksum-verified published archive plus dual-source and wrong-edge regression fixtures` / current state `local green; fresh CI consumer smokes pending`. This is a provenance check, not native release authority.
