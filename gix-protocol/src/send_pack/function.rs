//! The send-pack orchestrator: given a post-handshake Transport and a
//! Request + pack entries, drive the exchange and return the parsed report.
//!
//! Reference: `vendor/git/send-pack.c` (function `send_pack`, ~line 560).
//! Sequence:
//!   1. Write command list (pkt-line frames, flush).
//!   2. If any command is a non-delete, write the pack.
//!   3. Read and parse the report-status response.

#[cfg(feature = "blocking-client")]
pub use blocking::send_pack;

#[cfg(feature = "blocking-client")]
mod blocking {
    use gix_transport::client::{blocking_io::Transport, MessageKind, WriteMode};

    use crate::send_pack::{
        command_list::encode_into, pack_writer::write_pack, report::parse as parse_report, Error, Options, Outcome,
        Request,
    };

    /// Drive one send-pack exchange over `transport`.
    ///
    /// Preconditions: `transport` must already have completed the `receive-pack`
    /// handshake and the caller must have consumed the server's ref advertisement.
    ///
    /// Sequence:
    ///   1. Write the command list (pkt-line frames + trailing flush-pkt).
    ///   2. If any command is a non-delete, write the pack data.
    ///   3. Flush the writer and turn it into a reader.
    ///   4. Read and parse the `report-status` pkt-line stream.
    pub fn send_pack<T, I>(
        transport: &mut T,
        request: Request,
        pack_entries: I,
        _options: Options,
        hash_kind: gix_hash::Kind,
    ) -> Result<Outcome, Error>
    where
        T: Transport,
        I: IntoIterator<Item = gix_pack::data::output::Entry>,
    {
        let has_updates = request.commands.iter().any(|c| !c.is_delete());

        // Acquire the request writer.  `WriteMode::Binary` passes writes
        // verbatim (no per-write pkt-line framing); we drive the framing
        // ourselves via `encode_into` / `write_pack`.
        // `MessageKind::Flush` causes `into_read()` to emit a trailing flush
        // before switching to the read side — consistent with git's send-pack
        // which flushes after writing the command list + pack.
        let mut writer = transport.request(WriteMode::Binary, MessageKind::Flush, false)?;

        // Step 1 — command list + flush-pkt.
        encode_into(&request, hash_kind, &mut writer)?;

        // Step 2 — pack (only when there is at least one non-delete command).
        if has_updates {
            write_pack(pack_entries, hash_kind, &mut writer)?;
        }

        // Step 3 — flush and switch to read side.
        // `into_read()` writes the `on_into_read` message (Flush) and flushes
        // the underlying write channel before returning the reader.
        let mut reader = writer.into_read()?;

        // Step 4 — parse the report-status stream.
        let report = parse_report(&mut reader)?;
        Ok(Outcome { report })
    }
}
