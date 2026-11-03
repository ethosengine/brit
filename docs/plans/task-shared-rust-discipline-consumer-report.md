---
id: shared-rust-discipline-consumer-report
title: Brit local Rust discipline adapter implementation
status: DONE_WITH_CONCERNS
gap: plans__2026-11-02-shared-rust-discipline#5
actor: agent:implementer@gpt-6
commits: []
cites: []
---

# Brit consumer implementation

The local, committed pre-push and required CI adapters are implemented. The actual
addressed discipline and source bundle are held in Brit, the standalone checker
was built and installed offline, and the working-tree trial passed: 14 files in
one crate (`brit-build-ref`), zero debt, enrollment true. The coordinator's receipt
is `genesis/local-dev/shared-rust-discipline/brit-trial.json` in the parent campaign.
Missing artifacts or installation refuse. This seat made no commit or push;
local trial evidence establishes neither committed enrollment nor remote adoption.

`scripts/ci/rust-style.sh` invokes only an explicitly installed checker after the
installer verifies its receipt. Local checks default to `origin/main`. Committed
checks require distinct full source/baseline IDs, and require the held source
declaration and archive to byte-match their committed objects. Native Rust code
owns policy/method/guide verification and diagnostics; Bash does not reimplement
those semantics. The standalone distribution agent owns the installer and its
source manifest/archive.

The pre-push hook passes the exact source and ancestor already established by its
existing clean-tree guards. It rechecks the source after both native evaluators.
The new required `brit-rust-style` CI job installs from held addressed sources,
runs boundary and native fixtures, and evaluates the checked commit. `Tests pass`
and `.epr-meta/readiness.json` both require it. CI never compares a source against
itself: an absent event base selects main, or the source's first parent when main
is that source; unavailable or unrelated ancestors refuse.

The root manifest's appended guidance and `docs/rust-discipline.md` describe the
authorized `brit-build-ref` trial, independent local standing, unresolved identity,
enrollment semantics, installation, and source-only coverage. Existing unrelated
root-manifest changes and other dirty Brit files were preserved. No permanent
strict binding was added before native debt measurement.

## Evidence

- Gate evidence: `bash scripts/ci/test-rust-style.sh` — `EXIT=0`; isolated doubles
  prove exact input binding, missing installation, native refusal propagation,
  absence of PATH fallback, self/zero baseline refusal, and dirty committed source
  declaration/archive refusal. The coordinator subsequently ran the actual native
  fixture successfully, including inherited debt, enrollment, explained and
  unexplained suppressions, committed failure despite dirty repair, corrupt guide
  refusal and committed closure isolation.
- `bash scripts/ci/test-rust-style-ci.sh` — `EXIT=0`; initial-source refusal,
  main/manual/new-branch ancestor selection, self/stale/unrelated refusal.
- `bash scripts/ci/test-brit-reach-hook.sh` — `EXIT=0`; prior boundary suite plus
  style failure/unmeasured/mutation refusal and actual remote preservation.
  Optional installed-epr proof was not run.
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/ci/test_qualify_readiness.py` —
  12 tests passed, `EXIT=0`; new required job matches the aggregate workflow.
- `bash scripts/ci/test-install-rust-style.sh` — `EXIT=0`; distribution agent's
  isolated installation boundary fixtures. Separately, the coordinator completed
  the actual offline source-bundle build and installation successfully.
- `sh -n .githooks/pre-push` and `bash -n` on the new adapter/CI/test scripts —
  `EXIT=0`.
- Coordinator-reported integrated gate: `just gate rust-style` — PASS;
  30 native tests, eight hook tests, four packaging tests, and 2090 files across
  84 crates. Committed scope fixtures and the reach-hook suite also passed.
- Actual working-tree trial: PASS, 14 files, one crate, debt zero, enrollment true;
  the receipt named above records the measured source and verified discipline.

Brief context returned zero current assertions and an ambiguous gate. Parent
requested these focused Brit checks. Brit's root gate is upstream CI attestation;
no remote check or submodule-pin acceptance is claimed for these uncommitted edits.

## Delivered addressed closure

`.epr-meta/rust-style-policy.json` selects `scope_roots: ["brit-build-ref"]`, the
RS-GOV-04 rule and the verified discipline references. Policy and method objects
are held under `.epr-meta/rust-style/objects/`. The guide pointer selects
`docs/rust-style-guide.md` directly: the readable projection is the same verified
raw guide bytes, with no separate editable guide copy in the object directory.
`source.json` identifies the held archive used for the actual standalone build.

- Full policy: `bafkreidd2ije344x7jldsfitomqcc6smrnsp6bhuinn2sblfsiuth6yf6i`.
- Guide: `bafkreidv7yjl7bp5gls3uwbldjp5rdni3robbnbdfkxwsjvcnjn2wpt2j4`.
- Checker method: `bafkreidmczh47krdmia4qsv4sgmovx6cyxj4d4bjvtybpot6qb2ft36h2y`.
- Source archive: `bafkreiekgcq6dzpw2r5asipbeyjlvbffrac72e4pjtjulvviwmghvpcx2u`.

## Remaining qualification

Status remains HOLD pending the coordinator's final integrated proof, including
the full EPRFS gate. The passing committed-source fixtures are isolated tests;
they do not invent Brit's enrollment commit or establish upstream CI success.
Actual committed enrollment, remote CI, submodule-pin attestation and any
peer-verifiable network adoption remain separate claims. No additional human
permission gate is introduced.

The terminal HOLD observation was attempted from Brit's root and returned
`EXIT=2`: its sidecar does not know the parent's commitment identifier. No
fulfilment was recorded and the parent was notified; the report remains the
local evidence.

## Final parent verification

The parent's normal Rust-style, orchestrator and EPRFS gates all pass. EPRFS
reports 1,970 passing Rust tests and zero failures; the six Rust-style a2o
scenarios pass. This resolves the pending parent verification recorded above;
local delivery is complete. Committed enrollment, remote CI, network witnessing
and future scope/checker migration remain distinct and unclaimed. The canonical
parent report is `genesis/docs/superpowers/sprints/task-srd-5-report.md`.
