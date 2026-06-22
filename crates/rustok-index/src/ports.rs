use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::{PortCallPolicy, PortContext, PortError};

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

    fn context() -> PortContext {
        PortContext::new("tenant-a", PortActor::service("index"), "ru", "corr-a")
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
