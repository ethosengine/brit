---
id: task-parity-reconcile-review
status: complete
serves: brit-git-compatibility
tags: [mishpat, middot, brit, parity, evidence, review]
---
# Independent parity evidence review

Reviewed all three reconciliation briefs, sealed implementation reports and scoped
source changes against base `1017e94646275d167b21425100c2655a18e84826`.
The reviewer authored none of this runtime, reconciler or observation adapter.
Unrelated dirty work and earlier readiness implementation were excluded.
All three commitments are approved for evidence reconciliation. No unresolved
Important or Minor findings remain. Each receives one independent technical verdict;
this does not discharge product acceptance or qualify a readiness habit.

## Findings resolved

- Reset fixtures could fail while later exit codes matched. Their observations now
  retain invalid-fixture status instead of claiming a successful comparison.
- Infrastructure smoke overrides the implementation executable, and generic helper
  commands do not necessarily execute Brit. Runtime executable identity and strict
  admission prevent those observations from becoming Brit behavioral evidence.
- Generic diagnostic reasons are separate from compatibility deferrals. A successful
  compat wrapper remains deferred, never a passing feature assertion.
- Reconciliation requires the complete current input inventory, all runtime methods,
  measured executable metadata, paired invocation identity and consistent exits.
  Missing provenance and input drift cannot qualify fresh passing assertions.
- Command attribution requires the actual command token. A suite filename alone
  cannot attribute another command's assertion to its namesake.
- Native recording re-reconciles original receipts against current source before
  recording. A previously fresh report cannot silently retain that standing after
  source drift. Scalar counts and outcome partitions are checked before notes.

The source parser remains deliberately bounded. Dynamic Bash is unresolved, literal
option spellings are candidate associations, and one guard covering several rows
remains an ambiguous observation rather than duplicated evidence. Snapshot bootstrap
and filtered-pipeline modes remain weak or unclassified. Completed helper assertions
never imply complete rows, lanes, commands or option semantics.

## Independent evidence

The reviewer repeated the integrated Python suite: 50 tests passed, `EXIT=0`.
Full-tree diff check passed. The actual 44-lane receipt was independently reconciled,
and the final observation preview re-derived the full report successfully, `EXIT=0`.
Its report SHA-256 is
`e56726088d0afe6a3baf7efdd20f951c0c8b2cbb7b64714e6e8478ce1c2ac57a`.
All implementer reports supply mandatory gate evidence and focused verification;
the existing pin attestation does not certify these uncommitted source bytes.

The campaign captured all 22 suites in two hash lanes: five lanes completed and
39 failed. Measurement completion describes valid observation production, not
passing conformance. The final 1,577 source rows yield 3,154 potential lanes;
2,952 lack fresh bound observations. There are 220 matched observations, 71 ambiguous
skip observations and 18 unverified infrastructure observations. Fresh matched
outcomes are 60 exit-comparison passes, 35 exit-comparison failures, four normalized
output failures and 103 skips. Eleven of 159 reference commands have at least one
fresh passing exit comparison; this is not a parity percentage. There are no fresh
passing normalized-output assertions in this campaign.

Installed Git 2.43.0 is the measured oracle; pinned Git 2.54.0 is the documentation
reference. Executable digests identify measured binaries without attesting dirty
working-tree source. CI now requires measurement integrity and preserves artifacts;
legacy assertion failures remain visible observations. Local raw receipts and native
flow state remain private. Fifteen claimed native observations are evidenced by the
integration report; no acceptance threshold or new authority is introduced.

No commits, pushes, releases or remote CI execution were performed by this review.
