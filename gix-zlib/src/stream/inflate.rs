//! Decompression support for reading zlib streams into caller-provided buffers.

use gix_error::ErrorExt;
use std::{io, io::BufRead};

use crate::{Decompress, FlushDecompress, Status};

/// Read bytes from `rd` and decompress them using `state` into a pre-allocated fitting buffer `dst`, returning the amount of bytes written.
pub fn read(rd: &mut impl BufRead, state: &mut Decompress, mut dst: &mut [u8]) -> io::Result<usize> {
    let mut total_written = 0;
    loop {
        let (written, consumed, ret, eof);
        {
            let input = rd.fill_buf()?;
            eof = input.is_empty();
            let before_out = state.total_out();
            let before_in = state.total_in();
            let flush = if eof {
                FlushDecompress::Finish
            } else {
                FlushDecompress::None
            };
            ret = state.decompress(input, dst, flush);
            written = (state.total_out() - before_out) as usize;
            total_written += written;
            dst = &mut dst[written..];
            consumed = (state.total_in() - before_in) as usize;
        }
        rd.consume(consumed);

        match ret {
            // The stream has officially ended, nothing more to do here.
            Ok(Status::StreamEnd) => return Ok(total_written),
            // Either input our output are depleted even though the stream is not depleted yet.
            Ok(Status::Ok | Status::BufError) if eof || dst.is_empty() => return Ok(total_written),
            // Some progress was made in both the input and the output, it must continue to reach the end.
            Ok(Status::Ok | Status::BufError) if consumed != 0 || written != 0 => continue,
            // A strange state, where zlib makes no progress but isn't done either. Call it out.
            Ok(Status::Ok | Status::BufError) => unreachable!("Definitely a bug somewhere"),
            // Keep the underlying zlib error so callers can tell a checksum mismatch
            // (`incorrect data check`) apart from genuine stream corruption.
            Err(err) if err.is_resource_exhausted() => {
                return Err(io::Error::other(err));
            }
            Err(err) => {
                let cause = state.error_message().map_or_else(|| err.to_string(), String::from);
                let err = err.and_raise(gix_error::message!("corrupt deflate stream: {cause}").corrupted());
                return Err(io::Error::new(io::ErrorKind::InvalidInput, err));
            }
        }
    }
}

/// An incremental zlib reader which refuses truncated streams instead of treating them as EOF.
pub struct Reader<R> {
    input: R,
    state: crate::Decompress,
    ended: bool,
}
impl<R: BufRead> Reader<R> {
    /// Decode one zlib stream from `input`, leaving any following bytes unread.
    pub fn new(input: R) -> Self {
        Self {
            input,
            state: crate::Decompress::new(),
            ended: false,
        }
    }
}
impl<R: BufRead> io::Read for Reader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.ended {
            return Ok(0);
        }
        loop {
            let input = self.input.fill_buf()?;
            let before_in = self.state.total_in();
            let before_out = self.state.total_out();
            let status = self
                .state
                .decompress(input, out, crate::FlushDecompress::None)
                .map_err(io::Error::other)?;
            let consumed = (self.state.total_in() - before_in) as usize;
            let written = (self.state.total_out() - before_out) as usize;
            self.input.consume(consumed);
            self.ended = status == crate::Status::StreamEnd;
            if written != 0 || self.ended {
                return Ok(written);
            }
            if consumed == 0 {
                return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Truncated zlib stream"));
            }
        }
    }
}
