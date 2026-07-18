use std::path::{Path, PathBuf};

use async_trait::async_trait;
use athanor_app::{
    entity_text, search_project_with_composition, ProjectConfig, RuntimeComposition, SearchOptions,
};
use athanor_core::CanonicalSnapshotStore as _;
use athanor_domain::Entity;
use rustok_ai::{
    RagAtom, RagCandidate, RagError, RagExpandRequest, RagResult, RagRetrievalPort,
    RagSearchRequest, RagSourceRef,
};
use serde_json::json;
use uuid::Uuid;

/// Stable source identifier used by the Rustok/Athanor adapter.
pub const ATHANOR_SOURCE_ID: &str = "athanor";

/// Configuration for one embedded Athanor project.
#[derive(Debug, Clone)]
pub struct AthanorRagConfig {
    pub root: PathBuf,
    pub tenant_id: Uuid,
    pub max_atoms: usize,
}

impl AthanorRagConfig {
    pub fn new(
        root: impl Into<PathBuf>,
        tenant_id: Uuid,
        max_atoms: usize,
    ) -> Result<Self, RagError> {
        if max_atoms == 0 {
            return Err(RagError::InvalidLimit { max: 0 });
        }
        Ok(Self {
            root: root.into(),
            tenant_id,
            max_atoms,
        })
    }
}

/// Athanor-backed Basic RAG provider.
///
/// Search is delegated to Athanor's canonical project search (Tantivy/BM25). Expansion reads
/// the latest canonical snapshot from Athanor's configured store and turns entities/relations
/// into bounded AI atoms. Vector retrieval is deliberately not guessed here: until Athanor's
/// Phase 9 vector adapter is available, `Vector` requests fail explicitly.
pub struct AthanorRagAdapter {
    composition: RuntimeComposition,
    config: AthanorRagConfig,
}

impl AthanorRagAdapter {
    pub fn new(composition: RuntimeComposition, config: AthanorRagConfig) -> Self {
        Self {
            composition,
            config,
        }
    }

    pub fn production(config: AthanorRagConfig) -> Self {
        Self::new(athanor_runtime_defaults::production(), config)
    }

    fn ensure_source_allowed(&self, source_ids: &[String]) -> RagResult<()> {
        if source_ids.is_empty() || source_ids.iter().any(|id| id == ATHANOR_SOURCE_ID) {
            Ok(())
        } else {
            Err(RagError::Provider(
                "requested RAG source is not available in this Athanor adapter".to_string(),
            ))
        }
    }

    fn ensure_tenant_allowed(&self, tenant_id: Uuid) -> RagResult<()> {
        if tenant_id == self.config.tenant_id {
            Ok(())
        } else {
            Err(RagError::Provider(
                "tenant is not allowed to access this Athanor project".to_string(),
            ))
        }
    }

    async fn load_snapshot(&self) -> RagResult<athanor_core::CanonicalSnapshot> {
        let root = canonical_root(&self.config.root)?;
        let config = load_project_config(&root)?;
        let store = self
            .composition
            .init_store(&root, &config)
            .await
            .map_err(provider_error)?;
        store
            .load_latest_snapshot()
            .await
            .map_err(provider_error)?
            .ok_or_else(|| RagError::Provider("Athanor has no indexed canonical snapshot".into()))
    }
}

#[async_trait]
impl RagRetrievalPort for AthanorRagAdapter {
    async fn search(&self, request: RagSearchRequest) -> RagResult<Vec<RagCandidate>> {
        self.ensure_tenant_allowed(request.tenant_id)?;
        self.ensure_source_allowed(&request.source_ids)?;
        if matches!(request.strategy, rustok_ai::RagRetrievalStrategy::Vector) {
            return Err(RagError::Provider(
                "Athanor vector retrieval is not available until its Phase 9 adapter is enabled"
                    .to_string(),
            ));
        }

        let root = canonical_root(&self.config.root)?;
        let report = search_project_with_composition(
            SearchOptions {
                root,
                query: request.query,
                limit: request.limit,
            },
            &self.composition,
        )
        .await
        .map_err(provider_error)?;

        let revision = report.snapshot.clone();
        Ok(report
            .results
            .into_iter()
            .map(|item| RagCandidate {
                atom_id: item.entity_id.0.clone(),
                source: source_ref(
                    &item.entity_id.0,
                    &revision,
                    item.source.as_ref().map(|source| source.path.clone()),
                ),
                score: item.score,
            })
            .collect())
    }

    async fn expand_structure(&self, request: RagExpandRequest) -> RagResult<Vec<RagAtom>> {
        self.ensure_tenant_allowed(request.tenant_id)?;
        let snapshot = self.load_snapshot().await?;
        let revision = snapshot
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.0.as_str())
            .unwrap_or("unknown");
        let wanted = request
            .atom_ids
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        let mut atoms = Vec::new();

        for entity in snapshot
            .entities
            .iter()
            .filter(|entity| wanted.contains(&entity.id.0))
        {
            if atoms.len() >= request.max_atoms.min(self.config.max_atoms) {
                break;
            }
            atoms.push(entity_atom(entity, &snapshot, revision));
        }

        Ok(atoms)
    }
}

fn entity_atom(
    entity: &Entity,
    snapshot: &athanor_core::CanonicalSnapshot,
    revision: &str,
) -> RagAtom {
    let locator = entity.source.as_ref().map(|source| source.path.clone());
    let related_atom_ids = snapshot
        .relations
        .iter()
        .filter_map(|relation| {
            if relation.from == entity.id {
                Some(relation.to.0.clone())
            } else if relation.to == entity.id {
                Some(relation.from.0.clone())
            } else {
                None
            }
        })
        .collect();
    RagAtom {
        atom_id: entity.id.0.clone(),
        source: source_ref(&entity.id.0, revision, locator.clone()),
        text: entity_text(entity),
        path: locator.into_iter().collect(),
        related_atom_ids,
        metadata: json!({
            "stable_key": entity.stable_key.0.clone(),
            "kind": &entity.kind,
            "payload": &entity.payload,
        }),
    }
}

fn source_ref(external_id: &str, revision: &str, locator: Option<String>) -> RagSourceRef {
    RagSourceRef {
        source_id: ATHANOR_SOURCE_ID.to_string(),
        revision: revision.to_string(),
        external_id: external_id.to_string(),
        locator,
    }
}

fn canonical_root(root: &Path) -> RagResult<PathBuf> {
    root.canonicalize().map_err(provider_error)
}

fn load_project_config(root: &Path) -> RagResult<ProjectConfig> {
    let path = root.join(".athanor/config.toml");
    if path.exists() {
        toml::from_str(&std::fs::read_to_string(path).map_err(provider_error)?)
            .map_err(provider_error)
    } else {
        Ok(ProjectConfig::default())
    }
}

fn provider_error(error: impl std::fmt::Display) -> RagError {
    RagError::Provider(error.to_string())
}
