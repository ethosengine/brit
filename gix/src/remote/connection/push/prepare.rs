use gix_transport::client::blocking_io::Transport;

use crate::{bstr::BString, remote::Connection};

/// A push in preparation — configure refspecs, then call [`Prepare::transmit`].
///
/// Obtain an instance via [`Connection::prepare_push`] (Task 7.2).
pub struct Prepare<'remote, 'repo, T>
where
    T: Transport,
{
    /// The established (post-handshake) connection.
    pub(super) connection: Connection<'remote, 'repo, T>,
    /// Refspecs, expressed in the push direction (e.g. `refs/heads/main:refs/heads/main`).
    pub(super) refspecs: Vec<BString>,
    /// When `true`, skip the actual wire exchange (useful for `--dry-run`).
    pub(super) dry_run: bool,
}

/// Builder
impl<'remote, 'repo, T> Prepare<'remote, 'repo, T>
where
    T: Transport,
{
    /// Create a `Prepare` directly from a post-handshake `Connection`.
    ///
    /// Callers should prefer [`Connection::prepare_push`] once Task 7.2 is complete.
    /// This constructor is exposed for tests that bypass Task 7.2.
    pub fn new(connection: Connection<'remote, 'repo, T>) -> Self {
        Prepare {
            connection,
            refspecs: Vec::new(),
            dry_run: false,
        }
    }

    /// Set the refspecs that define what to push.
    ///
    /// Refspecs should be push-direction (e.g. `refs/heads/main:refs/heads/main`).
    pub fn with_refspecs<I, S>(mut self, specs: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<[u8]>,
    {
        self.refspecs = specs.into_iter().map(|s| BString::from(s.as_ref())).collect();
        self
    }

    /// When enabled, perform all steps (ref matching, pack enumeration) but skip the wire exchange.
    pub fn with_dry_run(mut self, dry: bool) -> Self {
        self.dry_run = dry;
        self
    }
}
