---
title: Brit bounded-memory backlog
status: active
serves: brit-native-daily-driver
---
# Bounded-memory backlog

> The work in flight is [Brit sprint 2](2026-11-16-brit-sprint-2.md): seven items drawn from this pool.
> Add new findings here as one line each; do not work from this list directly.

- [ ] Release Brit for every platform for which we can obtain a runtime to build and verify the installed artifact; restore macOS ARM when a usable runtime is available. The steward suspended hosted ARM on 2026-11-17 after release 44145119161 attempts 1–4 were cancelled before any ARM step; current releases cover Linux x86_64, macOS Intel and Windows x86_64, and ordinary macOS CI uses Intel. Record per-platform build, installation and operational evidence before enabling each target.
- [ ] Investigate archive cancellation cleanup on Linux ARM: CI 44165551814 job 134330507155 failed `archive_cancels_during_large_object_spooling` with "cancelled archive must reclaim its spools" (5,964 other tests passed); this workflow-only change leaves the archive and test source unchanged. Preserve the assertion and failure receipt; runner-capacity cancellations do not explain this executed test failure.
- [ ] Investigate the intermittent Intel feedback diagnostic fixture timeout: CI 44169198431 job 134343393999 reached the two-second recorder deadline; unchanged retry 134371763105 passed. The separate filter-spawn failure reproduced twice and probe 44185287324 measured native macOS `NotFound` for the symlink loop, so its fixture now compares the preserved error with native spawning instead of prescribing Linux's errno.

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

- [ ] Intermittent HTTPS `free pack receive` teardown abort: CI 43804826832 failed protocol 1 with `double free or corruption (out)` and 43807568719 attempt 1 failed protocol 2 with `corrupted double-linked list`; attempt 2's complete `test-journey` job passed on 2026-11-16 without source changes, on the same runner image as green 43783334614. Retain the test and rerun this job if it recurs on sprint-2 branches; cause remains unproven.
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

## Found while releasing 0.2.0

- [x] Port `a1263002` (disk-sized object-store slots above 32 pack indexes). Not needed: `fe6da99ff`, already in 0.2.0, sizes slots from the final trust. Its 33-pack tests and a clearer slot-exhaustion message landed instead; see the plan's 2026-11-15 correction.
- [ ] At reduced trust above 32 pack indexes, 0.2.0 reports "rev-spec is malformed" instead of naming the slot cap and its cure (`safe.directory` or a repack). The message is fixed on main after 0.2.0; it reaches devices with the next release.
- [ ] `epr bin follow` cannot carry a lineage ruling: it refuses `non-descendant` before building, and `--rebase-lineage` exists only on `epr bin land`, which needs an ingested record; a device whose registry predates `brit-rebase-retained` then refuses the release closure with `checksums-mismatch` (second-device report).
- [ ] `brit tag` only lists; a release tag cannot be created, and the pre-push reach gate refuses tag targets ("only a remote branch is supported").
- [ ] `brit config` has no write form; setting `branch.<name>.britReachBase` needed `git config`.
- [ ] No command stages a submodule gitlink at a named commit without a checked-out submodule; the parent pin move needed `git update-index --cacheinfo`.
- [ ] `brit diff <revision>` is refused as an unexpected argument in 0.2.0.
- [ ] No patch-apply or cherry-pick command; porting one reviewed commit needed `git apply --3way`.
- [ ] `epr bin follow --tool brit` builds Brit from source although the published `brit-build-<sha>` release exists for the same commit.
- [ ] `json` `revision previous-branches` under an allocation limit is the step most sensitive to scratch-filesystem throughput: over a 200,000-entry reflog it exceeded 600 s on single-threaded fuse2fs and finished in 64-84 s natively (second-device observation, not a defect claim).

## Found dogfooding 0.2.0 across two devices (2026-11-15 and 16)

- [ ] The feedback store is per checkout root, so every fresh worktree prints `feedback capture gap` and drops its usage records until that root's store is migrated. Usage feedback is the prioritisation input, and worktrees are where most agent work happens.
- [ ] A hook refusal on push prints a `Stack backtrace` of `<unknown>` frames after the refusal text.
- [ ] `brit pull` refuses with "shallow repositories require Git" and moves nothing; both devices fast-forwarded with `git pull --ff-only` or `git merge --ff-only`.
- [ ] `brit rebase` refuses on the same shallow repository.
- [ ] A stale `.git/worktrees/<name>/AUTO_MERGE` file makes `brit commit` refuse; it had to be cleared with Git.
- [ ] No path restore, no stash or object-only snapshot, and no three-way file merge: bringing a dirty checkout onto `dev` needed `git checkout HEAD -- <paths>`, `git stash create` and `git merge-file`.
- [ ] No remote-ref listing; reading a remote branch head needed `git ls-remote`.
- [ ] No submodule init or update (`brit submodule` only lists); a fresh worktree needed `git submodule update --init` before its push hook would pass.
- [ ] `brit log -- <path>` answers that file-based lookup is not implemented.

## Usage feedback across commands

- [ ] `Tests pass` can report success after a hosted macOS matrix member is cancelled without acquiring a runner (CI 44117076422 and 44117418305, attempt 2); expose that missing coverage while distinguishing provider capacity from code/test failures.
- [ ] Sprint 2 CI 44116534675 reports RUSTSEC-2026-0543 (chrono unmaintained; used directly and by EPR/Rakia/ts-rs) and RUSTSEC-2026-0544 (libz-sys 1.1.31 through curl; patched in >=1.1.33); remediate dependencies in their owners. The existing advisory job is explicitly non-blocking; no exception or gate change was added.
- [ ] Sharing a Cargo target/profile between worktrees reused a `cli-journey` harness from another item: item 2 executed item 5's merge test, absent from its own source. Isolate test artifact profiles per checkout; frozen CLI executables alone do not isolate the harness.
- [ ] Journey build tests still infer the surrounding Elohim checkout from the source path; sprint 2's full run hit an archived manifest with a missing `profile`. The blocking discovery test now uses a disposable manifest; the other checkout-dependent probes remain to isolate.
- [ ] Feedback capture on the shem device was failing because the launching terminal exported a stale `EPR_BIN` pointing at elohim-epr-cli 0.3.2, which has no `feedback` verb; the device's installed `epr` 0.3.3 has it, and with the override removed the gap line stops. Corrects the earlier entry that blamed the installed `epr`. Remaining: a device check that reports a session `EPR_BIN` older than the installed head.
- [ ] Say why the recorder refused: `Feedback::record` discards the recorder's stderr, so a missing `epr feedback` verb reads the same as any other refusal.
- [ ] `brit diff` in a worktree of this repository refuses with "diff attributes and text conversion are not supported; use git diff" (observed with installed 0.1.13 on 2026-11-15, reviewing this document's own change).

82 items
