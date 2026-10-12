/// Errors from `Prepare::transmit` and
/// [`Connection::prepare_push`](crate::remote::Connection::prepare_push).
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
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
    #[error("revision walk: {0}")]
    Walk(String),
    #[error("build pack entries: {0}")]
    Pack(String),
    #[error("send-pack")]
    SendPack(#[from] gix_protocol::send_pack::Error),
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
}
