# brit

[![Contribute](https://www.eclipse.org/che/contribute.svg)](https://code.ethosengine.com/#https://github.com/ethosengine/brit)

**Brit** (בְּרִית, "covenant") is an expansion of [gitoxide](https://github.com/GitoxideLabs/gitoxide) — a pure-Rust implementation of git — that integrates protocol-level primitives for tracking who built code, what value it creates, and who governs it. Every commit in a brit repo is a covenant: a witnessed agreement whose terms travel with the code, no matter where it goes.

The name rhymes with *git* on purpose. Git is the substrate. Brit is the covenant laid on top.

A brit repo is a valid git repo. You can `git clone` it from GitHub. You can push it to GitLab, Codeberg, sourcehut. Everything works. But inside the [Elohim Protocol](https://github.com/ethosengine/elohim) network, the same repo resolves to a richer view: provenance, economic events, governance context, and content-addressed links that know where your code is running.

## Why this exists

### The problem: power is siloed

The world has three forms of power, and today they're separated:

- **Economic power** — money, wealth, capital. Concentrated in institutions that extract value from the systems they control.
- **Informational power** — knowledge, data, distribution. Concentrated in platforms that control what you see and who sees you.
- **Social and network power** — trust, governance, collective decision-making. Concentrated in corporations and governments that make rules for everyone while being accountable to almost no one.

These silos aren't accidental. They're profitable. When economic power is decoupled from the knowledge it was built on, you get proprietary lock-in. When informational power is decoupled from governance, you get surveillance capitalism. When social power is decoupled from economic accountability, you get institutions that privatize gains and socialize costs.

Every open-source project lives at the intersection of all three — code is knowledge (informational), contributors create value (economic), and maintainers make decisions for everyone who depends on them (governance) — but git, the tool that tracks it all, knows about exactly *none* of it. Git tracks content. It doesn't track value. It doesn't track governance. It doesn't even reliably track who contributed what, beyond a name and email in a commit header.

### The solution: couple them at the protocol level

The [Elohim Protocol](https://github.com/ethosengine/elohim) introduces three coupled primitives — **lamad** (knowledge), **shefa** (value), and **qahal** (governance) — and requires that every notarized artifact in the network carries all three. You cannot create a content-addressed artifact that declares what it is without also declaring who stewards it and what governance applies. The architecture makes it structurally difficult to circulate knowledge without recognizing its stewards, and structurally easy to honor their care.

Brit brings this coupling to version control.

## What this means for code

### 1. A way to pay the open source contributor, built in

Today, open source runs on unpaid labor. Contributions are tracked by git (author, committer), but the economic relationship between contribution and value is invisible to the tooling. Payment is an afterthought — a GitHub Sponsors button, a Patreon link, a corporate donation. None of it is wired into the act of building.

In a brit repo, every commit carries a **shefa** trailer that declares the economic event: who contributed, what kind of work it was, what stewardship changed. When someone builds your package, the protocol's economic layer records a recognition event — not a financial transaction, but a protocol-level acknowledgment that serving knowledge generates value for those who care for it. Recognition flows proportionally to stewards based on their allocation.

This isn't "add a token to npm." This is the substrate knowing, at the commit level, that contribution has value and tracking it the same way git tracks authorship: as a first-class primitive that travels with the code.

### 2. Provenance-aware code — choose who you trust, not just what you run

Here's a thought experiment. Imagine there's a critical piece of infrastructure — call it a cloud platform — built by a large corporation. The code is open source. You can read every line. But the corporation starts doing things you disagree with: surveillance, labor violations, environmental harm. You want to keep using the code, but you don't want your usage to legitimize their stewardship.

Today, you fork the repo on GitHub and hope people notice. The fork has no formal relationship to the original. No one can tell, from the code alone, whether your fork is a legitimate community effort or a fly-by-night copy.

With brit, a fork is a **first-class covenant** — a new `ForkContentNode` with its own stewardship, its own attestations, its own peers. The code is the same; the stewardship graph is different. When you choose to depend on Coop AWS instead of Amazon AWS, that choice is visible on the protocol's content graph. Your dependency isn't just a semver string in a lockfile — it's an EPR reference that points at specific stewards, specific attestations, specific governance. Everyone on the graph can see which collective you're trusting, and every steward can independently attest that the tags and branches they serve have the integrity needed for deployment.

Provenance isn't metadata bolted on after the fact. It's the address.

### 3. Deployment-aware code — links that know where they're running

Have you ever thought it would be nice if a config reference could resolve differently depending on which environment you're in? Or if a link in your documentation could point at staging when you're on the `dev` branch and production when you're on `main`?

With an Elohim Protocol Reference (EPR) link, now it can. An EPR is a content address that carries context: `epr:my-service[@v2.1.0][/head][?via=doorway.example.org]`. The same link, in a brit repo, resolves differently based on:

- **Which branch you're on** — each branch has a reach level (`private`, `self`, `trusted`, `familiar`, `community`, `public`, `commons`) that determines who sees it and what it resolves to.
- **Which doorway you're connected to** — a doorway is a gateway node that bridges web2 (GitHub, GitLab) and the protocol network. Your doorway knows your environment.
- **Who's asking** — the protocol's context-aware resolution adapts to the requester's position in the knowledge graph.

Code is no longer limited to a SHA graph address. It's a living artifact in a network that knows what it is, who built it, and where it's running.

### 4. A fully distributed landing — not just another crypto project

Under the hood, brit uses [IPFS/IPLD](https://ipld.io/) primitives through [rust-ipfs](https://github.com/ethosengine/rust-ipfs) to take the actual blobs of a codebase and place them on a distributed content-addressed graph. Every tree, every blob, every commit object gets a CID (content identifier) that any peer can resolve. The codebase isn't hosted on a server you hope stays up — it's distributed across a network of peers who can independently verify every byte.

Other P2P and crypto projects do this too. IPFS, Radicle, and various blockchain-based package registries all make code content-addressed and peer-distributed.

What makes brit different is *where the code lands*.

Most distributed code projects land in a network optimized for financial incentives — mine tokens, stake coins, speculate on protocol value. The network exists to create economic returns for participants. Code is the payload; speculation is the purpose.

Brit lands in the Elohim Protocol network — a network designed to scale **wisdom and care**: the human capacity to steward shared resources responsibly. The three pillars (knowledge, value, governance) are coupled at the substrate level specifically so that code can't circulate without acknowledging who cares for it, and stewardship can't accumulate without the community's consent. The network exists to serve the humans who depend on the code, not to create returns for token holders.

This is not a philosophical distinction. It's an architectural one. The same content-addressing that makes code distributed also makes stewardship trackable, governance enforceable, and value flows transparent — but only if the network those primitives land in is *designed for care rather than extraction*. A content-addressed blob on a speculation-optimized network is still a blob someone will try to rent-seek from. A content-addressed blob on a care-optimized network is a shared resource the community can actually govern.

## How it works

### Commit trailers — the protocol surface

Every brit commit carries three trailer lines in its message, using the same RFC-822 format as `Signed-off-by:`:

```
feat: add two-factor auth to login flow

Implements TOTP-based 2FA with QR code provisioning and backup codes.

Signed-off-by: Dan <dan@example.org>
Lamad: teaches two-factor-auth pattern; advances auth learning path
Shefa: human contributor | effort=medium | stewards=dan,sofia
Qahal: steward | mechanism=self-review | ref=refs/heads/dev
```

Stock git reads this commit just fine. GitHub renders it. `git log` prints it. Nothing breaks. But a brit-aware tool (or an LLM agent with a brit skill) knows that this commit teaches something (`Lamad`), that Dan and Sofia steward the value it creates (`Shefa`), and that it was self-reviewed for merge to `dev` (`Qahal`).

### Backward-compatible with every git host

A brit repo is a git repo. `git clone https://github.com/your-org/your-brit-repo` works from any machine with stock git. Outside the Elohim Protocol network, you get the full commit history with the trailer lines — readable, diffable, `git log --format=fuller` compatible. You lose the EPR resolution (linked ContentNodes, rich provenance graph, deployment-aware links) because those live on the protocol network, but nothing is broken. The code works. The trailers are there. The provenance is readable.

Inside the network, a file called `.brit/doorway.toml` in the repo points at the primary steward's doorway node. That doorway resolves the full EPR view — linked ContentNodes for each commit, per-branch README ContentNodes, attestation graphs, economic event streams, and context-aware link resolution.

### Engine and app schema — pluggable by design

The `brit-epr` crate has two layers:

- **Engine** (unconditional) — a generic covenant engine that parses trailer blocks, validates them against an `AppSchema` trait, and manages `TrailerSet` types. Knows nothing about Lamad, Shefa, or Qahal specifically.
- **Elohim Protocol schema** (feature-gated, default on) — the first-party implementation of `AppSchema` for the Elohim Protocol's three pillars.

A downstream project could disable the `elohim-protocol` feature and plug in a different schema — a carbon-accounting protocol, a biological-sequence protocol, a music-composition protocol — without forking brit. The engine is the covenant substrate; the schema is the vocabulary.

## Current status

**Phase 1 complete** (trailer foundation):

- `brit-epr` crate with engine/elohim feature split
- `AppSchema` trait — the dispatch contract for app schemas
- `TrailerSet` type and `parse_trailer_block` via gitoxide's `gix-object`
- `ElohimProtocolSchema` implementing `AppSchema` with closed Lamad/Shefa/Qahal vocabulary
- `parse_pillar_trailers` and `validate_pillar_trailers` convenience functions
- `brit-verify` binary — verifies pillar trailers on a commit, exits 0/1
- 9 tests passing; engine compiles cleanly with `--no-default-features`

**Phases 2-6** (planned, not yet implemented): ContentNode adapter, libp2p transport, per-branch READMEs, DHT peer discovery, merge-as-reach-elevation with async consent, fork-as-governance. See [docs/plans/README.md](docs/plans/README.md) for the roadmap.

## Quick start

```bash
# Build
cargo build -p brit-verify

# Verify a commit's pillar trailers
cargo run -p brit-verify -- HEAD

# Expected (on a brit-aware commit):
# ✓ pillar trailers valid for abc1234
#   Lamad: teaches two-factor-auth pattern
#   Shefa: human contributor | effort=medium | stewards=dan,sofia
#   Qahal: steward | mechanism=self-review | ref=refs/heads/dev

# Expected (on a stock gitoxide commit):
# ✗ pillar validation failed for abc1234: required pillar trailer missing: Lamad
```

## Relationship to gitoxide

Brit is a fork of [gitoxide](https://github.com/GitoxideLabs/gitoxide) by Sebastian Thiel and contributors. Gitoxide is an excellent pure-Rust git implementation with a clean modular design — each concern lives in its own `gix-*` crate and swaps independently. Brit builds on that modularity.

**What brit adds:** new crates (`brit-epr`, `brit-verify`, and future `brit-cli`, `brit-transport`, `brit-store`) that layer protocol semantics onto gitoxide's object model. Zero modifications to existing `gix-*` crates. The goal is to remain upstream-rebaseable: bug fixes and additive extension points are proposed upstream where possible; protocol-specific divergence earns its own crate.

**What brit does not change:** gitoxide's core — object storage, pack format, protocol negotiation, ref management, diff, blame, worktree. Brit consumes these; it doesn't rewrite them.

## Further reading

- **[EPR-git roadmap](docs/plans/README.md)** — seven-phase plan from trailer foundation through fork-as-governance
- **[App-level schema design](docs/schemas/elohim-protocol-manifest.md)** — the normative reference for ContentNode types, trailer grammar, signal catalog, and the engine/app-schema boundary
- **[Merge consent critique](docs/schemas/reviews/2026-04-11-merge-consent-critique.md)** — pressure test of async-default merge design against distributed stewardship scenarios
- **[Elohim Protocol](https://github.com/ethosengine/elohim)** — the parent protocol repository
- **[gitoxide](https://github.com/GitoxideLabs/gitoxide)** — the upstream Rust git implementation brit is built on

## License

MIT OR Apache-2.0, following gitoxide's dual license.
