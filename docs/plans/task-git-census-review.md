---
id: task-git-census-review
status: complete
serves: brit-git-compatibility
---
# Independent census review

Reviewed new census scope against Brit base
1017e94646275d167b21425100c2655a18e84826 and all three implementation reports.
Reviewer authored none of this census implementation. Earlier readiness edits and
pre-existing dirty authoring work were excluded from review.

All three commitments are approved for this documented-surface foundation. No
unresolved Important or Minor findings remain. Each received one independent
technical verdict in Brit's native flow. These are not product parity verdicts.

## Findings resolved

- Immutable pin reads previously honored mutable Git replacement refs. The reviewer
  reproduced a pinned commit returning replacement documentation in an isolated
  fixture. Git reads now disable replacement refs; a regression proves original bytes.
- Option extraction previously invented short options from hyphens in placeholders
  and removed underscores from real option names. Token-boundary parsing preserves
  spellings and excludes placeholder values, with targeted regressions.
- Indented real `git multi-pack-index` options were omitted. Nested definitions now
  remain in the inventory, verified against the real pin. Literal/example exclusion
  and inherited `git log` options also have regression coverage.
- Synthetic Git fixtures now ignore global/system configuration and user hooks.
- Comparison now validates census schema, immutable revision and parser provenance;
  conditional options remain separately identified. Authored exclusions preserve every
  command node, require decision rationale and distinguish proposed from adopted.

The compiled introspection uses the actual composed Clap command tree, accepted
aliases and build metadata. Independent public capture succeeded with no operation
invocation. Its source-state declaration does not attest dirty working-tree bytes.
The comparison records the executed binary digest and keeps historical declarations,
parser exposure and unmeasured behavior separate.

## Final evidence

Reviewer independently ran the final Python suite: 20 tests passed, `EXIT=0`.
Freshness check `bash etc/parity/enumerate.sh --check`: `EXIT=0`, with 159 commands,
3,008 command option groups, 3,919 alias occurrences, 33 interfaces and 146 extraction
diagnostics. The 146 final diagnostics supersede the earlier 147-diagnostic capture
in the immutable task-3 report; that earlier fulfillment/report was not rewritten.

Live compiled comparison passed `EXIT=0`: 22 command/alias matches, 137 unmatched,
159 undecided boundaries and zero adopted terminating edges. No behavioral receipts
were consumed. Scoped diff check passed `EXIT=0`. The task-2 report supplies fresh
binary build, four unit tests, two public journeys, strict Clippy and format evidence,
all `EXIT=0`. The task-3 report records the required CI freshness job wiring and
qualifier regression checks; the existing parent gate attests its old committed pin,
not these uncommitted sources.

The remaining diagnostics are declared extraction limits: unresolved guards,
generated include targets and parameterized option forms. Top-level command matching
does not yet compare global Git options or nested verbs. Future conformance work must
join actual public behavior receipts; no denominator percentage or daily-driver
qualification follows from this review. No commits, pushes or remote CI runs occurred.
