//! Coverage computation: compare discovered subcommand universe against
//! the set of subcommands that have captured outputs in the staging dir.

use std::collections::BTreeSet;

use crate::discover::SubcommandPath;

#[derive(Debug, Clone)]
pub struct BinaryCoverage {
    pub binary: String,
    pub covered: usize,
    pub total: usize,
    pub uncovered: Vec<SubcommandPath>,
}

impl BinaryCoverage {
    pub fn percent(&self) -> u32 {
        if self.total == 0 {
            100
        } else {
            ((self.covered * 100) / self.total) as u32
        }
    }
}

pub fn compute_coverage(
    binary: &str,
    universe: &[SubcommandPath],
    captured: &BTreeSet<SubcommandPath>,
) -> BinaryCoverage {
    let total = universe.len();
    let mut covered = 0;
    let mut uncovered = Vec::new();
    for path in universe {
        if captured.contains(path) {
            covered += 1;
        } else {
            uncovered.push(path.clone());
        }
    }
    BinaryCoverage {
        binary: binary.to_string(),
        covered,
        total,
        uncovered,
    }
}
