# brit

**Brit** (בְּרִית, “covenant”) is a Git-compatible developer tool for the
[Elohim Protocol](https://github.com/ethosengine/elohim), built on
[gitoxide](https://github.com/GitoxideLabs/gitoxide). One `brit` executable brings
together Git operations, build planning and verified, content-addressed source
snapshots.

The ambition is bigger than a different Git client: connect what a developer
intended, the exact source they changed, who reviewed it, what was built, and what
another person can retrieve and verify. **The model composes; the record proves.**
That complete governed delivery journey is the direction, not a claim that every
commit already has it.

## Why a covenant?

Code carries knowledge, contributors create value, and maintainers exercise
governance. Conventional version control records content and authorship but does
not establish the relationships between those three concerns. Elohim names them
**lamad** (knowledge), **shefa** (value) and **qahal** (governance).

Brit aims to make attributable work and verifiable evidence part of finishing
development—not paperwork attached afterward. The distinctive goal is a chain
from source-tree CID through reproducible build and authorized release to
verified bytes on another peer. A CID proves content identity; it does **not**
by itself prove authorship, permission, elected-head authority or availability.

## What works today

| Surface | Current capability |
| --- | --- |
| Git | Inspection and operations including status, diff, log, clone, fetch and push; use each command's help for its actual contract. |
| `brit build` | Build graph discovery, affected-step analysis, plans, fingerprints and baseline refs. This namespace plans builds; it is not a complete build/release executor. |
| `brit snapshot` | Seal a selected Git tree into an explicit local content store, verify its closure, restore supported content to a fresh directory, or export its exact Git tree into another object database. |
| `brit-epr` / `brit-verify` | Covenant trailer parsing and structural pillar validation. These do not automatically authorize publication. |
| `brit-build-ref` | Separate supporting executable for build evidence and metadata operations; not a synonym for `brit verify`. |

The public executable is **`brit`**, built by package **`brit-cli`**. The root
package `gitoxide` supplies linked Git CLI functionality and the auxiliary `ein`
binary. Rakia remains a separately owned build/release concern; Brit consumes its
published libraries. The old invocation name `rakia` has a deprecated
compatibility path, but installation does not require renaming binaries or
replacing a standalone Rakia installation.

### Not yet a drop-in daily driver

Command names are not a promise of Git porcelain parity. In particular, current
`brit commit` creation supports `--allow-empty`, not the ordinary staged-change
commit journey. The complete path-scoped stage/review/commit, worktree recovery,
required-evidence enforcement and fast-forward integration story still needs
qualification. Keep stock Git available; do not alias `git` to `brit`.

Two milestones remain distinct:

- **Developer entry point:** Brit owns the accountable development journey;
  explicitly identified Git bridge operations may remain underneath.
- **Canonical authority:** native content/history and peer-held evidence become
  authoritative, with Git as an interoperability projection. This additionally
  requires history/head semantics, authorized release, peer retrieval and a
  fresh-peer restore—not merely a successful local snapshot.

See the [active through-line plan](docs/plans/2026-09-27-native-developer-through-line.md)
for acceptance checks, ownership, evidence and remaining work. Older phase
roadmaps are historical context, not a current capability inventory.

## Install from this checkout

Standalone Brit consumes published Elohim, EPRFS and Rakia crates; no Elohim
monorepo build or sibling checkout is required. You need Rust/Cargo, Git, and a
read credential for the private `elohim` Cargo registry configured in
[`.cargo/config.toml`](.cargo/config.toml). Provision credentials through secure
local storage or the process environment; never put tokens in tracked files.
Cargo also needs a credential provider such as `cargo:token`.

The frontend declares Rust 1.88, but the dedicated MSRV CI job currently checks
`gix`, not the whole frontend. The shared-tree local qualification used Rust 1.98;
use that qualified toolchain or verify your chosen toolchain independently.

From the repository root, choose an **unused** install prefix and an external
target directory. In an Elohim workspace, follow its Cargo pool policy instead
of creating another build cache.

```bash
export RUSTFLAGS=""
export CARGO_TARGET_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/brit/target"
export BRIT_INSTALL_ROOT="${XDG_DATA_HOME:-$HOME/.local/share}/brit-dev-0.1.3"
export CARGO_REGISTRY_GLOBAL_CREDENTIAL_PROVIDERS=cargo:token

cargo install --locked --path brit-cli --bin brit --root "$BRIT_INSTALL_ROOT"
export PATH="$BRIT_INSTALL_ROOT/bin:$PATH"
brit --version --verbose
brit --help
brit build --help
brit snapshot --help
```

The default frontend feature preset is `max`; other presets include `max-pure`,
`small` and `lean-async`. Source version output is diagnostic, not an attestation
of clean or authorized source. Install auxiliary tools only if you need them:

```bash
cargo install --locked --path brit-build-ref --root "$BRIT_INSTALL_ROOT"
cargo install --locked --path brit-verify --root "$BRIT_INSTALL_ROOT"
brit-verify HEAD
```

`brit-verify HEAD` checks pillar trailers and may correctly reject an ordinary
Git commit. `brit verify` instead checks repository/object integrity.

## Try a verified local snapshot

Run against a disposable repository first. Sealing reads the selected committed
tree; it does not collect unstaged or staged changes absent from that revision.
The following creates a new private store outside the source checkout:

```bash
snapshot_demo=$(mktemp -d)
brit snapshot seal --repo . --revision HEAD --store "$snapshot_demo/store"
```

The JSON result contains `root` (the tree CID), `gitTree` (the original Git tree
OID), and `published: false`. Copy the returned values into these variables:

```bash
tree_cid='<root from seal>'
git_tree='<gitTree from seal>'
brit snapshot verify --root "$tree_cid" --store "$snapshot_demo/store"

# Linux: restore into an absent directory; existing destinations are refused.
brit snapshot restore --root "$tree_cid" --store "$snapshot_demo/store" \
  --destination "$snapshot_demo/restored"

# Use the source repository's object format (sha1 or sha256).
git init --bare --object-format=sha1 "$snapshot_demo/export.git"
brit snapshot export-git --root "$tree_cid" --store "$snapshot_demo/store" \
  --repo "$snapshot_demo/export.git" --expected-tree "$git_tree"
```

This preserves tree content and Git tree identity, **not commit history**. Export
writes objects, not refs or the index. Stock Git can inspect the exported tree.
Verification reports local closure integrity separately from authority.

Important boundaries:

- Tree nodes use CIDv1 DAG-CBOR with SHA-256; file bytes use raw-codec CIDs.
  Shared EPRFS crates own encoding, byte custody and safe local restore;
  `brit-bridge` owns Git translation. There is no second Brit storage protocol.
- Git round trips cover binary content, byte-oriented names, executable modes,
  symlinks and SHA-1/SHA-256 object formats. Submodule pins are opaque external
  boundaries: sealing preserves them but does not fetch their content.
- Filesystem restore currently requires Linux, an absent destination under a
  trusted parent, and no external boundaries. Git can hold names a particular
  host cannot materialize. Restore does not promise power-loss durability or
  protection from a hostile process replacing the trusted parent.
- Verification and expansion are bounded. Current defaults include 1 GiB of
  expanded content and a 64 MiB per-object limit in the local store. The source
  Git object database is trusted for decompressor resource safety.
- These commands do not elect heads, distribute content to peers, publish
  releases or confer governance authority. They do not start an IPFS/libp2p
  service. Peer delivery belongs to the existing Elohim transport architecture.

## Covenant metadata

`brit-epr` separates a generic covenant engine (`AppSchema`, trailer parsing and
validation) from the feature-gated Elohim vocabulary (`elohim-protocol`, enabled
by default). Lamad/Shefa/Qahal trailers remain ordinary Git commit-message text;
they are not automatically added to every commit. Structural validity is not
proof that a claim is true or that its signer has authority.

Git objects and refs remain interoperable with stock Git. Additional build or
evidence refs need explicit transport; an ordinary clone does not automatically
carry every custom notes ref. Environment-aware EPR resolution, governed forks
and automatic economic recognition are aspirations, not guaranteed behavior of
the installed CLI.

## Development and delivery

With the registry credential and external native target configured:

```bash
cargo test --locked -p brit-bridge -p brit-cli
cargo build --locked -p brit-cli --bin brit
BRIT_BIN="$CARGO_TARGET_DIR/debug/brit" \
  cargo test --locked -p cli-journey --test shared_tree
```

Reuse the isolated fixtures in `tests/cli-journey` and `gix-testtools`; never run
write/recovery probes against a developer's live checkout. See [AGENTS.md](AGENTS.md)
for repository instructions and `just --list` for the broader feature matrix.

Brit's [own CI](https://github.com/ethosengine/brit/actions/workflows/ci.yml)
qualifies changes on `run-ci/**` before integration into `main`. Main publication
to Nexus, its fresh registry-consumer check, and tagged binary releases are
separate delivery steps. At source `f8a0e14cd`, the
[main CI and Nexus publication](https://github.com/ethosengine/brit/actions/runs/36458363380)
passed, including the repaired macOS fixtures and Windows ARM tests. That receipt
does not declare the full developer cutover or native authority complete.

`gix-main` is the maintained upstream mirror, not a tested Brit integration.
First-party composition lives in `brit-*` crates; upstream `gix-*` maintenance
and Git parity remain distinct from native Elohim workflow work.

## Further reading

- [Native developer through-line](docs/plans/2026-09-27-native-developer-through-line.md)
- [Canonical EPR metadata and Git bridge design](docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md)
- [Composition snapshots and canonical citations](docs/specs/2026-06-29-epr-meta-composition-snapshot-canonical-cites-design.md)
- [Elohim schema and trailer vocabulary](docs/schemas/elohim-protocol-manifest.md)
- [Historical roadmap](docs/plans/README.md)

## License

MIT OR Apache-2.0, following gitoxide's dual license. Brit builds on the work of
Sebastian Thiel and the gitoxide contributors.
