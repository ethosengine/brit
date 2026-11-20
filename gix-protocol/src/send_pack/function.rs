//! The send-pack orchestrator: given a post-handshake Transport and a
//! Request + pack entries, drive the exchange and return the parsed report.
//!
//! Reference: `vendor/git/send-pack.c` (function `send_pack`, ~line 560).
//! Sequence:
//!   1. Acquire a request handle and immediately dissolve it into (raw_writer, reader).
//!   2. Write the command list as pkt-line frames directly to raw_writer.
//!   3. If any command is a non-delete, write the pack as raw bytes to raw_writer.
//!   4. Drop raw_writer so the transport's read path is unblocked.
//!   5. Reset the reader; enable sideband demux.
//!   6. Read and parse the report-status response — also after a failed write,
//!      because the read side is where the transport reports why it stopped.

#[cfg(feature = "blocking-client")]
pub use blocking::{DEFAULT_MAX_BUFFERED_BODY, send_pack};

#[cfg(feature = "blocking-client")]
mod blocking {
    /// How large a push body a stateless transport sends whole, unless
    /// [`Options::max_buffered_body`](crate::send_pack::Options::max_buffered_body) says otherwise.
    ///
    /// 64 MiB: brit's packs are undeltified, so ordinary landings run to a few MiB and always
    /// get the informative error path, while the extra copy stays below about 90 MB even at the
    /// limit, well under any workspace memory guard.
    pub const DEFAULT_MAX_BUFFERED_BODY: usize = 64 * 1024 * 1024;

    /// An upper bound of the pack's size on the wire: header, trailer, and each entry's
    /// compressed bytes plus its largest possible header.
    fn body_size_bound(entries: &[gix_pack::data::output::Entry]) -> usize {
        entries
            .iter()
            .fold(12 + 64, |size, entry| size + entry.compressed_data.len() + 48)
    }

    /// Pass writes through to `inner`, keeping the first error it raised in `first`.
    struct FirstError<'a, W: std::io::Write + ?Sized> {
        inner: &'a mut W,
        first: &'a mut Option<std::io::Error>,
    }

    impl<W: std::io::Write + ?Sized> FirstError<'_, W> {
        fn keep(&mut self, err: std::io::Error) -> std::io::Error {
            let echo = std::io::Error::new(err.kind(), err.to_string());
            self.first.get_or_insert(err);
            echo
        }
    }

    impl<W: std::io::Write + ?Sized> std::io::Write for FirstError<'_, W> {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.inner.write(buf).map_err(|err| self.keep(err))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.inner.flush().map_err(|err| self.keep(err))
        }
    }

    use gix_transport::client::{MessageKind, WriteMode, blocking_io::Transport};

    use crate::send_pack::{
        Error, Options, Outcome, Request, command_list::encode_into, pack_writer::write_pack,
        report::parse as parse_report, response::blocking_io::enable_sideband_demux,
    };

    /// Drive one send-pack exchange over `transport`.
    ///
    /// Preconditions: `transport` must already have completed the `receive-pack`
    /// handshake and the caller must have consumed the server's ref advertisement.
    ///
    /// Sequence:
    ///   1. Acquire a request handle from the transport.
    ///   2. Immediately dissolve it via `into_parts()` into `(raw_writer, reader)`.
    ///      `raw_writer` writes bytes verbatim to the wire (no pkt-line framing);
    ///      `reader` reads from the server's response stream.
    ///   3. Write the command list as pkt-line frames through `encode_into`, which
    ///      calls `data_to_write` / `flush_to_write` directly on `raw_writer`.
    ///   4. If any command is a non-delete, write the PACK as raw bytes through
    ///      `write_pack` on `raw_writer`.
    ///   5. Flush and drop `raw_writer` before switching to reads.
    ///   6. Reset the reader past any stop-state; enable sideband demux.
    ///   7. Parse the `report-status` pkt-line stream.
    ///
    /// # Why `into_parts()` immediately?
    ///
    /// `RequestWriter::write()` routes through an internal `Writer` that adds
    /// pkt-line framing to every `write()` call.  `encode_into` already applies
    /// pkt-line framing itself (via `data_to_write`); routing through a second
    /// layer would double-frame the command bytes on the wire.
    ///
    /// `into_parts()` extracts the verbatim inner writer without emitting any
    /// additional protocol message (unlike `into_read()` which writes the
    /// `on_into_read` sentinel, adding a spurious `0000` before the pack).
    pub fn send_pack<T, I>(
        transport: &mut T,
        request: Request,
        pack_entries: I,
        options: Options,
        hash_kind: gix_hash::Kind,
    ) -> Result<Outcome, Error>
    where
        T: Transport,
        I: IntoIterator<Item = gix_pack::data::output::Entry>,
    {
        let has_updates = request.commands.iter().any(|c| !c.is_delete());

        // Acquire a request handle and immediately dissolve it.
        //
        // We never use the `Writer`'s framing or the `on_into_read` sentinel because
        // we call `into_parts()` before any data is written; `MessageKind::Flush` is a
        // placeholder, and the write mode only tells a stateless transport how to send the body.
        //
        // `into_parts()` returns (verbatim_writer, reader):
        //   - verbatim_writer: writes bytes straight to the socket, no framing.
        //   - reader: reads from the server's stdout (a `WithSidebands` wrapping
        //     a `StreamingPeekableIter` backed by the child process's stdout).
        //
        // A stateless (HTTP) transport gets a body up to `options.max_buffered_body` whole rather
        // than streamed. A streamed body hides why an upload failed: reqwest's blocking client
        // then reports only that its body forwarder lost its receiver, without reading the
        // response or the connection error that says why (status, reset, timeout). Buffering
        // costs another copy of the pack, so larger bodies are streamed, chunked, as before.
        let pack_entries: Vec<_> = pack_entries.into_iter().collect();
        let streamed = transport.connection_persists_across_multiple_requests()
            || body_size_bound(&pack_entries) > options.max_buffered_body.unwrap_or(DEFAULT_MAX_BUFFERED_BODY);
        let write_mode = if streamed {
            WriteMode::Binary
        } else {
            WriteMode::OneLfTerminatedLinePerWriteCall
        };
        let stateless_stream = streamed && !transport.connection_persists_across_multiple_requests();
        let (mut raw_writer, mut reader) = transport.request(write_mode, MessageKind::Flush, false)?.into_parts();

        // Step 1 — command list.
        //
        // `encode_into` calls `data_to_write` and `flush_to_write` directly on
        // `raw_writer`, adding exactly one layer of pkt-line framing for each
        // command and writing the mandatory `0000` flush separator.
        //
        // Step 2 — pack (only when there is at least one non-delete command).
        //
        // `write_pack` writes raw PACK bytes.  No pkt-line framing must be added
        // here — git-receive-pack reads the pack stream verbatim after the `0000`.
        //
        // Step 3 — flush and drop the write handle.
        //
        // Flush to ensure all bytes (command list + pack) reach git-receive-pack
        // before we switch to reading its response.  Drop immediately to avoid
        // deadlocking on process-spawned transports where the write and read
        // channels share a single pipe pair.
        use std::io::Write as _;
        // The pack writer reports a failed write as its own error, which loses the
        // transport's error kind; keep the first error the transport itself raised.
        let mut first_error = None;
        let mut sink = FirstError {
            inner: &mut raw_writer,
            first: &mut first_error,
        };
        let written = encode_into(&request, hash_kind, &mut sink)
            .and_then(|()| {
                if has_updates {
                    write_pack(pack_entries, hash_kind, &mut sink).map(drop)
                } else {
                    Ok(())
                }
            })
            .and_then(|()| sink.flush())
            .map_err(|err| first_error.take().unwrap_or(err));
        drop(raw_writer);
        // Only a write the other side refused leaves an answer to read. Any other failure
        // may have left the remote waiting for the rest of the pack, so reading would hang.
        let written = match written {
            Err(err)
                if !matches!(
                    err.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::ConnectionAborted
                ) =>
            {
                return Err(Error::Io(err));
            }
            written => written,
        };

        // Step 4 — reset the reader past the handshake stop-state.
        //
        // After the handshake the underlying `StreamingPeekableIter` may have
        // stopped at the flush-pkt that terminated the ref advertisement.
        // `ExtendedBufRead::reset()` clears the `is_done` flag so subsequent
        // reads proceed.
        reader.reset(crate::transport::Protocol::V0);

        // Step 5 — enable sideband demux.
        //
        // git-receive-pack wraps the report-status stream in sideband band-1
        // frames when `side-band` / `side-band-64k` is in effect.  Setting a
        // progress handler switches `WithSidebands::fill_buf` into band-decode
        // mode: band-1 bytes are yielded as data; band-2/3 are discarded.
        enable_sideband_demux(&mut *reader);

        // Step 6 — parse the report-status stream.
        //
        // A broken upload is only a symptom: with the writer gone, the read side yields
        // what the transport knows. An HTTP worker parks its request error (status,
        // timeout, refused or reset connection) for the reader; a process transport
        // leaves the remote's last words on stdout. Prefer the remote's own report.
        match (written, parse_report(&mut reader)) {
            (_, Ok(report)) => Ok(Outcome { report }),
            (Ok(()), Err(err)) => Err(err),
            // A streamed HTTP body loses its cause inside the client; say so rather than
            // passing the dropped channel off as the reason.
            (Err(_), Err(Error::ReadReport(cause))) if stateless_stream => Err(Error::StreamInterrupted(Some(cause))),
            (Err(_), Err(Error::UnexpectedEof)) if stateless_stream => Err(Error::StreamInterrupted(None)),
            (Err(write), Err(Error::UnexpectedEof)) => Err(Error::ConnectionClosed(write)),
            (Err(_), Err(Error::ReadReport(cause))) => Err(Error::SendInterrupted(cause)),
            (Err(_), Err(err)) => Err(err),
        }
    }
}
