---
epr-habit-version: 1
id: brit-native-daily-driver
invariant: >
  A native developer can safely author, recover, reconcile, independently review
  and integrate scoped work through Brit with enforced gates and stock Git interoperability.
status: unwired
active: false
refs:
  - docs/plans/2026-09-28-daily-driver.md
retire-when: >
  when the full native workflow is the continuously verified release and integration floor
  and this separate cutover marker adds no information
---
DELTA 2026-09-28: Declared, unqualified. Local staged-authoring tests and shared-tree
proof do not establish pre-push enforcement, independent target-bound acceptance,
recovery across interrupted operations, or installed release delivery. Git remains
the explicit bridge for unsupported operations. Qualification must first wire all
stations in docs/plans/2026-09-28-governed-readiness.md and the readiness contract.
DELTA 2026-10-12: UNWIRED preserved; independently rebuilt public binary passed 56 CLI tests (pre_push 8, commit_cleanup 7, authoring 19, existing journeys 22). Exact resolved pre-push input, rejection safety, 2,000-ref early-close handling and noninteractive cleanup now have local proof; unsupported lease protections and unverified overwrites fail closed. Sprint evidence and remaining engine/installation/integration gaps: docs/plans/2026-09-28-daily-driver.md. Remote CI and whole-workflow acceptance remain unproven.
DELTA 2026-10-12: Qualification sprint preserves UNWIRED. Independent rebuild passed 60 public CLI tests and strict scoped Clippy, including scoped interruption/recovery and installed-evaluator forwarding. Installer fault fixtures, 14 consumer tests and 12 readiness tests passed. Native evaluator has 8 standalone governance and 5 integration fixture passes; combined installation, exact-source remote CI, real released-version upgrade and two-human acceptance remain distinct stations in the existing plan.
DELTA 2026-10-12: Native gate/integration implementation now passes the full EPR workspace (1,194 tests, strict lint and formatting). An actual clean-source Linux release candidate passed installation, 60 CLI journeys and six real installed Brit-to-epr integration cases. UNWIRED remains: exact-source CI found lint/snapshot/macOS fixture corrections, and actual released-version upgrade plus two-human acceptance remain open. Detailed source and binary pins are in the existing daily-driver plan.
DELTA 2026-10-12: Bounded repository reach adapter independently passes 27 mechanics checks plus real installed epr setup/doctor/readiness and an invalid-governance push refusal with no remote movement. It binds one clean checked-out branch to the advertised commit or explicit first-push baseline and refuses hidden index flags and unsupported ref shapes. Candidate d6cb92865 passes remote lint and inherited journey jobs; complete CI, final installed artifact and human acceptance remain pending. UNWIRED preserved.
DELTA 2026-10-12: Final clean-source Linux artifact d6cb92865 passed installed journeys 60/60, real installed integration 6/6 and actual same-version A-to-B-to-A-to-B replacement/rollback with exact checksums. Hook eff40d6ab was activated and its own push passed native readiness; a combined installed Brit-to-hook-to-EPR probe accepted valid governance and refused invalid governance without remote mutation. Exact-source CI runs 38259066507 and 38260354830 each passed all 29 jobs, with publication skipped. The existing plan records artifact pins and launch instructions. Published-version upgrade and two-developer acceptance remain open; UNWIRED preserved.
DELTA 2026-10-12: Installed acceptance sprint preserves UNWIRED. Pinned private Brit/EPR pair installation passes fault, interruption and overlapping-activation regression checks plus source-free operations and 27 hook mechanics with real EPR governance refusal. Manual branch qualification now prepares the existing installed-platform matrix without release writes; remote execution remains pending. Public native ceremony passes with explicit agent identities and refuses replay; human commands are documented, not accepted on their behalf. Compression-library isolation reproduces exact inherited pack/index hashes and all ten unchanged file-transport assertions; full inherited journey is separately blocked by sandbox TCP restrictions. Published-version upgrade and actual two-person operation remain open in the owning daily-driver plan.
