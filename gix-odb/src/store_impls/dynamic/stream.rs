use crate::store::handle;
use gix_error::{OptionExt, Result, ResultExt, bail, corruption, not_found};
use gix_object::{FindStream, Kind};
use gix_pack::data::entry::Header;
use gix_utils::stream::{self, Budget, Spool};
use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom, Write},
    ops::Deref,
    sync::{Arc, atomic::AtomicBool},
};

struct Packed {
    file: Arc<File>,
    path: std::path::PathBuf,
    entry: gix_pack::data::Entry,
}
enum Source {
    Packed(Packed),
    Loose(File),
}

impl<S: Deref<Target = super::Store> + Clone> super::Handle<S> {
    fn stream_source(&self, id: &gix_hash::oid) -> Result<Source> {
        let mut snapshot = self.snapshot.borrow_mut();
        loop {
            let marker = snapshot.marker;
            for index in &mut snapshot.indices {
                if let Some(handle::index_lookup::Outcome {
                    object_index: handle::IndexForObjectInPack { pack_id, pack_offset },
                    pack,
                    ..
                }) = index.lookup(id)?
                {
                    if pack.is_none() {
                        *pack = self.store.load_pack(pack_id, marker).or_error()?;
                    }
                    if let Some(pack) = pack {
                        return Ok(Source::Packed(Packed {
                            file: Arc::new(File::open(pack.path()).or_error()?),
                            path: pack.path().to_owned(),
                            entry: pack.entry(pack_offset)?,
                        }));
                    }
                }
            }
            for loose in snapshot.loose_dbs.iter() {
                let hex = id.to_string();
                let path = loose.path().join(&hex[..2]).join(&hex[2..]);
                match File::open(path) {
                    Ok(file) => return Ok(Source::Loose(file)),
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                    Err(err) => return Err(err).or_error(),
                }
            }
            match self.store.load_one_index(self.index_ctx(snapshot.marker))? {
                Some(next) => {
                    *snapshot = next;
                    self.clear_cache();
                }
                None => bail!(not_found(format!("Object {id} is missing"))),
            }
        }
    }
}

impl<S: Deref<Target = super::Store> + Clone> FindStream for super::Handle<S> {
    fn find_stream(&self, id: &gix_hash::oid, budget: &Arc<Budget>, interrupt: &AtomicBool) -> Result<(Kind, Spool)> {
        let id = if !self.ignore_replacements {
            self.store
                .replacements
                .binary_search_by(|(from, _)| from.as_ref().cmp(id))
                .ok()
                .map(|pos| self.store.replacements[pos].1.as_ref())
                .unwrap_or(id)
        } else {
            id
        };
        let mut source = self.stream_source(id)?;
        let mut deltas = budget.spool().or_error()?;
        let mut packs: Vec<(std::path::PathBuf, Arc<File>)> = Vec::new();
        let mut refs = budget.spool().or_error()?;
        let mut cross_pack_depth = 0;
        let (kind, mut result) = loop {
            stream::check(interrupt).or_error()?;
            match source {
                Source::Loose(file) => {
                    let (kind, size, mut decoder) = crate::store_impls::loose::stream::reader(file)?;
                    let mut out = budget.spool().or_error()?;
                    let copied = stream::copy(&mut decoder, &mut out, interrupt).or_error()?;
                    if copied != size {
                        bail!(corruption("Loose object size mismatch"));
                    }
                    break (kind, out);
                }
                Source::Packed(packed) => {
                    let base = match packed.entry.header {
                        Header::OfsDelta { base_distance } => {
                            let offset = packed
                                .entry
                                .checked_base_pack_offset(base_distance)
                                .ok_or_raise(|| corruption("Invalid delta base offset"))?;
                            let mut file = packed.file.as_ref();
                            file.seek(SeekFrom::Start(offset)).or_error()?;
                            let mut header = [0; 64];
                            let n = file.read(&mut header).or_error()?;
                            let entry = gix_pack::data::Entry::from_bytes(&header[..n], offset, id.kind())?;
                            Source::Packed(Packed {
                                file: packed.file.clone(),
                                path: packed.path.clone(),
                                entry,
                            })
                        }
                        Header::RefDelta { base_id } => {
                            refs.rewind().or_error()?;
                            let mut seen = [0; 32];
                            for _ in 0..refs.len() / base_id.as_slice().len() as u64 {
                                stream::check(interrupt).or_error()?;
                                let seen = &mut seen[..base_id.as_slice().len()];
                                refs.read_exact(seen).or_error()?;
                                if seen == base_id.as_slice() {
                                    bail!(corruption("Delta base cycle"));
                                }
                            }
                            refs.write_all(base_id.as_slice()).or_error()?;
                            let base = self.stream_source(&base_id)?;
                            if !matches!(&base, Source::Packed(base) if base.path == packed.path) {
                                cross_pack_depth += 1;
                                if cross_pack_depth > self.max_recursion_depth {
                                    bail!(corruption("Cross-pack delta recursion limit"));
                                }
                            }
                            base
                        }
                        _ => {
                            let kind = packed.entry.header.as_kind().expect("non-delta header");
                            let mut out = budget.spool().or_error()?;
                            let size = packed.entry.decompressed_size;
                            let mut decoder = packed.reader()?;
                            if stream::copy(&mut decoder, &mut out, interrupt).or_error()? != size {
                                bail!(corruption("Pack entry size mismatch"));
                            }
                            break (kind, out);
                        }
                    };
                    let pack = match packs.iter().position(|(path, _)| *path == packed.path) {
                        Some(position) => position,
                        None => {
                            packs.push((packed.path, packed.file));
                            packs.len() - 1
                        }
                    };
                    deltas.write_all(&(pack as u32).to_le_bytes()).or_error()?;
                    deltas.write_all(&packed.entry.pack_offset().to_le_bytes()).or_error()?;
                    source = base;
                }
            }
        };
        while !deltas.is_empty() {
            let end = deltas.len() - 12;
            deltas.seek(SeekFrom::Start(end)).or_error()?;
            let mut record = [0; 12];
            deltas.read_exact(&mut record).or_error()?;
            deltas.truncate(end).or_error()?;
            let pack = u32::from_le_bytes(record[..4].try_into().expect("fixed record")) as usize;
            let offset = u64::from_le_bytes(record[4..].try_into().expect("fixed record"));
            let (path, file) = packs[pack].clone();
            let mut read = file.as_ref();
            read.seek(SeekFrom::Start(offset)).or_error()?;
            let mut header = [0; 64];
            let n = read.read(&mut header).or_error()?;
            let packed = Packed {
                file,
                path,
                entry: gix_pack::data::Entry::from_bytes(&header[..n], offset, id.kind())?,
            };
            stream::check(interrupt).or_error()?;
            let size = packed.entry.decompressed_size;
            let mut delta = packed.reader()?.take(size);
            let base_size = gix_pack::data::delta::stream::size(&mut delta).or_error()?;
            let result_size = gix_pack::data::delta::stream::size(&mut delta).or_error()?;
            if base_size != result.len() {
                bail!(corruption("Delta base size mismatch"));
            }
            let mut out = budget.spool().or_error()?;
            gix_pack::data::delta::stream::apply(&mut delta, &mut result, base_size, &mut out, result_size, interrupt)?;
            if delta.limit() != 0 {
                bail!(corruption("Truncated delta instructions"));
            }
            let mut decoder = delta.into_inner();
            if decoder.read(&mut [0]).or_error()? != 0 {
                bail!(corruption("Pack delta size mismatch"));
            }
            result = out; // releases the previous base before the next reconstruction
        }
        result.rewind().or_error()?;
        let mut hash = gix_hash::hasher(id.kind());
        hash.update(&gix_object::encode::loose_header(kind, result.len()));
        let mut buf = [0; stream::CHUNK];
        loop {
            stream::check(interrupt).or_error()?;
            let n = result.read(&mut buf).or_error()?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        if hash.try_finalize()? != id {
            bail!(corruption("Streamed object hash mismatch"));
        }
        result.rewind().or_error()?;
        Ok((kind, result))
    }
}
impl Packed {
    fn reader(self) -> Result<gix_zlib::stream::inflate::Reader<BufReader<File>>> {
        let mut file = self.file.try_clone().or_error()?;
        file.seek(SeekFrom::Start(self.entry.data_offset)).or_error()?;
        Ok(gix_zlib::stream::inflate::Reader::new(BufReader::new(file)))
    }
}
impl<S: FindStream> FindStream for crate::Cache<S> {
    fn find_stream(&self, id: &gix_hash::oid, budget: &Arc<Budget>, interrupt: &AtomicBool) -> Result<(Kind, Spool)> {
        self.inner.find_stream(id, budget, interrupt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gix_object::FindExt;

    fn single_pack(
        root: &std::path::Path,
        id: gix_hash::ObjectId,
        header: Header,
        data: &[u8],
    ) -> gix_testtools::TestResult {
        let mut pack = gix_pack::data::header::encode(gix_pack::data::Version::V2, 1).to_vec();
        header.write_to(data.len() as u64, &mut pack)?;
        let mut compressed = gix_zlib::stream::deflate::Write::new(Vec::new(), gix_zlib::Compression::BEST_SPEED);
        compressed.write_all(data)?;
        compressed.flush()?;
        pack.extend(compressed.into_inner());
        let mut hash = gix_hash::hasher(id.kind());
        hash.update(&pack);
        let pack_id = hash.try_finalize()?;
        pack.extend_from_slice(pack_id.as_slice());
        let mut index = Vec::new();
        for byte in 0..=255 {
            index.extend_from_slice(&u32::from(byte >= id.first_byte()).to_be_bytes());
        }
        index.extend_from_slice(&12u32.to_be_bytes());
        index.extend_from_slice(id.as_slice());
        index.extend_from_slice(pack_id.as_slice());
        let mut hash = gix_hash::hasher(id.kind());
        hash.update(&index);
        index.extend_from_slice(hash.try_finalize()?.as_slice());
        std::fs::create_dir_all(root.join("pack"))?;
        std::fs::write(root.join(format!("pack/pack-{pack_id}.pack")), pack)?;
        std::fs::write(root.join(format!("pack/pack-{pack_id}.idx")), index)?;
        Ok(())
    }

    #[test]
    fn bounded_cross_pack_alternates_replacements_and_hash_formats() -> gix_testtools::TestResult {
        use gix_object::Write as _;
        for kind in [gix_hash::Kind::Sha1, gix_hash::Kind::Sha256] {
            for packed_base in [false, true] {
                let main = tempfile::tempdir()?;
                let alternate = tempfile::tempdir()?;
                let scratch = tempfile::tempdir()?;
                let base_id = gix_object::compute_hash(kind, Kind::Blob, b"base")?;
                let blob_id = gix_object::compute_hash(kind, Kind::Blob, b"based")?;
                if packed_base {
                    single_pack(alternate.path(), base_id, Header::Blob, b"base")?;
                } else {
                    crate::at(alternate.path(), kind)?.write_buf(Kind::Blob, b"base")?;
                }
                single_pack(
                    main.path(),
                    blob_id,
                    Header::RefDelta { base_id },
                    &[4, 5, 0x90, 4, 1, b'd'],
                )?;
                std::fs::create_dir(main.path().join("info"))?;
                std::fs::write(
                    main.path().join("info/alternates"),
                    format!("{}\n", alternate.path().display()),
                )?;
                let db = crate::at_opts(main.path(), kind, Some((kind.null(), blob_id)), Default::default())?;
                let budget = Budget::new(scratch.path())?;
                let (_, mut out) = db.find_stream(&kind.null(), &budget, &AtomicBool::new(false))?;
                let mut data = Vec::new();
                out.read_to_end(&mut data)?;
                assert_eq!(data, b"based");
                drop(out);
                assert_eq!(budget.usage().0, 0);
            }
        }
        Ok(())
    }

    #[test]
    fn bounded_ref_delta_cycle_and_object_hash_mismatch_are_refused() -> gix_testtools::TestResult {
        let dir = tempfile::tempdir()?;
        let scratch = tempfile::tempdir()?;
        let kind = gix_hash::Kind::Sha1;
        let id = kind.empty_blob();
        single_pack(dir.path(), id, Header::RefDelta { base_id: id }, &[0, 0])?;
        let db = crate::at(dir.path(), kind)?;
        let budget = Budget::new(scratch.path())?;
        assert!(db.find_stream(&id, &budget, &AtomicBool::new(false)).is_err());
        assert_eq!(budget.usage().0, 0);
        let other = tempfile::tempdir()?;
        single_pack(other.path(), id, Header::Blob, b"not the empty blob")?;
        assert!(
            crate::at(other.path(), kind)?
                .find_stream(&id, &budget, &AtomicBool::new(false))
                .is_err()
        );
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }

    fn size(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 127) as u8;
            value >>= 7;
            out.push(byte | if value == 0 { 0 } else { 128 });
            if value == 0 {
                break;
            }
        }
    }
    fn packed_chain(
        root: &std::path::Path,
        bytes: u64,
        count: u32,
        refs: bool,
    ) -> gix_testtools::Result<gix_hash::ObjectId> {
        let packs = root.join("pack");
        std::fs::create_dir_all(&packs)?;
        let path = packs.join("building.pack");
        let mut file = File::create(&path)?;
        file.write_all(&gix_pack::data::header::encode(gix_pack::data::Version::V2, count + 1))?;
        let mut previous_offset = file.stream_position()?;
        Header::Blob.write_to(bytes, &mut file)?;
        let mut hash = gix_hash::hasher(gix_hash::Kind::Sha1);
        hash.update(&gix_object::encode::loose_header(Kind::Blob, bytes));
        let chunk = [b'x'; stream::CHUNK];
        {
            let mut compressed = gix_zlib::stream::deflate::Write::new(&mut file, gix_zlib::Compression::BEST_SPEED);
            let mut left = bytes;
            while left != 0 {
                let n = left.min(chunk.len() as u64) as usize;
                compressed.write_all(&chunk[..n])?;
                hash.update(&chunk[..n]);
                left -= n as u64;
            }
            compressed.flush()?;
        }
        let mut previous_id = hash.try_finalize()?;
        for depth in 0..count {
            let mut delta = Vec::new();
            let base_size = bytes + u64::from(depth);
            size(base_size, &mut delta);
            size(base_size + 1, &mut delta);
            let mut offset = 0u32;
            while u64::from(offset) < base_size {
                let n = (base_size - u64::from(offset)).min(1024 * 1024) as u32;
                delta.push(0xff);
                delta.extend_from_slice(&offset.to_le_bytes());
                delta.extend_from_slice(&n.to_le_bytes()[..3]);
                offset += n;
            }
            delta.extend_from_slice(&[1, b'z']);
            let entry_offset = file.stream_position()?;
            let header = if refs {
                Header::RefDelta { base_id: previous_id }
            } else {
                Header::OfsDelta {
                    base_distance: entry_offset - previous_offset,
                }
            };
            header.write_to(delta.len() as u64, &mut file)?;
            let mut compressed = gix_zlib::stream::deflate::Write::new(&mut file, gix_zlib::Compression::BEST_SPEED);
            compressed.write_all(&delta)?;
            compressed.flush()?;
            drop(compressed);
            let mut hash = gix_hash::hasher(gix_hash::Kind::Sha1);
            hash.update(&gix_object::encode::loose_header(Kind::Blob, base_size + 1));
            let mut left = bytes;
            while left != 0 {
                let n = left.min(chunk.len() as u64) as usize;
                hash.update(&chunk[..n]);
                left -= n as u64;
            }
            hash.update(&vec![b'z'; depth as usize + 1]);
            previous_id = hash.try_finalize()?;
            previous_offset = entry_offset;
        }
        drop(file);
        let mut file = File::open(&path)?;
        let mut hash = gix_hash::hasher(gix_hash::Kind::Sha1);
        let mut buf = [0; stream::CHUNK];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        let checksum = hash.try_finalize()?;
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)?
            .write_all(checksum.as_slice())?;
        let named = packs.join(format!("pack-{checksum}.pack"));
        std::fs::rename(path, &named)?;
        let output = gix_testtools::git_command(root)
            .args(["-c", "core.deltaBaseCacheLimit=16m"])
            .arg("index-pack")
            .arg("--threads=1")
            .arg(&named)
            .output()?;
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        Ok(previous_id)
    }

    #[test]
    fn bounded_delta_chains_preserve_allocation_limit_and_reclaim_bases() -> gix_testtools::TestResult {
        for (bytes, depth, refs) in [
            (65536, 40, false),
            (64, 1500, false),
            (65536, 40, true),
            (20 * 1024 * 1024, 3, false),
            (20 * 1024 * 1024, 3, true),
        ] {
            let dir = tempfile::tempdir()?;
            let scratch = tempfile::tempdir()?;
            let id = packed_chain(dir.path(), bytes, depth, refs)?;
            let db = crate::at_opts(
                dir.path(),
                gix_hash::Kind::Sha1,
                None,
                crate::store::init::Options {
                    alloc_limit_bytes: Some(16 * 1024 * 1024),
                    ..Default::default()
                },
            )?;
            if bytes > 16 * 1024 * 1024 {
                let err = db
                    .find_blob(&id, &mut Vec::new())
                    .expect_err("the buffered allocation limit is unchanged");
                assert!(err.is_resource_exhausted(), "expected allocation refusal, got {err:?}");
            }
            let budget = Budget::new(scratch.path())?;
            let (_, mut out) = db.find_stream(&id, &budget, &AtomicBool::new(false))?;
            assert_eq!(out.len(), bytes + u64::from(depth));
            out.seek(SeekFrom::Start(bytes))?;
            let mut tail = Vec::new();
            out.read_to_end(&mut tail)?;
            assert_eq!(tail, vec![b'z'; depth as usize]);
            assert!(budget.usage().1 <= 2 * (bytes + u64::from(depth)) + 32 * u64::from(depth));
            drop(out);
            assert_eq!(budget.usage().0, 0);
        }
        Ok(())
    }
    #[test]
    fn bounded_large_chain_succeeds_where_retaining_intermediates_exceeds_one_gib() -> gix_testtools::TestResult {
        let dir = tempfile::tempdir()?;
        let scratch = tempfile::tempdir()?;
        let bytes = 384 * 1024 * 1024;
        eprintln!("constructing 384 MiB delta-chain fixture");
        let id = packed_chain(dir.path(), bytes, 3, false)?;
        eprintln!("reading 384 MiB delta-chain fixture with bounded storage");
        let db = crate::at(dir.path(), gix_hash::Kind::Sha1)?;
        let budget = Budget::new(scratch.path())?;
        let started = std::time::Instant::now();
        let (_, out) = db.find_stream(&id, &budget, &AtomicBool::new(false))?;
        assert_eq!(out.len(), bytes + 3);
        assert!(4 * bytes > stream::SCRATCH_LIMIT);
        assert!(budget.usage().1 <= 2 * (bytes + 3) + 36);
        eprintln!(
            "bounded delta bytes={bytes} depth=3 peak_scratch={} elapsed={:?}",
            budget.usage().1,
            started.elapsed()
        );
        drop(out);
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }
    #[test]
    fn malformed_delta_and_cancellation_release_scratch() -> gix_testtools::TestResult {
        let dir = tempfile::tempdir()?;
        let budget = Budget::new(dir.path())?;
        let mut base = budget.spool()?;
        base.write_all(b"base")?;
        for bytes in [&[0][..], &[0x91, 9, 4], &[5, b'x'], &[1, b'x']] {
            let mut out = budget.spool()?;
            assert!(
                gix_pack::data::delta::stream::apply(
                    &mut &bytes[..],
                    &mut base,
                    4,
                    &mut out,
                    4,
                    &AtomicBool::new(false)
                )
                .is_err()
            );
        }
        let mut out = budget.spool()?;
        assert!(
            gix_pack::data::delta::stream::apply(&mut &b"\x01x"[..], &mut base, 4, &mut out, 1, &AtomicBool::new(true))
                .is_err()
        );
        assert_eq!(out.len(), 0);
        drop(out);
        drop(base);
        assert_eq!(budget.usage().0, 0);
        Ok(())
    }
}
