//! Lossless bridge between committed Git trees and the shared EPRFS canonical tree.
//!
//! Git remains an explicit source format. This crate never moves a ref, writes an
//! index, resolves standing, or treats an opaque gitlink as available content.
//! Git object headers are checked before body reads, but header preflight is not
//! a hard allocation cap against a malicious, lying, or concurrently changed
//! object database. The source ODB must be trusted for decompressor safety.

#![deny(missing_docs, rust_2018_idioms)]
#![forbid(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use eprfs_core::{
    tree::{encode_tree, load_verified_tree, TreeEntry, TreeEntryKind, TreeLimits, TreeNode},
    BlobCid, BlobLink, EprfsStorage, FetchPolicy,
};
/// The exact Git API version accepted by this bridge's public functions.
pub use gix;

use gix::{bstr::BString, objs::WriteTo, ObjectId, Repository};

/// The canonical root and source Git tree identity produced by a seal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedGitTree {
    /// Canonical EPRFS tree root.
    pub root: BlobCid,
    /// Exact Git tree object originally imported.
    pub git_tree_id: ObjectId,
}

#[derive(Default)]
struct SealBudget {
    entries: usize,
    bytes: usize,
    completed: HashMap<ObjectId, BlobCid>,
    active: HashSet<ObjectId>,
}

impl SealBudget {
    fn add_entries(&mut self, count: usize, limits: &TreeLimits) -> Result<()> {
        self.entries = self
            .entries
            .checked_add(count)
            .context("Git tree entry count overflow")?;
        if self.entries > limits.max_total_entries {
            bail!("Git tree exceeds total entry budget");
        }
        Ok(())
    }

    fn add_bytes(&mut self, count: usize, limits: &TreeLimits) -> Result<()> {
        self.bytes = self.bytes.checked_add(count).context("Git tree byte count overflow")?;
        if self.bytes > limits.max_total_bytes {
            bail!("Git tree exceeds total byte budget");
        }
        Ok(())
    }
}

/// Resolve a simple HEAD, ref name, or full object ID to a tree ID without
/// loading the tree body. Tags and commits are peeled through bounded headers;
/// complex revspec expressions are deliberately outside this local snapshot
/// command because revision parsing may load objects before this preflight.
/// The source ODB trust boundary in this module's documentation still applies.
pub fn resolve_tree_revision(repo: &Repository, revision: &str, limits: &TreeLimits) -> Result<ObjectId> {
    let mut id = if revision == "HEAD" {
        match repo.head()?.kind {
            gix::head::Kind::Symbolic(reference) => reference
                .target
                .try_id()
                .context("HEAD branch is not a direct object reference")?
                .to_owned(),
            gix::head::Kind::Detached { target, .. } => target,
            gix::head::Kind::Unborn(_) => bail!("HEAD is unborn"),
        }
    } else if matches!(revision.len(), 40 | 64) && revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let oid = ObjectId::from_hex(revision.as_bytes())?;
        if oid.kind() != repo.object_hash() {
            bail!("revision OID hash format differs from repository");
        }
        oid
    } else {
        if revision.is_empty() {
            bail!("snapshot revision must be HEAD, a simple ref, or a full object ID");
        }
        repo.find_reference(revision)
            .context("snapshot revision must be HEAD, a valid simple ref, or a full object ID")?
            .try_id()
            .context("snapshot ref is symbolic; use HEAD or a direct ref")?
            .detach()
    };

    let mut seen = HashSet::new();
    let mut peel_bytes = 0usize;
    for _ in 0..16 {
        if !seen.insert(id) {
            bail!("revision tag/commit peel cycle at {id}");
        }
        let header = repo
            .find_header(id)
            .with_context(|| format!("read revision object header {id}"))?;
        let size = usize::try_from(header.size()).context("revision object size exceeds host address space")?;
        match header.kind() {
            gix::objs::Kind::Tree => {
                if size > limits.max_node_bytes {
                    bail!("revision tree exceeds per-node byte budget before body load");
                }
                return Ok(id);
            }
            gix::objs::Kind::Commit | gix::objs::Kind::Tag => {
                if size > limits.max_node_bytes {
                    bail!("revision commit/tag exceeds byte budget before body load");
                }
                peel_bytes = peel_bytes.checked_add(size).context("revision peel byte overflow")?;
                if peel_bytes > limits.max_total_bytes {
                    bail!("revision peel exceeds aggregate byte budget before body load");
                }
                let object = repo
                    .find_object(id)
                    .with_context(|| format!("read revision object {id}"))?;
                if object.kind != header.kind() || object.data.len() != size {
                    bail!("revision object {id} changed after header preflight");
                }
                if gix::objs::compute_hash(repo.object_hash(), object.kind, &object.data)? != id {
                    bail!("revision object {id} has bytes with a different object ID");
                }
                id = match object.kind {
                    gix::objs::Kind::Commit => object
                        .try_to_commit_ref_iter()
                        .context("invalid revision commit")?
                        .tree_id()?,
                    gix::objs::Kind::Tag => object
                        .try_to_tag_ref_iter()
                        .context("invalid revision tag")?
                        .target_id()?,
                    _ => unreachable!(),
                };
            }
            gix::objs::Kind::Blob => bail!("revision resolves to a blob, not a tree"),
        }
    }
    bail!("revision tag chain exceeds 16 objects")
}

/// Charge the declared Git object size before asking the ODB to materialize
/// its body. The body is still rehashed and length-checked after retrieval.
fn preflight_git_object(
    repo: &Repository,
    id: ObjectId,
    expected_kind: gix::objs::Kind,
    per_object_limit: Option<usize>,
    limits: &TreeLimits,
    budget: &mut SealBudget,
) -> Result<usize> {
    let header = repo
        .find_header(id)
        .with_context(|| format!("read Git object header {id}"))?;
    if header.kind() != expected_kind {
        bail!("Git object {id} is {:?}, expected {expected_kind:?}", header.kind());
    }
    let size = usize::try_from(header.size()).context("Git object size exceeds host address space")?;
    if per_object_limit.is_some_and(|limit| size > limit) {
        bail!("Git object {id} exceeds per-node byte budget before body load");
    }
    budget
        .add_bytes(size, limits)
        .with_context(|| format!("Git object {id} exceeds aggregate byte budget before body load"))?;
    Ok(size)
}

/// Import one committed Git tree and every reachable tree, blob and symlink
/// into verified EPRFS storage. A gitlink retains its commit OID as an opaque
/// external reference; it does not assert availability of submodule objects.
pub async fn seal_git_tree<S: EprfsStorage>(
    repo: &Repository,
    tree_id: ObjectId,
    storage: &S,
    limits: &TreeLimits,
) -> Result<SealedGitTree> {
    if tree_id.kind() != repo.object_hash() {
        bail!("Git tree hash algorithm differs from repository object format");
    }
    let mut budget = SealBudget::default();
    let root = seal_node(repo, tree_id, storage, limits, &mut budget, 0).await?;
    // The shared loader enforces whole-closure limits and verifies every CID,
    // including leaves, before this operation advertises a usable root.
    let closure = load_verified_tree(storage, &root, FetchPolicy::LocalOnly, limits)
        .await
        .context("sealed Git tree is not a complete verified closure")?;
    let predicted = recreate_node(repo, &root, &closure, false)?;
    if predicted != tree_id {
        bail!("Git tree {tree_id} cannot round-trip through canonical tree bytes (would become {predicted})");
    }
    Ok(SealedGitTree {
        root,
        git_tree_id: tree_id,
    })
}

fn seal_node<'a, S: EprfsStorage>(
    repo: &'a Repository,
    tree_id: ObjectId,
    storage: &'a S,
    limits: &'a TreeLimits,
    budget: &'a mut SealBudget,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<BlobCid>> + 'a>> {
    Box::pin(async move {
        if depth > limits.max_depth {
            bail!("Git tree exceeds canonical maximum depth");
        }
        if let Some(cid) = budget.completed.get(&tree_id) {
            return Ok(cid.clone());
        }
        if !budget.active.insert(tree_id) {
            bail!("Git tree cycle at {tree_id}");
        }
        let declared_tree_size = preflight_git_object(
            repo,
            tree_id,
            gix::objs::Kind::Tree,
            Some(limits.max_node_bytes),
            limits,
            budget,
        )?;
        let tree = repo
            .find_tree(tree_id)
            .with_context(|| format!("find Git tree {tree_id}"))?;
        if tree.data.len() != declared_tree_size {
            bail!("Git tree {tree_id} size changed after header preflight");
        }
        if gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Tree, &tree.data)? != tree_id {
            bail!("Git tree {tree_id} has bytes with a different object ID");
        }
        // A decoder may accept spellings that its writer canonicalizes. Refuse
        // such a tree instead of silently changing its Git identity on export.
        let decoded = tree.decode()?;
        if decoded.entries.windows(2).any(|pair| pair[0] >= pair[1]) {
            bail!("Git tree {tree_id} has unsorted or duplicate entries");
        }
        let mut reencoded = Vec::new();
        decoded.write_to(&mut reencoded)?;
        if reencoded != tree.data {
            bail!("Git tree {tree_id} uses a noncanonical byte encoding");
        }

        let mut entries = Vec::new();
        for entry in tree.iter() {
            let entry = entry?;
            budget.add_entries(1, limits)?;
            if entries.len() >= limits.max_entries_per_node {
                bail!("Git tree {tree_id} exceeds per-node entry budget");
            }
            let mode = entry.mode().value();
            let name = entry.filename().to_vec();
            let oid = entry.id().detach();
            let kind = match mode {
                0o040000 => {
                    let child = seal_node(repo, oid, storage, limits, budget, depth + 1).await?;
                    TreeEntryKind::Directory {
                        tree: BlobLink::from(child),
                    }
                }
                0o100644 | 0o100755 | 0o120000 => {
                    let declared_blob_size =
                        preflight_git_object(repo, oid, gix::objs::Kind::Blob, None, limits, budget)?;
                    let blob = repo.find_blob(oid).with_context(|| format!("find Git blob {oid}"))?;
                    if blob.data.len() != declared_blob_size {
                        bail!("Git blob {oid} size changed after header preflight");
                    }
                    if gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, &blob.data)? != oid {
                        bail!("Git blob {oid} has bytes with a different object ID");
                    }
                    let cid = BlobCid::compute_raw(&blob.data);
                    storage
                        .put_blob_verified(&cid, Bytes::copy_from_slice(&blob.data))
                        .await
                        .with_context(|| format!("store Git blob {oid}"))?;
                    if mode == 0o120000 {
                        TreeEntryKind::Symlink {
                            target: BlobLink::from(cid),
                        }
                    } else {
                        TreeEntryKind::File {
                            blob: BlobLink::from(cid),
                            executable: mode == 0o100755,
                        }
                    }
                }
                0o160000 => TreeEntryKind::External {
                    kind: gitlink_kind(repo)?,
                    reference: oid.as_slice().to_vec(),
                },
                _ => bail!("unsupported Git tree mode {mode:o} at byte name {name:?}"),
            };
            entries.push(TreeEntry { name, kind });
        }
        let node = TreeNode::new(entries)?;
        let (cid, bytes) = encode_tree(&node, limits)?;
        budget.add_bytes(bytes.len(), limits)?;
        storage
            .put_blob_verified(&cid, Bytes::from(bytes))
            .await
            .with_context(|| format!("store canonical tree for Git tree {tree_id}"))?;
        budget.active.remove(&tree_id);
        budget.completed.insert(tree_id, cid.clone());
        Ok(cid)
    })
}

fn gitlink_kind(repo: &Repository) -> Result<String> {
    match repo.object_hash() {
        gix::hash::Kind::Sha1 => Ok("git-commit-sha1".into()),
        gix::hash::Kind::Sha256 => Ok("git-commit-sha256".into()),
        _ => bail!("unsupported Git object hash for gitlink"),
    }
}

fn gitlink_oid(repo: &Repository, kind: &str, reference: &[u8]) -> Result<ObjectId> {
    if kind != gitlink_kind(repo)? {
        bail!("gitlink kind {kind} does not match destination Git object format");
    }
    let oid = ObjectId::from_hex(hex::encode(reference).as_bytes())?;
    if oid.kind() != repo.object_hash() {
        bail!("gitlink object ID does not match destination Git object format");
    }
    Ok(oid)
}

/// Rebuild a verified canonical closure in an existing Git object database.
/// The expected root Git tree OID must match exactly. This writes objects only;
/// it never modifies HEAD, references, the index or a working tree.
pub async fn restore_git_tree<S: EprfsStorage>(
    repo: &Repository,
    root: &BlobCid,
    expected_git_tree_id: ObjectId,
    storage: &S,
    limits: &TreeLimits,
) -> Result<ObjectId> {
    if expected_git_tree_id.kind() != repo.object_hash() {
        bail!("expected Git tree hash algorithm differs from destination repository");
    }
    let closure = load_verified_tree(storage, root, FetchPolicy::LocalOnly, limits)
        .await
        .context("load complete canonical tree closure before Git writes")?;
    let predicted = recreate_node(repo, root, &closure, false)?;
    if predicted != expected_git_tree_id {
        bail!("canonical closure would restore Git tree {predicted}, expected {expected_git_tree_id}");
    }
    let actual = recreate_node(repo, root, &closure, true)?;
    if actual != predicted {
        bail!("Git object write returned {actual}, expected preflight tree {predicted}");
    }
    Ok(actual)
}

fn recreate_node(
    repo: &Repository,
    cid: &BlobCid,
    closure: &eprfs_core::tree::VerifiedTree,
    write: bool,
) -> Result<ObjectId> {
    let node = closure
        .nodes
        .get(cid)
        .with_context(|| format!("verified tree node missing from closure: {cid}"))?;
    let mut entries = Vec::with_capacity(node.entries.len());
    for entry in &node.entries {
        let (mode, oid) = match &entry.kind {
            TreeEntryKind::Directory { tree } => (0o040000, recreate_node(repo, tree.as_blob_cid(), closure, write)?),
            TreeEntryKind::File { blob, executable } => {
                let bytes = closure.leaf(blob.as_blob_cid()).context("verified file blob missing")?;
                let oid = if write {
                    repo.write_blob(bytes)?.detach()
                } else {
                    gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, bytes)?
                };
                (if *executable { 0o100755 } else { 0o100644 }, oid)
            }
            TreeEntryKind::Symlink { target } => {
                let bytes = closure
                    .leaf(target.as_blob_cid())
                    .context("verified symlink target missing")?;
                let oid = if write {
                    repo.write_blob(bytes)?.detach()
                } else {
                    gix::objs::compute_hash(repo.object_hash(), gix::objs::Kind::Blob, bytes)?
                };
                (0o120000, oid)
            }
            TreeEntryKind::External { kind, reference } => (0o160000, gitlink_oid(repo, kind, reference)?),
        };
        entries.push(gix::objs::tree::Entry {
            mode: gix::objs::tree::EntryMode::try_from(mode) // exact canonical Git modes only
                .map_err(|bad| anyhow::anyhow!("invalid Git mode {bad:o}"))?,
            filename: BString::from(entry.name.clone()),
            oid,
        });
    }
    // Canonical EPRFS name order is bytewise; Git compares tree names as if
    // suffixed by '/'. Let gix impose Git's native encoding order.
    entries.sort();
    let tree = gix::objs::Tree { entries };
    if write {
        Ok(repo.write_object(tree)?.detach())
    } else {
        let mut encoded = Vec::new();
        tree.write_to(&mut encoded)?;
        Ok(gix::objs::compute_hash(
            repo.object_hash(),
            gix::objs::Kind::Tree,
            &encoded,
        )?)
    }
}
