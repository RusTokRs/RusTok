//! AI-owned retrieval contracts for the embedded Athanor data plane.
//!
//! `rustok-ai` owns the request, policy, context and citation contracts. The
//! Athanor implementation owns SurrealDB/Tantivy storage and index details and
//! is connected through [`AthanorRagPort`]. No provider-specific database type
//! is exposed here.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RagRetrievalStrategy {
    Structure,
    Hybrid,
    Vector,
}

impl Default for RagRetrievalStrategy {
    fn default() -> Self {
        Self::Hybrid
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RagSourceRef {
    pub source_id: String,
    pub revision: String,
    pub external_id: String,
    pub locator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagSearchRequest {
    pub tenant_id: Uuid,
    pub query: String,
    pub strategy: RagRetrievalStrategy,
    pub limit: usize,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagCandidate {
    pub atom_id: String,
    pub source: RagSourceRef,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagExpandRequest {
    pub tenant_id: Uuid,
    pub atom_ids: Vec<String>,
    pub max_atoms: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagAtom {
    pub atom_id: String,
    pub source: RagSourceRef,
    pub text: String,
    pub path: Vec<String>,
    pub related_atom_ids: Vec<String>,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagCitation {
    pub atom_id: String,
    pub source: RagSourceRef,
    pub path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RagContext {
    pub tenant_id: Uuid,
    pub query: String,
    pub strategy: RagRetrievalStrategy,
    pub atoms: Vec<RagAtom>,
    pub citations: Vec<RagCitation>,
}

#[derive(Debug, Error)]
pub enum RagError {
    #[error("RAG query must not be empty")]
    EmptyQuery,
    #[error("RAG result limit must be between 1 and {max}")]
    InvalidLimit { max: usize },
    #[error("Athanor RAG provider failed: {0}")]
    Provider(String),
    #[error("Athanor returned no atom for candidate `{0}`")]
    MissingAtom(String),
}

pub type RagResult<T> = Result<T, RagError>;

/// Provider-neutral seam implemented by the embedded Athanor module.
///
/// Implementations enforce tenant/source access filters before returning
/// candidates or atoms. The AI layer never receives a SurrealDB/Tantivy handle.
#[async_trait]
pub trait AthanorRagPort: Send + Sync {
    async fn search(&self, request: RagSearchRequest) -> RagResult<Vec<RagCandidate>>;

    async fn expand_structure(&self, request: RagExpandRequest) -> RagResult<Vec<RagAtom>>;
}

pub struct RagCoordinator<P> {
    provider: Arc<P>,
    max_context_atoms: usize,
}

impl<P> RagCoordinator<P>
where
    P: AthanorRagPort + 'static,
{
    pub fn new(provider: Arc<P>, max_context_atoms: usize) -> RagResult<Self> {
        if max_context_atoms == 0 {
            return Err(RagError::InvalidLimit { max: 0 });
        }
        Ok(Self {
            provider,
            max_context_atoms,
        })
    }

    pub async fn retrieve(&self, mut request: RagSearchRequest) -> RagResult<RagContext> {
        validate_request(&request, self.max_context_atoms)?;
        request.limit = request.limit.min(self.max_context_atoms);

        let candidates = self.provider.search(request.clone()).await?;
        let candidates: Vec<_> = candidates.into_iter().take(request.limit).collect();
        let atom_ids: Vec<_> = candidates
            .iter()
            .map(|candidate| candidate.atom_id.clone())
            .collect();
        let atoms = self
            .provider
            .expand_structure(RagExpandRequest {
                tenant_id: request.tenant_id,
                atom_ids,
                max_atoms: self.max_context_atoms,
            })
            .await?;

        let mut by_id: HashMap<String, RagAtom> = atoms
            .into_iter()
            .map(|atom| (atom.atom_id.clone(), atom))
            .collect();
        let mut ordered_atoms = Vec::with_capacity(candidates.len());
        let mut citations = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let atom = by_id
                .remove(&candidate.atom_id)
                .ok_or_else(|| RagError::MissingAtom(candidate.atom_id.clone()))?;
            citations.push(RagCitation {
                atom_id: atom.atom_id.clone(),
                source: atom.source.clone(),
                path: atom.path.clone(),
            });
            ordered_atoms.push(atom);
        }

        Ok(RagContext {
            tenant_id: request.tenant_id,
            query: request.query,
            strategy: request.strategy,
            atoms: ordered_atoms,
            citations,
        })
    }
}

fn validate_request(request: &RagSearchRequest, max_limit: usize) -> RagResult<()> {
    if request.query.trim().is_empty() {
        return Err(RagError::EmptyQuery);
    }
    if request.limit == 0 || request.limit > max_limit {
        return Err(RagError::InvalidLimit { max: max_limit });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StubProvider {
        candidates: Vec<RagCandidate>,
        atoms: Vec<RagAtom>,
    }

    #[async_trait]
    impl AthanorRagPort for StubProvider {
        async fn search(&self, request: RagSearchRequest) -> RagResult<Vec<RagCandidate>> {
            assert_eq!(request.strategy, RagRetrievalStrategy::Hybrid);
            Ok(self.candidates.clone())
        }

        async fn expand_structure(&self, request: RagExpandRequest) -> RagResult<Vec<RagAtom>> {
            Ok(self
                .atoms
                .iter()
                .filter(|atom| request.atom_ids.contains(&atom.atom_id))
                .cloned()
                .collect())
        }
    }

    fn source() -> RagSourceRef {
        RagSourceRef {
            source_id: "athanor-doc".to_string(),
            revision: "rev-1".to_string(),
            external_id: "doc-1".to_string(),
            locator: Some("docs/example.md".to_string()),
        }
    }

    fn candidate(id: &str, score: f32) -> RagCandidate {
        RagCandidate {
            atom_id: id.to_string(),
            source: source(),
            score,
        }
    }

    fn atom(id: &str) -> RagAtom {
        RagAtom {
            atom_id: id.to_string(),
            source: source(),
            text: format!("text for {id}"),
            path: vec!["document".to_string(), id.to_string()],
            related_atom_ids: Vec::new(),
            metadata: serde_json::json!({"kind": "paragraph"}),
        }
    }

    #[tokio::test]
    async fn coordinator_bounds_and_orders_context_by_search_rank() {
        let provider = Arc::new(StubProvider {
            candidates: vec![candidate("second", 0.9), candidate("first", 0.8)],
            atoms: vec![atom("first"), atom("second")],
        });
        let coordinator = RagCoordinator::new(provider, 1).expect("valid limit");
        let context = coordinator
            .retrieve(RagSearchRequest {
                tenant_id: Uuid::nil(),
                query: "return policy".to_string(),
                strategy: RagRetrievalStrategy::Hybrid,
                limit: 1,
                source_ids: Vec::new(),
            })
            .await
            .expect("retrieval succeeds");

        assert_eq!(context.atoms.len(), 1);
        assert_eq!(context.atoms[0].atom_id, "second");
        assert_eq!(context.citations[0].path, vec!["document", "second"]);
    }

    #[tokio::test]
    async fn coordinator_rejects_empty_queries_before_provider_call() {
        let provider = Arc::new(StubProvider {
            candidates: Vec::new(),
            atoms: Vec::new(),
        });
        let coordinator = RagCoordinator::new(provider, 4).expect("valid limit");
        let error = coordinator
            .retrieve(RagSearchRequest {
                tenant_id: Uuid::nil(),
                query: "  ".to_string(),
                strategy: RagRetrievalStrategy::Structure,
                limit: 1,
                source_ids: Vec::new(),
            })
            .await
            .expect_err("empty query must fail");

        assert!(matches!(error, RagError::EmptyQuery));
    }

    #[test]
    fn default_strategy_is_hybrid() {
        assert_eq!(RagRetrievalStrategy::default(), RagRetrievalStrategy::Hybrid);
    }
}
