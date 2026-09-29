---
id: task-git-atomic-1-report
gap: plans__2026-09-28-git-atomic-census#1
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
serves: brit-git-compatibility
commits: []
---
# Atomic documentation graph

Implemented `etc/parity/atoms.py` and eight graph regressions. The graph derives
from the existing census without another checked-in catalog. CLI walks one command
or interface as readable text or JSON. Context groups, shared definitions, option
spellings, source occurrences and section edges have separate identities/counts.
Repeated includes and section references do not multiply definition groups.

The extractor now records each source occurrence's parsed section and heading
origin. Older census bytes retain explicitly aggregate-only associations instead
of inventing a source/section cross-product. Sections and nested-verb candidates
are documentation context; executable scope and behavioral parity remain unresolved.
The narrow prerequisite parser correction recognizes variable-width literal blocks,
ignores indented diagram text as headings, and distinguishes underline headings
from ATX headings and conditional directives. Regenerated artifacts exclude literal
example definitions previously inventoried as features.

The pin still retains 159 commands and 33 interfaces, with 3,008 command option
groups and 3,919 group-to-option-name occurrences. Those correspond to 3,843 distinct
command-context name atoms. Commands now have 5,727 total definition groups and
3,807 unique shared definitions; the combined command/interface graph has 4,190
shared groups and 1,185 shared spellings. No combined parity percentage is inferred.
The remaining 145 diagnostics consist of 125 unresolved conditional guards,
14 unresolved generated/attribute includes and six parameterized option forms.
No unclosed literal blocks remain at the pin.

Gate evidence: native `epr flow context` on the named extractor sources reports no
owning gate project; no Cargo gate was substituted. Focused verification:

- `python3 -m unittest discover -s elohim/brit/etc/parity -p 'test_*.py' -v` — 32 tests, `EXIT=0` (log `/tmp/brit-atomic-tests.log`).
- `python3 elohim/brit/etc/parity/census.py` — regeneration, `EXIT=0`.
- `bash elohim/brit/etc/parity/enumerate.sh --check` — generated freshness, `EXIT=0`.
- `bash elohim/brit/etc/parity/enumerate.sh --walk --command add --json` — public wrapper walk, `EXIT=0`.
- `git -C elohim/brit diff --check` — `EXIT=0`.

Fixtures cover shared/context identity, alias counts, repeated references, exact
source guards, legacy associations, line movement, unresolved candidate scope,
conflicting identities, deterministic output, wide literal blocks, CLI refusal,
real-pin denominator conservation and graph-edge resolution. The comparator's
focused tests ran in the same suite. Documentation extraction remains bounded:
headings are parsed documentation labels, not a semantic hierarchy; options only
in prose/synopsis are outside the definition-group census. No behavior is certified,
and no commits or pushes were made. Independent review remains required.
