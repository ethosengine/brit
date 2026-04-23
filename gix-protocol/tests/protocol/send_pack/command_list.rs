use bstr::{BString, ByteSlice};
use gix_hash::ObjectId;
use gix_protocol::send_pack::{command_list::encode_into, Command, Request};

fn oid(hex: &str) -> ObjectId {
    // Accepts a short spec; pads with zeros to 40 chars (sha1 tests).
    let mut s = hex.to_string();
    while s.len() < 40 {
        s.push('0');
    }
    ObjectId::from_hex(s.as_bytes()).expect("valid oid")
}

#[test]
fn single_create_with_capabilities() {
    let req = Request {
        commands: vec![Command {
            refname: BString::from("refs/heads/main"),
            old_oid: ObjectId::null(gix_hash::Kind::Sha1),
            new_oid: oid("abcdef"),
        }],
        capabilities: vec![
            BString::from("report-status"),
            BString::from("side-band-64k"),
            BString::from("agent=gix/test"),
        ],
    };
    let mut out = Vec::new();
    encode_into(&req, gix_hash::Kind::Sha1, &mut out).unwrap();

    // Expect one pkt-line (length prefix + payload), then flush (0000).
    // Payload: "<zero-oid> <new-oid> refs/heads/main\0report-status side-band-64k agent=gix/test\n"
    let payload_text = format!(
        "{zero} {new} refs/heads/main\0report-status side-band-64k agent=gix/test\n",
        zero = "0".repeat(40),
        new = "abcdef".to_owned() + &"0".repeat(34),
    );
    let expected_len = 4 + payload_text.len();
    let len_hex = format!("{:04x}", expected_len);

    let mut expected = Vec::new();
    expected.extend_from_slice(len_hex.as_bytes());
    expected.extend_from_slice(payload_text.as_bytes());
    expected.extend_from_slice(b"0000");

    assert_eq!(out, expected, "actual: {:?}", out.as_bstr());
}

#[test]
fn multiple_commands_only_first_carries_caps() {
    let req = Request {
        commands: vec![
            Command {
                refname: BString::from("refs/heads/a"),
                old_oid: ObjectId::null(gix_hash::Kind::Sha1),
                new_oid: oid("aa"),
            },
            Command {
                refname: BString::from("refs/heads/b"),
                old_oid: ObjectId::null(gix_hash::Kind::Sha1),
                new_oid: oid("bb"),
            },
        ],
        capabilities: vec![BString::from("report-status")],
    };
    let mut out = Vec::new();
    encode_into(&req, gix_hash::Kind::Sha1, &mut out).unwrap();
    let s = out.as_bstr();
    // First pkt payload contains NUL + capabilities; second does not.
    assert_eq!(s.iter().filter(|&&b| b == 0).count(), 1);
    assert!(s.ends_with(b"0000"));
}

#[test]
fn delete_only_uses_zero_new_oid() {
    let req = Request {
        commands: vec![Command {
            refname: BString::from("refs/heads/gone"),
            old_oid: oid("dd"),
            new_oid: ObjectId::null(gix_hash::Kind::Sha1),
        }],
        capabilities: vec![BString::from("report-status")],
    };
    let mut out = Vec::new();
    encode_into(&req, gix_hash::Kind::Sha1, &mut out).unwrap();
    assert!(out
        .as_bstr()
        .contains_str(&format!(" {} refs/heads/gone", "0".repeat(40))));
}
