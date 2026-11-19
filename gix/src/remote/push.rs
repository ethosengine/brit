use crate::bstr::BString;

#[cfg(feature = "blocking-network-client")]
pub use super::connection::push::{Authorized, Delivery, Error, MovedRef, SignMode};

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
    /// Things the caller should tell the user although the push succeeded, such as a lost
    /// report whose updates a fresh handshake verified.
    pub notes: Vec<String>,
}

/// brit: request deadlines for the reqwest HTTP backend, for push, fetch and clone alike.
///
/// reqwest's blocking client bounds a request with one `timeout`: a deadline from the start of
/// the request until the response headers (upload included), then a per-read stall guard on the
/// response body. Its default of 30 s cut off every push whose upload or server-side processing
/// took longer. The rule here:
///
/// | request                                 | deadline to headers | body stall guard |
/// |-----------------------------------------|---------------------|------------------|
/// | handshake GET (fetch and push)          | 60 s                | 60 s per read    |
/// | upload-pack POST (fetch, clone)         | 60 s                | 60 s per read    |
/// | receive-pack POST (the push upload)     | none                | none             |
///
/// The push upload and the server's processing of it are legitimately unbounded, as with Git.
/// Unbounded by design, therefore: the receive-pack response, headers and body; and on macOS
/// and Windows a peer that stops reading during the upload. In exchange a server that computes
/// a fetch for more than 60 s without sending sideband keepalives now fails, where Git waits.
/// `upload_body_unbounded` selects the receive-pack rule: the push re-configures its transport
/// with it after the handshake, so that it only reaches the POST. A request-configuration hook
/// makes the transport refuse to follow redirects, which is why it is not installed for the
/// handshake; with the default `http.followRedirects` only the first request follows them anyway.
/// reqwest takes a request's own timeout over the client's (`blocking/client.rs`,
/// `execute_request`, and the async client's per-request configuration).
///
/// With `http.lowSpeedLimit` and `http.lowSpeedTime` both set, sent data that stays
/// unacknowledged for `lowSpeedTime` seconds aborts the connection (`TCP_USER_TIMEOUT` on Linux,
/// Android and Fuchsia). This is approximate: the byte rate is not measured, and reqwest already
/// sets `TCP_USER_TIMEOUT` to 30 s on Linux, so a `lowSpeedTime` above 30 loosens that bound and
/// one below tightens it. On macOS and Windows nothing bounds a peer that stops reading during
/// the push upload.
#[cfg(all(
    feature = "blocking-http-transport-reqwest",
    not(feature = "blocking-http-transport-curl")
))]
pub(crate) fn reqwest_backend(
    options: &gix_protocol::transport::client::blocking_io::http::Options,
    upload_body_unbounded: bool,
) -> std::sync::Arc<std::sync::Mutex<dyn std::any::Any + Send + Sync + 'static>> {
    use std::time::Duration;

    use gix_protocol::transport::client::blocking_io::http::reqwest::{ConfigureClientFn, ConfigureRequestFn, Options};

    /// The deadline every request gets unless it is the push upload.
    const DEADLINE: Duration = Duration::from_secs(60);
    /// A test and diagnostic knob: seconds to use instead of `DEADLINE`, so that journeys can
    /// show the rule and its exception in seconds. Not a supported configuration.
    const DEADLINE_OVERRIDE: &str = "BRIT_HTTP_REQUEST_DEADLINE_SECONDS";
    /// reqwest has no per-request "never"; a century is one without overflowing an `Instant`.
    const UNBOUNDED: Duration = Duration::from_secs(100 * 365 * 24 * 60 * 60);

    let deadline = std::env::var(DEADLINE_OVERRIDE)
        .ok()
        .and_then(|seconds| seconds.parse().ok())
        .map_or(DEADLINE, Duration::from_secs);
    let stall = (options.low_speed_limit_bytes_per_second > 0 && options.low_speed_time_seconds > 0)
        .then(|| Duration::from_secs(options.low_speed_time_seconds));
    let configure_client: Box<ConfigureClientFn> = Box::new(move |builder| {
        let builder = builder.timeout(deadline);
        #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
        let builder = match stall {
            Some(stall) => builder.tcp_user_timeout(stall),
            None => builder,
        };
        #[cfg(not(any(target_os = "android", target_os = "fuchsia", target_os = "linux")))]
        let _ = stall;
        Ok(builder)
    });
    let backend = Options {
        configure_client: Some(configure_client),
        configure_request: upload_body_unbounded.then(|| {
            let unbounded: Box<ConfigureRequestFn> = Box::new(|request| {
                if request.method().as_str() == "POST" && request.url().path().ends_with("/git-receive-pack") {
                    *request.timeout_mut() = Some(UNBOUNDED);
                }
                Ok(())
            });
            unbounded
        }),
    };
    std::sync::Arc::new(std::sync::Mutex::new(backend))
}
