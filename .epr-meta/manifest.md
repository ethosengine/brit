---
epr-meta-version: 1
id: brit-native-developer-boundary
root: true
purpose: >
  Brit is the standalone Git bridge. The local brit-cli package builds one public
  executable with Git and build commands. This anchor terminates the parent
  repository's governance cascade.
---

# Brit's native developer boundary

`brit-cli` owns the public `brit` executable. Local source and focused tests now
exercise Git and build commands through that one executable. Installation,
publication, and the accountable record/prove/release journey remain separate
unqualified stations. The root `src/plumbing/` owns parsed Git command dispatch;
`gitoxide-core`, `gix` and the other gitoxide crates own Git operations, objects,
index, refs and compatibility. Git plumbing does not decide Elohim authority.
Keep upstream Git parity work separate from first-party orchestration.

The existing `epr` flow in the Elohim workspace owns intent, commitment,
attribution, independent review, acceptance and gates. The `eprfs-core` and
`eprfs-local` crates in that workspace own canonical trees, addressing and safe
local projection; standalone brit consumes their published interfaces when
available. A planned shared tree link needs an explicit `BlobLink` migration
with byte and CID conformance tests: a blind `BlobCid` alias can change serde
bytes and therefore addresses. Content governance owns channel standing,
lineage, delegation, revocation and election. A CID returned by an HTTP `/epr-head` endpoint is a
locator, not a verdict of authority. Storage carries verified bytes and evidence;
possession does not elect a head. Rakia consumes declared source and toolchain
inputs to plan reproducible outputs. Brit and hApp are peer consumers of the
artifact-neutral core; generic Git and build APIs must not require Holochain
packaging.

The more specific manifests below carry the edit-time signals. This root is a
standalone anchor and human guide, not a copy of the parent repository's policy
registry or hook implementation.
