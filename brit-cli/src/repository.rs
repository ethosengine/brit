//! Repository opening for build commands.

use std::path::Path;

/// Discover the repository containing `path` for a build command.
///
/// The in-tree `gix` resolves the object-store slot policy only once trust is
/// final (`gix::open::Options::object_store_slots()`): a repository that
/// `safe.directory` promotes to Full trust sizes its slots from the pack
/// indexes on disk, and one that stays at Reduced trust keeps the 32-slot cap.
/// No slot policy is set here, because an explicit value would override that
/// resolution for both trust levels.
pub(crate) fn discover(path: &Path) -> Result<gix::Repository, Error> {
    gix::discover(path).map_err(Error::Discover)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("repository discovery failed")]
    Discover(#[source] gix::Error),
}
