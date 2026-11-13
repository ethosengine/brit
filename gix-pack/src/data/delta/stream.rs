//! Apply pack delta instructions without allocating complete base or result buffers.
use std::{
    io::{self, Read, Seek, SeekFrom, Write},
    sync::atomic::AtomicBool,
};

use gix_error::{Result, ResultExt, bail, corruption};
use gix_utils::stream::{self, Spool};

fn read_exact(input: &mut impl Read, bytes: &mut [u8]) -> io::Result<()> {
    input.read_exact(bytes).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            io::Error::new(io::ErrorKind::UnexpectedEof, corruption("Truncated delta instruction"))
        } else {
            error
        }
    })
}

/// Read a base or result size from the start of an inflated delta.
pub fn size(input: &mut impl Read) -> io::Result<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let mut byte = [0];
        read_exact(input, &mut byte)?;
        if shift == 63 && byte[0] > 1 {
            return Err(io::Error::other(corruption("Delta size overflow")));
        }
        value |= u64::from(byte[0] & 127) << shift;
        if byte[0] & 128 == 0 {
            return Ok(value);
        }
    }
    Err(io::Error::other(corruption("Delta size overflow")))
}

/// Apply instructions after the two size headers to an initially empty charged spool.
/// The caller validates the declared base size and inflated instruction stream length.
pub fn apply(
    input: &mut impl Read,
    base: &mut (impl Read + Seek),
    base_len: u64,
    output: &mut Spool,
    size: u64,
    interrupt: &AtomicBool,
) -> Result {
    let mut command = [0];
    let mut buf = [0; stream::CHUNK];
    while input.read(&mut command).or_error()? != 0 {
        stream::check(interrupt).or_error()?;
        let cmd = command[0];
        if cmd & 128 != 0 {
            let mut offset = 0u64;
            let mut count = 0u64;
            for bit in 0..7 {
                if cmd & (1 << bit) != 0 {
                    read_exact(input, &mut command).or_error()?;
                    if bit < 4 {
                        offset |= u64::from(command[0]) << (8 * bit);
                    } else {
                        count |= u64::from(command[0]) << (8 * (bit - 4));
                    }
                }
            }
            if count == 0 {
                count = 65536;
            }
            if offset.checked_add(count).is_none_or(|end| end > base_len) {
                bail!(corruption("Delta copy exceeds base"));
            }
            base.seek(SeekFrom::Start(offset)).or_error()?;
            if output.len().checked_add(count).is_none_or(|end| end > size) {
                bail!(corruption("Delta exceeds declared result"));
            }
            let copied = stream::copy(&mut base.take(count), output, interrupt).or_error()?;
            if copied != count {
                bail!(corruption("Truncated delta base"));
            }
        } else {
            let count = u64::from(cmd);
            if count == 0 || output.len().checked_add(count).is_none_or(|end| end > size) {
                bail!(corruption("Invalid delta insertion"));
            }
            read_exact(input, &mut buf[..cmd as usize]).or_error()?;
            output.write_all(&buf[..cmd as usize]).or_error()?;
        }
    }
    if output.len() != size {
        bail!(corruption("Delta result size mismatch"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gix_error::ErrorExt;

    #[test]
    fn size_preserves_io_failure_and_classifies_truncation() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "fixture read denied"))
            }
        }
        let io = size(&mut Broken).expect_err("read failure");
        assert_eq!(io.kind(), io::ErrorKind::PermissionDenied);
        assert!(!io.raise().is_corrupted());
        let truncated = size(&mut &b"\x80"[..]).expect_err("unterminated size");
        assert_eq!(truncated.kind(), io::ErrorKind::UnexpectedEof);
        assert!(truncated.raise().is_corrupted());
        assert!(size(&mut &[255; 10][..]).expect_err("overflow").raise().is_corrupted());
    }
}
