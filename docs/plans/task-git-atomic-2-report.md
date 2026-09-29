---
id: task-git-atomic-2-report
gap: plans__2026-09-28-git-atomic-census#2
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
commits: []
serves: brit-git-compatibility
---
# Atomic comparison and developer entry

Extended the existing comparator with optionGroups and atomicCounts per command and
whole-reference totals. Groups retain command-context and shared definition IDs,
source occurrences, sections, conditional applicability and parameterized labels.
The overlay retains actual matching parser argument shapes, including value names
and possible values when supplied by Clap. Each group inherits its command boundary
by reference; exclusions remain in all denominators.

All/some/none means extracted option spellings present in the exact top-level
parser, including accepted aliases. Descendant flags never count on a parent.
Parameterized groups with no literal names are unresolved, not vacuously all.
Scope is explicitly unqualified and behavior unmeasured even when every name
matches. Non-option definitions and nested-verb candidates remain in the graph,
not in the option denominator. Filtered reports retain explicitly labeled whole
reference totals and selected-command counts on the selected row.

Added enumerate.sh --walk dispatch, compare --command drill-down, and README usage
with count conventions, scope/prose-extraction limits, boundary inheritance and
parser-vs-behavior distinctions. The owning parity manifest records that discipline.
Existing CI discovery includes the added regression tests without a second job or
register. No agentic projections, Rust, commits, push or installation changed.

Gate evidence: `just gate brit` → `EXIT=0`, attesting the unchanged parent pin
ethosengine/brit@bd915393df11 Tests pass success; this is not a dirty-tree test.
Local evidence executed 2026-09-28:

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s etc/parity -p 'test_compare.py'` → `EXIT=0`, 11 passed. Covers all/some/none/unresolved, conditional groups, excluded/missing commands, ambiguous mappings, invalid provenance, inherited boundary IDs, and descendant-option non-inflation.
- `bash etc/parity/enumerate.sh --compare --brit /tmp/brit-merge-target/debug/brit` → `EXIT=0`. Captured 3008 groups: 44 all, 13 some, 2945 none, 6 unresolved; 72 matched spellings out of 3919 group/name occurrences. These are syntactic observations, not parity percentages.
- Same command with `--command push --markdown` → `EXIT=0`; push detail 20/6/1/1 across 28 groups, with source lines and matched/missing names.
- `bash etc/parity/enumerate.sh --walk --command remote` and `--walk --json` → `EXIT=0`; graph entry and JSON execution smoke checks.
- `bash -n etc/parity/enumerate.sh`, `git diff --check`, and candidate `epr check` from Brit → `EXIT=0` each.

Companion #1 owns graph/extractor regressions and generated-reference freshness;
its report and independent review provide the final integrated evidence. The
compiled Brit used here was built and tested by the prior census sprint; this
change did not alter Rust. The comparison captures its SHA-256 and unverified
working-tree source provenance. Raw captures remain private under /tmp.

Concerns: inline narrative/synopsis flags, valid value combinations, option
interactions and exact nested invocation grammar are not exhaustive in this
source-definition census. No behavior receipts or engine API mappings have been
joined. The Git compatibility habit and daily-driver qualification remain unchanged.
Recall used a parent governed session because standalone Brit lacks the recall
algorithm package; focused direct source reads establish no memory-edge acceptance.
