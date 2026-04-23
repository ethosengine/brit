//! The send-pack "response" side is the [`Report`] — the `report-status`
//! pkt-line stream that the server sends back after applying the push.
//!
//! Re-exported from [`crate::send_pack::types`] for API symmetry with
//! [`crate::fetch::Response`].

pub use crate::send_pack::Report;
