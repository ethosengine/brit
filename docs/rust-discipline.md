# Brit's shared Rust discipline trial

Brit applies the shared Rust source-suppression discipline to `brit-build-ref`
only. The local selection in `.epr-meta/rust-style-policy.json` identifies the
complete policy, guide and checker method by content address. The objects held
beside it are immutable copies of shared bytes, rather than a separately edited
style guide. New artifacts can be proposed without changing this adoption.
This initial checker refuses replacement of an already enrolled closure and changes
to its existing scope: a later guide/checker upgrade or scope expansion needs an
explicit migration path that preserves historical baseline verification. That
migration is not implemented by this trial.

The user authorized this reversible trial. Two repository-grounded agents reached
the trial recommendation under that one steward's delegation. Two agent seats
are not two independent human constituencies or demographic representation.
Imagodei linkage remains unresolved by explicit exception. No signed network
adoption, DHT witness or remote reconciliation is claimed.

## Install and check

The checker comes from the pinned source bundle in
`.epr-meta/rust-style/source.json`. It is built outside the source checkout from
the addressed original sources, using locked third-party dependencies. No Elohim
sibling checkout or registry publication is required. Initial installation needs
Rust 1.96.1, Bash, jq, coreutils, xxd, tar, and the dependency cache or access to
crates.io; checking is offline.
Use an absolute Cargo target directory appropriate to your machine:

```sh
export CARGO_TARGET_DIR=/absolute/build-cache/brit-rust-style
export RUSTFLAGS= RUSTC_WRAPPER= CARGO_BUILD_JOBS=1
bash scripts/ci/install-rust-style.sh "$PWD/.eprfs/tools/rust-style"
bash scripts/ci/rust-style.sh
```

The adapter verifies the installation before invoking its exact executable.
`BRIT_RUST_STYLE_PREFIX` can select another absolute private prefix installed by
the same script. There is no PATH lookup or sibling fallback. The native checker
verifies the full addressed closure and its checker method; unavailable or
corrupt inputs return UNMEASURED, never success.

Local checking defaults to `origin/main`. To inspect a committed candidate:

```sh
bash scripts/ci/rust-style.sh --base <full-ancestor-commit> --head <full-source-commit>
```

The native checker reads source and governance from that committed tree, so a
working-tree repair cannot hide a committed violation. The adapter also requires
the held checker source declaration and archive to match the committed bytes;
a differently declared local installation cannot supply committed reach.
Pre-push obtains both IDs
from the actual push and requires a clean, stable checked-out source. A first
push uses the hook's explicit reviewed `branch.<name>.britReachBase`. CI builds
the same addressed checker and runs the trial as a required `Tests pass` leg.
CI uses the pull-request base or push's prior commit. Without one, it compares
against `origin/main`; if main is the checked source, it uses that source's first
parent. Missing ancestors and source-as-baseline comparisons refuse measurement.

## Enrollment and limits

The first commit containing the local policy is the explicit enrollment point;
these uncommitted changes do not invent its hash. No-new enforcement applies
immediately. Per-touch debt paydown begins when that enrollment is present in
the comparison baseline. Strict zero-debt scope is a separate enrollment after
measurement; it is not implied by possession of a strict-scope policy object.

Only `allow`/`expect` source attributes are checked. Existing Rust compiler,
feature, MSRV, formatting and upstream compatibility checks continue to apply.
The checker establishes that a rationale is a nonempty literal, while reviewers
assess whether its explanation is sound. Command-line/Cargo suppression allowances,
other first-party crates and mixed root source ownership remain outside this trial.

`bash scripts/ci/test-rust-style.sh` tests adapter mechanics with isolated doubles.
Pass `--native /absolute/path/to/epr-rust-style` for the actual installed-engine
proof: enrollment, unexplained suppression refusal, explained exception, and a
dirty repair that cannot conceal the committed failure. CI requires both.

## Integration candidate correction — 2026-11-03

The initial candidate used an `eprfs-meta` 0.3.3 manifest, but that version was
already published with different bytes by the native-source sprint. The integration
candidate holds the corrected 0.3.4 checker source and regenerated method/policy
objects. The guide, scope, rules and licensing restrictions are unchanged. Both
sets of immutable objects are retained. This candidate is evaluated against the
current main baseline, which has not enrolled the trial; it does not implement or
authorize migration of an already adopted policy. The earlier published candidate
remains available for comparison.

The isolated source installation and native adapter fixtures passed: enrollment,
paydown, committed-input isolation, corrupt-guide refusal and unexplained-attribute
refusal. The first candidate's CI run 41671223402 reported a 32-bit fixture race:
restore published before the test harness could suspend the process (5,657 tests
passed, one failed). That run is not green integration evidence. The corrected
candidate must obtain its own complete CI verdict before the parent pin moves.
