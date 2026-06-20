use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::models::IndexDocument;

/// Transport-agnostic index port context for host/runtime boundary calls.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortContext {
    pub tenant_id: String,
    pub correlation_id: String,
    pub deadline_ms: Option<u64>,
}

impl PortContext {
    pub fn require_deadline_semantics(&self) -> Result<(), PortError> {
        if self.deadline_ms.unwrap_or_default() == 0 {
            return Err(PortError::new(
                PortErrorKind::Timeout,
                "index.deadline_required",
                "index read-model port calls require deadline semantics",
                true,
            ));
        }
        Ok(())
    }

    pub fn require_write_semantics(&self) -> Result<(), PortError> {
        self.require_deadline_semantics()?;
        if self.correlation_id.trim().is_empty() {
            return Err(PortError::new(
                PortErrorKind::Validation,
                "index.correlation_id_required",
                "index ingestion/rebuild port calls require a correlation id for idempotency tracing",
                false,
            ));
        }
        Ok(())
    }
}

/// Transport-neutral error returned by index owner ports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortError {
    pub kind: PortErrorKind,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl PortError {
    pub fn new(
        kind: PortErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortErrorKind {
    Validation,
    NotFound,
    Unavailable,
    Timeout,
}

/// Transport-neutral selector for index read-model queries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndexReadSelector {
    DocumentId(Uuid),
    Slug { doc_type: String, locale: String, slug: String },
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
