//! Repository opening for build commands using the crates.io `gix` dependency.

use std::path::Path;

use gix::{
    odb::store::init::{Options as StoreOptions, Slots},
    sec::Trust,
};

const REDUCED_TRUST_SLOTS: usize = 32;

/// Options applied to the first and only repository discovery.
pub(crate) struct InitialOpenOptions {
    open: fn(
        &Path,
        gix::sec::trust::Mapping<gix::open::Options>,
    ) -> Result<gix::ThreadSafeRepository, gix::discover::Error>,
}

impl Default for InitialOpenOptions {
    fn default() -> Self {
        Self {
            open: |path, trust_map| gix::ThreadSafeRepository::discover_opts(path, Default::default(), trust_map),
        }
    }
}

impl InitialOpenOptions {
    #[allow(
        dead_code,
        reason = "test seam: only tests inject an initial open; production uses the default discovery"
    )]
    pub(crate) fn open(
        mut self,
        open: fn(
            &Path,
            gix::sec::trust::Mapping<gix::open::Options>,
        ) -> Result<gix::ThreadSafeRepository, gix::discover::Error>,
    ) -> Self {
        self.open = open;
        self
    }
}

/// Discover a repository once with a capped object store, then follow its final trust.
///
/// `gix` 0.81 chooses the object-store policy before repository configuration
/// can change the initial trust. Capping both initial policies prevents a
/// Full-to-Reduced transition from retaining a disk-sized store. A repository
/// whose final trust is Full gets a disk-sized replacement below.
#[allow(
    clippy::result_large_err,
    reason = "gix 0.81's discover::Error is large and is carried by value to keep its source chain typed"
)]
pub(crate) fn discover(path: &Path, initial: InitialOpenOptions) -> Result<gix::Repository, Error> {
    let mut trust_map: gix::sec::trust::Mapping<gix::open::Options> = Default::default();
    let initial_slots = Slots::Given(REDUCED_TRUST_SLOTS as u16);
    trust_map
        .full
        .modify(|options| options.object_store_slots(initial_slots));
    trust_map
        .reduced
        .modify(|options| options.object_store_slots(initial_slots));

    let repo = (initial.open)(path, trust_map)
        .map(|repo| repo.to_thread_local())
        .map_err(Error::Discover)?;
    correct_object_store_slots(repo)
}

/// Replace only a promoted repository's reduced-cap object database.
///
/// The replacement handle is assigned directly to `Repository.objects`; refs,
/// worktree binding, configuration and all other repository state stay in the
/// same value. The new store uses the original objects directory, current
/// directory, hash, multi-pack-index policy and replacement object mappings.
/// Alternates are therefore resolved from the same source.
#[allow(
    clippy::result_large_err,
    reason = "shares discover()'s Error type, whose gix 0.81 variant is large"
)]
pub(crate) fn correct_object_store_slots(repo: gix::Repository) -> Result<gix::Repository, Error> {
    if repo.git_dir_trust() == Trust::Reduced {
        return Ok(repo);
    }

    let (objects_dir, current_dir, object_hash, use_multi_pack_index, replacements) = {
        let store = repo.objects.store_ref();
        (
            store.path().to_owned(),
            repo.current_dir().to_owned(),
            store.object_hash(),
            store.use_multi_pack_index(),
            store.replacements().collect::<Vec<_>>(),
        )
    };
    let objects = gix::odb::at_opts(
        objects_dir,
        replacements,
        StoreOptions {
            slots: Slots::AsNeededByDiskState {
                multiplier: 1.1,
                minimum: REDUCED_TRUST_SLOTS,
            },
            object_hash,
            use_multi_pack_index,
            current_dir: Some(current_dir),
        },
    )
    .map_err(Error::ObjectDatabase)?;

    let mut repo = repo;
    repo.objects = gix::odb::memory::Proxy::from(objects).with_write_passthrough();
    Ok(repo)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("repository discovery failed")]
    Discover(#[source] gix::discover::Error),
    #[error("could not rebuild repository object database")]
    ObjectDatabase(#[source] std::io::Error),
}
