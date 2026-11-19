use gix_transport::client::blocking_io::Transport;

use crate::{bstr::BString, remote::Connection};

/// A push in preparation — configure refspecs, then call [`Prepare::transmit`].
///
/// Obtain an instance via [`Connection::prepare_push`] (Task 7.2).
pub struct Prepare<'remote, 'auth, 'repo, T>
where
    T: Transport,
{
    /// The established (post-handshake) connection.
    pub(super) connection: Connection<'remote, 'auth, 'repo, T>,
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
    /// GPG-sign the push: [`SignMode::Never`] (the default) never signs,
    /// [`SignMode::IfAsked`] signs only when the server advertised the
    /// `push-cert` capability, [`SignMode::Always`] fails if it didn't.
    pub(super) sign: SignMode,
}

/// `--signed` argument — mirrors git's `SEND_PACK_PUSH_CERT_*` states.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum SignMode {
    /// `--signed=no` / `--signed=false` / `--signed=off` / `--signed=0`.
    #[default]
    Never,
    /// `--signed=if-asked`.
    IfAsked,
    /// `--signed=yes` / `--signed=true` / `--signed=on` / `--signed=1`.
    Always,
}

/// Builder
impl<'remote, 'auth, 'repo, T> Prepare<'remote, 'auth, 'repo, T>
where
    T: Transport,
{
    /// Create a `Prepare` directly from a post-handshake `Connection`.
    ///
    /// Callers should prefer [`Connection::prepare_push`] once Task 7.2 is complete.
    /// This constructor is exposed for tests that bypass Task 7.2.
    pub fn new(connection: Connection<'remote, 'auth, 'repo, T>) -> Self {
        Prepare {
            connection,
            refspecs: Vec::new(),
            dry_run: false,
            prune: false,
            push_options: Vec::new(),
            sign: SignMode::Never,
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

    /// Configure `--signed` behavior: [`SignMode::Always`] fails at transmit
    /// when the server didn't advertise `push-cert`; [`SignMode::IfAsked`]
    /// silently falls back to unsigned; [`SignMode::Never`] is a no-op.
    pub fn with_sign_mode(mut self, mode: SignMode) -> Self {
        self.sign = mode;
        self
    }
}

/// The receive-pack capabilities a push relies on, as one advertisement announced them.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Contract {
    report_status: bool,
    side_band_64k: bool,
    ofs_delta: bool,
    atomic: bool,
    push_options: bool,
    push_cert: bool,
    object_format: Option<BString>,
}

impl Contract {
    pub(super) fn of(handshake: Option<&gix_protocol::Handshake>) -> Self {
        let has = |name: &str| handshake.is_some_and(|h| h.capabilities.contains(name));
        Contract {
            report_status: has("report-status"),
            side_band_64k: has("side-band-64k"),
            ofs_delta: has("ofs-delta"),
            atomic: has("atomic"),
            push_options: has("push-options"),
            push_cert: has("push-cert"),
            object_format: handshake
                .and_then(|h| h.capabilities.capability("object-format"))
                .and_then(|cap| cap.value().map(BString::from)),
        }
    }

    /// The name of the first capability `now` announces differently, if any.
    pub(super) fn first_difference(&self, now: &Contract) -> Option<&'static str> {
        [
            ("report-status", self.report_status == now.report_status),
            ("side-band-64k", self.side_band_64k == now.side_band_64k),
            ("ofs-delta", self.ofs_delta == now.ofs_delta),
            ("atomic", self.atomic == now.atomic),
            ("push-options", self.push_options == now.push_options),
            ("push-cert", self.push_cert == now.push_cert),
            ("object-format", self.object_format == now.object_format),
        ]
        .into_iter()
        .find_map(|(name, same)| (!same).then_some(name))
    }
}
