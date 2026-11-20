//! Parser for the `report-status` response (receive-pack → send-pack).
//!
//! Grammar (simplified from `Documentation/gitprotocol-pack.adoc`):
//!
//! ```text
//! report-status     =  PKT-LINE("unpack" SP ("ok" / error-msg) LF)
//!                      *command-status
//!                      flush-pkt
//! command-status    =  PKT-LINE("ok" SP refname LF) /
//!                      PKT-LINE("ng" SP refname SP error-msg LF)
//! ```
//!
//! Reference: `vendor/git/send-pack.c::receive_status()`.

use bstr::BString;

use crate::send_pack::{Error, RefStatus, Report};

/// Parse a `report-status` pkt-line stream from `input`.
///
/// The stream must be positioned after any sideband demux — `input` yields
/// raw report bytes as pkt-lines. Terminates on the flush-pkt (`0000`).
pub fn parse(input: &mut impl std::io::Read) -> Result<Report, Error> {
    use gix_transport::packetline::{PacketLineRef, blocking_io::StreamingPeekableIter};

    let mut reader = StreamingPeekableIter::new(
        input,
        &[PacketLineRef::Flush],
        false, // trace
    );

    // Only a stream that really ends is an end of stream; a failed read keeps its cause.
    let unpack = match reader.read_line() {
        Some(Ok(Ok(line))) => parse_unpack(data_of(line)?)?,
        Some(Ok(Err(err))) => return Err(malformed(err)),
        Some(Err(err)) => return Err(read_failure(err)),
        None => return Err(Error::UnexpectedEof),
    };

    let mut refs = Vec::new();
    while let Some(item) = reader.read_line() {
        let line = match item {
            Ok(Ok(l)) => l,
            Ok(Err(_)) => break, // decode error — treat as done
            Err(err) => return Err(read_failure(err)),
        };
        refs.push(parse_command_status(data_of(line)?)?);
    }

    Ok(Report { unpack, refs })
}

fn data_of(line: gix_transport::packetline::PacketLineRef<'_>) -> Result<&[u8], Error> {
    line.as_slice().ok_or_else(|| Error::MalformedReport {
        line: BString::from(format!("{line:?}")),
    })
}

/// A bare end of stream (the packet-line reader ran out of bytes) is an end of stream;
/// anything else the transport raised keeps its cause.
fn read_failure(err: std::io::Error) -> Error {
    if err.kind() == std::io::ErrorKind::UnexpectedEof && err.get_ref().is_none() {
        Error::UnexpectedEof
    } else {
        Error::ReadReport(err)
    }
}

fn malformed(err: impl std::fmt::Display) -> Error {
    Error::MalformedReport {
        line: BString::from(err.to_string()),
    }
}

fn trim_lf(line: &[u8]) -> &[u8] {
    if line.last().copied() == Some(b'\n') {
        &line[..line.len() - 1]
    } else {
        line
    }
}

fn parse_unpack(line: &[u8]) -> Result<Result<(), BString>, Error> {
    let line = trim_lf(line);
    let rest = line.strip_prefix(b"unpack ").ok_or_else(|| Error::MalformedReport {
        line: BString::from(line),
    })?;
    if rest == b"ok" {
        Ok(Ok(()))
    } else {
        Ok(Err(BString::from(rest)))
    }
}

fn parse_command_status(line: &[u8]) -> Result<RefStatus, Error> {
    let line = trim_lf(line);
    if let Some(rest) = line.strip_prefix(b"ok ") {
        Ok(RefStatus {
            refname: BString::from(rest),
            result: Ok(()),
        })
    } else if let Some(rest) = line.strip_prefix(b"ng ") {
        let sp = rest
            .iter()
            .position(|&b| b == b' ')
            .ok_or_else(|| Error::MalformedReport {
                line: BString::from(line),
            })?;
        let (refname, reason) = rest.split_at(sp);
        let reason = &reason[1..]; // strip the leading space
        Ok(RefStatus {
            refname: BString::from(refname),
            result: Err(BString::from(reason)),
        })
    } else {
        Err(Error::MalformedReport {
            line: BString::from(line),
        })
    }
}
