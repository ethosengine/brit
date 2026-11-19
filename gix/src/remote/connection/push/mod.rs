//! High-level push over an already-established [`Connection`].
//!
//! Mirrors [`super::fetch`]. Flow:
//!
//! 1. [`super::super::Connection::prepare_push`] performs the handshake and
//!    ref-advertisement, then returns a `Prepare` builder.
//! 2. The builder accepts refspecs and options.
//! 3. [`Prepare::transmit`] runs the revision walk, builds the pack, and
//!    delegates to [`gix_protocol::send_pack`] to deliver it.

mod error;
mod prepare;
mod transmit;

pub use error::Error;
pub use prepare::{Prepare, SignMode};

/// What a `before_send` callback tells
/// [`Prepare::transmit_with_before_send()`](Prepare::transmit_with_before_send()) about the
/// connection it held while authorizing the push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Authorized {
    /// The callback judged the updates from local state and returned at once: send on the
    /// connection whose advertisement it judged.
    OnThisConnection,
    /// The callback may have taken a while or run other programs, as a `pre-push` hook does.
    /// Its connection may have gone stale and the remote may have moved since: drop it, connect
    /// again with [`Remote::connect()`](crate::Remote::connect()), and send only if every
    /// remote ref still has the id the callback judged.
    Revalidate,
}

/// The conservative default: anything that returns `()` may have waited.
impl From<()> for Authorized {
    fn from((): ()) -> Self {
        Authorized::Revalidate
    }
}

/// A remote ref whose id differs from the one a push was based on or aimed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedRef {
    /// The remote ref name.
    pub refname: crate::bstr::BString,
    /// The id the push expected: the judged old id, or the new id after a failed send.
    pub expected: gix_hash::ObjectId,
    /// The id the remote advertised instead; null if the ref is absent.
    pub found: gix_hash::ObjectId,
}

impl MovedRef {
    fn id(id: &gix_hash::ObjectId) -> String {
        if id.is_null() { "absent".into() } else { id.to_string() }
    }

    pub(crate) fn list(refs: &[MovedRef]) -> String {
        refs.iter()
            .map(|moved| {
                format!(
                    "{} was {} when pre-push ran, now {}",
                    moved.refname,
                    Self::id(&moved.expected),
                    Self::id(&moved.found)
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }
}

/// What a fresh connection showed after a send failed.
#[derive(Debug)]
pub enum Delivery {
    /// Every remote ref has its new id: the push arrived and only its report was lost.
    Delivered,
    /// These remote refs do not have their new id; `expected` is the new id. Nothing was resent.
    NotDelivered(Vec<MovedRef>),
    /// The check itself failed, so whether the push arrived is unknown.
    Unverified(Box<Error>),
}

impl Delivery {
    pub(crate) fn judge(
        commands: &[gix_protocol::send_pack::Command],
        now: &std::collections::HashMap<crate::bstr::BString, gix_hash::ObjectId>,
        null: gix_hash::ObjectId,
    ) -> Self {
        let missing: Vec<_> = commands
            .iter()
            .filter_map(|command| {
                let found = now.get(&command.refname).copied().unwrap_or(null);
                (found != command.new_oid).then(|| MovedRef {
                    refname: command.refname.clone(),
                    expected: command.new_oid,
                    found,
                })
            })
            .collect();
        if missing.is_empty() {
            Delivery::Delivered
        } else {
            Delivery::NotDelivered(missing)
        }
    }
}

impl std::fmt::Display for Delivery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Delivery::Delivered => f.write_str(
                "the push was delivered, but its report was lost; the remote has every update, nothing was resent",
            ),
            Delivery::NotDelivered(missing) => {
                f.write_str("the push was not delivered, and nothing was resent: ")?;
                let listed = missing
                    .iter()
                    .map(|moved| {
                        format!(
                            "{} is {}, not {}",
                            moved.refname,
                            MovedRef::id(&moved.found),
                            MovedRef::id(&moved.expected)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                f.write_str(&listed)
            }
            Delivery::Unverified(err) => {
                f.write_str("whether the push was delivered could not be checked (")?;
                let mut next: Option<&(dyn std::error::Error + 'static)> = Some(err.as_ref());
                let mut first = true;
                while let Some(err) = next {
                    if !first {
                        f.write_str(": ")?;
                    }
                    first = false;
                    write!(f, "{err}")?;
                    next = err.source();
                }
                f.write_str("); nothing was resent")
            }
        }
    }
}

use gix_transport::client::blocking_io::Transport;

use crate::remote::{Connection, Direction};

impl<'remote, 'auth, 'repo, T> Connection<'remote, 'auth, 'repo, T>
where
    T: Transport,
{
    /// Perform a `ReceivePack` handshake with the remote and return a `Prepare` builder.
    ///
    /// For V0/V1 connections (the common case for `file://`, `ssh://`, and `git://` push),
    /// the ref advertisement is returned inline during the handshake.  No separate
    /// `ls-refs` round-trip is required for `receive-pack`.
    ///
    /// # Usage
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let repo = gix::open(".")?;
    /// # let remote = repo.find_remote("origin")?;
    /// let outcome = remote
    ///     .connect(gix::remote::Direction::Push)?
    ///     .prepare_push(gix::progress::Discard)?
    ///     .with_refspecs(["refs/heads/main:refs/heads/main"])
    ///     .transmit(gix::progress::Discard, &Default::default())?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn prepare_push(
        mut self,
        mut progress: impl crate::progress::Progress,
    ) -> Result<Prepare<'remote, 'auth, 'repo, T>, Error> {
        let _span = gix_trace::coarse!("remote::Connection::prepare_push()");

        // Resolve transport options from repository config if not already set.
        // Convert to owned early so the immutable borrow of `self.transport.inner`
        // is released before the mutable borrow needed by `configure()`.
        let url_owned: crate::bstr::BString = self.transport.inner.to_url().as_ref().into();
        if self.transport_options.is_none() {
            self.transport_options = self
                .remote
                .repo
                .transport_options(
                    crate::bstr::ByteSlice::as_bstr(url_owned.as_slice()),
                    self.remote.name().map(crate::remote::Name::as_bstr),
                )
                .map_err(|err| Error::GatherTransportConfig {
                    source: err,
                    url: url_owned.clone(),
                })?;
        }
        if let Some(config) = self.transport_options.as_ref() {
            self.transport.inner.configure(&**config)?;
        }

        // Build the credentials callback the same way `ref_map_by_ref` does.
        let mut credentials_storage;
        let authenticate: &mut dyn FnMut(
            gix_credentials::helper::Action,
        ) -> gix_error::Result<Option<gix_credentials::protocol::Outcome>> = match self.authenticate.as_mut() {
            Some(f) => f,
            None => {
                let url = self.remote.url(Direction::Push).map_or_else(
                    || {
                        gix_url::parse(crate::bstr::ByteSlice::as_bstr(url_owned.as_slice()))
                            .expect("valid URL to be provided by transport")
                    },
                    ToOwned::to_owned,
                );
                credentials_storage = self.configured_credentials(url).map_err(Error::ConfigureCredentials)?;
                &mut credentials_storage
            }
        };

        // Perform the handshake for `receive-pack`.
        // V0/V1: server advertises refs inline.
        // V2: receive-pack hasn't been extended to V2; we'll always negotiate down.
        let handshake = gix_protocol::handshake(
            &mut self.transport.inner,
            gix_transport::Service::ReceivePack,
            authenticate,
            Vec::new(), // no extra handshake parameters for push
            &mut progress,
        )
        .map_err(Error::Handshake)?;

        self.handshake = Some(handshake);
        Ok(Prepare::new(self))
    }
}
