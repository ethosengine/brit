---
title: Brit capability boundaries and compatibility horizon
status: active
serves: brit-git-compatibility
---
# Capability boundaries

This is the routing map, not a second capability-status ledger. Read `crate-status.md`
for engine details, `commands.md` for the public surface, and generated `SHORTCOMINGS.md`
for declared deltas. Test the public `brit` executable before upgrading a CLI claim.
Upstream gix does not promise complete Git command parity. Brit can choose additional
workflow behavior while keeping generic improvements separable for upstream return.

The local reference checkout is `vendor/git` at v2.54.0, commit
`94f057755b7941b321fd11fec1b2e3ca5313a4e0`. Record this source revision plus the
upstream `gix-main` integration SHA and Brit HEAD with any comparison. Installed Git
is a separate oracle whose version must be recorded. This map does not infer current
upstream capability from unverified remote heads.

| Family | Upstream gix boundary | Fork / Brit boundary | Git comparison and next proof |
| --- | --- | --- | --- |
| Objects and addressing | gix-hash, gix-object, gix-odb, gix-pack; see crate-status | Git translation in brit-bridge, immutable trees in eprfs; CLI snapshot journeys | Object formats, reachability and roundtrip bytes; keep Git OIDs distinct from EPRFS CIDs |
| Refs and history | gix-ref, gix-revision, gix-revwalk | gitoxide-core history operations, public CLI dispatch | Concurrent ref updates, detached/unborn HEAD, reflogs, ancestry and stock Git readability |
| Index and worktree | gix-index, gix-worktree, gix-worktree-state | gitoxide-core authoring and workspace orchestration | Scope isolation, staged bytes/modes/gitlinks, conflicts, linked worktrees, interrupted recovery |
| Diff, merge and sequencing | gix-diff, gix-merge and repository APIs | gitoxide-core merge/rebase, public command policy | Conflict preservation and abort/recovery; cherry-pick/revert/stash gaps stay explicit |
| Transport | gix-transport, gix-protocol, gix-negotiate | gitoxide-core fetch/push and public refusal policy | Authentication, rejection, leases, push options and submodule policy; pre-push must not silently disappear |
| Configuration and hooks | gix-config and repository config APIs | Brit hook invocation and public config behavior | Trust, core.hooksPath, executable hooks, stdin/env, refusal semantics and explicit bypass evidence |
| Delivery and governance | upstream feature/platform/test matrix | brit-cli, installed epr flow, eprfs; packaging and first-party CI | Standalone installation, native review/integration, exact source-bound matrix evidence, upgrade/rollback |

## Markers

`brit-native-daily-driver` is the native workflow marker. It allows explicit Git
bridge operations with declared limitations. Its stations require safe scoped
work, enforced gates, target-bound independent acceptance, recoverability, Git
interoperability and installable delivery. It is currently unwired.

`brit-git-compatibility` is the maximal comparison horizon. Intentional differences
stay in the denominator with rationale. There is no percentage until enumeration
and public conformance are wired. Neither marker follows automatically from the
forge's aggregate `Tests pass` check.

## Feedback upstream

Keep `gix-main` a pristine fast-forward mirror. Generic fixes belong in the owning
gix crate with focused tests, proposed separately from Brit orchestration; approval
and submission remain explicit actions. Integration reruns inherited and first-party
checks on the merged revision. Never copy the gix engine into a parallel Brit layer.
