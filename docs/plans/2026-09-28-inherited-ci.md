---
title: Brit inherited ci stations
status: active
serves: brit-inherited-ci
---
# Inherited ci

This is the per-habit work projection of `2026-09-28-governed-readiness.md`.
The owning habit declares status; a checked station never establishes acceptance.
Dependencies below are explicit review requirements, not an implemented checkbox
scheduler. Use native context for observed environment and citation blockers.

- [ ] Inherited discipline: exact CI run/source/job/step evidence qualifies the inherited matrix contract; excluded legs remain debt and unsupported milestones refuse. Serves brit-inherited-ci.

## Capture and qualify evidence

Use the GitHub Actions run and the paginated jobs endpoint for its exact attempt.
The [Actions jobs API](https://docs.github.com/en/rest/actions/workflow-jobs)
provides job/run IDs, attempt, source SHA, step conclusions and evidence URLs.
For example, substitute an actual completed run ID and its attempt:

```sh
gh api repos/ethosengine/brit/actions/runs/RUN_ID > run.json
# pipefail prevents a failed network request from looking like a successful capture.
set -o pipefail
gh api --paginate 'repos/ethosengine/brit/actions/runs/RUN_ID/attempts/ATTEMPT/jobs?per_page=100' | jq -s . > jobs.json
python3 scripts/ci/qualify_readiness.py --milestone inherited-ci --sha FULL_HEAD_SHA --run run.json --jobs jobs.json
```

Keep captures private or in the existing evidence carrier; they are not another
status ledger. The qualifier exits 0 only for complete required legs, 1 for unmet
requirements and 2 for malformed input. It does not authenticate supplied JSON.
`.epr-meta/readiness.json` declares the required job and step selectors; its test
checks these against the workflow. Platform axes have explicit stable job names.
Nonblocking upstream legs remain listed as debt. Publication remains separate.

The foundation's new CI contract must itself run on the tested revision before
that revision qualifies. A green run from before this contract cannot prove it.
