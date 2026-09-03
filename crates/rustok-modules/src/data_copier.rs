//! Maintenance-only crash-safe cross-revision artifact data copier.
//!
//! Provides durable page-level intents, create-only item idempotency,
//! and terminal page checkpoints for migrating structured artifact data
//! between data contract revisions under explicit maintenance authority.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, Statement, TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleCommandContext,
    data::{
        configure_tenant_scope, now_expression, placeholder, revision_value, uuid_from_row,
        uuid_value,
    },
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataCopyError {
    #[error("Cross-revision copy requires distinct source and target contract revisions")]
    SameRevision,
    #[error("Command context tenant does not match request tenant")]
    TenantMismatch,
    #[error("Reason must not be empty")]
    EmptyReason,
    #[error(
        "Target key '{0}' already exists with different value; create-only copier refuses overwrite"
    )]
    TargetKeyConflict(String),
    #[error("Source namespace for revision {0} not found")]
    SourceNamespaceMissing(u64),
    #[error("Target namespace for revision {0} not found")]
    TargetNamespaceMissing(u64),
    #[error("Storage error: {0}")]
    Storage(String),
}

impl From<crate::ArtifactDataError> for ArtifactDataCopyError {
    fn from(err: crate::ArtifactDataError) -> Self {
        ArtifactDataCopyError::Storage(err.to_string())
    }
}

fn storage_error<E: std::fmt::Display>(e: E) -> ArtifactDataCopyError {
    ArtifactDataCopyError::Storage(e.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossRevisionDataCopyRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub source_contract_revision: u64,
    pub target_contract_revision: u64,
    pub page_size: u32,
    pub page_cursor: Option<String>,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossRevisionDataCopyReceipt {
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub source_contract_revision: u64,
    pub target_contract_revision: u64,
    pub page_cursor: Option<String>,
    pub next_page_cursor: Option<String>,
    pub page_digest: String,
    pub items_copied: u64,
    pub is_terminal_page: bool,
    pub status: String,
}

pub struct ArtifactDataCrossRevisionCopier {
    db: DatabaseConnection,
}

impl ArtifactDataCrossRevisionCopier {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Copies a single page of structured records from source contract revision to target contract revision.
    ///
    /// The copy operation is strictly create-only: if a target key already exists with identical value,
    /// it is treated idempotently; if it exists with different value, the operation aborts with `TargetKeyConflict`
    /// without overwriting target data.
    pub async fn copy_page(
        &self,
        request: CrossRevisionDataCopyRequest,
    ) -> Result<CrossRevisionDataCopyReceipt, ArtifactDataCopyError> {
        // 1. Validation
        if request.source_contract_revision == request.target_contract_revision {
            return Err(ArtifactDataCopyError::SameRevision);
        }
        if request.context.tenant_id != Some(request.tenant_id) {
            return Err(ArtifactDataCopyError::TenantMismatch);
        }
        if request.reason.trim().is_empty() {
            return Err(ArtifactDataCopyError::EmptyReason);
        }

        let backend = self.db.get_database_backend();
        let page_size = request.page_size.clamp(1, 100);

        // 2. Check for existing committed operation by idempotency key
        let existing_op = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT operation_id, page_digest, items_count, status \
                     FROM module_artifact_data_copy_operations \
                     WHERE tenant_id = {} AND module_slug = {} \
                       AND source_contract_revision = {} AND target_contract_revision = {} \
                       AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.source_contract_revision)?,
                    revision_value(request.target_contract_revision)?,
                    uuid_value(request.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;

        if let Some(row) = existing_op {
            let status: String = row.try_get("", "status").map_err(storage_error)?;
            if status == "committed" {
                let operation_id = uuid_from_row(&row, "operation_id", backend)?;
                let page_digest: String = row.try_get("", "page_digest").map_err(storage_error)?;
                let items_count: i64 = row.try_get("", "items_count").map_err(storage_error)?;

                return Ok(CrossRevisionDataCopyReceipt {
                    operation_id,
                    tenant_id: request.tenant_id,
                    module_slug: request.module_slug,
                    source_contract_revision: request.source_contract_revision,
                    target_contract_revision: request.target_contract_revision,
                    page_cursor: request.page_cursor,
                    next_page_cursor: None,
                    page_digest,
                    items_copied: items_count as u64,
                    is_terminal_page: true,
                    status: "committed".to_string(),
                });
            }
        }

        // 3. Query source page
        let (query, values) = match &request.page_cursor {
            None => (
                format!(
                    "SELECT data_key, CAST(value AS TEXT) AS value_text, revision \
                     FROM module_artifact_data \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                     ORDER BY data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    page_size,
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.source_contract_revision)?,
                ],
            ),
            Some(cursor) => (
                format!(
                    "SELECT data_key, CAST(value AS TEXT) AS value_text, revision \
                     FROM module_artifact_data \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                       AND data_key > {} \
                     ORDER BY data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    page_size,
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.source_contract_revision)?,
                    cursor.clone().into(),
                ],
            ),
        };

        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(backend, query, values))
            .await
            .map_err(storage_error)?;

        // 4. Compute deterministic page digest
        let mut hasher = Sha256::new();
        hasher.update(request.tenant_id.as_bytes());
        hasher.update(request.module_slug.as_bytes());
        hasher.update(&request.source_contract_revision.to_be_bytes());
        hasher.update(&request.target_contract_revision.to_be_bytes());

        let mut source_items = Vec::with_capacity(rows.len());
        for row in rows {
            let data_key: String = row.try_get("", "data_key").map_err(storage_error)?;
            let value_text: String = row.try_get("", "value_text").map_err(storage_error)?;
            let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;

            hasher.update(data_key.as_bytes());
            hasher.update(value_text.as_bytes());
            hasher.update(&revision.to_be_bytes());

            source_items.push((data_key, value_text));
        }
        let page_digest = format!("sha256:{}", hex::encode(hasher.finalize()));

        let operation_id = Uuid::new_v4();
        let items_count = source_items.len() as u64;
        let is_terminal_page = items_count < page_size as u64;
        let next_page_cursor = if is_terminal_page {
            None
        } else {
            source_items.last().map(|(k, _)| k.clone())
        };

        // 5. Transaction: reserve intent -> create-only copy -> commit receipt
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;

        // 5a. Record durable page intent
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_copy_operations (\
                        operation_id, tenant_id, module_slug, source_contract_revision, target_contract_revision, \
                        page_cursor, page_digest, items_count, status, actor_id, trace_id, correlation_id, \
                        idempotency_key, reason, created_at\
                     ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, 'intent', {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    placeholder(backend, 10),
                    placeholder(backend, 11),
                    placeholder(backend, 12),
                    placeholder(backend, 13),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(operation_id, backend),
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.source_contract_revision)?,
                    revision_value(request.target_contract_revision)?,
                    request.page_cursor.clone().map(SqlValue::from).unwrap_or(SqlValue::String(None)),
                    page_digest.clone().into(),
                    (items_count as i64).into(),
                    uuid_value(request.context.actor_id, backend),
                    request.context.trace_id.clone().into(),
                    uuid_value(request.context.correlation_id, backend),
                    uuid_value(request.context.idempotency_key, backend),
                    request.reason.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;

        // 5b. For each item: create-only idempotency check & insert
        for (data_key, value_text) in &source_items {
            let existing = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT CAST(value AS TEXT) AS value_text \
                         FROM module_artifact_data \
                         WHERE tenant_id = {} AND module_slug = {} \
                           AND data_contract_revision = {} AND data_key = {}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                    ),
                    vec![
                        uuid_value(request.tenant_id, backend),
                        request.module_slug.clone().into(),
                        revision_value(request.target_contract_revision)?,
                        data_key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;

            if let Some(target_row) = existing {
                let existing_value_text: String = target_row
                    .try_get("", "value_text")
                    .map_err(storage_error)?;
                if existing_value_text != *value_text {
                    return Err(ArtifactDataCopyError::TargetKeyConflict(data_key.clone()));
                }
                // Identical value: idempotent no-op
                continue;
            }

            // Parse json to store as proper Json value in DB
            let parsed_value: serde_json::Value =
                serde_json::from_str(value_text).map_err(|e| storage_error(e))?;
            let value_size_bytes = value_text.as_bytes().len() as u64;

            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_data (\
                            tenant_id, module_slug, data_contract_revision, data_key, value, \
                            value_size_bytes, revision, updated_at\
                         ) VALUES ({}, {}, {}, {}, {}, {}, 1, {})",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                        placeholder(backend, 5),
                        placeholder(backend, 6),
                        now_expression(backend),
                    ),
                    vec![
                        uuid_value(request.tenant_id, backend),
                        request.module_slug.clone().into(),
                        revision_value(request.target_contract_revision)?,
                        data_key.clone().into(),
                        SqlValue::Json(Some(Box::new(parsed_value))),
                        revision_value(value_size_bytes)?,
                    ],
                ))
                .await
                .map_err(storage_error)?;
        }

        // 5c. Commit operation receipt
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_copy_operations \
                     SET status = 'committed', committed_at = {} \
                     WHERE operation_id = {}",
                    now_expression(backend),
                    placeholder(backend, 1),
                ),
                vec![uuid_value(operation_id, backend)],
            ))
            .await
            .map_err(storage_error)?;

        // 5d. Monotonically advance target namespace revision
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_namespaces \
                     SET namespace_revision = namespace_revision + 1, updated_at = {} \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    now_expression(backend),
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.target_contract_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;

        transaction.commit().await.map_err(storage_error)?;

        Ok(CrossRevisionDataCopyReceipt {
            operation_id,
            tenant_id: request.tenant_id,
            module_slug: request.module_slug,
            source_contract_revision: request.source_contract_revision,
            target_contract_revision: request.target_contract_revision,
            page_cursor: request.page_cursor,
            next_page_cursor,
            page_digest,
            items_copied: items_count,
            is_terminal_page,
            status: "committed".to_string(),
        })
    }

    /// Reconciles or clears stale uncommitted intents left by crashed workers.
    pub async fn reconcile_stale_intents(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<u64, ArtifactDataCopyError> {
        let backend = self.db.get_database_backend();
        let result = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_copy_operations \
                     SET status = 'failed' \
                     WHERE tenant_id = {} AND module_slug = {} AND status = 'intent'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    module_slug.to_string().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;

        Ok(result.rows_affected())
    }
}
