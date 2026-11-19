//! Refuse, before it starts, a rebase whose retained copy could not fit.
//!
//! Cleanup and abort both capture a retained copy of the operation's whole object
//! closure, together with the whole journal, and every journal and retained copy
//! lives under the scratch quota. A rebase that outgrows either can publish and then
//! neither finish its cleanup nor be aborted, and its journal then blocks every
//! other Brit mutation. Three gates keep that from happening:
//!
//! 1. This preflight, before any journal exists and before any object is verified
//!    or spooled, refuses a rebase whose estimated copy cannot fit. It is the cheap,
//!    early gate for repositories far over the quota.
//! 2. Admission, once preparation knows its exact records
//!    (`Session::admit`), projects the peak journal and copy of the planned replay
//!    and cancels the preparation, nothing live changed, unless both fit with the
//!    safety factor to spare.
//! 3. Before every further step writes anything (`Session::reserve`), the journal
//!    and the copy an abort then needs, with the abort's own records reserved, must
//!    still fit; decisions a conflict or an edit adds are covered there. A refused
//!    step leaves `--abort` able to restore the original state.
//!
//! The preflight's bound adds, from the measured closure: every closure object's bytes and the
//! index's own objects; the index file twice (the original and observed copies);
//! for every object, an upper bound of its manifest entry and of its id in each
//! journal state image; a fixed upper bound per state image; every version a
//! replayed path can be written in (for each path the rebase touches, the sum of its
//! blob sizes in the upstream tip, the original tip and each replayed commit, once
//! for every decision), which covers both the journal's payloads and the merged
//! blobs the final index holds; and every index entry whose worktree file is not
//! clean by its stat, twice. It then scales that sum by [`SAFETY`] for members it
//! does not model (proposals, retention recipes, invocation records). The per-unit
//! constants are measured maxima rounded up; the merge term assumes a clean merge
//! result is no larger than its two sides, which holds for line merges. A
//! resolution a user authors at a conflict stop is not bounded by anything the
//! preflight can see; gate 3 reserves for it when it is staged.
//!
//! The measurement reads object headers for blobs and never their contents. It
//! loads each distinct commit and tree once, in memory, because their entries name
//! the rest of the closure, loads the trees of the replayed commits again to find
//! the paths they touch, and stops as soon as the bound passes the quota. On a
//! closure of N objects it costs N object lookups and the decompression of every
//! commit and tree, but no blob, no spool and no write; for the 218k-object
//! closure the cost report measured (2.10 GB of blobs, 91 MB of commits and trees)
//! that is the 91 MB of commits and trees plus about 200k header lookups, against
//! roughly 12.5 GB spooled by each full verification pass.
use std::collections::{BTreeSet, HashSet};

use anyhow::{Context, Result};

/// Bytes each closure object adds to one journal state image at most: its quoted
/// hex id inside the checksummed envelope, which spells every payload byte as a
/// decimal number. Measured at 171 on a 218k-object closure.
const STATE_BYTES_PER_OBJECT: u64 = 220;
/// Bytes each journal state image holds whatever the closure: repository paths,
/// preservation and metadata fingerprints. Measured at 18 KiB to 34 KiB.
const STATE_FIXED_BYTES: u64 = 64 * 1024;
/// Bytes each object entry adds to a retained-copy manifest. Measured at 920.
const MANIFEST_BYTES_PER_OBJECT: u64 = 1200;
/// The factor, as a ratio, applied to the modelled sum for the journal members the
/// bound does not model. Measured unmodelled members stayed under 10% of a copy.
const SAFETY: (u64, u64) = (5, 4);

/// The retained copy a rebase would need, as far as the preflight measured it.
#[derive(Debug, Default)]
struct Bound {
    objects: u64,
    object_bytes: u64,
    replayed: u64,
    index_bytes: u64,
    /// Every version the rebase can write of the paths it touches.
    written_bytes: u64,
    /// Worktree files whose index stat does not prove them clean.
    dirty_bytes: u64,
}

impl Bound {
    /// Journal state images in the final copy: `state.json`, then for the initial
    /// checkout, each replayed commit and the final publication a decision, its
    /// transition plan and a proposal or retention record.
    fn images(&self) -> u64 {
        1 + 3 * self.decisions()
    }

    fn decisions(&self) -> u64 {
        self.replayed + 2
    }

    fn needed(&self) -> u64 {
        let per_object = MANIFEST_BYTES_PER_OBJECT.saturating_add(STATE_BYTES_PER_OBJECT.saturating_mul(self.images()));
        let modelled = self
            .object_bytes
            .saturating_add(self.objects.saturating_mul(per_object))
            .saturating_add(STATE_FIXED_BYTES.saturating_mul(self.images()))
            .saturating_add(self.index_bytes.saturating_mul(2))
            .saturating_add(self.written_bytes.saturating_mul(self.decisions()))
            .saturating_add(self.dirty_bytes.saturating_mul(2));
        modelled.saturating_mul(SAFETY.0) / SAFETY.1
    }
}

pub(super) fn preflight(
    repo: &gix::Repository,
    original: gix::ObjectId,
    upstream: gix::ObjectId,
    interactive: bool,
    limit: u64,
) -> Result<()> {
    // Whether anything would be replayed is decided by the merge base alone. Any
    // failure here is reported, in order, by selection itself.
    let Ok(bases) = repo.merge_bases_many(original, &[upstream]) else {
        return Ok(());
    };
    if bases.len() != 1 || (bases[0].detach() == upstream && !(interactive && original != upstream)) {
        return Ok(());
    }
    let mut bound = Bound::default();
    let complete = measure(repo, original, upstream, limit, &mut bound).map_err(|error| {
        if gix::interrupt::is_triggered() {
            error
        } else {
            error.context(
                "rebase refused before it started: Brit could not measure the retained copy this rebase needs, \
                 so it cannot show the rebase would finish; nothing was changed; git rebase is the fallback",
            )
        }
    })?;
    let needed = bound.needed();
    if needed > limit {
        anyhow::bail!(
            "rebase refused before it started: its retained copy can need up to {needed} bytes ({} objects of {} \
             bytes{}, plus journal, manifest and index), over the {limit}-byte scratch limit every retained copy \
             lives under, so it could publish and then neither finish its cleanup nor be aborted; nothing was \
             changed; git rebase is the fallback",
            bound.objects,
            bound.object_bytes,
            if complete {
                format!(", replaying {} commits", bound.replayed)
            } else {
                " measured before the bound passed the limit".to_owned()
            },
        );
    }
    Ok(())
}

/// Accumulate the closure of both tips into `bound`. Returns `false` when it
/// stopped early because the bound already exceeded `limit`.
fn measure(
    repo: &gix::Repository,
    original: gix::ObjectId,
    upstream: gix::ObjectId,
    limit: u64,
    bound: &mut Bound,
) -> Result<bool> {
    let source = gix::object::bounded::Source::deferred(repo);
    let mut buffer = gix::objs::find::Buffer::default();
    let mut seen = HashSet::new();
    let mut ours = BTreeSet::new();
    let mut theirs = BTreeSet::new();
    for (tip, commits) in [(original, &mut ours), (upstream, &mut theirs)] {
        let mut pending = vec![tip];
        while let Some(commit) = pending.pop() {
            super::selection_interrupt()?;
            if !commits.insert(commit) {
                continue;
            }
            let data = super::read_commit(repo, commit, &mut buffer)?;
            let parents: Vec<_> = data.parents().collect();
            let tree = data.tree();
            drop(data);
            pending.extend(parents);
            if seen.insert(commit) {
                bound.objects += 1;
                bound.object_bytes += repo.find_header(commit)?.size();
            }
            let mut trees = vec![tree];
            while let Some(tree) = trees.pop() {
                super::selection_interrupt()?;
                if !seen.insert(tree) {
                    continue;
                }
                let data = source.load(&tree, &mut buffer)?;
                anyhow::ensure!(data.kind == gix::objs::Kind::Tree, "expected tree {tree}");
                bound.objects += 1;
                bound.object_bytes += data.data.len() as u64;
                for entry in gix::objs::TreeRefIter::from_bytes(data.data, repo.object_hash()) {
                    let entry = entry?;
                    if entry.mode.is_tree() {
                        trees.push(entry.oid.to_owned());
                    } else if !entry.mode.is_commit() && seen.insert(entry.oid.to_owned()) {
                        bound.objects += 1;
                        bound.object_bytes += repo
                            .find_header(entry.oid)
                            .with_context(|| format!("read the header of blob {}", entry.oid))?
                            .size();
                        // One giant tree is not walked to its end once past the limit.
                        if bound.needed() > limit {
                            return Ok(false);
                        }
                    }
                }
            }
            if bound.needed() > limit {
                return Ok(false);
            }
        }
    }
    let replayed: Vec<_> = ours.difference(&theirs).copied().collect();
    bound.replayed = replayed.len() as u64;
    bound.written_bytes = written(repo, original, upstream, &replayed, &mut buffer)?.0;
    if bound.needed() > limit {
        return Ok(false);
    }
    let index = repo.open_index()?;
    bound.index_bytes = match std::fs::metadata(repo.index_path()) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    };
    let root = repo.workdir().context("worktree required")?;
    let head = repo.index_from_tree(&super::commit_tree(repo, original)?)?;
    for entry in index.entries() {
        if entry.mode == gix::index::entry::Mode::COMMIT {
            continue;
        }
        if seen.insert(entry.id) {
            bound.objects += 1;
            bound.object_bytes += repo.find_header(entry.id)?.size();
        }
        // Staged, or not provably clean by size and modification second: its
        // worktree bytes become a preservation payload and a retained image.
        let path = entry.path(&index);
        let staged = head
            .entry_by_path(path)
            .is_none_or(|committed| committed.id != entry.id);
        if let Ok(metadata) = std::fs::symlink_metadata(root.join(gix::path::from_bstr(path)?)) {
            let seconds = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs());
            let clean = u64::from(entry.stat.size) == (metadata.len() & u64::from(u32::MAX))
                && seconds == Some(u64::from(entry.stat.mtime.secs));
            if staged || !clean {
                bound.dirty_bytes += metadata.len();
            }
        }
    }
    Ok(true)
}

/// For every path the rebase touches, the sum of its blob sizes in the upstream
/// tip, the original tip and each replayed commit. A clean merge result is no
/// larger than its two sides, so no version the rebase writes of that path, nor
/// their running sum over the replay, exceeds this.
///
/// Also returns the largest single version, the publication staged at once.
pub(super) fn written(
    repo: &gix::Repository,
    original: gix::ObjectId,
    upstream: gix::ObjectId,
    replayed: &[gix::ObjectId],
    buffer: &mut gix::objs::find::Buffer,
) -> Result<(u64, u64)> {
    let entries = |commit: gix::ObjectId| -> Result<std::collections::BTreeMap<Vec<u8>, gix::ObjectId>> {
        super::selection_interrupt()?;
        let index = repo.index_from_tree(&super::commit_tree(repo, commit)?)?;
        Ok(index
            .entries()
            .iter()
            .filter(|entry| entry.mode != gix::index::entry::Mode::COMMIT)
            .map(|entry| (entry.path(&index).to_vec(), entry.id))
            .collect())
    };
    let changed = |a: &std::collections::BTreeMap<Vec<u8>, gix::ObjectId>,
                   b: &std::collections::BTreeMap<Vec<u8>, gix::ObjectId>,
                   paths: &mut BTreeSet<Vec<u8>>| {
        for path in a.keys().chain(b.keys()) {
            if a.get(path) != b.get(path) {
                paths.insert(path.clone());
            }
        }
    };
    let mut trees = vec![entries(upstream)?, entries(original)?];
    let mut paths = BTreeSet::new();
    changed(&trees[0], &trees[1], &mut paths);
    for &commit in replayed {
        let data = super::read_commit(repo, commit, buffer)?;
        let parents: Vec<_> = data.parents().collect();
        drop(data);
        let tree = entries(commit)?;
        for parent in parents {
            changed(&entries(parent)?, &tree, &mut paths);
        }
        trees.push(tree);
    }
    let mut sizes = std::collections::HashMap::new();
    let mut total = 0u64;
    let mut largest = 0u64;
    for tree in &trees {
        for path in &paths {
            if let Some(id) = tree.get(path) {
                let size = match sizes.get(id) {
                    Some(size) => *size,
                    None => {
                        let size = repo.find_header(*id)?.size();
                        sizes.insert(*id, size);
                        size
                    }
                };
                total = total.saturating_add(size);
                largest = largest.max(size);
            }
        }
    }
    Ok((total, largest))
}
