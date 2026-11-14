use gix_error::Result;
use std::{
    io::Read,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use gix_error::{Class, ClassificationMarker, Message, OptionExt, ResultExt, bail, message, retryable};

use gix_utils::progress::{Count, DynNestedProgress, Progress};
use gix_utils::stream::{self, Budget};

use crate::loose::Store;

///
pub mod integrity {
    /// The outcome returned by [`verify_integrity()`][super::Store::verify_integrity()].
    #[derive(Debug, PartialEq, Eq, Hash, Ord, PartialOrd, Clone)]
    #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
    pub struct Statistics {
        /// The amount of loose objects we checked.
        pub num_objects: usize,
    }

    /// The progress ids used in [`verify_integrity()`][super::Store::verify_integrity()].
    ///
    /// Use this information to selectively extract the progress of interest in case the parent application has custom visualization.
    #[derive(Debug, Copy, Clone)]
    pub enum ProgressId {
        /// The amount of loose objects that have been verified.
        LooseObjects,
    }

    impl From<ProgressId> for gix_utils::progress::Id {
        fn from(v: ProgressId) -> Self {
            match v {
                ProgressId::LooseObjects => *b"VILO",
            }
        }
    }
}

impl Store {
    /// Check all loose objects for their integrity checking their hash matches the actual data and by decoding them fully.
    /// Verification failures include [metadata](gix_error::Error::metadata()) `object_id` (hex text), plus `kind` (object
    /// kind text) after lookup.
    pub fn verify_integrity(
        &self,
        progress: &mut dyn DynNestedProgress,
        should_interrupt: &AtomicBool,
    ) -> Result<integrity::Statistics> {
        self.verify_integrity_with_budget(progress, should_interrupt, None)
    }

    pub(crate) fn verify_integrity_with_budget(
        &self,
        progress: &mut dyn DynNestedProgress,
        should_interrupt: &AtomicBool,
        budget: Option<&Arc<Budget>>,
    ) -> Result<integrity::Statistics> {
        let mut buf = gix_object::find::Buffer::default();
        let mut budget = budget.cloned();

        let mut num_objects = 0;
        let start = Instant::now();
        let mut progress = progress.add_child_with_id("Validating".into(), integrity::ProgressId::LooseObjects.into());
        progress.init(None, gix_utils::progress::count("loose objects"));
        for id in self.iter() {
            check_interrupt(should_interrupt)?;
            let id = id.or_raise(|| message("Could not enumerate loose objects"))?;
            let header = self.try_header(&id).or_raise(|| {
                Message::new("Could not read loose object during verification").with("object_id", id.to_string())
            })?;
            if header.is_some_and(|(_, kind)| kind == gix_object::Kind::Blob) {
                self.verify_blob(&id, should_interrupt).or_raise(|| {
                    Message::new("Could not verify loose object")
                        .with("object_id", id.to_string())
                        .with("path", self.object_path(&id))
                        .with("kind", "blob")
                })?;
                progress.inc();
                num_objects += 1;
                continue;
            }
            let object = self
                .read_for_verification(
                    &id,
                    &mut buf,
                    &mut budget,
                    header.is_some_and(|(size, _)| self.alloc_limit_bytes.is_some_and(|limit| size > limit as u64)),
                    should_interrupt,
                )
                .or_raise(|| {
                    Message::new("Could not read loose object during verification").with("object_id", id.to_string())
                })?
                .ok_or_raise(|| retryable("Objects were deleted during iteration - try again"))?;
            let context = || {
                Message::new("Could not verify loose object")
                    .with("object_id", id.to_string())
                    .with("kind", object.kind.to_string())
            };
            gix_object::compute_hash(self.object_hash, object.kind, object.data)
                .and_then(|actual| actual.verify(&id))
                .or_raise(context)?;
            object.decode().or_raise(|| context().corrupted())?;

            progress.inc();
            num_objects += 1;
            buf.clear();
            check_interrupt(should_interrupt)?;
        }
        progress.show_throughput(start);

        Ok(integrity::Statistics { num_objects })
    }

    fn read_for_verification<'a>(
        &self,
        id: &gix_hash::oid,
        buffer: &'a mut gix_object::find::Buffer,
        budget: &mut Option<Arc<Budget>>,
        mapped: bool,
        interrupt: &AtomicBool,
    ) -> Result<Option<gix_object::Data<'a>>> {
        if !mapped {
            return self.try_find(id, buffer.owned());
        }
        buffer.clear();
        if budget.is_none() {
            *budget = Some(Budget::new(&std::env::temp_dir()).or_error()?);
        }
        let file = std::fs::File::open(self.object_path(id)).or_error()?;
        let (kind, size, reader) = super::stream::reader(file)?;
        let mut spool = budget.as_ref().expect("budget initialized").spool().or_error()?;
        let copied = stream::copy(&mut reader.take(size.saturating_add(1)), &mut spool, interrupt).or_error()?;
        if copied != size {
            bail!(
                gix_error::corruption("Loose object size mismatch")
                    .with("actual", copied)
                    .with("expected", size)
            );
        }
        Ok(Some(gix_object::Data::new(
            buffer.set_external(spool.into_mapped().or_error()?),
            kind,
            self.object_hash,
        )))
    }

    fn verify_blob(&self, id: &gix_hash::oid, interrupt: &AtomicBool) -> Result<()> {
        let file = std::fs::File::open(self.object_path(id)).or_error()?;
        let (kind, size, mut reader) = super::stream::reader(file)?;
        if kind != gix_object::Kind::Blob {
            bail!(retryable("Loose object kind changed during verification"));
        }
        let mut hash = gix_hash::hasher(self.object_hash);
        hash.update(&gix_object::encode::loose_header(kind, size));
        let mut buffer = [0; gix_utils::stream::CHUNK];
        let mut actual = 0u64;
        loop {
            check_interrupt(interrupt)?;
            let count = reader.read(&mut buffer).or_error()?;
            if count == 0 {
                break;
            }
            actual = actual
                .checked_add(count as u64)
                .ok_or_raise(|| gix_error::corruption("Loose object size overflow during verification"))?;
            if actual > size {
                break;
            }
            hash.update(&buffer[..count]);
        }
        if actual != size {
            bail!(
                gix_error::corruption("Loose object size mismatch")
                    .with("actual", actual)
                    .with("expected", size)
            );
        }
        hash.try_finalize()?.verify(id)
    }
}

fn check_interrupt(interrupt: &AtomicBool) -> Result<()> {
    if interrupt.load(Ordering::SeqCst) {
        bail!(ClassificationMarker::with_source(
            Class::Cancelled,
            std::io::Error::from(std::io::ErrorKind::Interrupted),
        ));
    }
    Ok(())
}
