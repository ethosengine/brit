# Git feature census

The [Mishpat/Middot framing](compatibility-measure-lens.md) explains how these
observations inform policy and how to recall that concern. Brit-local measures are explicitly consumed through native EPR observations;
lens ratification and standalone recall packaging remain separate work.

Git is the reference high-water mark, not a promise that Brit or gix must implement
all of it. Every command remains a node even when a governed decision terminates
Brit's implementation edge there.

From the Brit repository root:

```sh
# Requires the pinned Git object in vendor/git; reads committed objects, not its worktree.
bash etc/parity/enumerate.sh
bash etc/parity/enumerate.sh --check

# Use the binary you actually intend to assess.
bash etc/parity/enumerate.sh --compare --brit /path/to/brit --markdown
bash etc/parity/enumerate.sh --compare --brit /path/to/brit > /tmp/brit-comparison.json

# Focused extractor, comparison and governance regressions.
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s etc/parity -p 'test_*.py'
```

[`git-reference.json`](git-reference.json) selects the immutable Git commit.
[`GIT-CENSUS.md`](GIT-CENSUS.md) and [`git-census.json`](git-census.json) are generated
from its command catalog and documentation, including shared option definitions.
The `brit-feature-census` CI job checks out that exact commit and verifies freshness;
it is a required leg of `Tests pass`. Changing the reference requires regeneration.
The installed host Git version is not the feature reference.

## What the numbers mean

Commands, option groups, aliases, nested verbs and other documentation definitions
are different units. Shared definitions have stable source anchors; their command
occurrences are counted separately. Source paths and lines permit inspection.
Conditional branches remain marked `conditional-unresolved`; diagnostics disclose
extraction limits. This is a reproducible documentation census, not exhaustive
behavioral or configuration-semantic coverage.

The comparison calls the built `brit --cli-surface-json` and records its binary
SHA-256, build metadata and actual public parser names and accepted aliases.
Parser presence and option-name intersection do not prove working behavior. The
old [`commands.md`](commands.md) table remains a historical declaration overlay.
Native-only or unmatched Brit namespaces are listed separately. This first overlay
matches top-level commands; nested verbs and global Git interface options remain
in the source inventory without being promoted to matched coverage.

The parser comparison does not consume behavioral receipts. The separate legacy
assertion reconciliation below does. `unmeasured` means the parser report has no
behavioral verdict, not that existing tests failed. Daily-driver acceptance stays
with the separately declared habits and their evidence.

## Where an implementation edge ends

Author decisions in [`git-boundaries.json`](../../.epr-meta/git-boundaries.json),
which is governed source, not generated output. Each entry names a census command,
`disposition`, `decision`, `owner`, `reason`, `alternative`, and `reconsider_when`.

Dispositions are `undecided`, `planned`, `deferred`, `excluded`, `bridge-only`, or
`intentional-difference`. The decision must say `proposed` or `adopted`. Only an
adopted `excluded` or `bridge-only` decision terminates an implementation edge.
A difference remains an open comparison of intentionally different behavior.

For example, a proposed bridge-only command would explain which boundary owns the
choice, why native implementation stops, which explicit external workflow serves
the need, and what evidence would reopen the decision. It must not be marked
adopted just because no implementation was found. The initial decision list is
empty: missing commands are undecided until a reasoned decision is authored.

Terminated nodes remain in the Git denominator. This makes the boundary visible
without making Brit look more capable by shrinking its reference surface.

## Walk below command level

```sh
# Documentation graph, with no installed Brit required.
bash etc/parity/enumerate.sh --walk --command add
bash etc/parity/enumerate.sh --walk --command remote --json

# Full graph or focused parser comparison.
bash etc/parity/enumerate.sh --walk --json > /tmp/git-command-graph.json
bash etc/parity/enumerate.sh --compare --brit /path/to/brit --command add --markdown
```

The graph is derived on demand from the checked census. It distinguishes shared
source groups from their occurrences in each command, document sections from
executable scopes, and individual option names from the groups that declare them.
A group may have several section edges without becoming several capabilities.
Nested-verb candidates and non-option definitions remain visible rather than being
forced into the option denominator. Each source occurrence retains its section,
include chain and conditional guards. A documentation hierarchy is not proof of a
valid command invocation.

Comparison JSON contains `optionGroups` and `atomicCounts` per command and aggregate
`atomicCounts` for the whole reference. Each group has `matchedNames`, `missingNames`,
`parserArguments` (including actual value names/choices when supplied by Clap), a
`parserNameMatch` of `all`, `some`, `none` or `unresolved`, plus explicit unresolved
scope and unmeasured behavior. `all` means all extracted spellings match names; it
is not full implementation. Zero extractable names means unresolved, never all.
Conditional groups are counted and labeled separately, without pretending their
conditions have been evaluated. Parameter syntax remains in the original labels;
accepted values and interactions still need behavioral cases. Options mentioned only
in narrative prose or synopsis text are not automatically minted as definition
groups; this remains a documentation-definition census, not a complete grammar.

The same short flag can mean different things in different commands. Matches use
only the exact top-level command's parser options, including aliases/inherited
options actually present there. Descendant flags do not inflate its matches.
The option-name occurrence denominator counts each spelling in each contextual
group; the graph also reports deduplicated name atoms so shared reuse is visible.

`--command` selects the command detail while explicitly retaining whole-reference
totals. Its row provides selected-command totals. Each group references its owning
command boundary; excluding that command neither deletes the groups nor promotes
them to matches. Boundary decisions currently apply to commands, not individual
options. The existing required CI test discovery exercises the atomic graph and
comparison; no extra generated register or ranking authority is introduced.

## Reconcile the April parity journeys

The existing 21 command suites plus the infrastructure smoke suite are the test
source. The following run captures actual helper observations in disposable copies;
it does not evaluate shell source in the shared checkout:

```sh
python3 etc/parity/run.py --brit /path/to/bin/brit --ein /path/to/bin/ein \
  --jtt /path/to/bin/jtt --git "$(command -v git)" --timeout 45 \
  --output .eprfs/status/parity/local/receipt.json
python3 etc/parity/reconcile.py --receipt .eprfs/status/parity/local/receipt.json \
  > .eprfs/status/parity/local/reconciliation.json
python3 etc/parity/reconcile.py --receipt .eprfs/status/parity/local/receipt.json --markdown
```

All suite/hash lanes are attempted even if an earlier suite fails. Each individual
suite preserves its original fail-fast behavior; unreached rows stay unexecuted.
Timeouts and incomplete assertions are explicit. Runner exit 0 means the measurement
completed with stable inputs and valid receipts, **not** that the suites passed;
exit 2 means measurement integrity/preflight failed. Read each run's status and
assertion outcomes. Every receipt binds suite/fixture/helper bytes, actual binaries,
Git executable version, platform and hash lane. The installed oracle can differ
from the pinned census version; the receipt records that distinction.

`effect` establishes exit-code equality only. Legacy `bytes` compares merged
stdout/stderr captured by Bash, which normalizes trailing newlines; it is not an
exact binary-stream comparison. `compat_effect` remains deferred even when the
underlying comparison passes. `shortcoming`, hash skips, missing or ambiguous row
associations, infrastructure overrides and stale evidence cannot become passing
feature claims. Identical argv spellings identify candidate census groups, not a
proof that the group's full semantics were exercised. Command ratios require an
actual command-bound assertion, not merely a suite named after that command.

The reconciler reads literal source declarations without executing them. Direct
shortcoming blocks without `it` are included. One skip guarding several possible
rows remains a single ambiguous observation with candidate row IDs. All 159 census
commands remain in the denominator, including the 138 with no legacy command suite.

`brit-feature-census` runs regression tests and checks census/shortcomings freshness.
The journey job also produces and uploads `brit-parity-observations`: red legacy
assertions remain visible measurements. The required step certifies production of
valid observations; it does not impose or imply full parity acceptance. No remote
CI result is implied by local verification.

### Record and recall Middot

The Brit-local measure definitions live in `.epr-meta/measures.yaml`. EPR consumes
that file via its existing explicit `--measures` interface; this is not another
registry implementation. With the original receipts available, preview first or
record native observations with explicit actor attribution:

```sh
python3 etc/parity/observe.py --report .eprfs/status/parity/local/reconciliation.json \
  --receipt .eprfs/status/parity/local/receipt.json
# EPR_ACTOR names the real developer/agent recording this observation.
python3 etc/parity/observe.py --report .eprfs/status/parity/local/reconciliation.json \
  --receipt .eprfs/status/parity/local/receipt.json --record --actor "$EPR_ACTOR"
epr flow context .eprfs/status/parity/local/reconciliation.json
```

`--epr /path/to/epr` selects an installed interface explicitly. Before recording,
the adapter re-reconciles original receipts and refuses report/source drift. Notes
bind report bytes, method and registry hashes and receipt provenance, with claimed
standing. Counts of rows, row/hash lanes and assertions have distinct units;
assertion outcomes retain strength, status and freshness dimensions. No policy
threshold is adopted. Local raw receipts and native flow state stay private under
`.eprfs/status/`; the tagged implementation report provides durable recall context.
The original shortcomings generator remains authoritative for SHORTCOMINGS.md and
requires gawk plus Python 3. The wrapper's old GIX environment variable still accepts
a Brit path; the measured runner supplies absolute paths explicitly.
