use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};

use crate::models::IndexDocument;

/// Require shared read-port policy for indexed projection lookups.
pub fn require_index_read_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())
}

/// Require shared write-port policy for controlled index rebuild orchestration.
pub fn require_index_rebuild_policy(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::write())
}

/// Transport-neutral selector for index read-model queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexReadSelector {
    DocumentId(Uuid),
    Slug {
        doc_type: String,
        locale: String,
        slug: String,
    },
}

/// Transport-neutral request for reading a single indexed document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexReadRequest {
    pub selector: IndexReadSelector,
}

/// Transport-neutral request for listing indexed documents by owner module/type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexListRequest {
    pub doc_type: String,
    pub locale: Option<String>,
    pub limit: u32,
}

/// Transport-neutral request for rebuilding a module-owned read model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRebuildRequest {
    pub owner_module: String,
    pub entity_type: String,
    pub entity_ids: Vec<Uuid>,
    pub dry_run: bool,
}

/// Transport-neutral rebuild result exposed by the index owner module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexRebuildOutcome {
    pub discovered: u64,
    pub scheduled: u64,
    pub completed: u64,
    pub failed: u64,
    pub truncated: u64,
}

const MAX_INDEX_LIST_LIMIT: u32 = 100;

fn is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

/// Validate a single-document read request before any adapter performs a lookup.
pub fn validate_index_read_request(request: &IndexReadRequest) -> Result<(), PortError> {
    match &request.selector {
        IndexReadSelector::DocumentId(_) => Ok(()),
        IndexReadSelector::Slug {
            doc_type,
            locale,
            slug,
        } => {
            if is_blank(doc_type) {
                return Err(PortError::validation(
                    "index.read_selector_doc_type_empty",
                    "index read slug selector requires a document type",
                ));
            }
            if is_blank(locale) {
                return Err(PortError::validation(
                    "index.read_selector_locale_empty",
                    "index read slug selector requires a locale",
                ));
            }
            if is_blank(slug) {
                return Err(PortError::validation(
                    "index.read_selector_slug_empty",
                    "index read slug selector requires a slug",
                ));
            }
            Ok(())
        }
    }
}

/// Validate a list request and keep cross-module reads bounded.
pub fn validate_index_list_request(request: &IndexListRequest) -> Result<(), PortError> {
    if is_blank(&request.doc_type) {
        return Err(PortError::validation(
            "index.list_doc_type_empty",
            "index list request requires a document type",
        ));
    }
    if request.limit == 0 {
        return Err(PortError::validation(
            "index.list_limit_invalid",
            "index list request limit must be greater than zero",
        ));
    }
    if request.limit > MAX_INDEX_LIST_LIMIT {
        return Err(PortError::validation(
            "index.list_limit_too_large",
            "index list request limit exceeds the module boundary maximum",
        ));
    }
    Ok(())
}

/// Validate rebuild requests before scheduling any write-like rebuild work.
pub fn validate_index_rebuild_request(request: &IndexRebuildRequest) -> Result<(), PortError> {
    if is_blank(&request.owner_module) {
        return Err(PortError::validation(
            "index.rebuild_owner_module_empty",
            "index rebuild request requires an owner module",
        ));
    }
    if is_blank(&request.entity_type) {
        return Err(PortError::validation(
            "index.rebuild_entity_type_empty",
            "index rebuild request requires an entity type",
        ));
    }
    let _dry_run_preserved = request.dry_run;
    Ok(())
}

/// Ensure read adapters never leak another tenant's indexed projection.
pub fn ensure_index_document_tenant_scope(
    expected_tenant_id: Uuid,
    document: &IndexDocument,
) -> Result<(), PortError> {
    if document.tenant_id != expected_tenant_id {
        return Err(PortError::new(
            PortErrorKind::Forbidden,
            "index.tenant_scope_mismatch",
            "index document belongs to a different tenant",
            false,
        ));
    }
    Ok(())
}

/// Typed degraded-mode error for hosts that expose read-only index operations.
pub fn index_rebuild_disabled_error() -> PortError {
    PortError::new(
        PortErrorKind::Unavailable,
        "index.rebuild_disabled",
        "index rebuild orchestration is disabled for this runtime profile",
        true,
    )
}

/// Shared no-compile smoke harness for in-process adapters.
pub fn validate_index_read_smoke(
    context: &PortContext,
    request: &IndexReadRequest,
) -> Result<(), PortError> {
    require_index_read_policy(context)?;
    validate_index_read_request(request)?;
    Ok(())
}

/// Shared no-compile smoke harness for list adapters.
pub fn validate_index_list_smoke(
    context: &PortContext,
    request: &IndexListRequest,
) -> Result<(), PortError> {
    require_index_read_policy(context)?;
    validate_index_list_request(request)?;
    Ok(())
}

/// Shared no-compile smoke harness for rebuild adapters.
pub fn validate_index_rebuild_smoke(
    context: &PortContext,
    request: &IndexRebuildRequest,
) -> Result<(), PortError> {
    require_index_rebuild_policy(context)?;
    validate_index_rebuild_request(request)?;
    Ok(())
}

/// In-process adapter that serves indexed read-model documents from a seeded snapshot.
///
/// This adapter is intentionally persistence-agnostic: hosts can use it for embedded native
/// runtime smoke tests, fixtures, and short-lived read-only profiles while keeping the public
/// `IndexReadModelPort` contract identical to persistence-backed adapters.
#[derive(Debug, Clone, Default)]
pub struct InMemoryIndexReadModelAdapter {
    documents: Vec<IndexDocument>,
}

impl InMemoryIndexReadModelAdapter {
    /// Build an empty adapter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build an adapter from a preloaded document snapshot.
    pub fn from_documents(documents: impl IntoIterator<Item = IndexDocument>) -> Self {
        Self {
            documents: documents.into_iter().collect(),
        }
    }

    fn expected_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
        Uuid::parse_str(&context.tenant_id).map_err(|_| {
            PortError::validation(
                "index.context_tenant_id_invalid",
                "index port context tenant_id must be a UUID for in-process reads",
            )
        })
    }

    fn document_matches_selector(document: &IndexDocument, selector: &IndexReadSelector) -> bool {
        match selector {
            IndexReadSelector::DocumentId(id) => document.id == *id,
            IndexReadSelector::Slug {
                doc_type,
                locale,
                slug,
            } => {
                document.doc_type.to_string() == *doc_type
                    && document.locale == *locale
                    && document.slug == *slug
            }
        }
    }
}

#[async_trait]
impl IndexReadModelPort for InMemoryIndexReadModelAdapter {
    async fn read_index_document(
        &self,
        context: PortContext,
        request: IndexReadRequest,
    ) -> Result<Option<IndexDocument>, PortError> {
        validate_index_read_smoke(&context, &request)?;
        let expected_tenant_id = Self::expected_tenant_id(&context)?;

        let document = self
            .documents
            .iter()
            .find(|document| Self::document_matches_selector(document, &request.selector))
            .cloned();

        if let Some(document) = &document {
            ensure_index_document_tenant_scope(expected_tenant_id, document)?;
        }

        Ok(document)
    }

    async fn list_index_documents(
        &self,
        context: PortContext,
        request: IndexListRequest,
    ) -> Result<Vec<IndexDocument>, PortError> {
        validate_index_list_smoke(&context, &request)?;
        let expected_tenant_id = Self::expected_tenant_id(&context)?;

        let mut documents = Vec::new();
        for document in &self.documents {
            if document.doc_type.to_string() != request.doc_type {
                continue;
            }
            if let Some(locale) = &request.locale {
                if document.locale != *locale {
                    continue;
                }
            }
            ensure_index_document_tenant_scope(expected_tenant_id, document)?;
            documents.push(document.clone());
            if documents.len() >= request.limit as usize {
                break;
            }
        }

        Ok(documents)
    }
}

/// Rebuild adapter for runtime profiles that intentionally expose index reads only.
#[derive(Debug, Clone, Copy, Default)]
pub struct DisabledIndexRebuildAdapter;

#[async_trait]
impl IndexRebuildPort for DisabledIndexRebuildAdapter {
    async fn request_rebuild(
        &self,
        context: PortContext,
        request: IndexRebuildRequest,
    ) -> Result<IndexRebuildOutcome, PortError> {
        validate_index_rebuild_smoke(&context, &request)?;
        Err(index_rebuild_disabled_error())
    }
}

/// Transport-neutral owner boundary for indexed read projections.
#[async_trait]
pub trait IndexReadModelPort: Send + Sync {
    async fn read_index_document(
        &self,
        context: PortContext,
        request: IndexReadRequest,
    ) -> Result<Option<IndexDocument>, PortError>;

    async fn list_index_documents(
        &self,
        context: PortContext,
        request: IndexListRequest,
    ) -> Result<Vec<IndexDocument>, PortError>;
}

/// Transport-neutral owner boundary for controlled index rebuild orchestration.
#[async_trait]
pub trait IndexRebuildPort: Send + Sync {
    async fn request_rebuild(
        &self,
        context: PortContext,
        request: IndexRebuildRequest,
    ) -> Result<IndexRebuildOutcome, PortError>;
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use rustok_api::{PortActor, PortErrorKind};

    use super::*;

    fn tenant_id() -> Uuid {
        Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap()
    }

    fn context() -> PortContext {
        PortContext::new(
            tenant_id().to_string(),
            PortActor::service("index"),
            "ru",
            "corr-a",
        )
    }

    fn document(id: Uuid, tenant_id: Uuid, slug: &str, locale: &str) -> IndexDocument {
        IndexDocument {
            id,
            tenant_id,
            doc_type: crate::models::DocumentType::Product,
            locale: locale.to_string(),
            title: slug.to_string(),
            slug: slug.to_string(),
            content: None,
            keywords: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            published_at: None,
            status: "published".to_string(),
            price: None,
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn index_read_policy_requires_deadline_only() {
        let missing_deadline = context();
        let error = require_index_read_policy(&missing_deadline)
            .expect_err("index reads must carry shared deadline semantics");
        assert_eq!(error.kind, PortErrorKind::Timeout);
        assert_eq!(error.code, "port.deadline_required");

        let with_deadline = context().with_deadline(Duration::from_secs(2));
        assert!(require_index_read_policy(&with_deadline).is_ok());
    }

    #[tokio::test]
    async fn in_memory_adapter_reads_lists_and_preserves_tenant_scope() {
        let owned_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap();
        let adapter = InMemoryIndexReadModelAdapter::from_documents([
            document(owned_id, tenant_id(), "owned", "ru"),
            document(
                Uuid::parse_str("33333333-3333-3333-3333-333333333333").unwrap(),
                Uuid::parse_str("44444444-4444-4444-4444-444444444444").unwrap(),
                "foreign",
                "ru",
            ),
        ]);
        let context = context().with_deadline(Duration::from_secs(2));

        let read = adapter
            .read_index_document(
                context.clone(),
                IndexReadRequest {
                    selector: IndexReadSelector::DocumentId(owned_id),
                },
            )
            .await
            .expect("owned document should be readable")
            .expect("owned document should exist");
        assert_eq!(read.slug, "owned");

        let listed = adapter
            .list_index_documents(
                context.clone(),
                IndexListRequest {
                    doc_type: "product".to_string(),
                    locale: Some("ru".to_string()),
                    limit: 1,
                },
            )
            .await
            .expect("list should return tenant-scoped documents");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, owned_id);

        let error = adapter
            .read_index_document(
                context,
                IndexReadRequest {
                    selector: IndexReadSelector::Slug {
                        doc_type: "product".to_string(),
                        locale: "ru".to_string(),
                        slug: "foreign".to_string(),
                    },
                },
            )
            .await
            .expect_err("cross-tenant document must not leak");
        assert_eq!(error.kind, PortErrorKind::Forbidden);
        assert_eq!(error.code, "index.tenant_scope_mismatch");
    }

    #[tokio::test]
    async fn disabled_rebuild_adapter_validates_policy_before_degraded_error() {
        let adapter = DisabledIndexRebuildAdapter;

        let missing_idempotency = adapter
            .request_rebuild(
                context().with_deadline(Duration::from_secs(2)),
                IndexRebuildRequest {
                    owner_module: "product".to_string(),
                    entity_type: "product".to_string(),
                    entity_ids: Vec::new(),
                    dry_run: true,
                },
            )
            .await
            .expect_err("write policy must be enforced before degraded fallback");
        assert_eq!(missing_idempotency.code, "port.idempotency_key_required");

        let disabled = adapter
            .request_rebuild(
                context()
                    .with_deadline(Duration::from_secs(2))
                    .with_idempotency_key("rebuild-index-corr-a"),
                IndexRebuildRequest {
                    owner_module: "product".to_string(),
                    entity_type: "product".to_string(),
                    entity_ids: Vec::new(),
                    dry_run: true,
                },
            )
            .await
            .expect_err("read-only runtime profile must return typed degraded error");
        assert_eq!(disabled.kind, PortErrorKind::Unavailable);
        assert_eq!(disabled.code, "index.rebuild_disabled");
        assert!(disabled.retryable);
    }

    #[test]
    fn index_rebuild_policy_requires_deadline_and_idempotency_key() {
        let missing_idempotency = context().with_deadline(Duration::from_secs(2));
        let error = require_index_rebuild_policy(&missing_idempotency)
            .expect_err("index rebuilds are write-like controlled operations");
        assert_eq!(error.kind, PortErrorKind::Validation);
        assert_eq!(error.code, "port.idempotency_key_required");

        let valid = context()
            .with_deadline(Duration::from_secs(2))
            .with_idempotency_key("rebuild-index-corr-a");
        assert!(require_index_rebuild_policy(&valid).is_ok());
    }
}
