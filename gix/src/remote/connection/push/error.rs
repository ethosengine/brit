/// Errors from [`super::Prepare::transmit`].
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("parse refspec")]
    ParseRefspec(#[from] gix_refspec::parse::Error),
    #[error("enumerate local refs")]
    References(#[from] crate::reference::iter::Error),
    #[error("initialize local-ref iterator")]
    ReferencesInit(#[from] crate::reference::iter::init::Error),
    #[error("resolve local ref to object id")]
    Resolve(#[from] Box<dyn std::error::Error + Send + Sync + 'static>),
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
}
