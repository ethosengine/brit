//! Verify each immutable object once per pass, retaining its expected type.
//!
//! Two walks name what a rebase reads and keeps. The legacy walk ([`history`],
//! [`validate_tree`]) covers a commit's whole history and is what journals written
//! by Brit 0.2.6 to 0.3.1 recorded. The scoped walk ([`Validation::custody`])
//! covers only what a retained root holds above a *floor*: the chain base and the
//! upstream, both kept reachable by refs the user owns. It walks commits down to
//! the floor, and for each one only the tree entries that differ from its parent's
//! same-path entry (by id and mode); the parent trees it compares against are kept
//! too, so a retained copy can prove the same delta from its own bytes
//! (`cleanup::validate_release`). Its cost is proportional to the change, never to
//! the history below the floor.
use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

/// One verification pass: every object is verified at most once, and blobs share
/// one spool budget instead of creating a scratch directory each.
#[derive(Default)]
pub(super) struct Validation {
    checked: BTreeMap<gix::ObjectId, gix::objs::Kind>,
    budget: Option<std::sync::Arc<gix::utils::stream::Budget>>,
}

/// What a custody walk does with each object it names.
pub(super) trait Visit {
    /// Account for `id` of `kind`, returning whether it was new to this walk.
    fn visit(&mut self, repo: &gix::Repository, id: gix::ObjectId, kind: gix::objs::Kind) -> Result<bool>;
}

impl Validation {
    fn object(
        &mut self,
        repo: &gix::Repository,
        object_id: gix::ObjectId,
        kind: gix::objs::Kind,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<bool> {
        ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
        #[cfg(target_os = "linux")]
        super::super::switch::checkpoint("replay-selection-object")?;
        if let Some(checked_kind) = self.checked.get(&object_id) {
            ensure!(*checked_kind == kind, "pinned object has wrong type");
            return Ok(closure.insert(object_id));
        }
        if self.budget.is_none() {
            self.budget = Some(gix::utils::stream::Budget::new(&std::env::temp_dir())?);
        }
        let budget = self.budget.as_ref().context("verification budget")?;
        ensure!(
            verify_object_in(repo, object_id, budget)? == kind,
            "pinned object has wrong type"
        );
        #[cfg(all(target_os = "linux", feature = "switch-test-faults"))]
        super::super::switch::checkpoint("replay-object-verified")?;
        ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
        self.checked.insert(object_id, kind);
        closure.insert(object_id);
        Ok(true)
    }

    pub(super) fn history(
        &mut self,
        repo: &gix::Repository,
        tip: gix::ObjectId,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>> {
        let mut commits = BTreeMap::new();
        let mut pending = vec![tip];
        let mut buffer = gix::objs::find::Buffer::default();
        while let Some(commit_id) = pending.pop() {
            ensure!(!gix::interrupt::is_triggered(), "replay history validation interrupted");
            if commits.contains_key(&commit_id) {
                continue;
            }
            self.object(repo, commit_id, gix::objs::Kind::Commit, closure)?;
            let commit = super::read_commit(repo, commit_id, &mut buffer)?;
            let parents: Vec<_> = commit.parents().collect();
            let tree = commit.tree();
            drop(commit);
            buffer.clear();
            self.tree(repo, tree, closure)?;
            pending.extend(parents.iter().copied());
            commits.insert(commit_id, parents);
        }
        Ok(commits)
    }

    fn tree(
        &mut self,
        repo: &gix::Repository,
        tree_id: gix::ObjectId,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<()> {
        let mut pending = vec![tree_id];
        let source = gix::object::bounded::Source::deferred(repo);
        let mut buffer = gix::objs::find::Buffer::default();
        while let Some(tree_id) = pending.pop() {
            if !self.object(repo, tree_id, gix::objs::Kind::Tree, closure)? {
                continue;
            }
            let tree = source.load(&tree_id, &mut buffer)?;
            ensure!(tree.kind == gix::objs::Kind::Tree, "pinned object has wrong type");
            for entry in gix::objs::TreeRefIter::from_bytes(tree.data, repo.object_hash()) {
                ensure!(!gix::interrupt::is_triggered(), "replay tree validation interrupted");
                let entry = entry?;
                if entry.mode.is_tree() {
                    pending.push(entry.oid.to_owned());
                } else if !entry.mode.is_commit() {
                    self.object(repo, entry.oid.to_owned(), gix::objs::Kind::Blob, closure)?;
                }
            }
        }
        Ok(())
    }

    /// Verify, and add to `closure`, the custody of commit `tip` above `floor`
    /// (see the module documentation).
    pub(super) fn custody(
        &mut self,
        repo: &gix::Repository,
        tip: gix::ObjectId,
        floor: &BTreeSet<gix::ObjectId>,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<()> {
        struct Verify<'a> {
            validation: &'a mut Validation,
            closure: &'a mut BTreeSet<gix::ObjectId>,
        }
        impl Visit for Verify<'_> {
            fn visit(&mut self, repo: &gix::Repository, id: gix::ObjectId, kind: gix::objs::Kind) -> Result<bool> {
                self.validation.object(repo, id, kind, self.closure)
            }
        }
        walk_custody(
            repo,
            tip,
            floor,
            &mut Verify {
                validation: self,
                closure,
            },
        )
    }

    /// Verify, and add to `closure`, the whole tree `tree` (a stop's tree has no
    /// commit to take a delta against).
    pub(super) fn whole_tree(
        &mut self,
        repo: &gix::Repository,
        tree: gix::ObjectId,
        closure: &mut BTreeSet<gix::ObjectId>,
    ) -> Result<()> {
        struct Verify<'a> {
            validation: &'a mut Validation,
            closure: &'a mut BTreeSet<gix::ObjectId>,
        }
        impl Visit for Verify<'_> {
            fn visit(&mut self, repo: &gix::Repository, id: gix::ObjectId, kind: gix::objs::Kind) -> Result<bool> {
                self.validation.object(repo, id, kind, self.closure)
            }
        }
        walk_tree_delta(
            repo,
            tree,
            None,
            &mut Verify {
                validation: self,
                closure,
            },
        )
    }

    /// Verify what a replay reads of `commit` along `paths`: the commit, the trees
    /// on the way to each path and the blob at its end. Nothing is collected.
    pub(super) fn along(
        &mut self,
        repo: &gix::Repository,
        commit: gix::ObjectId,
        paths: &[gix::bstr::BString],
    ) -> Result<()> {
        let mut read = BTreeSet::new();
        self.object(repo, commit, gix::objs::Kind::Commit, &mut read)?;
        let root = super::commit_tree(repo, commit)?;
        let source = gix::object::bounded::Source::deferred(repo);
        let mut buffer = gix::objs::find::Buffer::default();
        for path in paths {
            let mut tree = root;
            let mut components = path.split(|byte| *byte == b'/').peekable();
            while let Some(name) = components.next() {
                self.object(repo, tree, gix::objs::Kind::Tree, &mut read)?;
                let data = source.load(&tree, &mut buffer)?;
                ensure!(data.kind == gix::objs::Kind::Tree, "pinned object has wrong type");
                let Some(entry) = gix::objs::TreeRefIter::from_bytes(data.data, repo.object_hash())
                    .filter_map(Result::ok)
                    .find(|entry| entry.filename == name)
                    .map(|entry| (entry.mode, entry.oid.to_owned()))
                else {
                    break;
                };
                if entry.0.is_tree() {
                    tree = entry.1;
                    continue;
                }
                if components.peek().is_none() && !entry.0.is_commit() {
                    self.object(repo, entry.1, gix::objs::Kind::Blob, &mut read)?;
                }
                break;
            }
        }
        Ok(())
    }
}

/// Name, through `visit`, the custody of commit `tip` above `floor`: every commit
/// from `tip` down to (not into) the floor, each commit's tree delta against its
/// parent's tree, the parent trees that delta compares against, and the floor
/// commits a walked commit has as its parent. Every walked commit must have one
/// parent: a merge or a root above the floor would leave the replayed range.
pub(super) fn walk_custody(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    floor: &BTreeSet<gix::ObjectId>,
    visit: &mut dyn Visit,
) -> Result<()> {
    let mut pending = vec![tip];
    let mut walked = BTreeSet::new();
    let mut buffer = gix::objs::find::Buffer::default();
    while let Some(commit_id) = pending.pop() {
        ensure!(!gix::interrupt::is_triggered(), "replay history validation interrupted");
        if floor.contains(&commit_id) || !walked.insert(commit_id) {
            continue;
        }
        visit.visit(repo, commit_id, gix::objs::Kind::Commit)?;
        let commit = super::read_commit(repo, commit_id, &mut buffer)?;
        let parents: Vec<_> = commit.parents().collect();
        let tree = commit.tree();
        drop(commit);
        buffer.clear();
        match parents.as_slice() {
            [] => walk_tree_delta(repo, tree, None, visit)?,
            [parent] => {
                if floor.contains(parent) {
                    visit.visit(repo, *parent, gix::objs::Kind::Commit)?;
                }
                let base = super::commit_tree(repo, *parent)?;
                walk_tree_delta(repo, tree, Some(base), visit)?;
                pending.push(*parent);
            }
            _ => anyhow::bail!("retained commit {commit_id} is a merge above the replay's floor"),
        }
    }
    Ok(())
}

/// Name `tree`'s delta against `base` (all of `tree` without one): `tree` and
/// `base` themselves where they differ, and every entry not equal, by id and mode,
/// to `base`'s same-path entry, descending into changed subtrees against the
/// base's subtree of that name. Gitlinks name submodule commits, never local objects.
pub(super) fn walk_tree_delta(
    repo: &gix::Repository,
    tree: gix::ObjectId,
    base: Option<gix::ObjectId>,
    visit: &mut dyn Visit,
) -> Result<()> {
    let source = gix::object::bounded::Source::deferred(repo);
    let mut buffer = gix::objs::find::Buffer::default();
    let mut pending = vec![(tree, base)];
    let mut done = BTreeSet::new();
    while let Some((tree, base)) = pending.pop() {
        ensure!(!gix::interrupt::is_triggered(), "replay tree validation interrupted");
        if Some(tree) == base || !done.insert((tree, base)) {
            continue;
        }
        visit.visit(repo, tree, gix::objs::Kind::Tree)?;
        let base_entries = match base {
            Some(base) => {
                visit.visit(repo, base, gix::objs::Kind::Tree)?;
                tree_entries(repo, &source, base, &mut buffer)?
            }
            None => BTreeMap::new(),
        };
        for (name, (mode, id)) in tree_entries(repo, &source, tree, &mut buffer)? {
            ensure!(!gix::interrupt::is_triggered(), "replay tree validation interrupted");
            let inherited = base_entries.get(&name);
            if inherited == Some(&(mode, id)) || mode.is_commit() {
                continue;
            }
            if mode.is_tree() {
                let sub = inherited.filter(|(mode, _)| mode.is_tree()).map(|(_, id)| *id);
                pending.push((id, sub));
            } else {
                visit.visit(repo, id, gix::objs::Kind::Blob)?;
            }
        }
    }
    Ok(())
}

type Entries = BTreeMap<Vec<u8>, (gix::objs::tree::EntryMode, gix::ObjectId)>;

fn tree_entries(
    repo: &gix::Repository,
    source: &gix::object::bounded::Source<'_>,
    tree: gix::ObjectId,
    buffer: &mut gix::objs::find::Buffer,
) -> Result<Entries> {
    let data = source.load(&tree, buffer)?;
    ensure!(data.kind == gix::objs::Kind::Tree, "pinned object has wrong type");
    gix::objs::TreeRefIter::from_bytes(data.data, repo.object_hash())
        .map(|entry| {
            let entry = entry?;
            Ok((entry.filename.to_vec(), (entry.mode, entry.oid.to_owned())))
        })
        .collect()
}

/// Verify blobs by streaming and metadata through bounded storage.
pub(super) fn verify_object(repo: &gix::Repository, id: gix::ObjectId) -> Result<gix::objs::Kind> {
    let budget = gix::utils::stream::Budget::new(&std::env::temp_dir())?;
    verify_object_in(repo, id, &budget)
}

/// [`verify_object`] spooling under `budget`, shared by every object of a pass.
fn verify_object_in(
    repo: &gix::Repository,
    id: gix::ObjectId,
    budget: &std::sync::Arc<gix::utils::stream::Budget>,
) -> Result<gix::objs::Kind> {
    use gix::objs::FindStream;
    ensure!(!gix::interrupt::is_triggered(), "replay object validation interrupted");
    let mut buffer = gix::objs::find::Buffer::default();
    let source = gix::object::bounded::Source::deferred(repo);
    let kind = repo.find_header(id)?.kind();
    let (kind, size, mut bytes): (_, _, Box<dyn std::io::Read + '_>) = if kind == gix::objs::Kind::Blob {
        let (kind, bytes) = repo
            .objects
            .find_stream(&id, budget, &gix::interrupt::IS_INTERRUPTED)
            .with_context(|| format!("verify pinned replay object {id}"))?;
        (kind, bytes.len(), Box::new(bytes))
    } else {
        let object = source.load(&id, &mut buffer)?;
        (
            object.kind,
            object.data.len() as u64,
            Box::new(std::io::Cursor::new(object.data)),
        )
    };
    ensure!(
        gix::objs::compute_stream_hash(
            repo.object_hash(),
            kind,
            &mut bytes,
            size,
            &mut gix::progress::Discard,
            &gix::interrupt::IS_INTERRUPTED
        )? == id,
        "pinned object checksum mismatch"
    );
    Ok(kind)
}

/// The legacy whole-history walk in one fresh pass (tests compare copies with it).
#[cfg(all(target_os = "linux", test))]
pub(super) fn history(
    repo: &gix::Repository,
    tip: gix::ObjectId,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<BTreeMap<gix::ObjectId, Vec<gix::ObjectId>>> {
    Validation::default().history(repo, tip, closure)
}

#[cfg(target_os = "linux")]
pub(super) fn validate_tree(
    repo: &gix::Repository,
    tree_id: gix::ObjectId,
    closure: &mut BTreeSet<gix::ObjectId>,
) -> Result<()> {
    Validation::default().tree(repo, tree_id, closure)
}
