use crate::{
    Pipeline,
    driver::{self, apply::MaybeDelayed},
    eol,
    pipeline::{convert::to_worktree, util::Configuration},
};
use bstr::BStr;
use gix_error::{Result, ResultExt, bail, validation};
use gix_utils::stream::{self, Budget, Spool};
use std::{
    io::{BufRead, BufReader, Read, Seek, Write},
    sync::{Arc, atomic::AtomicBool},
};

/// A converted spool, or a filter result which must be fetched later.
pub enum Outcome {
    /// All conversions completed; positioned at the beginning.
    Ready(Spool),
    /// The process accepted this path and will return it through its delayed protocol.
    Delayed(driver::Key),
}
impl Pipeline {
    /// Apply worktree conversions without allocating storage proportional to the input or output.
    pub fn convert_to_worktree_stream(
        &mut self,
        mut src: Spool,
        path: &BStr,
        attributes: &mut dyn FnMut(&BStr, &mut gix_attributes::search::Outcome),
        options: to_worktree::Options,
        budget: &Arc<Budget>,
        interrupt: &AtomicBool,
    ) -> Result<Outcome> {
        let Configuration {
            driver,
            digest,
            encoding,
            apply_ident_filter,
            ..
        } = Configuration::at_path(
            path,
            &self.options.drivers,
            &mut self.attrs,
            attributes,
            self.options.eol_config,
            options.unknown_encoding == to_worktree::UnknownEncoding::Ignore,
        )?;
        if apply_ident_filter {
            let mut hash = gix_hash::hasher(self.object_hash);
            hash.update(&gix_object::encode::loose_header(gix_object::Kind::Blob, src.len()));
            let mut buf = [0; stream::CHUNK];
            let mut matched = 0;
            let mut has_ident = false;
            loop {
                stream::check(interrupt).or_error()?;
                let n = src.read(&mut buf).or_error()?;
                if n == 0 {
                    break;
                }
                hash.update(&buf[..n]);
                for &byte in &buf[..n] {
                    if byte == b"$Id$"[matched] {
                        matched += 1;
                        if matched == 4 {
                            has_ident = true;
                            matched = 0;
                        }
                    } else {
                        matched = usize::from(byte == b'$');
                    }
                }
            }
            src.rewind().or_error()?;
            if has_ident {
                let replacement = format!("$Id: {}$", hash.try_finalize()?);
                let mut out = budget.spool().or_error()?;
                let mut reader = BufReader::new(&mut src);
                let mut matched = 0;
                loop {
                    stream::check(interrupt).or_error()?;
                    let input = reader.fill_buf().or_error()?;
                    if input.is_empty() {
                        break;
                    }
                    let n = input.len();
                    let mut writer = std::io::BufWriter::new(&mut out);
                    for &byte in input {
                        if byte == b"$Id$"[matched] {
                            matched += 1;
                            if matched == 4 {
                                writer.write_all(replacement.as_bytes()).or_error()?;
                                matched = 0;
                            }
                        } else {
                            writer.write_all(&b"$Id$"[..matched]).or_error()?;
                            matched = usize::from(byte == b'$');
                            if matched == 0 {
                                writer.write_all(&[byte]).or_error()?;
                            }
                        }
                    }
                    writer.flush().or_error()?;
                    reader.consume(n);
                }
                out.write_all(&b"$Id$"[..matched]).or_error()?;
                src = out;
                src.rewind().or_error()?;
            }
        }
        if digest.to_eol(self.options.eol_config) == Some(eol::Mode::CrLf) {
            let stats = statistics(&mut src, interrupt)?;
            src.rewind().or_error()?;
            if stats.will_convert_lf_to_crlf(digest, self.options.eol_config) {
                let mut out = budget.spool().or_error()?;
                let mut writer = std::io::BufWriter::new(&mut out);
                let mut buf = [0; stream::CHUNK];
                let mut previous_cr = false;
                loop {
                    stream::check(interrupt).or_error()?;
                    let n = src.read(&mut buf).or_error()?;
                    if n == 0 {
                        break;
                    }
                    let mut start = 0;
                    for position in 0..n {
                        if buf[position] == b'\n'
                            && !(if position == 0 {
                                previous_cr
                            } else {
                                buf[position - 1] == b'\r'
                            })
                        {
                            writer.write_all(&buf[start..position]).or_error()?;
                            writer.write_all(b"\r\n").or_error()?;
                            start = position + 1;
                        }
                    }
                    writer.write_all(&buf[start..n]).or_error()?;
                    previous_cr = buf[n - 1] == b'\r';
                }
                writer.flush().or_error()?;
                drop(writer);
                src = out;
                src.rewind().or_error()?;
            }
        }
        if let Some(encoding) = encoding {
            let mut out = budget.spool().or_error()?;
            if encode(&mut src, &mut out, encoding, interrupt)? {
                src = out;
            } else if options.unknown_encoding == to_worktree::UnknownEncoding::Fail {
                bail!(validation("Invalid UTF-8 or unmappable worktree encoding"));
            }
            src.rewind().or_error()?;
        }
        if let Some(driver) = driver {
            if driver.process.is_none() {
                src =
                    self.processes
                        .apply_single_stream(driver, src, self.context.with_path(path), budget, interrupt)?;
            } else {
                let outcome = self.processes.apply_delayed(
                    driver,
                    &mut src,
                    driver::Operation::Smudge,
                    options.can_delay,
                    self.context.with_path(path),
                )?;
                match outcome {
                    Some(MaybeDelayed::Delayed(key)) => return Ok(Outcome::Delayed(key)),
                    Some(MaybeDelayed::Immediate(mut read)) => {
                        let mut out = budget.spool().or_error()?;
                        stream::copy(&mut read, &mut out, interrupt).or_error()?;
                        src = out;
                    }
                    None => {}
                }
            }
        }
        src.rewind().or_error()?;
        Ok(Outcome::Ready(src))
    }
}
fn statistics(src: &mut Spool, interrupt: &AtomicBool) -> Result<eol::Stats> {
    let mut stats = eol::Stats::from_bytes(&[]);
    let mut buf = [0; stream::CHUNK];
    let mut previous = None;
    loop {
        stream::check(interrupt).or_error()?;
        let n = src.read(&mut buf).or_error()?;
        if n == 0 {
            break;
        }
        for &byte in &buf[..n] {
            if previous == Some(b'\r') {
                if byte == b'\n' {
                    stats.crlf += 1;
                    previous = Some(byte);
                    continue;
                }
                stats.lone_cr += 1;
            }
            match byte {
                b'\r' => {}
                b'\n' => stats.lone_lf += 1,
                0 => {
                    stats.null += 1;
                    stats.non_printable += 1;
                }
                8 | 9 | 12 | 27 => stats.printable += 1,
                1..=31 | 127 => stats.non_printable += 1,
                _ => stats.printable += 1,
            }
            previous = Some(byte);
        }
    }
    if previous == Some(b'\r') {
        stats.lone_cr += 1;
    }
    if previous == Some(0x1a) {
        stats.non_printable -= 1;
    }
    Ok(stats)
}
fn encode(
    src: &mut Spool,
    out: &mut Spool,
    encoding: &'static encoding_rs::Encoding,
    interrupt: &AtomicBool,
) -> Result<bool> {
    let mut encoder = encoding.new_encoder();
    let mut input = [0; stream::CHUNK];
    let mut output = [0; stream::CHUNK];
    let mut carry = 0;
    loop {
        stream::check(interrupt).or_error()?;
        let n = src.read(&mut input[carry..]).or_error()?;
        let end = carry + n;
        let eof = n == 0;
        let valid = match std::str::from_utf8(&input[..end]) {
            Ok(_) => end,
            Err(err) if err.error_len().is_none() && !eof => err.valid_up_to(),
            Err(_) => return Ok(false),
        };
        let mut text = std::str::from_utf8(&input[..valid]).expect("validated UTF-8");
        loop {
            stream::check(interrupt).or_error()?;
            let (result, read, written) = encoder.encode_from_utf8_without_replacement(text, &mut output, eof);
            out.write_all(&output[..written]).or_error()?;
            text = &text[read..];
            match result {
                encoding_rs::EncoderResult::Unmappable(_) => return Ok(false),
                encoding_rs::EncoderResult::InputEmpty => break,
                encoding_rs::EncoderResult::OutputFull => {}
            }
        }
        if eof {
            return Ok(true);
        }
        input.copy_within(valid..end, 0);
        carry = end - valid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incremental_statistics_match_whole_buffer() -> gix_testtools::TestResult {
        let temp = gix_testtools::tempfile::tempdir()?;
        let budget = Budget::new(temp.path())?;
        for tail in [&b"\r\n\0\x1a"[..], b"\r", b"\n", b"\x1a\x1a"] {
            let mut input = vec![b'x'; stream::CHUNK - 1];
            input.extend_from_slice(tail);
            let mut source = budget.spool()?;
            source.write_all(&input)?;
            source.rewind()?;
            assert_eq!(
                statistics(&mut source, &AtomicBool::new(false))?,
                eol::Stats::from_bytes(&input)
            );
        }
        Ok(())
    }
    #[test]
    fn encoding_is_incremental_and_failure_preserves_source() -> gix_testtools::TestResult {
        let temp = gix_testtools::tempfile::tempdir()?;
        let budget = Budget::new(temp.path())?;
        for encoding in [
            encoding_rs::UTF_16LE,
            encoding_rs::WINDOWS_1252,
            encoding_rs::ISO_2022_JP,
        ] {
            for tail in ["€", "日本語", "x"] {
                let mut input = vec![b'x'; stream::CHUNK - 1];
                input.extend_from_slice(tail.as_bytes());
                let mut expected = Vec::new();
                let success = crate::worktree::encode_to_worktree(&input, encoding, &mut expected).is_ok();
                let mut source = budget.spool()?;
                source.write_all(&input)?;
                source.rewind()?;
                let mut out = budget.spool()?;
                assert_eq!(
                    encode(&mut source, &mut out, encoding, &AtomicBool::new(false))?,
                    success
                );
                if success {
                    let mut actual = Vec::new();
                    out.rewind()?;
                    out.read_to_end(&mut actual)?;
                    assert_eq!(actual, expected);
                }
                source.rewind()?;
                let mut retained = Vec::new();
                source.read_to_end(&mut retained)?;
                assert_eq!(retained, input);
            }
        }
        Ok(())
    }
}
