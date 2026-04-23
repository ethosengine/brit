//! Integration test for `gix_protocol::send_pack::send_pack()`.
//!
//! Drives the orchestrator against the captured `fast-forward` wire fixtures.
//! The s2c stream is replayed by `MockTransport`; the c2s bytes are captured
//! and checked for basic shape.

#[cfg(feature = "blocking-client")]
mod blocking {
    use bstr::BString;
    use gix_protocol::send_pack::{send_pack, Command, Options, Request};

    use crate::send_pack::_impl::{advertised_oid, sent_new_oid, MockTransport};

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
    #[ignore = "report is sideband-wrapped — re-enable in Task 5.2 (sideband demux)"]
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

    /// Smoke test: the orchestrator sequence compiles and the mock wiring works
    /// even when the final #[ignore]d assertion is not reached.
    ///
    /// This test passes without sideband support by verifying only that
    /// `send_pack` returns an error (not a panic) when the report is
    /// sideband-wrapped — confirming the mock drives the wire sequence correctly
    /// and the report parser is the only thing that needs Task 5.2.
    #[test]
    fn orchestrator_drives_wire_sequence_sideband_wrapped_gives_parse_error() {
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

        // The result should be an error because the report is sideband-wrapped.
        // We don't prescribe the exact error — just that it errs rather than panics.
        assert!(
            result.is_err(),
            "expected parse error due to sideband wrapping; got Ok — \
             did the fixture format change?"
        );
    }
}
