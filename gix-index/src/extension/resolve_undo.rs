use bstr::BString;
use gix_hash::ObjectId;

use crate::{extension::Signature, util::split_at_byte_exclusive};

/// Paths with recorded conflict stages.
pub type Paths = Vec<ResolvePath>;

/// A resolved path and its original conflict stages.
#[derive(Clone)]
pub struct ResolvePath {
    /// relative to the root of the repository, or what would be stored in the index
    pub name: BString,

    /// 0 = ancestor/common, 1 = ours, 2 = theirs
    pub stages: [Option<Stage>; 3],
}

/// One original conflict stage.
#[derive(Clone, Copy)]
pub struct Stage {
    /// Git file mode.
    pub mode: u32,
    /// Object recorded at this stage.
    pub id: ObjectId,
}

/// Git resolve-undo extension signature.
pub const SIGNATURE: Signature = *b"REUC";

/// Decode resolve-undo bytes for the repository object hash format.
pub fn decode(mut data: &[u8], object_hash: gix_hash::Kind) -> Option<Paths> {
    let hash_len = object_hash.len_in_bytes();
    let mut out = Vec::new();

    while !data.is_empty() {
        let (path, rest) = split_at_byte_exclusive(data, 0)?;
        data = rest;

        let mut modes = [0u32; 3];
        for mode in &mut modes {
            let (mode_ascii, rest) = split_at_byte_exclusive(data, 0)?;
            data = rest;
            *mode = u32::from_str_radix(std::str::from_utf8(mode_ascii).ok()?, 8).ok()?;
        }

        let mut stages = [None, None, None];
        for (mode, stage) in modes.iter().zip(stages.iter_mut()) {
            if *mode == 0 {
                continue;
            }
            let (hash, rest) = data.split_at_checked(hash_len)?;
            data = rest;
            *stage = Some(Stage {
                mode: *mode,
                id: ObjectId::from_bytes_or_panic(hash),
            });
        }

        out.push(ResolvePath {
            name: path.into(),
            stages,
        });
    }
    out.into()
}

/// Serialize resolve-undo records with their extension header.
pub fn write_to(paths: &Paths, mut out: impl std::io::Write) -> std::io::Result<()> {
    use std::io::Write;
    let mut bytes = Vec::new();
    for path in paths {
        bytes.extend_from_slice(&path.name);
        bytes.push(0);
        for stage in &path.stages {
            write!(bytes, "{:o}\0", stage.map_or(0, |s| s.mode))?;
        }
        for stage in path.stages.iter().flatten() {
            bytes.extend_from_slice(stage.id.as_bytes());
        }
    }
    let size = u32::try_from(bytes.len()).map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    out.write_all(&SIGNATURE)?;
    out.write_all(&size.to_be_bytes())?;
    out.write_all(&bytes)
}
