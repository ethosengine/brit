---
gap: plans__2026-09-28-parity-evidence-reconciliation#3
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
commits: []
serves: brit-git-compatibility
title: Brit parity reconciliation — Middot evidence for the Mishpat lens
id: task-parity-reconcile-3-report
tags: [mishpat, middot, brit, git-compatibility, parity, evidence, measurement]
---
# April parity evidence reconciled with the atomic census

Executed all 22 existing suites (21 command suites plus infrastructure smoke) under
both hash lanes, using disposable input copies and exact binaries. The run produced
44 lane receipts, stable inputs and measurementComplete=true: EXIT=0 means valid
measurement production, not passing parity. Five lanes completed and 39 failed;
individual suites retain their legacy fail-fast behavior. No Rust source changed.

The reference census remains Git v2.54.0 at
94f057755b7941b321fd11fec1b2e3ca5313a4e0. The actual executable oracle was installed
Git 2.43.0, a different comparison context. Brit identified itself as 0.1.3, source
HEAD 1017e94646275d167b21425100c2655a18e84826, max frontend, with working-tree source
state explicitly unverified. Receipt SHA-256s identify the actual binaries, not an
assertion that source HEAD attests their bytes. No remote CI execution is claimed.

## Reconciled evidence

- 159 reference commands remain; 21 have legacy command-suite source rows.
- 1577 static source rows yield 3154 potential row/hash lanes, including infrastructure.
- 220 observations map to source rows; 71 skip observations remain ambiguous with candidate row IDs.
- 2952 source lanes lack admitted execution evidence. No inferred green tail.
- Fresh admitted observations: 60 exit-comparison passes, 35 exit-comparison failures, 4 normalized-output failures and 103 skips.
- 18 mapped infrastructure observations remain stale-or-unverified because their executable/provenance cannot establish Brit command behavior.
- 11/159 commands have at least one fresh passing exit comparison. This is not command completeness or a Git parity percentage; there were no fresh passing normalized-output comparisons in this run.

Effect mode proves exit-code equality only. Legacy bytes mode compares merged,
Bash-captured output, including trailing-newline normalization. Compatibility
wrappers retain deferred status. Static option associations are candidates only;
actual command binding is required for a command's passing-assertion numerator.
No option/state equivalence, daily-driver readiness or policy adoption is inferred.

## Native measures and recall

Added seven observational definitions in `.epr-meta/measures.yaml`, explicitly
consumed by the existing native `epr flow note --measures` interface. The observe
adapter re-reconciles original receipts against current source/method bytes before
recording and requires explicit actor attribution. Fifteen native observations
were recorded successfully: six separate scalar counts and nine outcome groups
partitioned by assertion strength, status and freshness. All have claimed standing,
not independently witnessed execution or a ruling. No new measurement engine,
protocol entity or acceptance watermark was introduced.

Report subject: `.eprfs/status/parity/2026-09-28/reconciliation.json`.
Original receipt: `.eprfs/status/parity/2026-09-28/receipt.json`.
Readable reconciliation: same directory, `reconciliation.md`.
These local execution records and native flow sidecar remain private. This tagged
report plus `docs/parity/compatibility-measure-lens.md` preserve recallable purpose,
method, findings and limits; native context on the report subject reads its notes.
Standalone recall algorithm packaging and adopted lens bindings remain separate.

## CI and verification

The existing required census job discovers all added regressions and checks both
census and legacy shortcomings freshness. The journey job now produces measured
receipts/reconciliation and uploads brit-parity-observations. Its required step
certifies measurement integrity; actual legacy failures remain visible outcomes
rather than being relabeled passing conformance. The local shortcomings generator
was rerun: restore links had drifted, and its canonical EOF now has one newline.
The original generator remains authoritative (gawk plus Python 3).

Gate evidence: `just gate brit` → `EXIT=0`, attesting unchanged parent committed pin
ethosengine/brit@bd915393df11 Tests pass success. This does not test the dirty tree.
Focused local verification on 2026-09-28:

- `python3 etc/parity/run.py --brit /tmp/brit-merge-target/debug/brit --ein /tmp/brit-merge-target/debug/ein --jtt /tmp/brit-merge-target/debug/jtt --git /usr/bin/git --timeout 45 --output .eprfs/status/parity/2026-09-28/receipt.json` → `EXIT=0`, 44 lanes, measurementComplete=true, complete=false.
- `python3 etc/parity/reconcile.py --receipt .eprfs/status/parity/2026-09-28/receipt.json` → `EXIT=0`; JSON and Markdown derived from the final method.
- `python3 etc/parity/observe.py --report .eprfs/status/parity/2026-09-28/reconciliation.json --receipt .eprfs/status/parity/2026-09-28/receipt.json --epr /tmp/eprfs-gate-target/debug/epr --record --actor agent:implementer@gpt-6` → `EXIT=0`; 15 native observations.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s etc/parity -p 'test_*.py'` → `EXIT=0`, 50 passed.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/ci -p 'test_qualify_readiness.py'` → `EXIT=0`, 9 passed.
- `bash etc/parity/enumerate.sh --check` → `EXIT=0`.
- `bash etc/parity/shortcomings.sh --check` with locally extracted gawk/library on PATH → `EXIT=0`.
- `git diff --check` → `EXIT=0`.

No commits, pushes or releases were performed. The next implementation work can
select failures or missing evidence by atom and required workflow. This sprint
reconciles the measuring instruments; it does not close their observed failures.
