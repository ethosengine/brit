---
gap: plans__2026-09-28-parity-evidence-reconciliation#1
actor: agent:implementer@gpt-6
status: active
serves: brit-git-compatibility
---
# Runtime receipts

Own tests/parity.sh, tests/helpers.sh, tests/utilities.sh only as needed, plus new etc/parity/run.py and event helper and test_runtime.py. Produce JSONL events from actual executions (assertion location via Bash callstack, suite/hash/title/it, helper kind, mode, argv, actual exit/status and compat reason). Preserve existing test semantics, no matching fabricated by static parse. Runner isolates cwd/environment, selects exact absolute Brit/ein/jtt/Git paths, fingerprints binaries/method and suite inputs, bounds per-suite time and captures failures/incomplete tails. Never execute fixtures in shared checkout. Run all suites later with parent; no Rust build needed binaries in /tmp/brit-merge-target/debug. Coordinate receipt schema with portable immediately.

Follow valueflow-implementer; report and exactly one terminal fulfil in Brit flow root via /tmp/eprfs-gate-target/debug/epr. Preserve all previous dirty changes. Independent reviewer /root/brit. No commits or pushes.
