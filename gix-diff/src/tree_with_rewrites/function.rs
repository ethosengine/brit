use bstr::BStr;
use gix_error::ErrorExt;
use gix_error::Result;
use gix_error::ResultExt;
use gix_object::TreeRefIter;

use super::{Action, ChangeRef, Options, directories::Directories};
use crate::{rewrites, rewrites::tracker, tree::Error};

/// Call `for_each` repeatedly with all changes that are needed to convert `lhs` to `rhs`.
/// Provide a `resource_cache` to speed up obtaining blobs for similarity checks.
/// `tree_diff_state` can be used to re-use tree-diff memory between calls.
/// `objects` are used to lookup trees while performing the diff.
/// Use `options` to further configure how the rename tracking is performed.
///
/// Reusing `resource_cache` between multiple invocations saves a lot of IOps as it avoids the creation
/// of a temporary `resource_cache` that triggers reading or checking for multiple gitattribute files.
/// Note that it's recommended to call [`clear_resource_cache()`](`crate::blob::Platform::clear_resource_cache()`)
/// between the calls to avoid runaway memory usage, as the cache isn't limited.
///
/// Note that to do rename tracking like `git` does, one has to configure the `resource_cache` with
/// a conversion pipeline that uses [`crate::blob::pipeline::Mode::ToGit`].
///
/// `rhs` or `lhs` can be empty to indicate deletion or addition of an entire tree.
///
/// Directory renames are inferred from finalized file renames from deleted trees into added or modified trees.
/// Each source directory needs a unique destination with the most renamed children. Renamed files support
/// their containing directories and ancestors with matching relative directory suffixes.
/// Directory inference does not affect file matching.
///
/// Note that the rewrite outcome is only available if [rewrite-tracking was enabled](Options::rewrites).
pub fn diff(
    lhs: TreeRefIter<'_>,
    rhs: TreeRefIter<'_>,
    resource_cache: &mut crate::blob::Platform,
    tree_diff_state: &mut crate::tree::State,
    objects: &impl gix_object::FindObjectOrHeader,
    for_each: impl FnMut(ChangeRef<'_>) -> Result<Action>,
    options: Options,
) -> std::result::Result<Option<rewrites::Outcome>, Error> {
    diff_inner(
        lhs,
        rhs,
        resource_cache,
        tree_diff_state,
        objects,
        objects,
        for_each,
        options,
        None,
    )
}

/// Compute the same tree diff as [`diff()`] with caller-controlled storage for similarity resources.
///
/// Trees are read through `objects`. `load_resource` loads blobs for similarity checks as described in
/// [`Tracker::emit_with_resource_loader()`](rewrites::Tracker::emit_with_resource_loader()), with `objects`
/// supplying the blobs whose scored bytes can't be obtained from it.
#[expect(
    clippy::too_many_arguments,
    reason = "Keep the tree-diff inputs and resource loader explicit"
)]
pub fn diff_with_resource_loader(
    lhs: TreeRefIter<'_>,
    rhs: TreeRefIter<'_>,
    resource_cache: &mut crate::blob::Platform,
    tree_diff_state: &mut crate::tree::State,
    objects: &dyn gix_object::find::Source,
    for_each: impl FnMut(ChangeRef<'_>) -> Result<Action>,
    options: Options,
    load_resource: &mut crate::blob::ResourceLoader<'_>,
) -> std::result::Result<Option<rewrites::Outcome>, Error> {
    diff_inner(
        lhs,
        rhs,
        resource_cache,
        tree_diff_state,
        objects,
        &SourceObjects(objects),
        for_each,
        options,
        Some(load_resource),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Keep the tree-diff inputs and resource loader explicit"
)]
fn diff_inner(
    lhs: TreeRefIter<'_>,
    rhs: TreeRefIter<'_>,
    resource_cache: &mut crate::blob::Platform,
    tree_diff_state: &mut crate::tree::State,
    trees: &dyn gix_object::find::Source,
    objects: &impl gix_object::FindObjectOrHeader,
    for_each: impl FnMut(ChangeRef<'_>) -> Result<Action>,
    options: Options,
    load_resource: Option<&mut crate::blob::ResourceLoader<'_>>,
) -> std::result::Result<Option<rewrites::Outcome>, Error> {
    fn callback_error(err: gix_error::Error) -> Error {
        Error::Failure(err.and_raise(gix_error::message("The user-provided callback failed")))
    }

    let mut delegate = Delegate {
        src_tree: lhs,
        recorder: crate::tree::Recorder::default().track_location(if options.rewrites.is_some() {
            Some(crate::tree::recorder::Location::Path)
        } else {
            options.location
        }),
        visit: for_each,
        location: options.location,
        trees,
        objects,
        tracked: options.rewrites.map(rewrites::Tracker::new),
        directories: Directories::default(),
        err: None,
    };
    match crate::tree(lhs, rhs, tree_diff_state, trees, &mut delegate) {
        Ok(()) => {
            let outcome = delegate.process_tracked_changes(resource_cache, load_resource)?;
            match delegate.err {
                Some(err) => Err(callback_error(err)),
                None => Ok(outcome),
            }
        }
        Err(crate::tree::Error::Cancelled) => delegate
            .err
            .map_or(Err(Error::Cancelled), |err| Err(callback_error(err))),
        Err(err) => Err(err),
    }
}

struct TreeSource<'a>(&'a dyn gix_object::find::Source);

impl gix_traverse::tree::source::Source for TreeSource<'_> {
    fn find_tree<'a>(&self, id: &gix_hash::oid, buffer: &'a mut gix_object::find::Buffer) -> Result<TreeRefIter<'a>> {
        self.0.find_tree(id, buffer)
    }
}

/// Supply the buffered similarity reads that a resource loader can't serve from the tree source.
/// These copy the object into the reusable buffer, and headers are obtained by reading the object.
struct SourceObjects<'a>(&'a dyn gix_object::find::Source);

impl gix_object::Find for SourceObjects<'_> {
    fn try_find<'a>(&self, id: &gix_hash::oid, buffer: &'a mut Vec<u8>) -> Result<Option<gix_object::Data<'a>>> {
        let mut source_buffer = gix_object::find::Buffer::default();
        let Some(data) = self.0.try_find(id, &mut source_buffer)? else {
            return Ok(None);
        };
        buffer.clear();
        buffer
            .try_reserve(data.data.len())
            .or_raise(|| gix_error::message("Memory allocation failed"))?;
        buffer.extend_from_slice(data.data);
        Ok(Some(gix_object::Data {
            kind: data.kind,
            object_hash: data.object_hash,
            data: buffer.as_slice(),
        }))
    }
}

impl gix_object::FindHeader for SourceObjects<'_> {
    fn try_header(&self, id: &gix_hash::oid) -> Result<Option<gix_object::Header>> {
        let mut buffer = gix_object::find::Buffer::default();
        Ok(self.0.try_find(id, &mut buffer)?.map(|data| gix_object::Header {
            kind: data.kind,
            size: data.data.len() as u64,
        }))
    }
}

struct Delegate<'a, 'old, VisitFn, Objects> {
    src_tree: TreeRefIter<'old>,
    recorder: crate::tree::Recorder,
    trees: &'a dyn gix_object::find::Source,
    objects: &'a Objects,
    visit: VisitFn,
    tracked: Option<rewrites::Tracker<crate::tree::visit::Change>>,
    directories: Directories,
    location: Option<crate::tree::recorder::Location>,
    err: Option<gix_error::Error>,
}

impl<VisitFn, Objects> Delegate<'_, '_, VisitFn, Objects>
where
    Objects: gix_object::FindObjectOrHeader,
    VisitFn: FnMut(ChangeRef<'_>) -> Result<Action>,
{
    /// Call `visit` with the requested path representation.
    fn emit_change(
        change: ChangeRef<'_>,
        location: Option<crate::tree::recorder::Location>,
        visit: &mut VisitFn,
        stored_err: &mut Option<gix_error::Error>,
    ) -> crate::tree::visit::Action {
        match visit(change.with_location(location)) {
            Ok(std::ops::ControlFlow::Break(())) => std::ops::ControlFlow::Break(()),
            Ok(std::ops::ControlFlow::Continue(())) => std::ops::ControlFlow::Continue(()),
            Err(err) => {
                *stored_err = Some(err);
                std::ops::ControlFlow::Break(())
            }
        }
    }

    fn process_tracked_changes(
        &mut self,
        diff_cache: &mut crate::blob::Platform,
        load_resource: Option<&mut crate::blob::ResourceLoader<'_>>,
    ) -> std::result::Result<Option<rewrites::Outcome>, Error> {
        let tracked = match self.tracked.as_mut() {
            Some(t) => t,
            None => return Ok(None),
        };

        self.directories.index_paths();
        let mut cancelled = false;
        let outcome = tracked.emit_inner(
            |dest, source| {
                let change = match source {
                    Some(source) => {
                        if source.kind == tracker::visit::SourceKind::Rename {
                            self.directories.record_rename(source.location, dest.location);
                        }
                        let (oid, mode) = dest.change.oid_and_entry_mode();
                        ChangeRef::Rewrite {
                            source_location: source.location,
                            source_entry_mode: source.entry_mode,
                            source_id: match source.change {
                                crate::tree::visit::Change::Modification { previous_oid, .. } => *previous_oid,
                                change => change.oid_and_entry_mode().0.to_owned(),
                            },
                            source_relation: source.change.relation(),
                            entry_mode: mode,
                            id: oid.to_owned(),
                            relation: dest.change.relation(),
                            similarity: source.similarity,
                            location: dest.location,
                            copy: match source.kind {
                                tracker::visit::SourceKind::Rename => false,
                                tracker::visit::SourceKind::Copy => true,
                            },
                        }
                    }
                    None => ChangeRef::from_tree_change(&dest.change, dest.location),
                };
                let action = Self::emit_change(change, self.location, &mut self.visit, &mut self.err);
                cancelled = action.is_break();
                action
            },
            diff_cache,
            self.objects,
            load_resource,
            |push| {
                let mut delegate = tree_to_changes::Delegate::new(push, Some(crate::tree::recorder::Location::Path));
                let state = gix_traverse::tree::breadthfirst::State::default();
                gix_traverse::tree::breadthfirst(self.src_tree, state, TreeSource(self.trees), &mut delegate)
            },
            Default::default(),
        )?;
        if !cancelled {
            let _ = self
                .directories
                .emit(|change| Self::emit_change(change, self.location, &mut self.visit, &mut self.err));
        }
        Ok(Some(outcome))
    }
}

impl<VisitFn, Objects> crate::tree::Visit for Delegate<'_, '_, VisitFn, Objects>
where
    Objects: gix_object::FindObjectOrHeader,
    VisitFn: FnMut(ChangeRef<'_>) -> Result<Action>,
{
    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.recorder.pop_front_tracked_path_and_set_current();
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.recorder.push_back_tracked_path_component(component);
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.recorder.push_path_component(component);
    }

    fn pop_path_component(&mut self) {
        self.recorder.pop_path_component();
    }

    fn visit(&mut self, change: crate::tree::visit::Change) -> crate::tree::visit::Action {
        let change = match self.tracked.as_mut() {
            Some(_) if change.entry_mode().is_tree() => {
                self.directories.push(change.clone(), self.recorder.path());
                if !matches!(change, crate::tree::visit::Change::Modification { .. }) {
                    return std::ops::ControlFlow::Continue(());
                }
                change
            }
            Some(tracked) => match tracked.try_push_change(change, self.recorder.path()) {
                Some(change) => change,
                None => return std::ops::ControlFlow::Continue(()),
            },
            None => change,
        };
        Self::emit_change(
            ChangeRef::from_tree_change(&change, self.recorder.path()),
            self.location,
            &mut self.visit,
            &mut self.err,
        )
    }
}

mod tree_to_changes {
    use bstr::BStr;
    use gix_object::tree::EntryRef;

    use crate::tree::visit::Change;

    pub struct Delegate<'a> {
        push: &'a mut dyn FnMut(Change, &BStr),
        recorder: gix_traverse::tree::Recorder,
    }

    impl<'a> Delegate<'a> {
        pub fn new(push: &'a mut dyn FnMut(Change, &BStr), location: Option<crate::tree::recorder::Location>) -> Self {
            let location = location.map(|t| match t {
                crate::tree::recorder::Location::FileName => gix_traverse::tree::recorder::Location::FileName,
                crate::tree::recorder::Location::Path => gix_traverse::tree::recorder::Location::Path,
            });
            Self {
                push,
                recorder: gix_traverse::tree::Recorder::default().track_location(location),
            }
        }
    }

    impl gix_traverse::tree::Visit for Delegate<'_> {
        fn pop_back_tracked_path_and_set_current(&mut self) {
            self.recorder.pop_back_tracked_path_and_set_current();
        }

        fn pop_front_tracked_path_and_set_current(&mut self) {
            self.recorder.pop_front_tracked_path_and_set_current();
        }

        fn push_back_tracked_path_component(&mut self, component: &BStr) {
            self.recorder.push_back_tracked_path_component(component);
        }

        fn push_path_component(&mut self, component: &BStr) {
            self.recorder.push_path_component(component);
        }

        fn pop_path_component(&mut self) {
            self.recorder.pop_path_component();
        }

        fn visit_tree(&mut self, _entry: &EntryRef<'_>) -> gix_traverse::tree::visit::Action {
            std::ops::ControlFlow::Continue(true)
        }

        fn visit_nontree(&mut self, entry: &EntryRef<'_>) -> gix_traverse::tree::visit::Action {
            (self.push)(
                Change::Modification {
                    previous_entry_mode: entry.mode,
                    previous_oid: entry.oid.to_owned(),
                    entry_mode: entry.mode,
                    oid: entry.oid.to_owned(),
                },
                self.recorder.path(),
            );
            std::ops::ControlFlow::Continue(true)
        }
    }
}
