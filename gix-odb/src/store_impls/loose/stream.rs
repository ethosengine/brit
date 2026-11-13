use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
};

use gix_error::{OptionExt, Result, ResultExt, corruption};
use gix_object::Kind;

/// Read the bounded loose header, leaving the decoder at the first body byte.
pub(crate) fn reader(file: File) -> Result<(Kind, u64, impl Read)> {
    let mut decoder = BufReader::new(gix_zlib::stream::inflate::Reader::new(BufReader::new(file)));
    let mut header = Vec::new();
    decoder
        .by_ref()
        .take(super::HEADER_MAX_SIZE as u64)
        .read_until(0, &mut header)
        .or_error()?;
    let header = header
        .strip_suffix(&[0])
        .ok_or_raise(|| corruption("Invalid loose object header"))?;
    let text = std::str::from_utf8(header).or_error()?;
    let (kind, size) = text
        .split_once(' ')
        .ok_or_raise(|| corruption("Invalid loose object header"))?;
    let kind = Kind::from_bytes(kind.as_bytes()).or_error()?;
    let size = size.parse::<u64>().or_error()?;
    Ok((kind, size, decoder))
}
