# Backstitch × brit — the realtime/covenant composition (brit-native note)

**Status:** Capture · 2026-08-15 · first entry in `docs/research/` (module-boundary research, per the
monorepo research-index convention). Grounded at brit HEAD `40aa3ddc` (branch
`fix/nexus-first-party-registry-split`); the working tree carried an unrelated session's
rustfmt-only churn in four `brit-epr` files, untouched by this note. The external subject is
**[Backstitch](https://backstitch.dev)** (Ink & Switch): realtime version control for Godot, pure
Automerge, zero git objects. The monorepo-side survey with the full external digest is
`genesis/research/backstitch-realtime-scm-cross-pollination-2026-08-15.md`; this note carries only
what is brit's.

**Verification key:** ✅ verified in source at the cited line · ◐ doc-asserted, code unverified.

---

## 1. The composition verdict

Backstitch's own answer to "what about git?" is **parallel coexistence**: two version-control
systems over one working directory, no history round-trip — git sees snapshots, the CRDT plane sees
changes ✅ (verified by absence of any git dependency in their Rust tree). Read against brit's
thesis — *"a commit is a witnessed agreement whose terms travel with the code; a merge is a
covenantal joining of lineages"* (`docs/specs/2026-04-12-brit-design.md:18`) — the two projects are
not competitors but **complementary planes**:

- **brit is the covenant plane realtime lacks.** Backstitch attributes work to a free-text
  username, runs an unauthenticated relay, and carries no signatures at all. Brit has a working
  ed25519 signing spine (§3) and a trailer grammar designed for witnessed terms.
- **backstitch is the collaboration plane brit never claimed.** Nothing in brit's design speaks to
  live co-editing; its unit of covenant is the commit, which is exactly the *quantization* a
  realtime plane needs at its ceremony boundary.
- **The boundary object is the ceremony.** Backstitch's Merge Preview → Confirm gate (structural
  auto-merge converges; a human confirms semantics) is the same shape as brit's covenantal joining
  and the monorepo's ratification-at-dev-merge acceptance act. A future realtime plane feeding
  quantized, trailer-carrying, `AgentKey`-signed commits at its Confirm gate composes with brit
  **without either side absorbing the other**.

## 2. Trailer key registry — where `Co-Authored-By` (or an actor vocabulary) would register

**The implementation registry is two const arrays plus a trait, not the schema-driven registry the
design doc describes.**

- `brit-epr/src/elohim/schema.rs:16-17` — `SUMMARY_KEYS` + `NODE_KEYS`, six keys total (`Lamad`,
  `Shefa`, `Qahal` + `-Node` forms), consumed by `owns_key`/`required_keys`/`cid_bearing_keys`
  (`:24-33`) ✅. The manifest's §6.3 ten-row token-namespace table (`Reviewed-By`, `Built-By`,
  `Signed-Off-By`, `Brit-Schema` included) is **spec-only**: those four keys appear nowhere in code
  ✅. The JSON-Schema-driven grammar + codegen + `schema_contract.rs` harness of
  `2026-04-12-brit-design.md:51` is unimplemented (`schemas/` holds a README and one enum schema;
  no contract test exists) ✅.
- **Unknown keys pass through unchanged**, and `Co-Authored-By` is the worked example in the code's
  own comment (`elohim/parse.rs:13-14`) ✅ — brit today preserves but does not understand it.
- **Registration path**: extend the elohim `AppSchema` impl (the 6-method dispatch trait at
  `engine/app_schema.rs:16` — `id`/`owns_key`/`required_keys`/`cid_bearing_keys`/`validate_pair`/
  `validate_set`), or supply a sibling schema. An actor/steward vocabulary
  (`agent:<role>@<model>` refs, steward emails) would land as owned keys with `validate_pair`
  shape-checks — the seam is real and thin. The engine reserves no keys itself (§6.3) —
  the vocabulary is the app schema's to own, which is the right home for the monorepo's
  actor-plane grammar too.
- **Divergence risk worth naming now**: the monorepo currently has **two independent trailer
  readers** — epr-cli delegates grammar to `git log --format=%(trailers:key=Co-Authored-By,…)`
  (`elohim/eprfs/epr-cli/src/flow/mod.rs:733-745`, deliberately: *"nothing here re-implements
  trailer grammar"*) while brit parses via gitoxide `BodyRef::trailers()`
  (`brit-epr/src/engine/trailer_block.rs:17`) with a disjoint key vocabulary ✅. Two readers, no
  shared registry: the BritCid/BlobCid shape the 2026-07-12 shared-crate consolidation spec exists
  to prevent. When the commit-lift lands (§4), the epr-cli `producing_commit` family (including its
  pure `normalize_co_author`) is the code that migrates behind brit's parser.

## 3. AgentKey — implemented; the plan doc is stale

`brit-epr/src/engine/signing.rs` is real code, not the phase-2a plan's fiction: `AgentKey` (`:11`),
`load_or_generate` (`:18`) at `<repo>/.git/brit/agent-key`, 0600 perms (`:53`), `sign` (`:62`),
`verify_signature` via `verify_strict` (`:109`), a `Signed` trait (`:87`) with
`verify_signed_node` zero-the-sig-recompute-canonical-bytes verification (`:100`), unit +
round-trip/tamper integration tests, and **live wiring into all three attestation writers**
(`brit-build-ref/src/{build_cmd.rs:23,deploy_cmd.rs:25,validate_cmd.rs:23}`) ✅. Two stale-doc
corrections: every checkbox in `docs/plans/2026-04-16-phase-2a-build-attestation-primitives.md` is
still unchecked though the code shipped, and its `:639` specifies PKCS#8 PEM where the
implementation stores a raw 32-byte seed ✅.

**What this buys the actor plane:** the monorepo's `ActorClaim` (session-scoped, honor-system,
`elohim/epr-rea/src/actor.rs`) names brit `AgentKey` as its cryptographic ceiling — and the ceiling
is not aspirational: a signed claim is `Signed` trait + `verify_signed_node` over the claim's
canonical bytes, mechanism already tested here. Backstitch's plaintext-username attribution is the
control group for why this ladder (asserted → self-claimed → signed → witnessed) is worth having.

## 4. Commit-lift — not built, deliberately; the interim seam stands

Strongly-evidenced negative ✅: no git-object ContentNodes exist (zero hits for
`Commit|Tree|Blob|Repo|BranchContentNode` across brit source; the five real `ContentNode` impls are
`EprMeta`, `NodeSeed`, and the three attestations). The 2026-06-30 epr-meta foundation seals
**filesystem bytes** (`brit-build-ref meta seal --dir` → `read_dir` → `BritCid::compute_raw` →
DAG-CBOR at `.git/brit/objects/<cid>`), not git objects. The oid↔CID bridge is specified
(`docs/specs/2026-06-29-canonical-epr-meta-git-bridge-design.md:86-87`, home = a `brit-bridge`
crate) and the phase summary lists it under *"genuinely deferred — no half-built stubs"* ✅. The
`brit` binary itself is upstream `gix` renamed, with zero EPR wiring and no `brit commit` ✅.

Consequences for the realtime question: (a) any near-term realtime plane quantizes into **ordinary
git commits** at its ceremony gate — brit's trailer+signing spine applies to them unchanged; (b) a
CRDT-native history (backstitch's `ChangeHash` DAG) could only join brit's content-addressed world
through the same bridge that git objects await — one bridge design, two clients, which strengthens
the deferred `brit-bridge` rather than competing with it.

## 5. Dependency direction (settled)

Consuming `brit-epr` from outside is a documented, shipped path: Nexus `cargo-internal` sparse
registry, 15 first-party + forked crates published topologically (`scripts/ci/cargo-publish-brit.sh`),
consumer adds `[registries.elohim]` + a read token ✅. The caveat is load-bearing: `brit-epr`
transitively pins the **forked `gix-object` at a crates.io-colliding version** (SHA-256 `feat!`
divergence — the fork must be consumed coherently, never cherry-picked,
`docs/specs/2026-07-01-publish-brit-crates-to-nexus-design.md`) ✅. So an epr-cli/epr-rea
dependency on `brit-epr` imports the gix fork into trees that today use subprocess git only —
a real cost, best paid once, in the future adapter that also carries the commit-lift migration.

## 6. What brit takes / leaves from backstitch

**Take:** the ceremony placement (Preview→Confirm as the covenant re-entry point — a shipped
exhibit for the 2026-04-12 "covenantal joining" thesis); the hash-based filesystem-index direction
(their fix for the editor-closed overwrite race) as independent confirmation of the
`meta seal`/content-addressed-bytes trajectory; the parallel-planes composition itself (brit need
never absorb realtime to be complete).

**Leave:** plaintext identity (brit's `AgentKey` + trailer vocabulary is the corrective, not the
student); CRDT history as covenant substrate (unsigned, unauthenticated `ChangeHash` DAGs cannot
carry witnessed agreements; quantized signed commits remain the covenant unit); any suggestion that
realtime pressure should reopen the deferred `brit-bridge` before its own design pass — the
deferral note ("no half-built stubs") is discipline worth keeping.
