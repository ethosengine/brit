---
id: task-git-census-3-report
gap: plans__2026-09-28-git-feature-census#3
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
commits: []
serves: brit-git-compatibility
---
# Census integration and boundary evidence

The read-only comparison joins the immutable Git census to compiled public Brit
parser names/aliases and the historical command declarations. It preserves all
159 command nodes: this fresh default/max binary exposes 22 exact command/alias
matches and 137 remain unmatched. It reports 22 native or unmapped namespaces
separately. No behavioral receipts are consumed; no compatibility percentage or
daily-driver acceptance is claimed.

Authored `.epr-meta/git-boundaries.json` is initially empty. Each future decision
requires a known command, disposition, proposed/adopted state, owner, reason,
alternative and reconsideration condition. Only adopted excluded/bridge-only
edges terminate; nodes remain in the denominator. Missing implementations default
to undecided. Invalid schemas, mutable reference names, absent parser provenance,
ambiguous mappings and incomplete boundary decisions refuse comparison. Conditional
documented option names are identified separately in each result.

Added standalone docs/parity/README.md usage, wrapper execution, generated-file
ownership guidance, habit DELTA and plan references. The compatibility habit stays
unwired because broad public behavioral conformance is still missing. CI now has
a required brit-feature-census job that reads the authored immutable reference,
checks out Git at that commit, runs focused tests and verifies census freshness.
The inherited readiness contract names that leg; public journeys include cli_surface.

Gate evidence: `just gate brit` → `EXIT=0`, attesting existing committed parent pin
ethosengine/brit@bd915393df11 Tests pass success. This does not test these uncommitted
changes. Local evidence, executed 2026-09-28:

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s etc/parity -p 'test_*.py'` → `EXIT=0`, 20 passed (including expected stale-artifact refusal fixture).
- `bash etc/parity/enumerate.sh --check` → `EXIT=0`: 159 commands, 3008 command option groups, 3919 alias occurrences, 33 interfaces, 147 diagnostics.
- `bash etc/parity/enumerate.sh --compare --brit /tmp/brit-merge-target/debug/brit` → `EXIT=0`; counts 159/22/137, 159 undecided, zero adopted terminating edges.
- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/ci -p 'test_qualify_readiness.py'` → `EXIT=0`, 9 passed.
- `/tmp/eprfs-gate-target/debug/epr check` from Brit → `EXIT=0`, governance floor coherent.
- `git diff --check` → `EXIT=0`.

The companion #2 report records the fresh binary build, six Rust checks/journeys,
strict Clippy and formatting. No commit, push, remote CI run or release occurred.
The source metadata explicitly cannot attest dirty working-tree bytes; comparison
records the executed binary SHA-256. JSON comparison output is disposable/private,
not another authoritative register.

Concerns: the census is documentation syntax, not exhaustive Git semantics. Its
147 diagnostics expose unresolved conditional branches, generated/attribute-based
includes, parameterized option names and one literal-block parsing limit. Global
Git options and nested verbs are inventoried but not matched by this top-level
overlay. Gix engine capability remains separate from CLI exposure and behavioral
proof. The next comparison layer must join actual public conformance receipts,
then author evidence-backed boundary choices rather than infer them from absence.
