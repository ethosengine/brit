---
epr-meta-version: 1
id: brit-git-tree-translation
purpose: >
  Translate committed Git trees to and from the published artifact-neutral
  EPRFS tree without moving refs, electing heads or inventing a second codec.
rules:
  - id: reuse-shared-canonical-tree
    class: inject
    when:
      write: "*.rs"
      contains-any: ["TreeNode", "TreeEntry", "BlobCid", "canonical_bytes"]
    dedupe-of: "eprfs_core::tree and eprfs_core::{BlobCid, BlobLink} (published elohim registry)"
    why: >
      Reuse the shared tree and CID/link contracts. This crate owns Git modes,
      object IDs, Git tree ordering and opaque gitlink interpretation, not an
      alternative canonical node format. Preserve existing EprMeta v1 bytes.
---

# Git is the bridge here

Seal explicit committed source; do not sweep dirty files or mutate an index.
Verify source Git OIDs and shared closure CIDs, then prove the expected Git tree
OID before export writes. Gitlinks preserve a named hash algorithm and commit
bytes but do not prove that submodule content is retrievable. Reject unsupported
semantics instead of normalizing them silently. Resource limits cover both
stored and expanded work.
Git object headers are checked before body reads. This is not a hard allocation
cap if the source object database lies about size or changes between header and
body reads; source ODB custody must be trusted for decompressor safety.

Filesystem staging and no-replace promotion belong to `eprfs-local`; immutable
byte custody belongs to `eprfs-storage`. Publication standing, attribution and
head election remain with their existing owners. A successful local round-trip
is neither peer delivery nor authority. Public composition lives in
`brit snapshot` in brit-cli; the existing `brit tree` remains Git-object plumbing.
