//! Immutable resolved dependency graphs for admitted module artifacts.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
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

/// Upper bound on lock-graph size accepted for validation. Real module
/// graphs are far smaller; the bound keeps validation of untrusted,
/// deserialized graphs cheap and cycle detection iterative-safe.
const MAX_LOCK_GRAPH_NODES: usize = 1024;

impl ModuleDependencyLockGraph {
    pub fn create(
        graph_revision: u64,
        mut nodes: Vec<ModuleDependencyLockNode>,
    ) -> Result<Self, ModuleDependencyLockError> {
        // Revision 0 means "unresolved placeholder". Admitted artifacts and
        // node-reconciliation identities require a real resolution fence, so
        // even an empty (dependency-free) graph carries revision >= 1,
        // assigned by the resolver from its durable counter.
        if graph_revision == 0 {
            return Err(ModuleDependencyLockError::ZeroGraphRevision);
        }
        canonicalize_nodes(&mut nodes);
        validate_nodes(&nodes)?;
        let digest = graph_digest(graph_revision, &nodes)?;
        Ok(Self {
            graph_revision,
            graph_digest: digest,
            nodes,
        })
    }

    pub fn validate(&self) -> Result<(), ModuleDependencyLockError> {
        if self.graph_revision == 0 {
            return Err(ModuleDependencyLockError::ZeroGraphRevision);
        }
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

/// Puts nodes and their dependency lists into the single canonical order
/// used for storage and digest computation.
fn canonicalize_nodes(nodes: &mut [ModuleDependencyLockNode]) {
    for node in nodes.iter_mut() {
        node.dependencies.sort();
    }
    nodes.sort_by(|left, right| left.slug.cmp(&right.slug));
}

fn validate_nodes(nodes: &[ModuleDependencyLockNode]) -> Result<(), ModuleDependencyLockError> {
    if nodes.len() > MAX_LOCK_GRAPH_NODES {
        return Err(ModuleDependencyLockError::TooManyNodes(nodes.len()));
    }
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
        let unique_dependencies: BTreeSet<&str> =
            node.dependencies.iter().map(String::as_str).collect();
        if unique_dependencies.len() != node.dependencies.len() {
            return Err(ModuleDependencyLockError::DuplicateDependency(
                node.slug.clone(),
            ));
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
    detect_cycle(&by_slug)
}

/// Iterative DFS cycle detection: recursion depth must never depend on an
/// untrusted graph's longest dependency chain.
fn detect_cycle(
    nodes: &BTreeMap<&str, &ModuleDependencyLockNode>,
) -> Result<(), ModuleDependencyLockError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Visiting,
        Visited,
    }

    let mut marks: BTreeMap<&str, Mark> = BTreeMap::new();
    for root in nodes.keys() {
        if marks.contains_key(root) {
            continue;
        }
        // Stack entries are (slug, next dependency index to explore).
        let mut stack: Vec<(&str, usize)> = vec![(*root, 0)];
        marks.insert(root, Mark::Visiting);
        while let Some(&(slug, next)) = stack.last() {
            let dependencies = &nodes[slug].dependencies;
            if next >= dependencies.len() {
                marks.insert(slug, Mark::Visited);
                stack.pop();
                continue;
            }
            let dependency = dependencies[next].as_str();
            if let Some(top) = stack.last_mut() {
                top.1 += 1;
            }
            match marks.get(dependency) {
                Some(Mark::Visiting) => {
                    return Err(ModuleDependencyLockError::Cycle(dependency.to_string()));
                }
                Some(Mark::Visited) => {}
                None => {
                    marks.insert(dependency, Mark::Visiting);
                    stack.push((dependency, 0));
                }
            }
        }
    }
    Ok(())
}

fn graph_digest(
    graph_revision: u64,
    nodes: &[ModuleDependencyLockNode],
) -> Result<String, ModuleDependencyLockError> {
    // The digest is always computed over the canonical ordering, so a
    // deserialized graph validates identically regardless of stored order.
    let mut canonical_nodes = nodes.to_vec();
    canonicalize_nodes(&mut canonical_nodes);
    let canonical = serde_json::to_vec(&(graph_revision, canonical_nodes))
        .map_err(|error| ModuleDependencyLockError::Serialize(error.to_string()))?;
    Ok(crate::installation::sha256_digest(&canonical))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModuleDependencyLockError {
    #[error("dependency lock graph revision must be greater than zero")]
    ZeroGraphRevision,
    #[error("dependency lock graph exceeds the maximum node count ({0} nodes)")]
    TooManyNodes(usize),
    #[error("dependency lock graph contains duplicate module nodes")]
    DuplicateNode,
    #[error("module `{0}` declares the same dependency more than once")]
    DuplicateDependency(String),
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
    fn lock_graph_rejects_zero_revision() {
        assert!(matches!(
            ModuleDependencyLockGraph::create(0, Vec::new()),
            Err(ModuleDependencyLockError::ZeroGraphRevision)
        ));
        let mut graph = ModuleDependencyLockGraph::create(1, Vec::new()).expect("empty graph");
        graph.graph_revision = 0;
        assert!(matches!(
            graph.validate(),
            Err(ModuleDependencyLockError::ZeroGraphRevision)
        ));
    }

    #[test]
    fn lock_graph_digest_ignores_dependency_declaration_order() {
        let left = ModuleDependencyLockGraph::create(
            3,
            vec![
                node("app", &["zeta", "base"]),
                node("base", &[]),
                node("zeta", &[]),
            ],
        )
        .expect("left graph");
        let right = ModuleDependencyLockGraph::create(
            3,
            vec![
                node("app", &["base", "zeta"]),
                node("base", &[]),
                node("zeta", &[]),
            ],
        )
        .expect("right graph");
        assert_eq!(left.graph_digest, right.graph_digest);

        // A deserialized graph whose stored order differs still validates.
        let mut shuffled = left.clone();
        shuffled.nodes.reverse();
        shuffled.validate().expect("canonical digest validation");
    }

    #[test]
    fn lock_graph_rejects_cycles() {
        assert!(matches!(
            ModuleDependencyLockGraph::create(1, vec![node("a", &["b"]), node("b", &["a"])]),
            Err(ModuleDependencyLockError::Cycle(_))
        ));
    }
}
