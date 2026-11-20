/// Errors from `Prepare::transmit` and
/// [`Connection::prepare_push`](crate::remote::Connection::prepare_push).
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs, reason = "variant names and messages document each failure")]
pub enum Error {
    #[error("push rejected before transmission")]
    BeforeSend(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    // ── prepare_push (handshake) errors ───────────────────────────────────
    #[error("failed to configure the transport before connecting to {url:?}")]
    GatherTransportConfig {
        url: crate::bstr::BString,
        source: gix_error::Error,
    },
    #[error("failed to configure the transport layer")]
    ConfigureTransport(#[from] gix_error::Error),
    #[error(transparent)]
    Handshake(gix_error::Error),
    #[error(transparent)]
    ConfigureCredentials(gix_error::Error),
    // ── transmit errors ───────────────────────────────────────────────────
    #[error("parse refspec")]
    ParseRefspec(#[source] gix_error::Error),
    #[error("enumerate local refs")]
    References(#[source] gix_error::Error),
    #[error("initialize local-ref iterator")]
    ReferencesInit(#[source] gix_error::Error),
    #[error("resolve local ref to object id")]
    Resolve(#[source] Box<dyn std::error::Error + Send + Sync + 'static>),
    #[error("revision walk")]
    Walk(#[source] gix_error::Error),
    #[error("build pack entries")]
    Pack(#[source] gix_error::Error),
    #[error("read object {id} while building push pack")]
    ReadObject {
        id: gix_hash::ObjectId,
        #[source]
        source: gix_error::Error,
    },
    #[error("send-pack")]
    SendPack(#[from] gix_protocol::send_pack::Error),
    /// Sending failed; a fresh connection then checked whether the updates arrived anyway.
    #[error("send-pack failed: {delivery}")]
    SendFailed {
        delivery: super::Delivery,
        #[source]
        source: gix_protocol::send_pack::Error,
    },
    #[error("could not connect to the remote")]
    Connect(#[source] gix_error::Error),
    #[error("could not reconnect to the remote after the before-send callback ran; nothing was sent")]
    Reconnect(#[source] Box<Error>),
    #[error("the remote refused authentication when reconnecting after the before-send callback ran; nothing was sent")]
    ReconnectAuthentication(#[source] Box<Error>),
    #[error(
        "remote moved while the before-send callback ran; nothing was sent: {}",
        super::MovedRef::list(moved)
    )]
    RemoteMoved { moved: Vec<super::MovedRef> },
    #[error("the remote's `{capability}` capability changed while the before-send callback ran; nothing was sent")]
    CapabilitiesChanged { capability: &'static str },
    #[error("no refspecs given — nothing to push")]
    NoRefspecs,
    #[error("push destination conflict: {0}")]
    RefspecConflict(#[from] gix_refspec::match_group::validate::Error),
    #[error("refspec matched no local refs: {spec:?}")]
    NoMatch { spec: crate::bstr::BString },
    #[error("the receiving end does not support push options")]
    PushOptionsNotSupported,
    #[error("the receiving end does not support --signed push")]
    SignedPushNotSupported,
    #[error("the receiving end does not support --atomic push")]
    AtomicNotSupported,
}
