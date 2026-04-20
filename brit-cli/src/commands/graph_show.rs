//! brit graph show — emit the build constellation as JSON or Graphviz DOT.

use std::path::Path;

use petgraph::dot::{Config, Dot};
use petgraph::graph::DiGraph;
use serde::Serialize;

use crate::error::{CliError, Result};

#[derive(Serialize)]
struct GraphJson {
    nodes: Vec<NodeJson>,
    edges: Vec<EdgeJson>,
}

#[derive(Serialize)]
struct NodeJson {
    qualified_name: String,
    pipeline: String,
    name: String,
    sources: Vec<String>,
    artifacts: Vec<String>,
}

#[derive(Serialize)]
struct EdgeJson {
    from: String,
    to: String,
}

pub fn run(repo: &Path, format: &str) -> Result<()> {
    let repo = repo.canonicalize().map_err(|source| CliError::RepoNotFound {
        path: repo.display().to_string(),
        source,
    })?;

    let manifests = rakia_core::discover::discover_manifests(&repo)
        .map_err(|e| CliError::ManifestDiscovery(format!("{e}")))?;
    let constellation = rakia_core::constellation::build_constellation(&manifests)?;

    match format {
        "json" => {
            let nodes: Vec<NodeJson> = constellation
                .steps
                .values()
                .map(|s| NodeJson {
                    qualified_name: s.qualified_name.clone(),
                    pipeline: s.pipeline.clone(),
                    name: s.step_name.clone(),
                    sources: s.source_patterns.clone(),
                    artifacts: s.artifacts.clone(),
                })
                .collect();
            let mut edges = Vec::new();
            for s in constellation.steps.values() {
                for dep in &s.resolved_depends {
                    edges.push(EdgeJson {
                        from: dep.clone(),
                        to: s.qualified_name.clone(),
                    });
                }
            }
            crate::output::print_json(&GraphJson { nodes, edges })?;
        }
        "dot" => {
            let mut g: DiGraph<String, ()> = DiGraph::new();
            let mut node_indices = std::collections::HashMap::new();
            for s in constellation.steps.values() {
                let idx = g.add_node(s.qualified_name.clone());
                node_indices.insert(s.qualified_name.clone(), idx);
            }
            for s in constellation.steps.values() {
                let to = node_indices[&s.qualified_name];
                for dep in &s.resolved_depends {
                    if let Some(&from) = node_indices.get(dep) {
                        g.add_edge(from, to, ());
                    }
                }
            }
            let dot = Dot::with_config(&g, &[Config::EdgeNoLabel]);
            println!("{dot:?}");
        }
        other => {
            return Err(CliError::Args(format!("unknown format: {other}")));
        }
    }

    Ok(())
}
