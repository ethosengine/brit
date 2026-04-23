//! High-level push over an already-established [`Connection`].
//!
//! Mirrors [`super::fetch`]. Flow:
//!
//! 1. [`super::super::Connection::prepare_push`] performs the handshake and
//!    ref-advertisement, then returns a [`Prepare`] builder. (Task 7.2.)
//! 2. The builder accepts refspecs and options.
//! 3. [`Prepare::transmit`] runs the revision walk, builds the pack, and
//!    delegates to [`gix_protocol::send_pack`] to deliver it.
//!
//! Note: [`Prepare`] has no call-site until [`Connection::prepare_push`] is
//! implemented (Task 7.2).  Dead-code lints are suppressed until that task lands.
#![allow(dead_code)]

mod error;
mod prepare;
mod transmit;

pub use error::Error;
pub use prepare::Prepare;
