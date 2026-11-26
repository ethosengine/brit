---
title: Brit bounded-memory backlog
status: active
serves: brit-native-daily-driver
---
# Bounded-memory backlog

> [Brit sprint 4](2026-11-22-brit-sprint-4.md) shipped as brit 0.3.3 (2026-11-23). [Brit sprint 5](2026-11-25-brit-sprint-5.md) draws from this pool and from the elohim friction register of 2026-11-25.
> Add new findings here as one line each; do not work from this list directly.

- [ ] Release Brit for every platform for which we can obtain a runtime to build and verify the installed artifact; restore macOS ARM when a usable runtime is available. The steward suspended hosted ARM on 2026-11-17 after release 44145119161 attempts 1–4 were cancelled before any ARM step; current releases cover Linux x86_64, macOS Intel and Windows x86_64, and ordinary macOS CI uses Intel. Record per-platform build, installation and operational evidence before enabling each target.
- [ ] Investigate archive cancellation cleanup on Linux ARM: CI 44165551814 job 134330507155 failed `archive_cancels_during_large_object_spooling` with "cancelled archive must reclaim its spools" (5,964 other tests passed); this workflow-only change leaves the archive and test source unchanged. Preserve the assertion and failure receipt; runner-capacity cancellations do not explain this executed test failure.
- [ ] Investigate the intermittent Intel feedback diagnostic fixture timeout: CI 44169198431 job 134343393999 reached the two-second recorder deadline; unchanged retry 134371763105 passed. The separate filter-spawn failure reproduced twice and probe 44185287324 measured native macOS `NotFound` for the symlink loop, so its fixture now compares the preserved error with native spawning instead of prescribing Linux's errno.
- [ ] Investigate the macOS Intel fsmonitor interop flakes: CI 45615352752 (docs-only plan branch) failed `native_queries_preserve_coverage_through_unicode_and_case_root_aliases` (interoperability.rs:648), and CI 45618069624 attempt 1 failed `linked_worktree_uses_external_cookie_coverage` (:542). Neither branch touched fsmonitor; Linux and Windows passed both times.
- [x] (Fixed in brit 0.3.5: scratch refs keep no reflog.) macOS concurrent build-ref writes: CI 45618069624 attempt 2 failed brit-epr `eight_writers_by_twenty_five_puts_every_ok_reads_back_verified` (cas_writes.rs:796). Git's `update-ref` returned "unable to append to .git/logs/refs/notes/brit_tmp/...: Invalid argument" under 8 writers. The put surfaced `GitFailed` rather than the bounded `Concurrent` refusal. Either classify that EINVAL as a lost race, or write the tmp ref without a reflog.
- [ ] Local-only failures on the laptop (Ubuntu 26.04, TMPDIR under ~/.cache): 6 `gitoxide-core/tests/organize.rs` tests and 2 `replay_retained.rs` tests (extraction-directory ownership checks) fail on unchanged brit main. Codex reproduced both on 2026-11-25; CI is green on them.
- [ ] Session-pinned brit 0.3.0 `config set` is a silent no-op, so the first push of every new branch needs `git config --local branch.<b>.britReachBase` (6 times in sprint 5).
- [ ] The regex fallback for `brit grep` basic regex (platforms without POSIX regcomp) isn't checked against Git.
- [ ] Recurring macOS Intel flake: fsmonitor `native_queries_preserve_coverage_through_unicode_and_case_root_aliases` (interoperability.rs:648) failed on CI 45615352752 and on CI 45632934300 (item 1, test-only change).
- [ ] Windows flake: gix-notify `windows::nonrecursive_directory_and_nested_file_keep_independent_scopes` (tests/notify/windows.rs:43), CI 45632934300.
- [ ] Code that compiles only on some platforms isn't checked locally: item 6's Windows/musl-only `grep_portable.rs` missed an import (E0599, CI 45636199854). A local `cargo check --target <musl>` needs a musl C toolchain for openssl-sys and aws-lc-sys. Consider a cfg(test) compile of the portable backend on Linux, or a fast-mode Windows check through item 8's narrow runs.
- [ ] `brit add -A` refuses when the selection includes a submodule or a nested fixture repository, so agents stage by path (shem 2026-11-25; also seen here on `gix-odb/tests/fixtures/repos/small-packs.git`).
- [ ] `brit fetch origin <branch>` printed nothing and left the ref unfetched once, seconds after the branch was pushed; a second fetch got it (shem 2026-11-25, not reproduced).
- [ ] Windows gix-notify flake is a whole file, not one test: CI 45640631727 also failed `windows::replacing_a_directory_identity_invalidates_coverage_and_can_be_retried`, at the same assertion (tests/notify/windows.rs:43) as `nonrecursive_directory…`. Look at the shared helper at :43. Likely shared cause (2026-11-26): `collect_ready` answers `Loss::Rescan` when ready completions do not drain within `MAX_READY_DURATION` (20 ms, `gix-notify/src/backend/windows/mod.rs:26`), and `observe` asserts no loss. Under full-suite load, CI 45821645766 ran `replacement_stops_old_coverage_and_starts_new_coverage_before_returning` at 205 ms and hit it; alone it passed 50 of 50 at 15 ms. Only that test is quarantined (a7c6148d3); the other two stay in the suite and are recorded here.
- [ ] `archive_cancels_during_large_object_spooling` now also fails on Linux x86_64 (CI 45640631727, item 4 notes branch, which doesn't touch archive). It was previously recorded only on Linux ARM.
- [ ] Narrow CI, the next rung: a single-test macOS dispatch took 19 min (CI 45658959262). The test ran in 50 s; about 18 min went to building every workspace test binary, because a `--tests` filter still selects the whole workspace. Add a `packages` input (or derive the package set from `package(...)` in the filterset) so only the owning crate builds. A one-crate check should then take a few minutes.
- [ ] A single green narrow run doesn't prove a racy test fixed. `eight_writers_by_twenty_five` passed on macOS without the scratch-reflog fix (CI 45658959262). A narrow run needs a repeat count (nextest `--stress-count`, if the pinned nextest has it; otherwise run the job N times) before it can confirm a flake fix.
- [ ] Sprint reserve, owned by thinkpad: brit has no pack-maintenance verb. The laptop's elohim checkout reached 50 pack indexes against the reduced-trust cap of 32, so brit reads fail at reduced trust, and `just status device` names `git repack -a -d` as the cure. A thin client that fetches often hits this by design. Add `brit maintenance` / `brit repack` (consolidate packs under the cap, never drop reachable objects), or have fetch consolidate when it crosses the cap.

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
- [ ] The pre-push reach gate refuses tag targets (`.githooks/pre-push:19`, "only a named local branch is supported"), so release tags are pushed with `git push --no-verify` (brit-v0.3.2, 2026-11-22). Corrected 2026-11-22: `brit tag <name> [-a -m]` creates tags (`gitoxide-core/src/repository/tag.rs:169`); only the gate is missing.
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

## Found while releasing 0.3.2 (2026-11-22)

- [x] A kill at checkpoint `replay-transition-directory-created` leaves an owner-less staging directory that the next run refuses as foreign (`gitoxide-core/src/repository/replay/journal.rs:3746-3761`); recovery needs `--retire`. Sprint 4 item 4, fixed in 0.3.3.
- [x] `brit rebase --status` in `ready-final` offers `--abort`, which a final decision cannot take (`journal.rs:4602`). Sprint 4 item 3, fixed in 0.3.3.
- [x] Recovery refuses as "replay configuration or hooks changed" when only `GIT_AUTHOR_*`/`GIT_COMMITTER_*` differ, because the policy digest hashes the whole config snapshot (`replay/hooks.rs:10-47`). Sprint 4 item 5, fixed in 0.3.3.
- [ ] A kill during child preparation (`recover_preparation`, `journal.rs:2808`) has no measured recovery fixture.
- [x] `brit diff` refuses the whole diff with "submodule patches are not supported" when any gitlink differs (`gitoxide-core/src/repository/diff.rs:358`), so it is unusable in the Elohim checkout whenever a pin moves; Git prints `Subproject commit` lines. Reproduced 2026-11-22 on 0.3.0; staged-only blank output matches Git. Sprint 4 item 7, fixed 2026-11-22.
- [ ] `brit diff -- <path>` refuses any directory, including a submodule path, because it shares `literal_paths` with staging (`gitoxide-core/src/repository/index/authoring.rs:69`); Git selects the gitlink.
- [x] `brit push` sends an annotated tag as its peeled commit (`gix/src/remote/connection/push/transmit.rs:206`, `peel_to_id`), so the remote loses the tag object (sprint 4 item 1 lane, scratch reproduction 2026-11-22). Sprint 4 item 1, fixed in 0.3.3; `brit-v0.3.3` was pushed by Brit as a tag object.
- [ ] The feedback recorder calls `epr feedback` without checking the verb exists: on shem (brit 0.3.1, an epr head without `feedback`) every command prints `feedback capture gap: epr: invalid arguments: unknown command feedback` (ethosengine/shem report, 2026-11-22).
- [ ] On ethosengine, `diff`, `fetch`, `worktree --help` and `worktree add` print `feedback capture gap: recorder timeout`; the line should name its budget, and a missing verb should fail fast rather than time out (same report).
- [ ] A whole-crate `cargo test -p cli-journey` in a fresh target fails on the unbuilt `brit-rebase-retained` helper; the journey crate neither builds nor names that prerequisite.

## Found during sprint 4 (2026-11-22 and 23)

- [ ] With the host umask 0002, two `replay_retained` journey tests fail on group-writable modes; the tests should set their own umask or the check should name it.
- [ ] Two replay cleanup kill tests time out under host load; their budget should scale or name the load it assumes.
- [ ] `memory_object_store::explicit_limit_streams_two_hundred_fifty_six_mib` failed once on CI (run 45203142969, `odb stats --extra-header-lookup`: "At least one object couldn't be looked up even though it must exist") and passed on rerun and on the parent commit; record each recurrence.
- [ ] macOS `gix-discover is_git::verify_on_exfat` failed once on CI (run 45249125300: `hdiutil: couldn't eject "disk2" - Resource busy`) and passed on rerun; the test should retry the eject or report a runner fault separately.
- [ ] Windows `cli-journey::pre_push early_close_accepts_large_input_and_rejection_preserves_remote` failed after 26 s in the second pass of run 45268061115 while the first pass was green; record each recurrence.
- [ ] `brit add -A` refuses the whole selection when the tree holds a nested repository fixture (`gix-odb/tests/fixtures/repos/small-packs.git`), so a broad stage needs an explicit path list; skip or report nested repositories instead of refusing everything.
- [ ] `brit-rust-style` on a `run-ci/**` branch needs `main` to be an ancestor, so a lane branched before a main move reads red until merged; the check should diff against the merge base.
- [ ] Every brit commit prints "attribution UNMEASURED"; the prepare-commit-msg path finds no session adapter in this submodule.
- [ ] `gix-protocol` has a clippy `drop_non_drop` warning under the workspace's current toolchain.
- [ ] The push journey tests never sent a tag before sprint 4; keep a tag-object case in every push transport test.
- [ ] The nightly toolchain for the scoped `brit-*` fmt pass is not pinned, so its output can change between hosts.
- [ ] The installed brit lags the released one: the 0.3.3 tag needed the freshly built binary, because 0.3.0 still peels tags on push; release notes should say which installed version a workflow needs.

- [ ] No value-only config read: `brit config <key>` prints the filtered section (e.g. `[core]` with unrelated entries) and has no exit-code contract, so scripts still need `git config --get core.hooksPath`; add `brit config get <key>` (prints the value, exit 1 when unset) beside 0.3.3's `set`/`unset` (integration session report, 2026-11-23).
- [ ] No `brit grep <rev> -- <pattern>` and no `brit log -S <string>` pickaxe: tracing a hook change needed `git grep <rev>` and `git log -S` (same report).

Peer feedback round, 2026-11-24 (integration session on ethosengine with installed 0.3.1; shem with 0.3.1 until 03:01Z, then 0.3.4; laptop with 0.3.3). Items already listed above (cherry-pick, gitlink staging, `config get`, `grep <rev>`, `log -S`, shallow `pull`, the feedback verb check) are not repeated here.

- [ ] Most commands on all three devices print `brit: feedback capture gap: epr feedback: invalid feedback: feedback capacity format requires explicit migrate --legacy-writers-quiesced`. An agent reads it as a failure. Print it at most once per session or checkout, and name the one command that cures it.
- [ ] `brit rev-parse` and `brit ls-remote` print `brit: …` lines on stdout beside the answer, so every scripted use needs `grep -v '^brit:'`. Diagnostics belong on stderr.
- [ ] `brit status --no-write` writes the ANSI erase-line `\x1b[2K` into piped output, so a naive parse read the first path as `M`. Suppress progress control codes when stdout is not a terminal.
- [ ] Missing read verbs that sent agents to Git: `ls-tree`, `notes show` (for `refs/notes/brit/build/*`), `commit-tree`, `branch --contains`, `branch --show-current`, `remote -v` / `remote get-url`.
- [ ] Inside a submodule or a worktree's submodule, 0.3.1 refuses fetch, `checkout --detach`, ls-remote and `remote get-url` with "Selected nested repositories and submodules require Git". Re-test on 0.3.4, now that `submodule update --init` exists.
- [ ] `brit pull --ff-only` refuses when a submodule pin moved; shem uses fetch plus `merge --ff-only`.
- [ ] `brit push origin <sha>:<ref>` and `HEAD:<ref>` are refused with "only a named local branch is supported", and the message does not say how to fix it. Suggest `brit branch create <name> <rev>` then `push origin <name>:<ref>`, or accept a revision.
- [ ] A refused pre-push prints only `pre-push hook rejected the operation (exit status: 1)`; the hook's own `REFUSED` line is above it and easy to lose. Repeat the hook's last `REFUSED`/`error` line in brit's refusal.
- [ ] `brit diff` and `diff --stat` printed nothing when shem wanted one committed change's patch for one file, and shem fell back to `git show`. Add a `brit show <commit> -- <path>` example to `diff --help` and `show --help`.
- [ ] A worktree's submodule has the main checkout's local submodule as its `origin`, so ls-remote there reads a stale local copy. That misreport said 0.3.4 was missing from GitHub when it was not. When origin is a local path, brit could add a note such as "origin is a local path; the forge may differ".
- [ ] The device follow of brit reads the main checkout's gitlink, so a main checkout on an old branch never gets a newer brit. ethosengine stayed on 0.3.1, and the laptop's follow refused with "brit pin does not descend from landed head". This is the device-runtime side of "installed lags released" above; follow should read the published ref (`origin/dev:elohim/brit`).
- [ ] The landing push still uses the Git exception (send-pack after a green hook) on every device. Re-test `brit push` for the `dev` landing on 0.3.4 and retire the exception if it holds.
- Worked well on every device: `worktree add`, add/commit (including `-r <worktree>` and a gitlink staged by `git add`), `show <rev>:<path>`, `merge-base --is-ancestor`, single-ref fetch, ls-remote of a URL, `log` and `status --no-write`.

## Found during sprint 6 (2026-11-26)

- [ ] Upstream offer: the gix-fsmonitor fixes on `sprint6/fsmonitor-race` (Linux cancelled-startup bind, 712e3fa6; macOS fence completes on the observed marker, 86e5a41b) go to gitoxide as one PR the steward opens or approves. When a gix-main sync brings them in, drop their `QUARANTINED` entries in `tests/it/src/qa/native_tests.rs`.
- [ ] `brit fetch origin` exits with an error ("rejected ref update(s): refs/tags/brit-v0.3.5 (would overwrite existing tag)") on every fetch. Branch refs still update. The local tag is annotated (c6ce14d0) and the remote one is lightweight, and both name commit 89ca6e18. Report it as a warning, and treat a tag that peels to the same commit as up to date.
- [ ] `cargo-publish-brit.sh --check` validates all 73 crates and their registry edges, then fails on the laptop at `cargo package -p gix-error` with "No such file or directory (os error 2)". Find which path is missing, so the rehearsal runs end to end off CI.
- [ ] The release archive lacks `brit-build-ref` and `brit-verify`, so a device installing from the archive has no build-ref or verify tool.
- [ ] `replay_retained` umask tests depend on the host umask (0002 here). This is the sprint 4 line again; the tests should set their own umask.
- [ ] Narrow CI `--packages` should add `brit-cli` on its own whenever the selection runs journey tests, which need the built binary; today the caller has to remember it.
- [ ] Narrow CI `--repeat` should refuse a filter that selects 0 tests. A wrong package reported "50/50 passed" with nothing run.
- [ ] `brit rebase` on a shallow repository still refuses (see the 0.3.2 lines); `pull` and `rebase` need a shallow path.
- [ ] `brit notes fetch|push` and `brit pull --ff-only` ship without help text describing their arguments.
- [ ] `brit restore` covers `--source` and the short forms, but not directories or pathspec patterns.
- [ ] `brit switch` refuses when unstaged changes overlap the target, with no option to carry them; the workaround is archive, restore, switch, then unarchive.

## Usage feedback across commands

- [ ] `Tests pass` can report success after a hosted macOS matrix member is cancelled without acquiring a runner (CI 44117076422 and 44117418305, attempt 2); expose that missing coverage while distinguishing provider capacity from code/test failures.
- [ ] `brit-build-ref` still uses `chrono` for RFC 3339 timestamps; move it to `jiff` (already a gix dependency) and drop the RUSTSEC-2026-0543 ignore in `deny.toml` once `rakia-core` and `elohim-epr` drop `chrono` too. Supersedes the sprint 2 report from CI 44116534675; 0.3.4 made the advisories job green with reasoned exceptions.
- [ ] RUSTSEC-2026-0574 through 0582 (heapless) are ignored because `gix-hash` pulls `faster-hex` 0.10.1, which requires `heapless` ^0.8; remove the nine ignores when the affected `gix-hash` versions move to a `faster-hex` release on `heapless` >= 0.9.4.
- [ ] Sharing a Cargo target/profile between worktrees reused a `cli-journey` harness from another item: item 2 executed item 5's merge test, absent from its own source. Isolate test artifact profiles per checkout; frozen CLI executables alone do not isolate the harness.
- [ ] Journey build tests still infer the surrounding Elohim checkout from the source path; sprint 2's full run hit an archived manifest with a missing `profile`. The blocking discovery test now uses a disposable manifest; the other checkout-dependent probes remain to isolate.
- [ ] Feedback capture on the shem device was failing because the launching terminal exported a stale `EPR_BIN` pointing at elohim-epr-cli 0.3.2, which has no `feedback` verb; the device's installed `epr` 0.3.3 has it, and with the override removed the gap line stops. Corrects the earlier entry that blamed the installed `epr`. Remaining: a device check that reports a session `EPR_BIN` older than the installed head.
- [ ] Say why the recorder refused: `Feedback::record` discards the recorder's stderr, so a missing `epr feedback` verb reads the same as any other refusal.
- [ ] `brit diff` in a worktree of this repository refuses with "diff attributes and text conversion are not supported; use git diff" (observed with installed 0.1.13 on 2026-11-15, reviewing this document's own change).

82 items

## Trust decision: known differences from git (sprint 3 trust lane)

Git decides `safe.directory` from protected configuration read without a repository. Brit now does so for the command scope, but these differences remain:

- [ ] An `includeIf` (`gitdir:`, `onbranch:`, `hasconfig:`) in a global or system file is evaluated against the repository being opened, so it can pull in a `safe.directory` entry git would not see; `onbranch:` and `hasconfig:` are then decided by that repository's own HEAD and remotes. This predates the lane and is the same on 0.2.6. Do not keep `safe.directory` behind a conditional include. A fix in the same shape as the environment one (re-read the system and global files with an empty conditional context for the ownership decision, reusing `config::cache::init::source_path`) is about 40 lines in `gix/src/open/repository.rs` plus tests, with no gix-config change; it is a candidate for upstream.
- [ ] gix does not follow `-c include.path=…`; git does.
- [ ] `GIT_CONFIG_PARAMETERS` (how `git -c` reaches git's children, such as a hook that runs brit) is not read.
- [ ] `safe.directory=<R>/*` also trusts `<R>` itself; git requires a path strictly below.
- [ ] With `GIT_DIR` and its own `GIT_WORK_TREE` set, git skips the ownership check; gix still makes it.
