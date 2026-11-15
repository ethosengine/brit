---
title: Brit bounded-memory backlog
status: active
serves: brit-native-daily-driver
---
# Bounded-memory backlog

These items are not release gates; order is set by usage feedback, not by this
list. The [0.2.0 scope ruling](2026-09-28-daily-driver.md#2026-11-15--020-release-scope-ruling)
supersedes the complete-matrix release requirement. Open work below comes from
the frozen [public-command audit](2026-09-28-daily-driver.md#public-command-audit-in-progress)
and its outstanding acceptance paragraph, with scratch and reported usage/fixture
limitations retained. Shared requirements are listed once; the habit stays RED.

## `diff`, `cat`, `add`, `restore`, `archive`

- [ ] Pinned diff stays open.
- [ ] Larger aggregate selections stay open.
- [ ] Remaining fault cases stay open.
- [ ] Other comparison-cache consumers remain open.

## `status`, `is-clean`, `is-changed`, `index`, `dirwalk`, `clean`

- [ ] Other metadata paths still need complete tracing.
- [ ] Dirwalk paths still need complete tracing.
- [ ] Clean paths still need complete tracing.

## `worktree`, `clone` checkout

- [ ] Other protocols remain open.
- [ ] Reduced-trust transfer paths remain open.

## `switch`, `pull`, `rebase`, `merge`, `commit`

- [ ] Other authoring metadata remains open.
- [ ] Other journal metadata remains open.
- [ ] External scratch remains open.
- [ ] External cancellation remains open.
- [ ] Large journals remain open.
- [ ] Resource-bound recovery/death fixtures remain open.

## `blame`, `log`, `revision`, `merge-base`, `tree`

- [ ] Large history metadata remains open.
- [ ] Larger SVG layouts stay open.
- [ ] Real-viewer lifetime stays open.
- [ ] Remaining history consumers stay open.

## `fetch`, `push`, `odb`, `fsck`, `verify`, `commit-graph`

- [ ] Additional protocols stay open.
- [ ] Other metadata families stay open.

## `branch`, `tag`, `remote`, `submodule`, `config`, `config-tree`, `attributes`, `exclude`, `mailmap`

- [ ] Other remote metadata stays open.
- [ ] Other submodule metadata stays open.
- [ ] Other path metadata stays open.
- [ ] Helper dispatch stays open.
- [ ] Resolve the case-11 FUSE hardlink fixture limitation: the second device's report records 10 of 28 `memory_metadata` cases passed; SHA-1 `describe_preserves_large_metadata_and_tag_selection` failed in stock Git's `submodule add` local clone with “hardlink different from source” on the FUSE scratch mount, after the Brit assertions before it passed; 17 cases had not run. This is a harness/fixture limitation, not a Brit blocker.

## `credential`, `env`, `editor`, `completions`, `free`, `corpus`, `tix`

- [ ] Applicable metadata audit remains open.
- [ ] Applicable output audit remains open.
- [ ] Applicable owned-child audit remains open; existing unsupported behavior cannot be relabeled a memory success.

## `snapshot`, `build`, `run`, `graph`, `affected`, `plan`, `fingerprint`, `baseline`, `integrate`, `context`

- [ ] Capture remains open.
- [ ] Other first-party metadata paths remain open.
- [ ] Other first-party output paths remain open; arbitrary external tasks are outside a universal success guarantee, but owned children still count in measured workloads.

## `tix`

- [ ] Other editing stays open.
- [ ] Interactive loads stay open.
- [ ] Large journals stay open.
- [ ] Large notes-tree populations stay open.
- [ ] CLI-only worker override propagation stays open.
- [ ] Retained edit state stays open.
- [ ] Owned processes stay open.

## `credential`, `editor`

- [ ] Adversarial helpers remain open.
- [ ] Interactive editor lifetime remains open; the recorded measurements do not qualify arbitrary programs.

## `env`, `config-tree`, `completions`, help/version/surface output

- [ ] Output I/O checks remain part of the ordinary fault/child suite.
- [ ] Owned installation-path helper checks remain part of the ordinary fault/child suite.

## `free` metadata and transport helpers

- [ ] Remaining metadata coverage stays open.
- [ ] Remaining transport helper coverage stays open.

## `corpus refresh/run` (normal default `max`)

Aggregate scratch and final-source qualification stay open under the shared
requirements below, as they do for the other affected command families.

## Across command families: outstanding acceptance and scratch measurement

- [ ] Broader reduced-trust command coverage remains open.
- [ ] Broader hash/alternate/replacement combinations beyond the expanded daily matrices remain open.
- [ ] Additional metadata families over 16 MiB remain open.
- [ ] Aggregate scratch accounting and telemetry remain open: scratch peaks remain unmeasured, with `qualificationComplete: false`; the separate 1 GiB scratch cap is not qualified by the memory measurements.
- [ ] Aggregate child-process storage remains open.
- [ ] Complete durable-journal inventory remains open.
- [ ] External-driver temporary files remain open.
- [ ] Allocation faults remain open.
- [ ] I/O faults remain open.
- [ ] Corruption remains open.
- [ ] Cancellation/death remains open.
- [ ] Cross-platform checks remain open.
- [ ] Ordinary recovery suites remain open.
- [ ] Fault-enabled recovery suites remain open.
- [ ] Committed-source receipts and final-source qualification remain open.

## Usage feedback across commands

- [ ] Restore feedback capture on the shem device. The installed Brit 0.1.13 prints `brit: feedback capture gap; command result unchanged` after every command. Observed cause, 2026-11-15: Brit's recorder runs `epr feedback capture`, and the installed `epr` (elohim-epr-cli 0.3.2) answers ``unknown command `feedback` ``, so the recorder exits non-zero. `brit-cli/src/feedback.rs` is unchanged in 0.2.0; the cure is a current `epr` on the device, not a Brit change. Usage feedback is now the prioritisation input.
- [ ] Say why the recorder refused: `Feedback::record` discards the recorder's stderr, so a missing `epr feedback` verb reads the same as any other refusal.
- [ ] `brit diff` in a worktree of this repository refuses with "diff attributes and text conversion are not supported; use git diff" (observed with installed 0.1.13 on 2026-11-15, reviewing this document's own change).

63 items
