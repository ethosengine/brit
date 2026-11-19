//! Integration test for `gix_protocol::send_pack::send_pack()`.
//!
//! Drives the orchestrator against the captured `fast-forward` wire fixtures.
//! The s2c stream is replayed by `MockTransport`; the c2s bytes are captured
//! and checked for basic shape.

#[cfg(feature = "blocking-client")]
mod blocking {
    use bstr::BString;
    use gix_protocol::send_pack::{Command, Error, Options, Request, send_pack};

    use crate::send_pack::_impl::{BrokenUploadTransport, MockTransport, advertised_oid, chain, sent_new_oid};

    /// Helper: read the post-handshake portion of the s2c fixture.
    ///
    /// The fixture layout is:
    ///   [ref-advertisement pkt-lines] [flush] [sideband report pkt-line(s)] [flush]
    ///
    /// After handshake the transport cursor is positioned after the ref-adv + flush.
    /// We skip past the first flush so the mock reader starts at the report bytes.
    fn post_handshake_s2c(fixture: &str) -> Vec<u8> {
        let full =
            std::fs::read(std::path::Path::new("tests/fixtures/push").join(fixture)).expect("fixture must exist");
        // Advance past the ref-advertisement block (ends with 0000).
        skip_past_first_flush(&full)
            .expect("fixture has ref-adv flush")
            .to_vec()
    }

    fn skip_past_first_flush(buf: &[u8]) -> Option<&[u8]> {
        let mut i = 0;
        while i + 4 <= buf.len() {
            let len = usize::from_str_radix(std::str::from_utf8(&buf[i..i + 4]).ok()?, 16).ok()?;
            if len == 0 {
                return Some(&buf[i + 4..]);
            }
            if len < 4 || i + len > buf.len() {
                return None;
            }
            i += len;
        }
        None
    }

    /// Full s2c bytes (including ref-adv) — used for `advertised_oid` lookup.
    fn full_s2c(fixture: &str) -> Vec<u8> {
        std::fs::read(std::path::Path::new("tests/fixtures/push").join(fixture)).expect("fixture must exist")
    }

    #[test]
    fn fast_forward_happy_path_against_fixture() {
        // ---- fixture setup ----
        let s2c_full = full_s2c("fast-forward.s2c.bin");
        let s2c_report = post_handshake_s2c("fast-forward.s2c.bin");

        let old_oid = advertised_oid(&s2c_full, b"refs/heads/main").expect("fixture advertises refs/heads/main");

        let c2s = std::fs::read("tests/fixtures/push/fast-forward.c2s.bin").expect("c2s fixture must exist");
        let new_oid = sent_new_oid(&c2s, b"refs/heads/main").expect("fixture sends refs/heads/main update");

        // ---- build request ----
        let req = Request {
            commands: vec![Command {
                refname: BString::from("refs/heads/main"),
                old_oid,
                new_oid,
            }],
            capabilities: vec![BString::from("report-status")],
        };

        // ---- mock transport carrying only the post-handshake bytes ----
        let mut transport = MockTransport::new(s2c_report);

        // We pass an empty pack-entries iterator.  The mock does not verify
        // pack content; the orchestrator will write a 0-entry pack (12-byte
        // header + 20-byte SHA1 trailer), which satisfies `has_updates = true`.
        let outcome = send_pack(
            &mut transport,
            req,
            std::iter::empty(),
            Options::default(),
            gix_hash::Kind::Sha1,
        )
        .expect("send_pack must succeed against fixture");

        // ---- assertions ----
        assert_eq!(outcome.report.unpack, Ok(()), "unpack status");
        assert_eq!(outcome.report.refs.len(), 1, "one ref status expected");
        assert_eq!(
            outcome.report.refs[0].refname,
            BString::from("refs/heads/main"),
            "ref name"
        );
        assert_eq!(outcome.report.refs[0].result, Ok(()), "ref accepted");

        // Verify the client emitted a non-empty c2s stream (command list + pack).
        assert!(
            !transport.captured_bytes().is_empty(),
            "client must have written command list and pack"
        );
    }

    /// Smoke test: the orchestrator sequence compiles, the mock wiring works,
    /// and sideband demux is enabled — confirmed by checking that the result
    /// is `Ok` (not an error) when the fixture uses sideband framing.
    ///
    /// This test was previously `orchestrator_drives_wire_sequence_sideband_wrapped_gives_parse_error`,
    /// which asserted `is_err()` before Task 5.2 added sideband demux.  It now
    /// asserts `is_ok()` to confirm that the fix holds.
    #[test]
    fn orchestrator_drives_wire_sequence_with_sideband_demux() {
        let s2c_full = full_s2c("fast-forward.s2c.bin");
        let s2c_report = post_handshake_s2c("fast-forward.s2c.bin");

        let old_oid = advertised_oid(&s2c_full, b"refs/heads/main").expect("fixture advertises refs/heads/main");

        let c2s = std::fs::read("tests/fixtures/push/fast-forward.c2s.bin").expect("c2s fixture must exist");
        let new_oid = sent_new_oid(&c2s, b"refs/heads/main").expect("fixture sends refs/heads/main update");

        let req = Request {
            commands: vec![Command {
                refname: BString::from("refs/heads/main"),
                old_oid,
                new_oid,
            }],
            capabilities: vec![BString::from("report-status")],
        };

        let mut transport = MockTransport::new(s2c_report);

        let result = send_pack(
            &mut transport,
            req,
            std::iter::empty(),
            Options::default(),
            gix_hash::Kind::Sha1,
        );

        // The orchestrator MUST have written something to the transport
        // (command list + pack) before attempting to read the report.
        assert!(
            !transport.captured_bytes().is_empty(),
            "client must write command list + pack before reading report"
        );

        // With sideband demux enabled, the result must be Ok.
        assert!(
            result.is_ok(),
            "expected Ok after sideband demux; got error: {:?}",
            result.err()
        );
    }

    fn update_request() -> Request {
        Request {
            commands: vec![Command {
                refname: BString::from("refs/heads/main"),
                old_oid: gix_hash::Kind::Sha1.null(),
                new_oid: gix_hash::ObjectId::from_hex(b"1111111111111111111111111111111111111111").expect("hex"),
            }],
            capabilities: vec![BString::from("report-status")],
        }
    }

    /// An HTTP worker that gives up on the upload parks its real error for the reader.
    /// send-pack must collect it rather than report the dropped channel.
    #[test]
    fn broken_upload_reports_the_cause_the_transport_parked() {
        let mut transport =
            BrokenUploadTransport::new(8, Vec::new(), Some(std::io::Error::other("Received HTTP status 413")));
        let err = send_pack(
            &mut transport,
            update_request(),
            std::iter::empty(),
            Options::default(),
            gix_hash::Kind::Sha1,
        )
        .expect_err("the upload broke");
        let chain = chain(&err);
        assert!(matches!(err, Error::SendInterrupted(_)), "{chain:?}");
        assert!(chain.iter().any(|line| line.contains("413")), "{chain:?}");
        assert!(
            !chain.iter().any(|line| line.contains("closed channel")),
            "the dropped channel is not the cause: {chain:?}"
        );
    }

    /// A transport that gives up in the middle of the pack, as an HTTP worker does once the
    /// server answered early or reset the connection: the pack writer must not hide the cause.
    #[test]
    fn upload_broken_mid_pack_reports_the_cause_the_transport_parked() {
        let request = update_request();
        let mut command_list = Vec::new();
        gix_protocol::send_pack::command_list::encode_into(&request, gix_hash::Kind::Sha1, &mut command_list)
            .expect("in-memory encoding");
        let mut entry = gix_pack::data::output::Entry::invalid();
        entry.id = gix_hash::ObjectId::from_hex(b"2222222222222222222222222222222222222222").expect("hex");
        entry.decompressed_size = 64 * 1024;
        entry.compressed_data = vec![7; 64 * 1024];
        // Past the 12-byte pack header, well inside the entry.
        let mut transport = BrokenUploadTransport::new(
            command_list.len() + 12 + 1000,
            Vec::new(),
            Some(std::io::Error::other("Received HTTP status 413")),
        );
        let err = send_pack(
            &mut transport,
            request,
            [entry],
            Options::default(),
            gix_hash::Kind::Sha1,
        )
        .expect_err("the upload broke mid-pack");
        let chain = chain(&err);
        assert!(
            transport.written.len() > command_list.len() + 12,
            "the failure happened inside the pack"
        );
        assert!(matches!(err, Error::SendInterrupted(_)), "{chain:?}");
        assert!(chain.iter().any(|line| line.contains("413")), "{chain:?}");
        assert!(
            !chain.iter().any(|line| line.contains("closed channel")),
            "the dropped channel is not the cause: {chain:?}"
        );
    }

    fn mid_pack_failure(max_buffered_body: usize) -> Error {
        let request = update_request();
        let mut command_list = Vec::new();
        gix_protocol::send_pack::command_list::encode_into(&request, gix_hash::Kind::Sha1, &mut command_list)
            .expect("in-memory encoding");
        let mut entry = gix_pack::data::output::Entry::invalid();
        entry.id = gix_hash::ObjectId::from_hex(b"3333333333333333333333333333333333333333").expect("hex");
        entry.decompressed_size = 64 * 1024;
        entry.compressed_data = vec![9; 64 * 1024];
        let mut transport = BrokenUploadTransport::new(
            command_list.len() + 12 + 1000,
            Vec::new(),
            Some(std::io::Error::other("request or response body error")),
        );
        send_pack(
            &mut transport,
            request,
            [entry],
            Options {
                max_buffered_body: Some(max_buffered_body),
                ..Default::default()
            },
            gix_hash::Kind::Sha1,
        )
        .expect_err("the upload broke mid-pack")
    }

    /// Just under the buffering limit the body goes whole and the transport's cause is reported.
    #[test]
    fn body_under_the_buffering_limit_reports_the_transport_cause() {
        let err = mid_pack_failure(64 * 1024 + 200);
        assert!(matches!(err, Error::SendInterrupted(_)), "{:?}", chain(&err));
    }

    /// Just over it the body is streamed, and the error says the cause could not be read.
    #[test]
    fn body_over_the_buffering_limit_says_the_cause_could_not_be_read() {
        let err = mid_pack_failure(64 * 1024);
        let chain = chain(&err);
        assert!(matches!(err, Error::StreamInterrupted(Some(_))), "{chain:?}");
        assert_eq!(
            chain[0],
            "the transport closed while the pack was being streamed; the cause could not be read"
        );
        assert!(!chain.iter().any(|line| line.contains("closed channel")), "{chain:?}");
    }

    /// With nothing to say, the remote still gets named as the party that closed.
    #[test]
    fn broken_upload_without_a_reason_says_the_connection_closed() {
        let mut transport = BrokenUploadTransport::new(8, Vec::new(), None);
        let err = send_pack(
            &mut transport,
            update_request(),
            std::iter::empty(),
            Options::default(),
            gix_hash::Kind::Sha1,
        )
        .expect_err("the upload broke");
        assert!(matches!(err, Error::ConnectionClosed(_)), "{:?}", chain(&err));
    }

    /// A server that stops reading still has the last word when it sends its report.
    #[test]
    fn broken_upload_returns_a_report_the_remote_still_sent() {
        // Band 1 carries the report, as receive-pack sends it under side-band-64k.
        let report = b"0025\x01001cunpack index-pack abort\n00000000".to_vec();
        let mut transport = BrokenUploadTransport::new(8, report, None);
        let outcome = send_pack(
            &mut transport,
            update_request(),
            std::iter::empty(),
            Options::default(),
            gix_hash::Kind::Sha1,
        )
        .expect("the remote's own report stands");
        assert_eq!(
            outcome.report.unpack,
            Err(BString::from("index-pack abort")),
            "the server's verdict is surfaced"
        );
    }

    /// A response the transport could not read keeps its cause (the held-response symptom).
    #[test]
    fn unreadable_report_keeps_the_transport_cause() {
        let mut transport = BrokenUploadTransport::new(
            usize::MAX,
            Vec::new(),
            Some(std::io::Error::new(std::io::ErrorKind::TimedOut, "operation timed out")),
        );
        let err = send_pack(
            &mut transport,
            update_request(),
            std::iter::empty(),
            Options::default(),
            gix_hash::Kind::Sha1,
        )
        .expect_err("the report could not be read");
        let chain = chain(&err);
        assert!(matches!(err, Error::ReadReport(_)), "{chain:?}");
        assert!(chain.iter().any(|line| line.contains("timed out")), "{chain:?}");
        assert!(
            !chain.iter().any(|line| line.contains("unexpected end of stream")),
            "{chain:?}"
        );
    }
}
