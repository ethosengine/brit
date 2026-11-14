use crate::OutputFormat;
use gix::{
    Result,
    error::{OptionExt, ResultExt},
};

pub fn previous_branches(repo: gix::Repository, mut out: impl std::io::Write, format: OutputFormat) -> Result<()> {
    let branches = repo
        .head()?
        .prior_checked_out_branches_iter()?
        .ok_or_raise(|| gix::error::not_found("The reflog for HEAD is required"))?;
    match format {
        OutputFormat::Human => {
            for branch in branches {
                let (name, id) = branch?;
                writeln!(out, "{id} {name}").or_error()?;
            }
        }
        #[cfg(feature = "serde")]
        OutputFormat::Json => {
            use serde::ser::{SerializeSeq, Serializer};
            let mut serializer = serde_json::Serializer::pretty(&mut out);
            let mut sequence = (&mut serializer).serialize_seq(None).or_error()?;
            for branch in branches {
                sequence.serialize_element(&branch?).or_error()?;
            }
            sequence.end().or_error()?;
        }
    }
    Ok(())
}
