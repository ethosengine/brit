---
epr-meta-version: 1
id: brit-native-developer-boundary
root: true
purpose: >
  Brit is the standalone Git bridge. The local brit-cli package builds one public
  executable with Git and build commands. This anchor terminates the parent
  repository's governance cascade.
---

# Brit's native developer boundary

## Shared application capabilities

Brit composes the protocol's working-version, sync-state, publication and serving
contracts; it does not define competing versions of them. In the Elohim repository,
`.epr-meta/manifest.md` section "Application lifecycle — authoritative homes and
composition" routes their owners. The standards are
`genesis/docs/superpowers/specs/2026-10-04-working-version-sdk-standard-design.md`
and `2026-08-29-sync-state-contract-design.md`; their implementation gaps remain
explicit. These are architectural references, not a required sibling checkout.

Author Git/source interpretation here; shared client lifecycle in the Elohim SDK,
node document sync/custody in storage, publication authority in DNA, and peer/web
serving in hub/doorway respectively. Rakia owns build/artifact/quality-service
interpretation, including its GUI. A Brit sprint names its producer dependency and
consumer journey rather than filling an absent shared primitive with a private
implementation. Local commits and Git refs remain Git semantics; an Automerge draft
does not replace them or establish acceptance. This root adopts the routing as review
guidance; parent manifest links do not mechanically govern this standalone repository.

`brit-cli` owns the public `brit` executable. Local source and focused tests now
exercise Git and build commands through that one executable. Installation,
publication, and the accountable record/prove/release journey remain separate
unqualified stations. The root `src/plumbing/` owns parsed Git command dispatch;
`gitoxide-core`, `gix` and the other gitoxide crates own Git operations, objects,
index, refs and compatibility. Git plumbing does not decide Elohim authority.
Keep upstream Git parity work separate from first-party orchestration.

The existing `epr` flow in the Elohim workspace owns intent, commitment,
attribution, independent review, acceptance and gates. The `eprfs-core` and
`eprfs-local` crates in that workspace own canonical trees, addressing and safe
local projection; standalone brit consumes their published interfaces when
available. A planned shared tree link needs an explicit `BlobLink` migration
with byte and CID conformance tests: a blind `BlobCid` alias can change serde
bytes and therefore addresses. Content governance owns channel standing,
lineage, delegation, revocation and election. A CID returned by an HTTP `/epr-head` endpoint is a
locator, not a verdict of authority. Storage carries verified bytes and evidence;
possession does not elect a head. Rakia consumes declared source and toolchain
inputs to plan reproducible outputs. Brit and hApp are peer consumers of the
artifact-neutral core; generic Git and build APIs must not require Holochain
packaging.

The more specific manifests below carry the edit-time signals. This root is a
standalone anchor and human guide, not a copy of the parent repository's policy
registry or hook implementation.

## Managed responsibility

This root manages Brit's developer composition and readiness, including its public
CLI, fork integration, delivery evidence and local work declarations. It consumes
gix discipline without claiming authority over upstream maintainers. The preserved
gix crate catalog (`crate-status.md`) describes engine capability; public command
qualification belongs to `docs/parity/`. The pinned `vendor/git` source defines a
comparison horizon, not a replacement implementation mandate.

Repository identity is declared in `repository.yaml`. Native context reads local
habit declarations; `recipes.yaml` binds the existing flow projector to Brit docs.
Run `epr flow project --root . --recipes .epr-meta/recipes.yaml`, then
`brit context docs/plans/2026-09-28-governed-readiness.md --root .`.
These commands require installed epr. No sibling checkout or copied governance
engine is an installation dependency. Context and readiness are derived views;
private recall packets never become acceptance or habit evidence by themselves.

## Shared Rust discipline trial

Brit locally adopts the addressed Rust discipline held by
`.epr-meta/rust-style-policy.json` for `brit-build-ref/**/*.rs`. That file names
the immutable policy, guide and checker method; `.epr-meta/rust-style/objects/`
holds the policy and method, and `docs/rust-style-guide.md` holds the verified guide.
These holdings have shared content identity. They
do not give Elohim's repository authority over Brit or establish network standing.
The first commit carrying this policy is the enrollment commit. Comparison with
an earlier baseline enforces no new unexplained suppressions; once enrollment is
in the baseline, touched crates must also reduce existing debt. A zero-debt
strict-scope enrollment must be separately measured and explicit.

The standalone `epr-rust-style` checker interprets this full discipline envelope.
The generic `.epr-meta` resolver does not interpret it as a new validator binding.
Run `bash scripts/ci/rust-style.sh` for working-tree advice. The checked-in
pre-push adapter evaluates the exact pushed commit and reviewed ancestor, and
the required `brit-rust-style` CI job evaluates committed inputs. Setup and
limitations are in `docs/rust-discipline.md`.

This trial preserves upstream `gix-*` and `vendor/git`, existing formatting/SLOC
constraints, per-crate MSRVs, and compiler/feature checks. It covers Rust source
attributes, including inactive cfg branches; it does not inspect command-line or
Cargo suppression allowances. Scope growth, migration and withdrawal require a
new explicit local decision. No automatic policy successor activation is granted.
