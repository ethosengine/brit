use crate::bstr::BString;

#[cfg(feature = "blocking-network-client")]
pub use super::connection::push::{Error, SignMode};

/// An update resolved from the refspecs and the remote advertisement, before transmission.
#[derive(Debug, Clone)]
pub struct Update {
    /// Local source name, or empty for deletion.
    pub local: BString,
    /// Destination ref name on the remote.
    pub remote: BString,
    /// Advertised destination value, or null for creation.
    pub old_oid: gix_hash::ObjectId,
    /// Resolved source value, or null for deletion.
    pub new_oid: gix_hash::ObjectId,
}

/// Per-ref status from a completed push.
#[derive(Debug, Clone)]
pub struct RefStatus {
    /// The local ref name that was pushed (empty for pure-delete commands).
    pub local: BString,
    /// The remote ref name that was updated.
    pub remote: BString,
    /// `Ok(())` when the ref update succeeded on the server; `Err(reason)` with the
    /// server's rejection message otherwise.
    pub result: Result<(), BString>,
    /// The ref's value on the remote before the push — null when the ref was
    /// newly created. Callers rendering porcelain / status lines use this to
    /// distinguish new-branch (null→value) from update (value→value) and
    /// delete (value→null) cases.
    pub old_oid: gix_hash::ObjectId,
    /// The ref's value after the push — null for delete commands.
    pub new_oid: gix_hash::ObjectId,
}

/// Outcome of a `Prepare::transmit` call.
#[derive(Debug, Clone)]
pub struct Outcome {
    /// Per-ref status in the order the server reported.
    pub status: Vec<RefStatus>,
}
