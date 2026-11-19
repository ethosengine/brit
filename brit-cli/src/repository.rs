//! Repository opening for build commands.

use std::path::Path;

/// Discover the repository containing `path` for a build command.
///
/// The in-tree `gix` resolves the object-store slot policy only once trust is
/// final (`gix::open::Options::object_store_slots()`): a repository that is
/// fully trusted, directly or through `safe.directory`, sizes its slots from
/// the pack indexes on disk, and one that stays at reduced trust is sized from
/// the index slots it needs plus headroom, up to
/// `gix::open::REDUCED_TRUST_MAX_PACK_INDEXES`. No slot policy is set here,
/// because an explicit value would override that resolution for both trust
/// levels.
pub(crate) fn discover(path: &Path) -> Result<gix::Repository, Error> {
    gix::discover(path).map_err(Error::Discover)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("repository discovery failed")]
    Discover(#[source] gix::Error),
}
