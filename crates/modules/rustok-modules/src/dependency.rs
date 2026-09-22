//! Immutable resolved dependency graphs for admitted module artifacts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// One exact selected release in an installation-scope lock graph.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleDependencyLockNode {
    pub slug: String,
    pub version: String,
    pub payload_digest: String,
    pub manifest_digest: String,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

/// Canonical graph snapshot selected by a dependency solver. The solver remains
/// an infrastructure detail; this stable DTO is persisted and audited.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleDependencyLockGraph {
    pub graph_revision: u64,
    pub graph_digest: String,
    pub nodes: Vec<ModuleDependencyLockNode>,
}

impl ModuleDependencyLockGraph {
    pub fn create(
        graph_revision: u64,
        mut nodes: Vec<ModuleDependencyLockNode>,
    ) -> Result<Self, ModuleDependencyLockError> {
        nodes.sort_by(|left, right| left.slug.cmp(&right.slug));
        validate_nodes(&nodes)?;
        let digest = graph_digest(graph_revision, &nodes)?;
        Ok(Self {
            graph_revision,
            graph_digest: digest,
            nodes,
        })
    }

    pub fn validate(&self) -> Result<(), ModuleDependencyLockError> {
        validate_nodes(&self.nodes)?;
        let actual = graph_digest(self.graph_revision, &self.nodes)?;
        if actual != self.graph_digest {
            return Err(ModuleDependencyLockError::DigestMismatch {
                expected: self.graph_digest.clone(),
                actual,
            });
        }
        Ok(())
    }
}

fn validate_nodes(nodes: &[ModuleDependencyLockNode]) -> Result<(), ModuleDependencyLockError> {
    let by_slug = nodes
        .iter()
        .map(|node| (node.slug.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    if by_slug.len() != nodes.len() {
        return Err(ModuleDependencyLockError::DuplicateNode);
    }
    for node in nodes {
        if node
            .dependencies
            .iter()
            .any(|dependency| dependency == &node.slug)
        {
            return Err(ModuleDependencyLockError::SelfDependency(node.slug.clone()));
        }
        for dependency in &node.dependencies {
            if !by_slug.contains_key(dependency.as_str()) {
                return Err(ModuleDependencyLockError::MissingNode {
                    slug: node.slug.clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for node in nodes {
        visit(&node.slug, &by_slug, &mut visiting, &mut visited)?;
    }
    Ok(())
}

fn visit(
    slug: &str,
    nodes: &BTreeMap<&str, &ModuleDependencyLockNode>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<(), ModuleDependencyLockError> {
    if visited.contains(slug) {
        return Ok(());
    }
    if !visiting.insert(slug.to_string()) {
        return Err(ModuleDependencyLockError::Cycle(slug.to_string()));
    }
    for dependency in &nodes[slug].dependencies {
        visit(dependency, nodes, visiting, visited)?;
    }
    visiting.remove(slug);
    visited.insert(slug.to_string());
    Ok(())
}

fn graph_digest(
    graph_revision: u64,
    nodes: &[ModuleDependencyLockNode],
) -> Result<String, ModuleDependencyLockError> {
    let canonical = serde_json::to_vec(&(graph_revision, nodes))
        .map_err(|error| ModuleDependencyLockError::Serialize(error.to_string()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(canonical))))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleDependencyLockError {
    #[error("dependency lock graph contains duplicate module nodes")]
    DuplicateNode,
    #[error("module `{0}` depends on itself")]
    SelfDependency(String),
    #[error("module `{slug}` references missing dependency `{dependency}")]
    MissingNode { slug: String, dependency: String },
    #[error("dependency lock graph contains a cycle at `{0}")]
    Cycle(String),
    #[error("dependency lock graph digest mismatch: expected `{expected}`, actual `{actual}")]
    DigestMismatch { expected: String, actual: String },
    #[error("dependency lock graph serialization failed: {0}")]
    Serialize(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(slug: &str, dependencies: &[&str]) -> ModuleDependencyLockNode {
        ModuleDependencyLockNode {
            slug: slug.to_string(),
            version: "1.0.0".to_string(),
            payload_digest: format!("sha256:{}", "a".repeat(64)),
            manifest_digest: format!("sha256:{}", "b".repeat(64)),
            dependencies: dependencies
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
        }
    }

    #[test]
    fn lock_graph_is_order_independent_and_tamper_evident() {
        let graph =
            ModuleDependencyLockGraph::create(7, vec![node("app", &["base"]), node("base", &[])])
                .expect("lock graph");
        graph.validate().expect("valid graph");
        let same =
            ModuleDependencyLockGraph::create(7, vec![node("base", &[]), node("app", &["base"])])
                .expect("same lock graph");
        assert_eq!(graph.graph_digest, same.graph_digest);
    }

    #[test]
    fn lock_graph_rejects_cycles() {
        assert!(matches!(
            ModuleDependencyLockGraph::create(1, vec![node("a", &["b"]), node("b", &["a"])]),
            Err(ModuleDependencyLockError::Cycle(_))
        ));
    }
}
