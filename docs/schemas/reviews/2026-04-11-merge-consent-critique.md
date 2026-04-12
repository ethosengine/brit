# Critique: async-default `brit merge` consent design

**Reviewer:** critic subagent (Claude Opus 4.6)
**Date:** 2026-04-11
**Subject:** §14.1 #4 of `docs/schemas/elohim-protocol-manifest.md`
**Status:** **Modify** (not affirm, not replace)

---

## 1. TL;DR recommendation

The async-default lean is **directionally right but underspecified**. Async-as-default is the correct choice for an LLM-first CLI, and `--wait` is a sensible escape hatch — but the current §3.9/§9 description treats `brit merge` as a single state transition (proposed → consented → completed) when it is in fact a **long-running, multi-actor settlement process** that has to survive offline stewards, collective tally latency, build-attestation latency, expiry, retraction, and elohim-as-delegate fast paths.

**Recommendation:** keep async-default, but reframe `brit merge` as **opening a Merge Proposal record** (a first-class ContentNode with a stable id, lifecycle, and TTL) rather than emitting a transient signal. Add three signals to §9 (`brit.merge.expired`, `brit.merge.withdrawn`, `brit.merge.tally.progress`), require `--wait` to internally implement polling-with-cap rather than blocking on a single signal, and let `brit status` surface pending proposals so an LLM driver can re-enter the flow asynchronously without holding session state.

The fundamental shift: **the merge proposal is a persistent governance artifact that brit happens to emit, not a CLI command's return value.** The LLM doesn't "wait for a merge"; it opens a proposal, gets a proposal id back, and the proposal lives in the protocol until it terminates. This makes every scenario below tractable.

---

## 2. What was under review

### 2.1 The decision

> §14.1 #4 — Does `brit merge` to a protected branch *block* synchronously waiting for qahal consent, or *gossip* the proposal and return immediately, with the developer (or LLM) checking back later? Lean: configurable per-repo, default async, with `--wait` flag for sync. Async is more LLM-friendly.

### 2.2 Framing constraints honored

1. **LLM-first CLI** — the happy path must serve an LLM agent that drives `brit merge` and then needs to know what to do next without indefinite blocking.
2. **Distributed stewardship** — multi-steward repos where some stewards may be offline for hours or days.
3. **Collectives** — qahal endpoints that internally aggregate many voices through any of six tally mechanisms; the tally itself is asynchronous.
4. **Protocol-nervous-system** — elohim agents may carry human interest models and consent-by-delegation, collapsing consent latency for offline humans.
5. **Consent is load-bearing.** No design that bypasses consent is acceptable, no matter how LLM-friendly.

---

## 3. Required reading — receipts

| Source | Takeaway |
|---|---|
| `docs/schemas/elohim-protocol-manifest.md` §3.1, §3.9 | `brit merge` "verifies the proposed merge satisfies the target branch's qahal protection rules" and "blocks (configurable) until consent arrives via the doorway." This is the only normative description of the flow. |
| §5.5 BranchContentNode + §5.7 RefUpdateContentNode | Protected refs carry a `protectionRules` CID; ref updates without satisfying authorization are **protocol violations**, not anomalies. The merge gate is hard. |
| §5.12 reserved type `MergeConsentContentNode` | The schema reserves this type but doesn't specify it. The current §13.1 scenario A treats it as a single ContentNode published when consent lands — no notion of partial / accumulating consent. |
| §6.5 qahal microgrammar | `auth-kind := "self" | "steward" | "consent" | "vote" | "attestation" | "council" | "retroactive"`. The vocabulary already distinguishes single-steward from collective from delegated authorization, but the merge command surface doesn't yet route through these distinctions. |
| §9 signal catalog | Four merge signals: `proposed`, `consented`, `rejected`, `completed`. No `expired`, no `withdrawn`, no `tally.progress`, no `partial`. QoS is "acknowledged" for the first three. |
| §13.1 Scenario A | Single-steward, optimistic happy path. Dan is the steward and clicks "consent" interactively. The async/sync question is invisible because there's nothing to wait for. The scenario does not test the design under stress. |
| §13.2 Scenario B | Cold-start onboarding from GitHub; merge consent happens inside the steward's elohim-app via the doorway, not via `brit merge`. Reinforces that the doorway is the consent surface — but `brit merge` is the proposal surface. |
| §14.1 #4 | The decision under review. |
| §14.1 #12 | Force-push / protection-rules DSL is Phase 2. The shape of `protectionRules` is undecided. **This is entangled with the merge consent decision.** You cannot specify a merge gate's wait semantics without specifying what the gate is. |
| `docs/plans/README.md` | Phases 4 (per-branch READMEs) and 6 (forking-as-governance) put the most pressure on the merge flow — Phase 6 needs to negotiate cross-fork merges, where neither side controls the consent gate alone. |
| `genesis/docs/content/elohim-protocol/governance-layers-architecture.md` | Governance is **layered**: individual → family → community → … → global, with sortition-selected councils and graduated consensus. Merges to a "community-stewarded" repo's `main` may need community-layer consent, not just steward consent. brit merge has to address layered consent endpoints. |
| `memory/project-elohim-as-governance-nervous-system.md` | "Quorum is irrelevant" because elohim represent humans by default; humans opt-in to override. Merge consent doesn't have to wait for *humans*; it can wait for the elohim collective to deliberate and produce a settlement, with humans free to override post-hoc. |
| `memory/project-pillar-topology-power-responsibility.md` | Attestation gates between pillars replace credentialing. Consent on a brit merge is one such attestation gate; the design must keep it transparent and community-governed. |
| `genesis/plans/2026-03-15-governance-gateway-sprint3-plan.md` | Six tally strategies live in elohim-storage: ranked-choice, approval, score, dot, consent, conviction. Each has its own latency profile. Each is an asynchronous tally engine. **brit cannot assume consent is fast.** |
| `elohim/sdk/domains/lamad/manifest.json` | Existing `governanceModel` vocabulary: `"steward-consent"`, `"community-vote"`. brit's qahal microgrammar shares this vocabulary; the merge gate has to route to the right backend per repo. |

---

## 4. Scenario walkthroughs

For each scenario I describe how the **current §3.9 description + async-default lean** handles it, then where it cracks, then what the LLM sees.

### 4.1 Scenario summary table

| # | Scenario | Best-case latency | Worst-case latency | Async-default verdict |
|---|---|---|---|---|
| 1 | Solo steward, LLM driving, steward asleep | 6h (when Dan wakes) | 1d+ | **Crack** — no expiry, signal may be lost, LLM has no resumption surface |
| 2 | Two co-stewards, 2-of-2, one on vacation | 5d | indefinite | **Crack** — no override, no partial-consent visibility, hostile to LLM driver |
| 3 | Collective 7-of-12 via tally engine | minutes | hours | **Crack** — no progress signal, no tally completion contract |
| 4 | Cascading feature → dev → main | seconds | days | **Crack** — no pipelined proposal |
| 5 | Hostile or stuck proposal | n/a | infinite | **Crack** — no expiry, no withdrawal |
| 6 | Merge with BuildAttestation dependency | 50min | hours | **Handled with caveats** — but only if proposal precedes build |
| 7 | Elohim agent acting as delegate | 60s | 60s | **Handled cleanly** — but design is silent on whether brit can distinguish delegate-consent |

### 4.2 Scenario 1 — Solo steward, LLM driving, steward asleep

**Setup.** Repo has one human steward Dan. `refs/heads/main` protectionRules: `{kind: steward-accept, count: 1}`. LLM agent runs `brit merge dev → main`. Dan is asleep (will wake in ~6h).

**Async-default behavior as currently described.** §3.9 says brit "emits `brit.merge.proposed` and blocks (configurable)". With async default the command returns immediately. The signal flies off into the doorway. The LLM gets… what? §3.9 doesn't say. Probably exit code 0, stdout containing the proposed commit CID. No proposal id. No "what to poll for" hint. The signal lives in the doorway's notification queue.

**Crack 1 — no proposal id.** The LLM has the merge commit CID, but the merge **commit hasn't been written yet** (the merge isn't authorized). What the LLM has is a *proposed* commit CID — and there's no schema for that. §13.1 step 8 calls the consent-time artifact `MergeConsentContentNode`, but the *proposal* itself has no named type. The LLM has nothing stable to remember.

**Crack 2 — Dan must come to brit, not the other way.** Dan wakes up, opens his elohim-app (per scenario A) and clicks consent. Fine. But what if Dan's elohim-app has been closed and the doorway dropped the notification? "Acknowledged" QoS in §9.2 says the producer expects an ack within "a configurable window before retrying" — but the producer here is brit-the-CLI, which has long since exited. There is **no party still alive that retries**. The signal must instead be reified as a persistent artifact in the doorway.

**Crack 3 — LLM-side resumption.** When the LLM next checks in (next prompt, hours later, possibly in a different session with no memory), how does it know the merge is still pending? `brit status` does not currently surface pending merge proposals — §3.1 says it shows "unresolved pillar drift in the staged commit." Pending proposals are invisible to the LLM until something pushes them.

**LLM experience.** The driving LLM exits with success but with no idea what to do next. If the user asks it again 2 hours later, it has to re-derive from git state that the dev branch hasn't actually merged into main. It cannot tell "consent pending" from "consent never asked for" from "consent denied silently."

**Failure mode.** Signal loss + steward goes back to sleep + LLM session ends = the proposal is in nobody's mind. **The merge is forgotten.**

**Verdict.** The current async-default *as described* leaks. The fix is to give merge proposals a persistent, addressable identity.

---

### 4.3 Scenario 2 — Two co-stewards, 2-of-2, one on vacation

**Setup.** Dan and Sofia co-steward. `protectionRules: {requires: [{kind: steward-accept, count: 2, of: [agent:dan, agent:sofia]}]}`. Sofia is on vacation 5 days. LLM drives merge.

**Async-default behavior.** Proposal goes out. Dan acks within minutes. Sofia is unreachable. brit waits. For 5 days. **Forever**, actually, since there is no expiry in §9.

**Crack 1 — partial consent is invisible.** §9 has `brit.merge.consented` (singular), not `brit.merge.consent.partial`. There's no signal for "1 of 2 in." The `MergeConsentContentNode` is a single artifact published at completion; the schema doesn't model the in-flight ledger. The doorway's UI may show progress, but the protocol surface doesn't.

**Crack 2 — no urgent override path.** Dan needs to push a security fix. He has Sofia's pre-authorized blanket consent for emergencies (delegated to her elohim agent). The schema has nowhere for "Sofia's elohim consents on her behalf for security-fix-class commits." This is the *exact* case the elohim-as-nervous-system memory is designed to solve, but the schema doesn't expose it.

**Crack 3 — what happens to the proposal when Sofia returns?** No expiry, no notification escalation. Sofia comes back, sees a 5-day-old proposal in her elohim-app, doesn't know whether the change is still relevant. The merge commit (not yet written) is computed against `dev` as it was 5 days ago — likely conflicts with current `dev`. The proposal becomes stale in two ways: politically (the change is no longer urgent) and technically (the merge no longer applies cleanly).

**LLM experience.** LLM gets exit 0, no insight into who's pending. If asked to re-check, must walk the doorway's proposal queue (no API for it specified). If asked to re-merge, will produce a *second* proposal with a different commit id, and now there are two stale proposals.

**Failure mode.** Stale proposals accumulate. No GC. No de-duplication. Sofia returns to a graveyard.

**Verdict.** The current design assumes "2-of-2" can be modeled as a serialized chain of single consents, which is true mechanically but false operationally. **Multi-steward consent needs first-class proposal lifecycle** — TTL, partial-consent ledger, withdrawable-by-proposer, notify-on-return.

---

### 4.4 Scenario 3 — Collective 7-of-12 via tally engine

**Setup.** Repo `qahal.protectionRules` points at a governance node owned by a 12-member collective. The node specifies `mechanism: ranked-choice, threshold: 7-of-12, deadline: 24h`. LLM drives merge.

**Async-default behavior.** Proposal is gossiped. The collective's governance gateway (sprint 3 substrate) opens a vote. Members vote at their leisure. After 24h or first-7, the `TallyStrategy` produces a result and the collective's elohim signs `brit.merge.consented` (or `rejected`).

This is the scenario the async-default was *designed* for. It mostly works.

**Crack 1 — tally completion contract.** §9 has `brit.merge.consented` but the collective's tally is itself a multi-step process. brit has no notion of "tally has begun, expect a result by deadline." The LLM (and the human in the elohim-app) can't tell whether the collective has even started deliberating, or whether the proposal is sitting in a queue waiting for a quorum to convene. **Need a `brit.merge.tally.progress` signal**: `{proposal_id, votes_so_far, threshold, deadline}`.

**Crack 2 — what is brit's identity in the collective's vote?** The collective's voting mechanism (sprint 3) has its own data model — `proposal_options`, `ranked_votes`, `governance_signals` tables. When brit emits `brit.merge.proposed`, who creates the corresponding `proposals` row in the collective's elohim-storage? The doorway? An adapter? §3.9 is silent. There's a missing translation layer between the brit signal and the governance gateway's intake.

**Crack 3 — tally timeout.** If the deadline passes and the threshold isn't met, what happens? §9 has `brit.merge.rejected` (which assumes a positive failure decision) but no `brit.merge.expired` (no decision at all). The LLM can't distinguish "rejected" from "ignored." This matters: ignored proposals can be retried; rejected proposals shouldn't be without addressing the rejection reason.

**LLM experience.** LLM exits, comes back 30 minutes later, runs `brit status` — sees nothing relevant. Has no way to know the vote is at 4 of 7. The only legible state is git's view, which still says dev hasn't merged.

**Failure mode.** Long-running tallies are operationally invisible. Humans in the elohim-app can see the vote progress (because the gateway has its own UI), but the LLM driver has no equivalent.

**Verdict.** Async-default is the right shape; the missing pieces are the **proposal id**, **progress signal**, **expired signal**, and an **adapter contract** between brit and the governance gateway.

---

### 4.5 Scenario 4 — Cascading feature → dev → main

**Setup.** LLM drives `brit merge feature/x → dev` (1-of-1 steward) then `brit merge dev → main` (2-of-2). LLM wants both done in one logical operation.

**Async-default behavior.** First merge proposal goes out. LLM exits. Hours later steward consents. Merge to dev completes. **Now the LLM has to re-engage** to propose the dev → main merge — but the LLM session is long gone, and nothing in the protocol auto-triggers the next step.

**Crack 1 — no continuation primitive.** §3.9 doesn't model "merge sequence." The LLM has no way to express "when this merge consents, propose the next." The doorway *could* hold a continuation, but the schema doesn't define one.

**Crack 2 — pipelining is impossible.** Nothing prevents the LLM from proposing both merges immediately, but proposal #2 references a target commit that doesn't exist yet (the merge from #1 hasn't been written). The schema doesn't define proposal-on-proposal.

**Crack 3 — what if proposal #1 is rejected?** No automatic invalidation of any downstream queued proposals.

**LLM experience.** The LLM has to choose between two bad options: (a) `--wait` on each merge, blocking sessions for hours; or (b) make one proposal, exit, and hope a future LLM session picks up the thread.

**Verdict.** The schema needs either (a) a `MergeProposalChain` primitive (queue of dependent proposals, each one auto-fires when its predecessor settles) or (b) explicit acceptance that cascades are out of scope and humans drive them stepwise. I think (a) is cleaner; option (b) deserves explicit text in §3.9 if chosen.

---

### 4.6 Scenario 5 — Hostile or stuck proposal

**Setup.** LLM proposes a merge. 24h pass. No consent, no rejection, no signal at all.

**Async-default behavior.** Nothing happens. The proposal exists in the doorway's notification queue (or doesn't, if QoS retries gave up). No expiry. No GC.

**Crack 1 — proposal expiry.** §9 has no `brit.merge.expired`. The LLM has to invent an expiry policy locally, and every LLM will invent it differently.

**Crack 2 — proposal withdrawal.** What if the LLM (or its driving human) wants to *cancel* a proposal — say, because they realized it shouldn't have been made? No `brit merge --cancel <proposal>`. No `brit.merge.withdrawn` signal. The proposal sits there forever, possibly gathering consent the proposer no longer wants.

**Crack 3 — proposal ambiguity.** Without a proposal id, even running `brit merge` again to "retry" creates a *different* proposal, because the merge commit base may have moved. Two stale proposals, no de-duplication, both consumable by stewards.

**LLM experience.** The LLM has no way to tell stuck from in-progress. It will either retry (creating duplicates) or give up.

**Verdict.** Critical gap. **Add `brit.merge.expired` and `brit.merge.withdrawn` signals; require every proposal to carry an expiry; allow proposers to withdraw; implement de-duplication keyed on `(repo_cid, source_branch_id, target_ref, proposer_agent)`.**

---

### 4.7 Scenario 6 — Merge with BuildAttestation dependency

**Setup.** `protectionRules: {requires: [{kind: steward-accept, count: 1}, {kind: build-attestation, count: 1, builder_capability: bafkreireproducible}]}`. Build takes 20 minutes. Consent takes another 30 minutes.

**Async-default behavior.** This is actually the strongest case for async-default. The LLM proposes the merge, both steward review and build attestation run in parallel. When both complete, the merge is consented and the doorway writes the merge commit.

**Crack 1 — sequencing.** Does the proposal trigger the build, or must the build be already complete before the proposal is meaningful? §3.9 doesn't say. **Lean: the proposal should trigger the build**, making `brit merge` the entry point to the full CI gate. This is how Phase 6 (Stage 2 of the build-system roadmap, §12) gets natural integration.

**Crack 2 — partial readiness.** What if the build attests but the steward hasn't acted yet? Or vice versa? §9 has no `brit.merge.requirement.satisfied` partial signal. We're back to scenario 3's missing-progress-signal.

**Crack 3 — `brit attest` interaction.** §3.11 lists `brit attest` as creating `Reviewed-By:` trailers or `ReviewAttestationContentNode`s. **Attestations are consent primitives**, not just review primitives. A `brit attest --consent <proposal>` should be the LLM-friendly way to record consent, equivalent to clicking the elohim-app button. The schema currently treats attestation and consent as separate things; they should be the same thing at different consent-mechanism endpoints.

**LLM experience.** Acceptable. The LLM can poll proposal status. Best case 50 minutes. The slowest leg dominates, which is acceptable in CI.

**Verdict.** Mostly handled, but the proposal-triggers-build behavior needs to be explicit in §3.9 and the relationship between `brit attest` and proposal consent needs naming.

---

### 4.8 Scenario 7 — Elohim agent acting as delegate

**Setup.** Dan has delegated consent to his elohim agent for a class of changes (e.g., dependency bumps under N lines). LLM proposes a merge. Dan's elohim agent reviews within 60 seconds, votes consent on Dan's behalf, and the doorway emits `brit.merge.consented` immediately.

**Async-default behavior.** Best case in the entire design. Async returns instantly, agent consents in 60s, completion signal arrives, LLM can re-poll one minute later and find merge complete.

**Crack 1 — distinguishability.** The `brit.merge.consented` payload is `{commit_cid, branch_id, decision_cid, dissent}`. There's no field for `consenting_agent` or `consent_kind`. brit cannot tell "Dan consented" from "Dan's delegate consented" from "the collective tallied to consent." This matters for audit and for the human override flow (the nervous-system memory: humans must always be able to look at what their elohim did and overrule it).

**Crack 2 — overridability.** What's the half-life on a delegated consent? Dan's elohim consents at T+60s, the merge completes at T+90s. At T+5h Dan looks at it and disapproves. There's no `brit merge --revoke-consent` or `brit revert --as-override`. The override has to happen via `brit revert`, which is a *new* commit, not a revocation of the consent. Acceptable for now, but worth naming.

**Crack 3 — delegate authority discovery.** How does brit know whether Dan's elohim has authority to consent on his behalf? This lives in `protectionRules` (Phase 2, §14.1 #12) — and the rules DSL must encode "Dan's authority is delegable to agents matching capability X." This is where the merge consent design and the protection-rules DSL design **must be co-designed**.

**LLM experience.** Excellent. Sub-minute. This is the scenario the LLM-first reframing was aiming at.

**Verdict.** Cleanly handled by async-default, but to *make use* of it, brit needs (a) a `consenting_agent` and `consent_kind` field on `brit.merge.consented`, and (b) a `protectionRules` DSL that can express delegation.

---

### 4.9 Scenario 8 (added) — Layered consent (community-stewarded repo)

**Setup.** A repo whose `qahal.protectionRules` requires consent at the community-governance layer (per `governance-layers-architecture.md`). The community has 30 members, uses sortition-selected councils, and deliberates at human-week timescales. LLM proposes a merge of a license change.

**Async-default behavior.** The proposal opens. The community's elohim collective begins deliberating. Days pass. Council members are eventually selected by sortition. They review. Eventually a settlement emerges.

**Crack.** The async-default lean assumed "minutes to hours" latency. Layered consent is **days to weeks**. The proposal has to survive at this scale: it's a long-running governance artifact, not a CLI side effect. This emphatically requires the proposal-as-ContentNode reframing.

**Verdict.** Forces the proposal to be a first-class persistent artifact. Cannot be a transient signal.

---

### 4.10 Scenario 9 (added) — Cross-fork merge negotiation (Phase 6)

**Setup.** A fork wants to merge upstream into the parent. Per §5.8 ForkContentNode, this is a *negotiated* event — neither side controls the consent gate alone. Two consent processes have to converge: the fork's steward agrees to surrender the changes, the parent's steward (or collective) agrees to accept them.

**Async-default behavior.** Two parallel proposals. They have to be linked. Currently no schema for that.

**Crack.** The schema needs `MergeProposalContentNode` to support **paired proposals** with mutual consent. The settlement is "both sides accept." This is essentially a two-phase commit at the governance layer.

**Verdict.** Out of Phase 0–1 scope, but the proposal-as-ContentNode design must not foreclose this. Listing the proposal type's required fields with a `counterpartProposal: optional CID` would do it.

---

## 5. Findings — answers to the 10 specific questions

### 5.1 Is async-default correct?

**Modify, don't replace.** Async is correct because:

1. The LLM-first constraint demands it — blocking on a 5-day vacation is hostile to LLM session lifecycles.
2. The collective scenarios (3, 8) cannot be supported with sync without making the LLM driver into a process supervisor.
3. The elohim-as-delegate scenario (7) is the natural fast path, and it composes cleanly with async.
4. Scenario 6 (build attestation) wants async because the build takes 20 minutes.

But the **lean as written is too thin** — it answers the blocking question without specifying the proposal lifecycle that async requires. The modification: reify the proposal as a `MergeProposalContentNode` with a stable id, lifecycle states, expiry, and resumption surface.

### 5.2 If modified — exact modification

1. **Promote `MergeProposalContentNode` from "reserved" (§5.12) to fully specified (new §5.13 or amendment to §5.12).** Required fields: `id`, `repo`, `sourceBranch`, `targetRef`, `proposedCommit` (the would-be merge commit's metadata), `proposer`, `requirements` (resolved from protection rules at proposal time, frozen), `expiryAt`, `state` (`open | partially-satisfied | consenting | consented | rejected | expired | withdrawn`), `progress` (per-requirement satisfaction map), three pillars.
2. **Default async with explicit return contract.** `brit merge` returns `{proposal_id, expiry_at, requirements: [...]}` on stdout as JSON; exit code 0 means "proposal opened cleanly," non-zero means "proposal could not be opened." It does NOT mean "merge completed."
3. **`brit merge --wait[=Nm]`** is not a single blocking call; it's polling-with-cap. Default cap 5 minutes. Can be run from a separate session against a known proposal id (`brit merge --wait --proposal <id>`). After cap, prints current state and exits non-zero, leaving the proposal alive.
4. **`brit status` extension** — when run in a brit repo, lists open merge proposals targeting any local ref, with state and expiry.
5. **`brit merge --withdraw <proposal>`** — proposer can cancel an open proposal. Emits `brit.merge.withdrawn`.

### 5.3 If replaced — n/a (not replaced).

### 5.4 New signals needed in §9

| Signal | Trigger | Payload | Pillar | QoS |
|---|---|---|---|---|
| `brit.merge.tally.progress` | A proposal's requirement set advances toward satisfaction (vote count, attestation arrival). | `{proposal_id, requirement_kind, satisfied, total, deadline}` | qahal | best-effort |
| `brit.merge.expired` | A proposal reached its expiry without satisfying its requirements. | `{proposal_id, last_state, partial_progress}` | qahal | best-effort |
| `brit.merge.withdrawn` | A proposer withdrew a proposal. | `{proposal_id, reason}` | qahal | best-effort |
| `brit.merge.requirement.satisfied` | One requirement in the proposal's set has been met (e.g., the build attestation arrived; one steward of N consented). | `{proposal_id, requirement_kind, satisfier_agent}` | qahal | best-effort |

Existing `brit.merge.consented` payload should add `consenting_kind: "steward" | "delegate" | "collective" | "council"` and `consenting_agents: [...]`.

### 5.5 What does `brit merge` output

JSON to stdout (LLMs parse JSON better than freeform text):

```json
{
  "result": "proposal_opened",
  "proposal_id": "bafkreimergeproposal...",
  "repo": "bafkreirepo...",
  "source_branch": "feature/x",
  "target_ref": "refs/heads/main",
  "requirements": [
    {"kind": "steward-accept", "needed": 2, "satisfied": 0, "remaining": ["agent:dan", "agent:sofia"]},
    {"kind": "build-attestation", "needed": 1, "satisfied": 0, "expected_capability": "bafkrei..."}
  ],
  "expiry_at": "2026-04-13T20:00:00Z",
  "wait_url": "https://doorway.elohim.host/proposals/bafkreimergeproposal.../events"
}
```

Fast paths (already-satisfied: solo-steward auto-consent via delegation; merge to a `self-governance` ref; nothing-to-do because target already contains source) should return `{"result": "merge_completed", "merge_commit": "..."}` instead. The LLM dispatches on `result`.

### 5.6 How `brit status` surfaces pending proposals

```text
$ brit status
On branch feature/per-hunk-witness-cards
Working tree clean.

Open merge proposals targeting refs in this clone:
  bafkreimerge1234... → refs/heads/main
    state: partially-satisfied (1/2 stewards consented; build pending)
    proposed: 2026-04-11 18:30 UTC by agent:claude-opus-4-6
    expires: 2026-04-13 20:00 UTC (in 1d 7h)
```

Same JSON shape on `brit status --json`.

### 5.7 What `protectionRules` DSL must express

This is §14.1 #12 territory but the entanglement matters:

1. **Requirement composition** — AND/OR over requirement kinds.
2. **Requirement kinds** — `steward-accept`, `agent-accept` (specific agents), `attestation` (capability-typed, e.g., reproducible builder, security audit), `vote` (with `mechanism`, `threshold`, `deadline`), `council-review` (sortition-selected, layer-bound).
3. **Delegation rules** — for each requirement, can the consent be satisfied by a delegate? With what authority class?
4. **Expiry default** — proposal TTL when proposer doesn't specify.
5. **Override classes** — emergency-fix override, with what counter-authorization?
6. **Layer routing** — does this rule resolve at the local/community/regional layer? For repos owned by collectives, this names which collective.

Without this DSL the merge consent design is half-built. **The two open questions (§14.1 #4 and #12) should be co-resolved in a single Phase 2 design pass.**

### 5.8 Interaction with `brit attest`

`brit attest` should be reframed as the **agent-side write surface for consent and review, not just review.**

```text
brit attest --proposal <id> --consent
brit attest --proposal <id> --reject --reason "..."
brit attest --proposal <id> --review --capability <cid> --decision approve
brit attest <commit> --reviewed-by --capability <cid>
```

A delegate elohim agent runs `brit attest --proposal <id> --consent --as-delegate-of agent:dan` to record delegated consent. A reproducible builder runs `brit attest --proposal <id> --build-attestation --output <cid>` to satisfy a build requirement.

This unifies the consent and attestation surfaces — both are "agent signs an artifact about another artifact." It also gives the LLM a single entry point for *responding to* a proposal it didn't create (the steward's-LLM scenario, where one LLM proposes and another consents).

### 5.9 Interaction with elohim-agent-as-delegate

Not orthogonal — **enabling**. The async-default design only feels acceptable because in practice, most consent will be delegated to fast-acting elohim agents. Without delegation, async-default produces the worst-case latency on every merge. With delegation, async-default produces sub-minute latency on the common case and gracefully degrades to days-or-weeks on the high-stakes case.

Schema-wise: the protection rules DSL must express delegation, the `brit.merge.consented` payload must distinguish delegate from human, and `brit attest` must support `--as-delegate-of`. None of those things is in the schema today.

### 5.10 The one thing to talk to Matthew about

**Should `MergeProposalContentNode` be hosted by brit-the-schema, or by the elohim-protocol governance gateway, with brit only emitting the open-proposal signal?**

In other words: is the proposal a brit ContentNode (defined in §5, validated by brit-epr) that the governance gateway *consumes*, or is it a governance gateway artifact (a row in the gateway's `proposals` table) that brit *triggers*? This is the brit-engine-vs-app-schema boundary applied to a specific case. There's an argument either way:

- **Brit owns it** — keeps the merge flow legible from inside a brit repo without depending on a doorway. Required for offline / cold-start. Matches §2's "engine has no opinions, schema does."
- **Gateway owns it** — avoids duplicating the governance gateway's `TallyStrategy` machinery, gives brit a clean dependency on the existing tally backend, lets the same proposal type cover non-brit governance.

I lean **brit owns the type, gateway owns the tally engine** — brit emits the proposal as a ContentNode, an adapter projects it into the gateway's `proposals` table for tally execution, and the tally result is signed back into a `MergeConsentContentNode` that brit can verify. But this is the load-bearing question for the entire feature and Matthew should call it.

---

## 6. Proposed schema changes (concrete edits, do NOT apply yet)

### 6.1 §3.9 — `brit merge` description

**Current:** "Verifies that the proposed merge satisfies the target branch's qahal protection rules… If consent is required, emits a `brit.merge.proposed` signal and blocks (configurable) until consent arrives via the doorway."

**Proposed replacement:**

> Opens a `MergeProposalContentNode` against the target ref. The proposal freezes the requirement set resolved from the target's protection rules at the moment of proposal. Default behavior is **async**: the command publishes the proposal, emits `brit.merge.proposed`, prints the proposal manifest as JSON to stdout, and exits 0. The proposal lives in the protocol with a configurable TTL (default 48 h). The LLM (or human) re-engages via `brit status`, `brit merge --wait --proposal <id>`, or by subscribing to the proposal's event stream at the doorway.
>
> **Fast paths (proposal not opened, merge happens immediately):**
> - Target ref has `self-governance: true` qahal — proposer is the steward, no consent needed.
> - All requirements are pre-satisfied (e.g., proposer is the sole required consenter and is consenting).
> - Source already contained in target (no-op merge).
>
> **`--wait[=duration]`** flag: after opening the proposal, poll its status with the given cap (default 5 minutes). On cap, exit non-zero with the current state; the proposal remains alive.
>
> **`--withdraw <proposal_id>`** flag: cancel an open proposal (proposer-only), emitting `brit.merge.withdrawn`.

### 6.2 §5.12 — promote `MergeProposalContentNode` from reserved to specified

Add a new §5.13 (renumber subsequent) with:

- Purpose, content-address strategy (CID over canonical bytes).
- Required fields including `id`, `repo`, `proposer`, `sourceBranch`, `targetRef`, `proposedMergeBase`, `proposedMergeMetadata`, `requirementsFrozen`, `expiryAt`, `state`, `progress`, three pillars.
- Optional fields including `counterpartProposal` (for cross-fork two-phase merges), `parentProposal` (for cascades), `withdrawnReason`.
- State machine: `open → partially-satisfied → consented | rejected | expired | withdrawn`. Terminal states are immutable.
- Pillar coupling (qahal load-bearing).

### 6.3 §6.5 — qahal microgrammar additions

No new auth-kinds (the resolved-closed §14.1 #6 prohibits that). But add a recognized `mechanism` short tag: `proposal-pending` for commits that *would* land if the proposal succeeds. This is for the not-yet-merged commit object that the proposal points at — its qahal trailer can read `Qahal: consent | mechanism=proposal-pending | ref=refs/heads/main`.

### 6.4 §9 — signal catalog additions

Add the four signals from §5.4 above. Update `brit.merge.consented` payload to include `consenting_kind` and `consenting_agents`.

### 6.5 §14.1 #4 — replace the open-question text with a resolution pointing at the new §5.13 + §3.9

> **Resolved 2026-04-12.** Async-by-default with a first-class `MergeProposalContentNode` lifecycle. See §3.9 for the command surface, §5.13 for the proposal type, §9 for the signal catalog including `expired`, `withdrawn`, `tally.progress`. The `--wait` flag becomes a polling cap, not a blocking call. The `protectionRules` DSL (§14.1 #12) co-resolves with this and must express delegation, requirement composition, and TTL defaults.

### 6.6 §14.1 #12 — note the entanglement

Add a sentence: "This must co-resolve with §14.1 #4 — the merge consent design depends on the protection rules DSL being expressive enough to encode delegation, requirement composition, layer routing, and TTL defaults."

### 6.7 §3.11 — `brit attest` reframing

Add a paragraph: "`brit attest` is also the consent surface for open merge proposals: `brit attest --proposal <id> --consent` records consent from the invoking agent (or `--as-delegate-of <agent>` for delegated consent). This unifies attestation and consent under one verb."

### 6.8 §13 — add Scenario C

Add a third target-persona scenario covering a 5-of-8 collective merge consent flow, walking through proposal open → `brit.merge.tally.progress` events → eventual settlement → completion. Without this, §13 doesn't actually exercise the collective path.

---

## 7. Open questions for Matthew

1. **Proposal ownership** — does `MergeProposalContentNode` live in brit's schema (§5) or in the governance gateway's data model? (See §5.10 above. This is the #1 question.)
2. **Default TTL** — 48 h is a guess. Layered/community proposals need days-to-weeks. Should the TTL come from the protection rule, not a brit default?
3. **Cascading proposals** — first-class type (`MergeProposalChain`) or LLM-side responsibility? Phase 6 forking-as-governance forces this.
4. **Two-phase cross-fork merges** — paired-proposal mechanism or out-of-scope until Phase 6?
5. **Override classes** — should the protection rules DSL express "emergency override allowed by single steward + post-hoc collective ratification within N days"? This is real (security fixes) but governance-hostile if abused.
6. **Doorway as proposal store** — if the proposal is in the doorway, does the doorway become a single point of failure for merge governance? Should proposals be DHT-gossiped instead, with the doorway as a cache?
7. **Should `brit verify` walk pending proposals** — i.e., should an offline clone with a stale proposal cache be able to validate proposal state, or only the doorway?
8. **Can a proposal change targets mid-life?** (E.g., target ref moved while the proposal was open — does the proposal apply against new HEAD or original?) Lean: **no**, proposal freezes its base; rebases require a new proposal. But this needs explicit text.

---

## 8. Cross-references

### 8.1 Governance docs

- `genesis/docs/content/elohim-protocol/governance-layers-architecture.md` — layered consensus, sortition-selected councils, graduated consent.
- `genesis/docs/content/elohim-protocol/constitution.md` — the protocol's constitutional framing (not deeply read; flagged for follow-up if proposal type approaches constitutional concerns).
- `genesis/docs/content/elohim-protocol/governance/epic.md` — governance epic; long-form rationale for the multi-mechanism backend.

### 8.2 Sprint plans (governance gateway)

- `genesis/plans/2026-03-15-governance-gateway-sprint3-plan.md` — `TallyStrategy` trait, six tally implementations, `proposals` table, `governance_signals`. **This is the substrate `brit merge` consent must route to.**
- Sprint 4–9 plans — Angular UI, constitutional immune system, signal accumulation, polis sensemaking, governance disposition, elohim deliberation. The merge consent flow eventually lives inside this stack.

### 8.3 Memory files

- `project-elohim-as-governance-nervous-system.md` — quorum is irrelevant; elohim represent humans by default; humans opt-in to override. **The async-default design is only viable because of this.**
- `project-pillar-topology-power-responsibility.md` — attestation gates between pillars replace credentialing. Merge consent is one such gate.
- `project-governance-medium-is-message.md` — the consent UX is itself the governance lesson.
- `project-elohim-token-theory-of-value.md` — proof of witnessed contribution; merge consent is one of the witnessing events.

### 8.4 Brit schema sections referenced

- §3.1 (command catalog), §3.9 (`brit merge`), §3.11 (`brit attest`)
- §5.5 (BranchContentNode protectionRules), §5.7 (RefUpdateContentNode), §5.8 (ForkContentNode), §5.12 (reserved types)
- §6.5 (qahal microgrammar)
- §9 (signal catalog)
- §13.1 / §13.2 (target-persona scenarios)
- §14.1 #4 (the question), §14.1 #12 (protection rules DSL)

---

## 9. Closing observation

The async-default lean is a small decision pretending to be a large one. The large decision underneath is **what kind of object a merge proposal is**. If it's a transient signal, async-default is fragile and the schema leaks. If it's a persistent ContentNode with a lifecycle, async-default becomes trivially correct and most of the open questions fall out as small decisions about that type's fields.

The §14.1 #4 lean hints at the right answer (async, configurable, LLM-friendly) without having committed to the underlying primitive. This critique's recommendation is to make that commitment explicit in §5.13, §3.9, and §9 — not to relitigate the async/sync axis itself.
