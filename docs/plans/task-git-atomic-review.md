---
id: task-git-atomic-review
status: complete
serves: brit-git-compatibility
---
# Independent atomic census review

Reviewed both atomic-census briefs, implementation reports and new source changes
against Brit base `1017e94646275d167b21425100c2655a18e84826`. Earlier readiness
implementation and unrelated dirty work were excluded. Both commitments are
approved as a documentation and parser-comparison foundation. No unresolved
Important or Minor findings remain.

The reviewer identified real revision-walk diagrams incorrectly appearing as
sections: the extractor recognized only four-character literal delimiters and
treated indented diagram rows as headings. Variable-width literal handling and
heading discrimination now exclude those examples, with real-pin and fixture
regressions. Exact occurrence provenance connects each source to its parsed
section; legacy census bytes retain aggregate-only association rather than an
invented cross-product. Decorated provenance is deduplicated without mutating
input, and every derived graph edge resolves.

Graph context groups and shared definitions remain separate identities. Option
spellings are distinct from capabilities; repeated references do not inflate
groups. Sections and nested-verb candidates remain documentation context with
unresolved executable scope. Comparison uses only literal names on the exact
top-level compiled parser command: descendant flags cannot inflate a match,
empty-name groups remain unresolved, and boundary decisions stay attached.
Value shapes are retained without claiming semantic equivalence.

Independent final verification: 32 Python tests passed, census wrapper freshness
passed, and `git diff --check` passed, all `EXIT=0`. A separate invariant probe
confirmed no source mutation, unique context identities, distinct provenance,
correct spelling memberships and no dangling edges. The reviewer also repeated
the focused comparator suite and public push Markdown drilldown successfully.
Both reports provide their gate evidence and focused verification; the parent
pin attestation does not certify these uncommitted source bytes.

The final census has 159 commands, 33 interfaces, 5,727 command definition groups,
3,008 command option groups, 3,843 distinct command-context spelling atoms and
3,919 group-to-spelling memberships. Commands contain 3,807 shared definitions;
the combined command/interface graph has 4,190. These are different count units.
The 145 remaining diagnostics comprise 125 unresolved guards, 14 generated or
attribute includes and six parameterized option forms. This final evidence
supersedes earlier census counts without rewriting previous fulfilled reports.

The parser overlay classifies 44 option groups as all names matched, 13 some,
2,945 none and six unresolved, with 72 matching spelling memberships. These are
name intersections, not behavioral parity. No daily-driver or Git-compatibility
habit is qualified by this review. Independent technical verdicts do not discharge
product acceptance. No commits, pushes or remote CI runs were performed.
