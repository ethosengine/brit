//! Encode the send-pack command list into pkt-line frames.
//!
//! Grammar (ABNF, from `Documentation/gitprotocol-pack.adoc`):
//!
//! ```text
//! update-request  =  *shallow command-list [pack-file]
//! command-list    =  PKT-LINE(command NUL capability-list LF)
//!                    *PKT-LINE(command LF)
//!                    flush-pkt
//! command         =  create / delete / update
//! create          =  zero-id SP new-id SP name
//! delete          =  old-id SP zero-id SP name
//! update          =  old-id SP new-id SP name
//! ```

use std::io::Write;

use crate::send_pack::Request;

/// Write the command list + trailing flush-pkt onto `out`.
///
/// `hash_kind` determines the zero-OID width (40 hex chars for sha1, 64 for
/// sha256) so encoding stays parametric across the workspace-wide hash scope.
pub fn encode_into(request: &Request, hash_kind: gix_hash::Kind, out: &mut impl Write) -> std::io::Result<()> {
    let hex_len = hash_kind.len_in_hex();

    for (i, cmd) in request.commands.iter().enumerate() {
        let mut payload = Vec::with_capacity(hex_len * 2 + 3 + cmd.refname.len() + 64);
        // old-id SP new-id SP name
        write_oid_hex(&mut payload, &cmd.old_oid, hex_len)?;
        payload.push(b' ');
        write_oid_hex(&mut payload, &cmd.new_oid, hex_len)?;
        payload.push(b' ');
        payload.extend_from_slice(&cmd.refname);
        if i == 0 {
            // NUL capability-list
            payload.push(0);
            for (j, cap) in request.capabilities.iter().enumerate() {
                if j > 0 {
                    payload.push(b' ');
                }
                payload.extend_from_slice(cap);
            }
        }
        payload.push(b'\n');
        gix_transport::packetline::blocking_io::encode::data_to_write(&payload, &mut *out)?;
    }
    gix_transport::packetline::blocking_io::encode::flush_to_write(&mut *out)?;
    Ok(())
}

fn write_oid_hex(out: &mut Vec<u8>, oid: &gix_hash::ObjectId, hex_len: usize) -> std::io::Result<()> {
    if oid.is_null() {
        out.extend(std::iter::repeat(b'0').take(hex_len));
    } else {
        write!(out, "{}", oid.to_hex())?;
    }
    Ok(())
}
