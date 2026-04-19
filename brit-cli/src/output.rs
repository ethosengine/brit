//! Output helpers — pretty JSON to stdout, errors to stderr.

use serde::Serialize;

pub fn print_json<T: Serialize>(value: &T) -> Result<(), serde_json::Error> {
    let s = serde_json::to_string_pretty(value)?;
    println!("{s}");
    Ok(())
}
