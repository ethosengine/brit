//! The send-pack "arguments" side is the [`Request`] — the command list +
//! capabilities that the client sends to the server.
//!
//! Re-exported from the internal `crate::send_pack::types` module for API symmetry with
//! [`crate::fetch::Arguments`].

pub use crate::send_pack::Request;
