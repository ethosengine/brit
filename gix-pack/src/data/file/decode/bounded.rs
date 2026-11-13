use std::{
    io::{Cursor, Read, Seek, SeekFrom, Write},
    ops::Deref,
    sync::{Arc, atomic::AtomicBool},
};

use gix_error::{OptionExt, Result, ResultExt, bail, corruption};
use gix_utils::stream::{self, Budget, Mapped, Spool};

use crate::data::{self, entry::Header};

/// Resolved bytes retain the existing contiguous access contract for object inspectors.
pub(crate) enum Bytes {
    Buffered(Vec<u8>),
    Mapped(Mapped),
}
impl Deref for Bytes {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Buffered(bytes) => bytes,
            Self::Mapped(bytes) => bytes.as_slice(),
        }
    }
}
impl Bytes {
    pub(crate) fn reusable(self) -> Option<Vec<u8>> {
        match self {
            Self::Buffered(bytes) => Some(bytes),
            Self::Mapped(_) => None,
        }
    }
}

pub(crate) fn allocation_limit(error: &gix_error::Error) -> bool {
    error.classify().any(|classification| {
        classification.class()
            == gix_error::Class::ResourceExhaustion(gix_error::ResourceExhaustionKind::AllocationLimit)
    })
}

/// Inflate a base or apply one delta to a charged result, preserving the consumed compressed length.
pub(crate) fn resolve(
    mut compressed: &[u8],
    expected_size: u64,
    base: Option<&[u8]>,
    budget: &Arc<Budget>,
    interrupt: &AtomicBool,
) -> Result<(Spool, usize)> {
    stream::check(interrupt).or_error()?;
    let original_len = compressed.len();
    let mut decoder = gix_zlib::stream::inflate::Reader::new(&mut compressed);
    let mut out = budget.spool().or_error()?;
    if let Some(base) = base {
        let mut delta = (&mut decoder).take(expected_size);
        let base_size = data::delta::stream::size(&mut delta).or_error()?;
        let result_size = data::delta::stream::size(&mut delta).or_error()?;
        if base_size != base.len() as u64 {
            bail!(corruption("Delta base size mismatch"));
        }
        data::delta::stream::apply(
            &mut delta,
            &mut Cursor::new(base),
            base_size,
            &mut out,
            result_size,
            interrupt,
        )?;
        if delta.limit() != 0 {
            bail!(corruption("Truncated delta instructions"));
        }
    } else {
        let copied = stream::copy(&mut (&mut decoder).take(expected_size), &mut out, interrupt).or_error()?;
        if copied != expected_size {
            bail!(corruption("Pack entry size mismatch"));
        }
    }
    if decoder.read(&mut [0]).or_error()? != 0 {
        bail!(corruption("Pack entry exceeds declared size"));
    }
    drop(decoder);
    Ok((out, original_len - compressed.len()))
}

impl<T: crate::FileData> data::File<T> {
    fn compressed_entry(&self, entry: &data::Entry) -> Result<&[u8]> {
        let offset =
            usize::try_from(entry.data_offset).or_raise(|| corruption("Pack entry offset is not representable"))?;
        self.data
            .get(offset..self.pack_end())
            .ok_or_raise(|| corruption("Pack entry data lies outside the pack"))
    }

    /// Follow the same in-pack delta chain as lookup decoding, storing offsets and resolved bodies on disk.
    pub(crate) fn decode_entry_bounded(
        &self,
        first: data::Entry,
        resolve_ref: impl Fn(&gix_hash::oid) -> Result<Option<data::Entry>>,
        budget: &Arc<Budget>,
        interrupt: &AtomicBool,
    ) -> Result<(Mapped, super::entry::Outcome)> {
        let mut current = first.clone();
        let mut chain = budget.spool().or_error()?;
        let mut cycle = super::DeltaCycle::new(first.pack_offset());
        let (kind, mut body, mut compressed_size) = loop {
            stream::check(interrupt).or_error()?;
            let next = match current.header {
                Header::OfsDelta { base_distance } => self.entry(
                    current
                        .checked_base_pack_offset(base_distance)
                        .ok_or_raise(|| corruption("Invalid delta base offset"))?,
                )?,
                Header::RefDelta { base_id } => {
                    resolve_ref(&base_id)?.ok_or_raise(|| super::DeltaBaseUnresolved(base_id))?
                }
                _ => {
                    let (body, consumed) = resolve(
                        self.compressed_entry(&current)?,
                        current.decompressed_size,
                        None,
                        budget,
                        interrupt,
                    )?;
                    break (current.header.as_kind().expect("base header"), body, consumed);
                }
            };
            chain.write_all(&current.pack_offset().to_le_bytes()).or_error()?;
            cycle.check(next.pack_offset())?;
            current = next;
        };
        let num_deltas = u32::try_from(chain.len() / 8).or_raise(|| corruption("Delta chain length overflow"))?;
        while !chain.is_empty() {
            let end = chain.len() - 8;
            chain.seek(SeekFrom::Start(end)).or_error()?;
            let mut offset = [0; 8];
            chain.read_exact(&mut offset).or_error()?;
            chain.truncate(end).or_error()?;
            let entry = self.entry(u64::from_le_bytes(offset))?;
            let base = body.into_mapped().or_error()?;
            let (result, consumed) = resolve(
                self.compressed_entry(&entry)?,
                entry.decompressed_size,
                Some(base.as_slice()),
                budget,
                interrupt,
            )?;
            body = result;
            compressed_size = consumed;
        }
        let outcome = super::entry::Outcome {
            kind,
            num_deltas,
            decompressed_size: first.decompressed_size,
            compressed_size,
            object_size: body.len(),
        };
        Ok((body.into_mapped().or_error()?, outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gix_error::ErrorExt;

    #[test]
    fn only_configured_allocation_limits_select_bounded_storage() {
        let limit = gix_error::message("configured limit").allocation_limit().raise();
        let failure = gix_error::message("allocator failure").allocation_failure().raise();
        assert!(allocation_limit(&limit));
        assert!(!allocation_limit(&failure));
        assert!(!allocation_limit(&corruption("invalid object").raise()));
    }

    fn pack(header: Header, bytes: &[u8], truncate: bool) -> gix_testtools::TestResult<data::File<Vec<u8>>> {
        let mut data = data::header::encode(data::Version::V2, 1).to_vec();
        header.write_to(bytes.len() as u64, &mut data)?;
        let mut encoder = gix_zlib::stream::deflate::Write::new(Vec::new(), gix_zlib::Compression::BEST_SPEED);
        encoder.write_all(bytes)?;
        encoder.flush()?;
        let mut compressed = encoder.into_inner();
        if truncate {
            compressed.truncate(compressed.len() / 2);
        }
        data.extend(compressed);
        data.extend([0; 20]);
        Ok(data::File::from_data(
            data,
            "fixture.pack".into(),
            gix_hash::Kind::Sha1,
        )?)
    }

    #[test]
    fn bounded_decode_preserves_corruption_cancellation_and_scratch_errors() -> gix_testtools::TestResult {
        let dir = gix_testtools::tempfile::tempdir()?;
        let budget = Budget::new(dir.path())?;
        let broken = pack(Header::Blob, b"payload", true)?;
        let error = broken
            .decode_entry_bounded(broken.entry(12)?, |_| Ok(None), &budget, &AtomicBool::new(false))
            .err()
            .expect("truncated compressed bytes are rejected");
        assert!(error.is_corrupted(), "{error:?}");
        assert_eq!(budget.usage().0, 0);

        let valid = pack(Header::Blob, b"payload", false)?;
        let error = valid
            .decode_entry_bounded(valid.entry(12)?, |_| Ok(None), &budget, &AtomicBool::new(true))
            .err()
            .expect("cancelled before reading");
        assert!(error.is_cancelled(), "{error:?}");
        assert_eq!(budget.usage().0, 0);
        let charge = budget.charge(stream::SCRATCH_LIMIT)?;
        let error = valid
            .decode_entry_bounded(valid.entry(12)?, |_| Ok(None), &budget, &AtomicBool::new(false))
            .err()
            .expect("scratch quota is enforced before writing");
        assert!(format!("{error:?}").contains("hard 1 GiB aggregate scratch budget"));
        assert!(!error.is_corrupted());
        drop(charge);
        assert_eq!(budget.usage().0, 0);

        let cycle = pack(
            Header::RefDelta {
                base_id: gix_hash::Kind::Sha1.null(),
            },
            &[4, 4, 0x90, 4],
            false,
        )?;
        let error = cycle
            .decode_entry_bounded(
                cycle.entry(12)?,
                |_| Ok(Some(cycle.entry(12)?)),
                &budget,
                &AtomicBool::new(false),
            )
            .err()
            .expect("bounded lookup detects a ref-delta cycle");
        assert!(error.is_corrupted(), "{error:?}");
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }
}
