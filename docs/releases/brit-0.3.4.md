# Brit 0.3.4

Brit 0.3.4 updates the release's dependency lockfiles, raises the package size
limit for `gitoxide-core`, and hardens the repository's CI workflows.

## What 0.3.4 contains

- **Dependency security update.** `url` 2.5.9 fixes the stack overflow tracked
  by RUSTSEC-2026-0592. `async-executor` 1.14.1 replaces the yanked 1.14.0.
  Published crates get new patch versions where their packaged lockfiles or
  workspace dependency requirements change. [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2026-0592.html)
- **Heapless advisory exceptions.** The affected `gix-hash` dependency pulls in
  `faster-hex` 0.10.1, which requires `heapless` ^0.8. Its non-allocating encoder
  uses only `heapless::String` and `heapless::Vec<u8>`, outside the collections
  flagged by RUSTSEC-2026-0574 through RUSTSEC-2026-0582. `faster-hex` 2.0.0
  supports `heapless` ^0.9.4. Remove the exceptions when the affected `gix-hash`
  dependency paths move to that release or a later compatible release.
- **Chrono advisory exception.** `brit-build-ref` uses `chrono` for RFC 3339
  timestamps, and the published `rakia-core` and `elohim-epr` dependencies also
  use it. RUSTSEC-2026-0543 marks `chrono` unmaintained. Remove the exception
  when `brit-build-ref` moves to `jiff` and those registry dependencies drop
  `chrono`.
- **Package size check.** CI raises the `gitoxide-core` package budget from
  225 KB to 525 KB to account for Brit's replay and submodule code.
- **Workflow hardening.** Commit `567fdbcfd` adds a required `zizmor` workflow
  lint to `Tests pass`, pins OSS-Fuzz actions by commit and the Debian image by
  digest, and grants workflow permissions at the jobs that need them.

## Allocated versions

| Crate | Previous | Release | Reason |
| --- | --- | --- | --- |
| `brit-bridge` | 0.1.8 | 0.1.9 | requirement |
| `brit-cli` | 0.3.3 | 0.3.4 | lockfile |
| `brit-graph` | 0.2.5 | 0.2.6 | requirement |
| `gitoxide` | 0.61.3 | 0.61.4 | lockfile |
| `gitoxide-core` | 0.64.3 | 0.64.4 | lockfile |
| `gix` | 0.90.2 | 0.90.3 | lockfile |
| `gix-archive` | 0.39.1 | 0.39.2 | lockfile |
| `gix-blame` | 0.20.1 | 0.20.2 | lockfile |
| `gix-diff` | 0.70.1 | 0.70.2 | lockfile |
| `gix-filter` | 0.36.5 | 0.36.6 | requirement |
| `gix-fsmonitor` | 0.1.1 | 0.1.2 | requirement |
| `gix-merge` | 0.23.1 | 0.23.2 | lockfile |
| `gix-odb` | 0.87.1 | 0.87.2 | requirement |
| `gix-pack` | 0.77.1 | 0.77.2 | requirement |
| `gix-packetline` | 0.24.2 | 0.24.3 | lockfile |
| `gix-protocol` | 0.68.1 | 0.68.2 | lockfile |
| `gix-status` | 0.37.1 | 0.37.2 | lockfile |
| `gix-tix` | 0.6.2 | 0.6.3 | lockfile |
| `gix-transport` | 0.62.1 | 0.62.2 | lockfile |
| `gix-worktree-state` | 0.37.1 | 0.37.2 | lockfile |
| `gix-worktree-stream` | 0.39.1 | 0.39.2 | lockfile |

## Recorded verification

- Publish rehearsal (`scripts/ci/cargo-publish-brit.sh --check`) found 52
  packages equivalent to their published Nexus archives and 21 unused versions.
  The rehearsal used a temporary source copy to work around the missing
  `vendor/git` directory and an isolated Cargo cache for the unpublished
  dependencies.
- The CI advisories scan, the all-category scans, the locked workspace check,
  and Clippy with `-D warnings` passed locally.
- The package size check reported 430.2 KB for `gitoxide-core` against its
  525 KB budget.
