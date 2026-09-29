---
epr-habit-version: 1
id: brit-feature-reconciliation
invariant: >
  A developer or agent entering a feature sees the same native reconciliation,
  explained priorities and claim or resume actions without reconstructing past sessions.
status: red
active: false
refs:
  - docs/plans/2026-09-28-governed-readiness.md
checks:
  - "cargo test -p cli-journey --test context"
  - "epr flow context docs/plans/2026-09-28-governed-readiness.md --root . --json"
retire-when: >
  when the common feature entry point is the independently accepted and continuously
  exercised developer default across standalone installations and harnesses
---
DELTA 2026-09-28: Public Brit dispatch delegates to installed epr with unchanged
arguments, output and failure status. Missing epr refuses with an installation
action. Full standalone source-to-claim-to-independent-acceptance and cold/warm
burden measurements remain required before green; dispatch tests alone are insufficient.
DELTA 2026-09-28 (local transport verification; RED preserved): Fresh public Brit
build passed; three CLI unit tests and two context journeys passed with no skips.
The journey preserves spaced arguments, stdout/stderr and exit 7, and refuses when
installed epr is absent. Combined brit-cli/gitoxide-core/cli-journey all-targets
Clippy with -D warnings exited 0. These establish the local transport, not remote
matrix qualification, standalone epr publication or complete habit delivery.

Run the journey with a freshly built BRIT_BIN under the native Cargo pool contract.
The context command is an inspection: assess evidence, ownership and actionable
ordering; exit zero alone never qualifies this habit.

DELTA 2026-09-28 (native public-command fixture; RED preserved): Fresh candidate
epr and Brit passed nine isolated checks: own repository attribution, idempotent
projection, read-only context, identical Brit/epr JSON, habit-to-feature entry,
retained owner after claim, production requiring review, review requiring separate
acceptance, and changed source exposing historical work. First entry 0.278s, warm
fresh-process median 0.272s after projection (not OS-cache-cold). Native owning gate
passed 1,142 tests; final view polish passed 281 library + 12 acceptance tests and
strict Clippy/format. Standalone memory bootstrap and independent operational
acceptance remain explicit stations; installed epr was not replaced.
