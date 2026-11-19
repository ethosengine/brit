//! Refuse, before it starts, a rebase whose journal the scratch quota could not
//! let finish.
//!
//! The scratch quota meters the rebase journal: every record and payload a
//! forward step writes is checked against it, and a rebase is admitted only when
//! its journal fits. The quota is a policy about starting and continuing work,
//! never about undoing it: an abort, its retained copy and the cleanup of a
//! terminal journal, with its retained copy, are not metered at all (see
//! `switch::payload::Recovery`), so no estimate here can strand a repository.
//!
//! Because those retained copies are never refused part-way, they are not
//! projected or reserved for either: reserving a projected copy under a limit the
//! copy is not held to refuses work that fits and protects nothing. An abort's or
//! a cleanup's retained copy can therefore exceed the quota. It holds every object
//! of the closure, the whole journal (which the quota bounds), the original and
//! observed index files, and for each path of the replay footprint its
//! before-image, its worktree file and up to three staged versions, plus a
//! manifest; it can exceed the quota by at most that much, which is what never
//! refusing an abort costs. Only what is certain is refused: a closure whose
//! objects alone exceed the quota, at the preflight, and a copy whose members
//! alone already exceed it (its manifest's size is not yet known), at admission.
//! After admission the copy is never tested: admission promises the plan, one
//! stop and its resolution, and a copy that is not metered must not refuse a step
//! admission promised, so it can grow past the quota with the rebase. Only the
//! metered journal, and a skip's metered copy, refuse after admission. A skip writes its
//! retained copy under the quota, so a skip reserves for that copy as for its
//! journal (`Session::reserve_with_copy`).
//!
//! Two gates decide whether work starts or continues:
//!
//! 1. This preflight, before any journal exists and before any object is verified
//!    or spooled, makes two tests and says which one refused: the journal's
//!    estimated peak, scaled by [`SAFETY`], must fit, and the closure's object
//!    bytes alone must fit. It is the cheap, early gate: a repository that cannot
//!    fit is refused in about a second, saying plainly that Brit's rebase is not
//!    yet available at that size and that `git rebase` is the fallback.
//! 2. Admission, once preparation knows its exact records (`Session::admit`),
//!    projects the journal's peak from the real journal, refuses unless it fits
//!    with the safety factor to spare, refuses a retained copy whose members
//!    already exceed the quota, and cancels the preparation, so nothing is left behind.
//!    It includes one stop and a resolution as large as the largest step, so the
//!    first forward step after a stop is not refused for its journal.
//!
//! Every forward step afterwards (`--continue`, a staged resolution, `--skip`, an
//! edit) is reserved for before anything of it is written (`Session::reserve`);
//! a refused step names `--abort`, which always restores the original state.
//!
//! The journal bound adds, from the measured closure: for every object, an upper
//! bound of its id in each journal state image; a fixed upper bound per state
//! image; the index file once per decision and twice more, and `logs/HEAD` once
//! per decision, both of which every decision stores again; the bytes preparation
//! keeps before anything changes: the original version of every path the replay
//! touches, every staged blob, and every index entry whose worktree file is not
//! clean by its stat; and what the replay writes ([`Plan`]) as journal payloads,
//! with one stop's bytes, a resolution as large as the largest step and the
//! largest version staged at once for publication. It then scales that sum by
//! [`SAFETY`] for the members it does not model (proposals, retention recipes,
//! invocation records) and for what the model under-estimates: a conflicted file
//! written in the `diff3` style also holds the base version, so it can exceed the
//! sum of the two versions a step is modelled with. The per-unit constants are
//! measured maxima rounded up. Admission, with the exact state image, also counts
//! the stop's records.
//!
//! What a decision writes is modelled from the write paths, where a payload of
//! bytes the journal already keeps is not stored again: the initial checkout
//! stores the upstream image of every path the replay touches that differs from
//! the original's (whose image preparation already kept); each replayed step
//! stores, for each path its commit changes, the replayed version, which is that
//! commit's version with the upstream's changes since the merge base applied, so
//! every line of it is a line of one or the other: at most the sum of the two, and
//! exactly the commit's version where the upstream left the path alone. Each step
//! also creates a commit and the trees along its changed paths, whose ids every
//! later state image lists. The cost is linear in the replayed commits and the
//! sizes of what they change; an earlier bound charged every version of every
//! touched path once per decision, which grew with the square of the replay.
//!
//! The measurement reads object headers for blobs and never their contents. It
//! loads each distinct commit and tree once, in memory, because their entries name
//! the rest of the closure, loads the trees of the replayed commits again to find
//! the paths they touch, and stops as soon as either test fails. On a closure of
//! N objects it costs N object lookups and the decompression of every commit and
//! tree, but no blob, no spool and no write; for the 218k-object closure the cost
//! report measured (2.10 GB of blobs, 91 MB of commits and trees) that is the
//! 91 MB of commits and trees plus about 200k header lookups, against roughly
//! 12.5 GB spooled by each full verification pass, and that closure is refused by
//! its object bytes alone. Because every state image lists the whole closure, a
//! closure of about 390,000 objects is refused for a one-commit replay under the
//! 1 GiB quota, and fewer for longer replays, which add state images.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::{Context, Result};

/// Bytes each closure object adds to one journal state image at most: its quoted
/// hex id inside the checksummed envelope, which spells every payload byte as a
/// decimal number. Measured at 171 on a 218k-object closure.
pub(super) const STATE_BYTES_PER_OBJECT: u64 = 220;
/// Bytes each journal state image holds whatever the closure: repository paths,
/// preservation and metadata fingerprints. Measured at 18 KiB to 34 KiB.
const STATE_FIXED_BYTES: u64 = 64 * 1024;
/// State images one decision adds to the journal: the decision record and its
/// transition plan, plus the rewritten state (or a proposal) beside them while
/// it is replaced.
pub(super) const DECISION_IMAGES: u64 = 3;
/// The factor, as a ratio, applied to the modelled journal for the members the
/// bound does not model and for a `diff3` conflict's base version. Measured
/// unmodelled members stayed under 10% of a copy.
pub(super) const SAFETY: (u64, u64) = (5, 4);

/// `bytes` scaled by [`SAFETY`].
pub(super) fn with_safety(bytes: u64) -> u64 {
    bytes.saturating_mul(SAFETY.0) / SAFETY.1
}

/// The journal a rebase would need, and the closure its retained copies hold,
/// as far as the preflight measured them.
#[derive(Debug, Default)]
struct Bound {
    objects: u64,
    /// Bytes of every closure object: what any retained copy holds at least.
    object_bytes: u64,
    replayed: u64,
    index_bytes: u64,
    log_bytes: u64,
    plan: Plan,
    /// Worktree files whose index stat does not prove them clean.
    dirty_bytes: u64,
    /// Blobs staged in the index, which preparation keeps as payloads.
    staged_bytes: u64,
}

impl Bound {
    /// The initial checkout, each replayed commit and the final publication. The
    /// records of the stop allowed for are left to admission, which knows the
    /// exact state image; this early gate has to refuse what is far over the quota.
    fn decisions(&self) -> u64 {
        self.replayed + 2
    }

    /// Journal state images in the final copy: `state.json` and each decision's.
    fn images(&self) -> u64 {
        1 + DECISION_IMAGES * self.decisions()
    }

    /// The journal's peak, scaled by [`SAFETY`].
    fn journal(&self) -> u64 {
        let objects = self.objects.saturating_add(self.plan.new_objects);
        with_safety(
            objects
                .saturating_mul(STATE_BYTES_PER_OBJECT.saturating_mul(self.images()))
                .saturating_add(STATE_FIXED_BYTES.saturating_mul(self.images()))
                .saturating_add(self.index_bytes.saturating_mul(self.decisions() + 2))
                .saturating_add(self.log_bytes.saturating_mul(self.decisions()))
                .saturating_add(self.plan.kept)
                .saturating_add(self.staged_bytes)
                .saturating_add(self.dirty_bytes)
                .saturating_add(self.plan.payloads())
                .saturating_add(self.plan.largest_version),
        )
    }

    /// Whether either test already refuses under `limit`.
    fn exceeds(&self, limit: u64) -> bool {
        self.object_bytes > limit || self.journal() > limit
    }
}

/// What a linear replay writes, step by step, for the paths it touches.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Plan {
    /// The original version of every touched path, which preparation keeps as
    /// its before-image.
    pub kept: u64,
    /// Payload bytes its decisions store: the upstream image of every touched path
    /// at the initial checkout, then each path a replayed commit changes, as
    /// replayed.
    pub written: u64,
    /// The most one step stores: the allowance for a conflict stop's resolution.
    pub largest_step: u64,
    /// The largest single version: the publication staged at once.
    pub largest_version: u64,
    /// Commits and trees the replay creates (its blobs are `written`), whose ids
    /// later state images list.
    pub new_objects: u64,
}

impl Plan {
    /// Payload bytes with one stop: its conflicted files are written in place of
    /// that step's result, which `written` counts, and a resolution as large as
    /// the largest step is kept beside them.
    pub(super) fn payloads(&self) -> u64 {
        self.written.saturating_add(self.largest_step)
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
                "rebase refused before it started: Brit could not measure the journal and history this rebase \
                 needs, so it cannot show the rebase would finish; nothing was changed; git rebase is the fallback",
            )
        }
    })?;
    let cause = if limit == gix::utils::stream::SCRATCH_LIMIT {
        "brit rebase is not available on repositories of this size yet"
    } else {
        "the scratch limit configured for testing is too small for this rebase"
    };
    let measured = if complete {
        format!("replaying {} commits", bound.replayed)
    } else {
        "measured before the bound passed the limit".to_owned()
    };
    if bound.object_bytes > limit {
        anyhow::bail!(
            "rebase refused before it started: {cause}; the retained copy an abort or a cleanup writes holds at \
             least its {} objects of {} bytes ({measured}), over the {limit}-byte scratch limit; nothing was \
             changed; git rebase is the fallback",
            bound.objects,
            bound.object_bytes,
        );
    }
    let journal = bound.journal();
    if journal > limit {
        anyhow::bail!(
            "rebase refused before it started: {cause}; its journal can need up to {journal} bytes with the {}/{} \
             safety factor ({} objects, {measured}), over the {limit}-byte scratch limit the journal lives under; \
             nothing was changed; git rebase is the fallback",
            SAFETY.0,
            SAFETY.1,
            bound.objects,
        );
    }
    Ok(())
}

/// Accumulate the closure of both tips into `bound`. Returns `false` when it
/// stopped early because a test already failed under `limit`.
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
                        if bound.exceeds(limit) {
                            return Ok(false);
                        }
                    }
                }
            }
            if bound.exceeds(limit) {
                return Ok(false);
            }
        }
    }
    let replayed: Vec<_> = ours.difference(&theirs).copied().collect();
    bound.replayed = replayed.len() as u64;
    bound.plan = plan(repo, original, upstream, &replayed, &mut buffer)?;
    if bound.exceeds(limit) {
        return Ok(false);
    }
    let index = repo.open_index()?;
    bound.index_bytes = file_size(&repo.index_path())?;
    bound.log_bytes = file_size(&repo.git_dir().join("logs").join("HEAD"))?;
    let root = repo.workdir().context("worktree required")?;
    let head = repo.index_from_tree(&super::commit_tree(repo, original)?)?;
    for entry in index.entries() {
        if entry.mode == gix::index::entry::Mode::COMMIT {
            continue;
        }
        // Staged, or not provably clean by size and modification second: its
        // worktree bytes become a preservation payload, and a staged blob is kept
        // as one too.
        let path = entry.path(&index);
        let staged = head
            .entry_by_path(path)
            .is_none_or(|committed| committed.id != entry.id);
        let unseen = seen.insert(entry.id);
        if unseen || staged {
            let size = repo.find_header(entry.id)?.size();
            if unseen {
                bound.objects += 1;
                bound.object_bytes += size;
            }
            if staged {
                bound.staged_bytes += size;
            }
        }
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

/// The size of a file, or 0 when it does not exist.
pub(super) fn file_size(path: &std::path::Path) -> Result<u64> {
    match std::fs::metadata(path) {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error.into()),
    }
}

/// What replaying `replayed` (oldest first or not; each is measured against its
/// own parent) onto `upstream` writes, as modelled in the module documentation.
pub(super) fn plan(
    repo: &gix::Repository,
    original: gix::ObjectId,
    upstream: gix::ObjectId,
    replayed: &[gix::ObjectId],
    buffer: &mut gix::objs::find::Buffer,
) -> Result<Plan> {
    type Entries = BTreeMap<Vec<u8>, gix::ObjectId>;
    let entries = |commit: gix::ObjectId| -> Result<Entries> {
        super::selection_interrupt()?;
        let index = repo.index_from_tree(&super::commit_tree(repo, commit)?)?;
        Ok(index
            .entries()
            .iter()
            .filter(|entry| entry.mode != gix::index::entry::Mode::COMMIT)
            .map(|entry| (entry.path(&index).to_vec(), entry.id))
            .collect())
    };
    let changed = |a: &Entries, b: &Entries| -> BTreeSet<Vec<u8>> {
        a.keys()
            .chain(b.keys())
            .filter(|path| a.get(*path) != b.get(*path))
            .cloned()
            .collect()
    };
    let mut sizes = HashMap::new();
    let mut size = |id: Option<&gix::ObjectId>| -> Result<u64> {
        let Some(id) = id else { return Ok(0) };
        if let Some(size) = sizes.get(id) {
            return Ok(*size);
        }
        let size = repo.find_header(*id)?.size();
        sizes.insert(*id, size);
        Ok(size)
    };
    let upstream_entries = entries(upstream)?;
    let original_entries = entries(original)?;
    // The upstream's changes since the merge base are what a replayed version adds
    // to the replayed commit's own; a path the upstream left alone is replayed as is.
    let base_entries = match repo.merge_base(original, upstream)? {
        Some(base) => entries(base.detach())?,
        None => Entries::new(),
    };
    let mut touched = changed(&upstream_entries, &original_entries);
    let mut steps = Vec::new();
    for &commit in replayed {
        let data = super::read_commit(repo, commit, buffer)?;
        let parents: Vec<_> = data.parents().collect();
        drop(data);
        let tree = entries(commit)?;
        let mut paths = BTreeSet::new();
        for parent in parents {
            paths.extend(changed(&entries(parent)?, &tree));
        }
        touched.extend(paths.iter().cloned());
        steps.push((tree, paths));
    }
    let mut plan = Plan::default();
    for path in &touched {
        // Preparation keeps the original's image of every touched path.
        plan.kept = plan.kept.saturating_add(size(original_entries.get(path))?);
        // The initial checkout stores the upstream image of every touched path the
        // original's own (already kept as its before-image) does not equal.
        if upstream_entries.get(path) != original_entries.get(path) {
            let version = size(upstream_entries.get(path))?;
            plan.written = plan.written.saturating_add(version);
            plan.largest_version = plan.largest_version.max(version);
        }
    }
    for (tree, paths) in steps {
        let mut step = 0u64;
        let mut directories = BTreeSet::from([Vec::new()]);
        for path in &paths {
            let upstream_changed = upstream_entries.get(path) != base_entries.get(path);
            let version = size(tree.get(path))?.saturating_add(if upstream_changed {
                size(upstream_entries.get(path))?
            } else {
                0
            });
            step = step.saturating_add(version);
            plan.largest_version = plan.largest_version.max(version);
            let mut end = 0;
            while let Some(slash) = path[end..].iter().position(|byte| *byte == b'/') {
                end += slash;
                directories.insert(path[..end].to_vec());
                end += 1;
            }
        }
        plan.written = plan.written.saturating_add(step);
        plan.largest_step = plan.largest_step.max(step);
        // The replayed commit, and the trees along its changed paths.
        plan.new_objects += 1 + directories.len() as u64;
    }
    Ok(plan)
}
