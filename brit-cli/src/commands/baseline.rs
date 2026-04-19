//! brit baseline — stubs for read/write/migrate. Implementation in Task 19.

use crate::error::Result;
use std::path::Path;

pub fn read(_repo: &Path, _pipeline: &str) -> Result<()> {
    eprintln!("brit baseline read: not yet implemented");
    Ok(())
}

pub fn write(_repo: &Path, _pipeline: &str, _commit: &str) -> Result<()> {
    eprintln!("brit baseline write: not yet implemented");
    Ok(())
}

pub fn migrate(_repo: &Path, _json_path: &Path) -> Result<()> {
    eprintln!("brit baseline migrate: not yet implemented");
    Ok(())
}
