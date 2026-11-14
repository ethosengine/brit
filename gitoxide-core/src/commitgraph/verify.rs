use crate::OutputFormat;

/// A general purpose context for many operations provided here
pub struct Context<W1: std::io::Write, W2: std::io::Write> {
    /// A stream to which to output errors
    pub err: W2,
    /// A stream to which to output operation results
    pub out: W1,
    pub output_statistics: Option<OutputFormat>,
}

impl Default for Context<Vec<u8>, Vec<u8>> {
    fn default() -> Self {
        Context {
            err: Vec::new(),
            out: Vec::new(),
            output_statistics: None,
        }
    }
}

pub(crate) mod function {
    use gix::Result;
    use gix::error::ResultExt;
    use std::{io, path::Path};

    use crate::OutputFormat;
    use gix::commitgraph::{Graph, verify::Outcome};

    pub fn verify<W1, W2>(
        path: impl AsRef<Path>,
        super::Context {
            err: _err,
            mut out,
            output_statistics,
        }: super::Context<W1, W2>,
    ) -> Result<Outcome>
    where
        W1: io::Write,
        W2: io::Write,
    {
        gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED).or_error()?;
        let g = Graph::at(path.as_ref())?;
        let stats = g.verify_integrity(|_| gix::utils::stream::check(&gix::interrupt::IS_INTERRUPTED))?;

        #[cfg_attr(not(feature = "serde"), allow(clippy::single_match))]
        match output_statistics {
            Some(OutputFormat::Human) => print_human_output(&mut out, &stats).or_error()?,
            #[cfg(feature = "serde")]
            Some(OutputFormat::Json) => serde_json::to_writer_pretty(out, &stats).or_error()?,
            _ => {}
        }

        Ok(stats)
    }

    fn print_human_output(out: &mut impl io::Write, stats: &Outcome) -> io::Result<()> {
        writeln!(out, "number of commits with the given number of parents")?;
        let mut parent_counts: Vec<_> = stats.parent_counts.iter().map(|(a, b)| (*a, *b)).collect();
        parent_counts.sort_by_key(|e| e.0);
        for (parent_count, commit_count) in parent_counts.into_iter() {
            writeln!(out, "\t{parent_count:>2}: {commit_count}")?;
        }
        writeln!(out, "\t->: {}", stats.num_commits)?;

        write!(out, "\nlongest path length between two commits: ")?;
        if let Some(n) = stats.longest_path_length {
            writeln!(out, "{n}")?;
        } else {
            writeln!(out, "unknown")?;
        }

        Ok(())
    }
}
