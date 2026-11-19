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
checks these against the workflow. Nonblocking upstream legs remain listed as debt,
each with its reason. Publication remains separate.

## Planned inherited jobs

Gitoxide's QA scheduler (`tests/it/src/qa`, described in `DEVELOPMENT.md`) plans the
inherited checks from each crate's `[package.metadata.qa]` table. `ci.yml` runs that
plan in its `plan` and `run` jobs, so a check added upstream arrives with the merge
that brings its manifest entry. Brit's first-party jobs stay ordinary jobs beside
them, and `Tests pass` needs both.

The scheduler decides which jobs exist. The `inherited-ci` milestone decides what
each one means here, by the name the scheduler gives it (`<group>@<platform>`):

- `required` legs with `"job": "run"` can fail `Tests pass`.
- `deferred` legs run and report, without gating. Each carries its reason.
- `replaced` legs leave the matrix because a first-party job covers them.
- `disabled` entries never start.
- `runners` moves a platform's jobs to a runner this repository can use.

`scripts/ci/inherited_ci_policy.py` applies these in the `plan` job. It refuses a
planned job the milestone does not name, and in an exhaustive plan it refuses a
named job the scheduler no longer plans. A merge that adds or removes an upstream
leg therefore stops at `plan` until the milestone records the decision.

Preview the result for the current checkout without running any check:

```sh
planned="$(mktemp)" && governed="$(mktemp)"
just qa-plan --all-platforms --mode exhaustive --github-output "$planned" >/dev/null
python3 scripts/ci/inherited_ci_policy.py --planned "$planned" --github-output "$governed"
```

Planned jobs run on the `stable` toolchain, as upstream runs them; first-party jobs
keep the `rust-toolchain.toml` pin. Two edits keep the scheduler usable in this
fork and are marked `Brit:` in place: its workflow check accepts first-party jobs
beside the planned ones, and `ci.yml` passes the planner's output through the
milestone before the matrix starts.

The foundation's new CI contract must itself run on the tested revision before
that revision qualifies. A green run from before this contract cannot prove it.
