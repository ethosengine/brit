//! brit fingerprint — deterministic content hash of step inputs.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::error::Result;

#[derive(Serialize)]
struct FingerprintOutput {
    manifest: String,
    fingerprints: Vec<StepFingerprint>,
}

#[derive(Serialize)]
struct StepFingerprint {
    pipeline: String,
    step: String,
    fingerprint: String,
    input_count: usize,
}

pub fn run(manifest_path: &Path, step_filter: Option<&str>) -> Result<()> {
    let text = std::fs::read_to_string(manifest_path)?;
    let m: rakia_core::manifest::BuildManifest = serde_json::from_str(&text)?;

    let mut out = Vec::new();
    for (name, step) in &m.steps {
        if let Some(filter) = step_filter {
            if name != filter {
                continue;
            }
        }
        let mut inputs: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        for src in &step.inputs.sources {
            inputs.insert(format!("source:{src}"), src.as_bytes().to_vec());
        }
        for p in &step.inputs.build_process {
            inputs.insert(format!("buildProcess:{p}"), p.as_bytes().to_vec());
        }
        let fp = brit_graph::fingerprint::ContentFingerprint::compute(&inputs);
        out.push(StepFingerprint {
            pipeline: m.pipeline.clone(),
            step: name.clone(),
            fingerprint: fp.cid.as_str().to_string(),
            input_count: inputs.len(),
        });
    }

    crate::output::print_json(&FingerprintOutput {
        manifest: manifest_path.display().to_string(),
        fingerprints: out,
    })?;
    Ok(())
}
