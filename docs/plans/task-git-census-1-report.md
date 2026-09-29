---
id: task-git-census-1-report
gap: plans__2026-09-28-git-feature-census#1
actor: agent:implementer@gpt-6
status: DONE_WITH_CONCERNS
serves: brit-git-compatibility
commits: []
---
# Pinned Git source census

Implemented `etc/parity/census.py`, focused fixtures, the explicit reference pin,
and generated JSON/human inventory. The extractor reads only Git objects at
`94f057755b7941b321fd11fec1b2e3ca5313a4e0` (v2.54.0), disables local replacement
refs, and never reads documentation from the mutable vendor worktree. The CLI
freshness mode compares exact bytes without writing.

The reference declares 159 commands and 33 separate interfaces, including Git's
global options. Commands retain 5,867 definition groups, of which 3,008 are option
groups, with 3,919 option alias occurrences and 3,842 unique shared definition
identities. These are separate measures, not a combined parity percentage.
All command-list entries survive missing documentation. All extracted definition
terms survive, including non-option terms; nested verbs in command/action sections
are distinguished from option spellings. Shared source identities ignore line
movement; every occurrence retains exact path, line, include chain and guards.

Includes expand recursively from pinned source with cycles, escaped paths, missing
references, unresolved attributes and unsupported include selectors diagnosed.
Adjacent alias labels form one group; repeated includes add provenance without
inflating a group. Short options are not invented from hyphens inside parameter
names, underscore spellings survive, and `--[no-]` records both forms. Literal and
comment examples do not become options or active include directives. Indented
nested definitions remain included; indented protocol ellipses are not mistaken
for block delimiters.

Gate evidence: native `epr flow context` reports no declared gate project covering
`etc/parity/census.py`. The coordinated focused checks are:

- `python3 -m unittest discover -s elohim/brit/etc/parity -p 'test_*.py' -v` — `EXIT=0`, 20 tests (13 extractor tests plus seven comparison tests). Log: `/tmp/git-census-all-tests.log`.
- `bash elohim/brit/etc/parity/enumerate.sh --check` — `EXIT=0`, exact JSON and Markdown freshness.
- `git -C elohim/brit diff --check` — `EXIT=0`.

Regression evidence covers immutable worktree/later-commit/replace-ref reads,
hermetic synthetic repositories, stable identities, alias counting, recursive
includes and their failure modes, conditional branches, literal blocks, nested
options, nonwriting freshness, duplicate declarations, and real-pin inherited
`git log` plus `multi-pack-index` options.

Concerns remain explicit in 146 extraction diagnostics: 126 unresolved conditional
guards, 14 generated/attribute-based include targets and six parameterized option
forms. Guarded branches remain conditional syntax, never universal applicability.
This is a source-document inventory, not a complete parser grammar or behavioral
proof. Intentional exclusions and boundary dispositions belong to the parent's
authored overlay; the extractor invents none and removes no declared commands.
Changes remain uncommitted and unpushed, preserving the shared dirty tree.
