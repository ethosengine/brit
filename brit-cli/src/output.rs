//! Output helpers — pretty JSON and text lines to stdout through the watched writer.
//!
//! Writes go through [`gitoxide::shared::closed_reader::Watch`] and return their I/O error, so a reader
//! that closed early is seen by its typed `ErrorKind::BrokenPipe` instead of a `println!` panic.

use std::io::Write;

use serde::Serialize;

use crate::error::Result;

/// Print `value` as pretty JSON followed by a newline.
pub fn print_json<T: Serialize>(value: &T) -> Result<()> {
    let s = serde_json::to_string_pretty(value)?;
    print_line(&s)
}

/// Print `line` followed by a newline.
pub fn print_line(line: &str) -> Result<()> {
    let mut stdout = gitoxide::shared::closed_reader::Watch(std::io::stdout().lock());
    writeln!(stdout, "{line}")?;
    stdout.flush()?;
    Ok(())
}
