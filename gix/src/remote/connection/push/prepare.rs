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
    /// When `true`, delete remote refs matching a push spec's RHS that have no
    /// local counterpart — git's MATCH_REFS_PRUNE (transport.c).
    pub(super) prune: bool,
    /// Client-side push options to transmit to the remote. Requires the
    /// remote to advertise the `push-options` capability (git fails with
    /// "the receiving end does not support push options" otherwise).
    pub(super) push_options: Vec<BString>,
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
            prune: false,
            push_options: Vec::new(),
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

    /// When enabled, synthesize delete commands for remote refs that match a push
    /// spec's RHS pattern but have no local counterpart (git's MATCH_REFS_PRUNE).
    pub fn with_prune(mut self, prune: bool) -> Self {
        self.prune = prune;
        self
    }

    /// Attach client-side push options — transmitted to the remote after the
    /// commands list. Requires the server to advertise the `push-options`
    /// capability or transmit will error before sending the pack.
    pub fn with_push_options<I, S>(mut self, options: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<[u8]>,
    {
        self.push_options = options.into_iter().map(|s| BString::from(s.as_ref())).collect();
        self
    }
}
