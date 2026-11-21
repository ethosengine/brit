# Brit 0.3.1

Brit 0.3.1 lets a device gate key a build step on what its recipe reads. A step's
`buildProcess`, its pipeline file, is an input of the executor that runs it (CI for a Jenkinsfile).
It is not an input of a device gate. Until now `build affected` and `build fingerprint` folded
both classes together, so a landing that edited two Jenkinsfiles ran a 1,133 s cargo gate.

## What 0.3.1 contains

- **`brit build affected --inputs sources|all`.** With the flag omitted, the legacy planner
  runs and the JSON is unchanged. With it, rakia's classified planner runs: each reason names
  its input class, and a step lists every class that matched. `sources` subtracts a step's own
  pipeline file from its sources, because five monorepo steps cover their pipeline file with a
  source glob. A step selected only by its pipeline file then selects neither itself nor its
  dependents.
- **`brit build fingerprint --inputs sources|all`.** With the flag omitted, the CID is
  unchanged. `sources` leaves the pipeline file's bytes out. `all` keys through the same rule
  the planner matches, so a change that selects a step also moves its key.
- **brit-graph** gains `ContentFingerprint::from_repo_globs_excluding`; `from_repo_globs` is
  unchanged.
- **rakia 0.1.2.** It is required for `rakia_core::inputs` and
  `plan_from_changes_classified`. rakia's own 0.1.2 note covers the `AffectedReason` field it
  adds.

Design record: elohim `genesis/docs/superpowers/sprints/2026-11-21-wall-clock/input-classes.md`.
Shem reviewed the change independently twice; the second read found it release-ready.

## Recorded verification

Before the release, on ethosengine (toolchain 1.100.0), against the rakia branch:

| Check | Result |
|---|---|
| clippy `--all-targets` on brit-cli, brit-graph and cli-journey, `-D warnings` | green |
| brit-graph tests | 23 passed |
| brit-cli tests | 114 passed |
| cli-journey `input_classes` | 6 passed |
| cli-journey `rakia` | 8 passed |
| cli-journey `memory_build` | 4 passed |

The release head's own `Tests pass` run is the release evidence. The flag-omitted bytes are
pinned as a literal in
`pipeline_only_change_preserves_legacy_output_and_sources_fingerprint`.

## Not qualified

- The exhaustive `just clippy` feature matrix (`small`, `max-pure`, `lean-async`) was not run
  locally. CI's legs are the record.
- The monorepo has not adopted the flag. The gate oracle and the evidence key move to
  `--inputs sources` in the monorepo after this release, together with both pin moves.
- The red legs recorded for 0.3.0 (`advisories`, `size`) are not addressed here.
