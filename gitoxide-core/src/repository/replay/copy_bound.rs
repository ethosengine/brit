//! Refuse, before it starts, a rebase whose retained copy could not fit.
//!
//! Cleanup and abort both capture a retained copy of the operation's whole object
//! closure, and every retained copy lives under the scratch quota. A rebase whose
//! copy exceeds the quota publishes and can then neither finish its cleanup nor be
//! aborted, and its journal blocks every other Brit mutation. This preflight
//! measures, before any journal exists and before any object is verified or
//! spooled, a lower bound of the bytes that copy needs, and refuses when it alone
//! exceeds the quota. Every term of the bound undercounts, so a refusal is
//! certain; a rebase within roughly its own journal's size of the quota can still
//! pass and then find its copy too large, which the copy reports before it writes.
//!
//! The measurement reads object headers for blobs and never their contents. It
//! loads each distinct commit and tree once, in memory, because their entries name
//! the rest of the closure, and stops as soon as the bound passes the quota. On a
//! closure of N objects it costs N object lookups and the decompression of every
//! commit and tree, but no blob, no spool and no write; for the 218k-object
//! closure the cost report measured (2.10 GB of blobs, 91 MB of commits and trees)
//! that is the 91 MB of commits and trees plus about 200k header lookups, against
//! roughly 12.5 GB spooled by each full verification pass.
use std::collections::{BTreeSet, HashSet};

use anyhow::{Context, Result};

/// Bytes each closure object adds to one journal state image: its quoted hex id
/// inside the checksummed envelope, which spells every payload byte as a decimal
/// number. Measured at about 171 on a 218k-object closure; this undercounts.
const STATE_BYTES_PER_OBJECT: u64 = 140;
/// Bytes each journal state image holds whatever the closure: repository paths,
/// preservation and metadata fingerprints. Measured at 18 KiB to 34 KiB on small
/// fixtures; this undercounts.
const STATE_FIXED_BYTES: u64 = 4096;
/// Bytes each object entry adds to a retained-copy manifest, measured at about 920.
const MANIFEST_BYTES_PER_OBJECT: u64 = 800;

/// The retained copy a rebase would need, as far as the preflight measured it.
#[derive(Debug, Default)]
struct Bound {
    objects: u64,
    object_bytes: u64,
    replayed: u64,
    index_bytes: u64,
}

impl Bound {
    /// Journal state images in the final copy: `state.json`, then a decision and
    /// its transition plan for the initial checkout, each replayed commit and the
    /// final publication.
    fn images(&self) -> u64 {
        1 + 2 * (self.replayed + 2)
    }

    fn needed(&self) -> u64 {
        let per_object = MANIFEST_BYTES_PER_OBJECT.saturating_add(STATE_BYTES_PER_OBJECT.saturating_mul(self.images()));
        self.object_bytes
            .saturating_add(self.objects.saturating_mul(per_object))
            .saturating_add(STATE_FIXED_BYTES.saturating_mul(self.images()))
            .saturating_add(self.index_bytes.saturating_mul(2))
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
            "rebase refused before it started: its retained copy needs at least {needed} bytes ({} objects of {} \
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
                    }
                }
            }
            if bound.needed() > limit {
                return Ok(false);
            }
        }
    }
    bound.replayed = ours.difference(&theirs).count() as u64;
    let index = repo.open_index()?;
    bound.index_bytes = match std::fs::metadata(repo.index_path()) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    };
    for entry in index.entries() {
        if entry.mode != gix::index::entry::Mode::COMMIT && seen.insert(entry.id) {
            bound.objects += 1;
            bound.object_bytes += repo.find_header(entry.id)?.size();
        }
    }
    Ok(true)
}
