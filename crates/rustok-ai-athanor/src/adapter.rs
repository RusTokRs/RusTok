use std::path::{Path, PathBuf};

use async_trait::async_trait;
use athanor_app::{
    entity_text, search_project_with_composition, ProjectConfig, RuntimeComposition, SearchOptions,
};
use athanor_core::{
    AtomicSnapshotPublication, CanonicalSnapshot, CanonicalSnapshotStore, KnowledgeStore,
    SnapshotBatch,
};
use athanor_domain::{
    Entity, EntityId, EntityKind, RepoId, SnapshotBase, SnapshotId, SourceLocation, StableKey,
};
use rustok_ai::{
    chunk_document, RagAtom, RagCandidate, RagChunk, RagError, RagExpandRequest, RagIngestRequest,
    RagIngestResult, RagIngestionPort, RagResult, RagRetrievalPort, RagSearchRequest, RagSourceRef,
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
impl RagIngestionPort for AthanorRagAdapter {
    async fn publish(
        &self,
        request: RagIngestRequest,
        chunks: Vec<RagChunk>,
    ) -> RagResult<RagIngestResult> {
        self.ensure_tenant_allowed(request.tenant_id)?;
        self.ensure_source_allowed(std::slice::from_ref(&request.document.source.source_id))?;
        if chunks.is_empty() {
            return Err(RagError::EmptyDocument);
        }
        if chunks
            .iter()
            .any(|chunk| chunk.source != request.document.source)
        {
            return Err(RagError::Provider(
                "Athanor ingestion chunks must belong to the requested source document".to_string(),
            ));
        }

        let root = canonical_root(&self.config.root)?;
        let config = load_project_config(&root)?;
        let store = self
            .composition
            .init_store(&root, &config)
            .await
            .map_err(provider_error)?;
        let previous = store.load_latest_snapshot().await.map_err(provider_error)?;
        let parent_snapshot = previous
            .as_ref()
            .and_then(|snapshot| snapshot.snapshot.clone());
        let snapshot = store
            .begin_snapshot(
                RepoId(format!("rustok-ai:{}", request.tenant_id)),
                SnapshotBase {
                    branch: None,
                    commit: None,
                    parent_snapshot,
                    working_tree: true,
                },
            )
            .await
            .map_err(provider_error)?;
        let batch = ingestion_batch(previous, &snapshot, &request, &chunks);

        if let Err(error) = store.publish_snapshot_batch(snapshot.clone(), batch).await {
            let _ = store.abort_snapshot(snapshot).await;
            return Err(provider_error(error));
        }

        Ok(RagIngestResult {
            source: request.document.source,
            chunks,
        })
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
    let rag_payload = entity.payload.get("rag");
    let source = rag_payload
        .and_then(|payload| payload.get("source"))
        .and_then(|source| serde_json::from_value::<RagSourceRef>(source.clone()).ok())
        .unwrap_or_else(|| source_ref(&entity.id.0, revision, locator.clone()));
    let mut path = locator.into_iter().collect::<Vec<_>>();
    if let Some(range) = rag_payload
        .and_then(|payload| payload.get("start_byte").zip(payload.get("end_byte")))
        .and_then(|(start, end)| Some(format!("bytes:{}-{}", start.as_u64()?, end.as_u64()?)))
    {
        path.push(range);
    }
    let text = rag_payload
        .and_then(|payload| payload.get("text"))
        .and_then(|text| text.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| entity_text(entity));
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
        source,
        text,
        path,
        related_atom_ids,
        metadata: json!({
            "stable_key": entity.stable_key.0.clone(),
            "kind": &entity.kind,
            "payload": &entity.payload,
        }),
    }
}

fn ingestion_batch(
    previous: Option<CanonicalSnapshot>,
    snapshot: &SnapshotId,
    request: &RagIngestRequest,
    chunks: &[RagChunk],
) -> SnapshotBatch {
    let mut batch = previous
        .map(|previous| SnapshotBatch {
            entities: previous.entities,
            facts: previous
                .facts
                .into_iter()
                .map(|mut fact| {
                    fact.snapshot = snapshot.clone();
                    fact
                })
                .collect(),
            relations: previous
                .relations
                .into_iter()
                .map(|mut relation| {
                    relation.snapshot = snapshot.clone();
                    relation
                })
                .collect(),
            diagnostics: previous
                .diagnostics
                .into_iter()
                .map(|mut diagnostic| {
                    diagnostic.snapshot = snapshot.clone();
                    diagnostic
                })
                .collect(),
        })
        .unwrap_or_default();

    batch
        .entities
        .retain(|entity| !belongs_to_document(entity, request));
    batch
        .entities
        .extend(chunks.iter().map(|chunk| chunk_entity(request, chunk)));
    batch
}

fn belongs_to_document(entity: &Entity, request: &RagIngestRequest) -> bool {
    let Some(rag) = entity.payload.get("rag") else {
        return false;
    };
    let Some(source) = rag.get("source") else {
        return false;
    };
    let tenant_matches = entity
        .payload
        .get("tenant_id")
        .and_then(|tenant| tenant.as_str())
        .is_some_and(|tenant| tenant == request.tenant_id.to_string());
    let source_matches = source
        .get("source_id")
        .and_then(|source_id| source_id.as_str())
        .is_some_and(|source_id| source_id == request.document.source.source_id)
        && source
            .get("external_id")
            .and_then(|external_id| external_id.as_str())
            .is_some_and(|external_id| external_id == request.document.source.external_id);
    tenant_matches && source_matches
}

fn chunk_entity(request: &RagIngestRequest, chunk: &RagChunk) -> Entity {
    let locator = chunk.source.locator.clone().unwrap_or_else(|| {
        format!(
            "rag://{}/{}",
            chunk.source.source_id, chunk.source.external_id
        )
    });
    let title = request
        .document
        .title
        .clone()
        .or_else(|| Some(format!("RAG chunk {}", chunk.ordinal)));
    Entity {
        id: EntityId(format!("rag:{}", chunk.chunk_id)),
        stable_key: StableKey(format!("rag:{}", chunk.chunk_id)),
        kind: EntityKind::DocumentationSection,
        name: chunk.chunk_id.clone(),
        title,
        source: Some(SourceLocation {
            path: locator,
            line_start: None,
            line_end: None,
        }),
        language: None,
        aliases: vec![chunk.source.external_id.clone()],
        ownership: Vec::new(),
        payload: json!({
            "summary": chunk.text.clone(),
            "tenant_id": request.tenant_id.to_string(),
            "rag": {
                "chunk_id": chunk.chunk_id.clone(),
                "ordinal": chunk.ordinal,
                "start_byte": chunk.start_byte,
                "end_byte": chunk.end_byte,
                "text": chunk.text.clone(),
                "source": chunk.source.clone(),
                "metadata": chunk.metadata.clone(),
            },
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use rustok_ai::{
        RagChunkingPolicy, RagDocument, RagIngestionCoordinator, RagRetrievalStrategy,
    };

    #[tokio::test]
    async fn publishes_and_retrieves_chunks_through_athanor_snapshot() {
        let root = std::env::temp_dir().join(format!("rustok-rag-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("test root should be created");
        let tenant_id = Uuid::new_v4();
        let adapter = Arc::new(AthanorRagAdapter::production(
            AthanorRagConfig::new(&root, tenant_id, 8).expect("config should be valid"),
        ));
        let coordinator = RagIngestionCoordinator::new(adapter.clone());
        let request = RagIngestRequest {
            tenant_id,
            document: RagDocument {
                source: RagSourceRef {
                    source_id: ATHANOR_SOURCE_ID.to_string(),
                    revision: "rev-1".to_string(),
                    external_id: "manual-doc".to_string(),
                    locator: Some("docs/manual.md".to_string()),
                },
                title: Some("Manual".to_string()),
                text: "Athanor stores durable knowledge for retrieval. This chunk is searchable."
                    .to_string(),
                metadata: json!({"kind": "manual"}),
            },
            chunking: RagChunkingPolicy {
                max_chars: 32,
                overlap_chars: 4,
            },
        };
        let expected_chunks =
            chunk_document(&request.document, request.chunking).expect("chunking works");
        let result = coordinator
            .ingest(request.clone())
            .await
            .expect("Athanor publication works");

        assert_eq!(result.source, request.document.source);
        assert_eq!(result.chunks, expected_chunks);
        let candidates = adapter
            .search(RagSearchRequest {
                tenant_id,
                query: "durable knowledge".to_string(),
                strategy: RagRetrievalStrategy::Hybrid,
                limit: 4,
                source_ids: vec![ATHANOR_SOURCE_ID.to_string()],
            })
            .await
            .expect("Tantivy search works");
        assert!(!candidates.is_empty());

        let atoms = adapter
            .expand_structure(RagExpandRequest {
                tenant_id,
                atom_ids: candidates
                    .iter()
                    .map(|candidate| candidate.atom_id.clone())
                    .collect(),
                max_atoms: candidates.len(),
            })
            .await
            .expect("chunk expansion works");
        assert!(!atoms.is_empty());
        assert!(atoms
            .iter()
            .all(|atom| atom.source.external_id == "manual-doc"));
        assert!(atoms
            .iter()
            .all(|atom| atom.path.iter().any(|path| path.starts_with("bytes:"))));
        assert!(atoms.iter().any(|atom| atom.text.contains("knowledge")));

        let mut replacement = request;
        replacement.document.source.revision = "rev-2".to_string();
        replacement.document.text = "Replacement knowledge is published atomically.".to_string();
        let replacement_chunks = chunk_document(&replacement.document, replacement.chunking)
            .expect("replacement chunks");
        coordinator
            .ingest(replacement)
            .await
            .expect("replacement publication works");
        let snapshot = adapter
            .load_snapshot()
            .await
            .expect("snapshot remains readable");
        let rag_entities = snapshot
            .entities
            .iter()
            .filter(|entity| entity.payload.get("rag").is_some())
            .collect::<Vec<_>>();
        assert_eq!(rag_entities.len(), replacement_chunks.len());
        assert!(rag_entities.iter().all(|entity| {
            entity
                .payload
                .pointer("/rag/source/revision")
                .and_then(|revision| revision.as_str())
                == Some("rev-2")
        }));

        let _ = std::fs::remove_dir_all(root);
    }
}
