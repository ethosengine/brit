---
id: task-parity-reconcile-2-report
gap: plans__2026-09-28-parity-evidence-reconciliation#2
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
serves: brit-git-compatibility
commits: []
---
# Reconcile source rows with witnessed assertions

Implemented `etc/parity/reconcile.py` and ten focused regressions. The reconciler
retains all 159 census commands, derives explicit title/it and title-only rows from
the existing suites, and joins observations by source range and literal labels.
A guard shared by multiple rows remains ambiguous with candidate row identities;
no assertion or source row is duplicated to resolve that ambiguity. Arbitrary Bash
is never evaluated. Literal option spellings link only to candidate shared atoms.

Fresh admission checks all six runtime method hashes, complete fixture/suite/snapshot
inventory, stable measurement inputs, measured executable fingerprints before/after,
actual Brit execution, assertion start/end identity, source/hash lane, known assertion
strength and consistent exit evidence. Cross-command argv cannot qualify a command
merely because it appeared in that command's suite. Unknown modes, stale sources,
wrong binaries, infrastructure probes, skips, deferred assertions and failures remain
visible. Passing a helper does not certify its row, suite, option or command complete.

The JSON and Markdown CLI expose disjoint outcome/strength/freshness counts, source
row/hash-lane denominators, candidate associations and all command detail. Reports
bind current reconciler bytes and canonical census/receipt digests for the parent's
native observation adapter. `unexecutedSourceLanes` means no fresh bound observation;
stale evidence does not reduce it, while observed skips remain explicitly skipped.

Actual measurement integration used the parent's local receipt
`.eprfs/status/parity/2026-09-28/receipt.json`. The current source census has 1,577
rows and 3,154 row/hash lanes. Reconciliation retained 220 matched observations,
71 ambiguous guard skips and 18 unverified infrastructure observations. The fresh
matched outcomes were 60 passing exit comparisons, 35 failing exit comparisons,
four failing normalized-output comparisons and 103 skips. Eleven of 159 commands
have at least one fresh passing exit assertion; this is not a parity percentage.
The remaining 2,952 source lanes lack fresh bound observations. The runtime suite
contains real failures, and this work does not admit them as green behavior.

Gate evidence: `epr flow context` for both owned files reports
`GATE (no gate project covers this path)`. Focused verification:

- `python3 -m unittest discover -s elohim/brit/etc/parity -p 'test_*.py' -v` — 50 tests, `EXIT=0`; log `/tmp/brit-reconcile-tests.log`.
- `python3 elohim/brit/etc/parity/reconcile.py --receipt elohim/brit/.eprfs/status/parity/2026-09-28/receipt.json` — actual receipt integration, `EXIT=0`.
- `python3 elohim/brit/etc/parity/reconcile.py --markdown` — no-receipt report keeps all commands and unexecuted rows, `EXIT=0`.
- `git -C elohim/brit diff --check -- etc/parity/reconcile.py etc/parity/test_reconcile.py` — `EXIT=0`.

The complete-tree diff check initially reported an unrelated generated
`docs/parity/SHORTCOMINGS.md` trailing blank line; the parent owns its generator fix.
Synthetic tests cover failures, deferrals, negative inputs, missing tails, dynamic
source, fixture additions, method/binary drift, invalid receipts, duplicate inputs,
wrong executable/command, title-only rows, ambiguous guards and the real denominator.
No Cargo, commits or pushes were performed. Independent review remains required.
