//! Maintenance-only broker-owned object migration service for persistence revision changes.
//!
//! Provides frozen and digest-pinned source object inventory, per-copy durable intents,
//! verified target reference checkpointing, and strict acceptance verification.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleCommandContext,
    data::{
        configure_tenant_scope, now_expression, placeholder, revision_value, uuid_value,
    },
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataObjectMigrationError {
    #[error("Object migration requires distinct source and target contract revisions")]
    SameRevision,
    #[error("Command context tenant does not match request tenant")]
    TenantMismatch,
    #[error("Reason must not be empty")]
    EmptyReason,
    #[error("Target object '{0}' already exists with different digest; cannot overwrite")]
    TargetObjectConflict(String),
    #[error("Manifest digest mismatch after copy: source {source_manifest}, target {target_manifest}")]
    ManifestMismatch {
        source_manifest: String,
        target_manifest: String,
    },
    #[error("Object count mismatch after copy: source {source_count}, target {target_count}")]
    CountMismatch {
        source_count: u64,
        target_count: u64,
    },
    #[error("Storage error: {0}")]
    Storage(String),
}

impl From<crate::ArtifactDataError> for ArtifactDataObjectMigrationError {
    fn from(err: crate::ArtifactDataError) -> Self {
        ArtifactDataObjectMigrationError::Storage(err.to_string())
    }
}

fn storage_error<E: std::fmt::Display>(e: E) -> ArtifactDataObjectMigrationError {
    ArtifactDataObjectMigrationError::Storage(e.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectMigrationRequest {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub source_contract_revision: u64,
    pub target_contract_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectMigrationReceipt {
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub source_contract_revision: u64,
    pub target_contract_revision: u64,
    pub inventory_manifest_digest: String,
    pub objects_migrated: u64,
    pub accepted: bool,
}

pub struct ArtifactDataObjectMigrationService {
    db: DatabaseConnection,
}

impl ArtifactDataObjectMigrationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Calculates deterministic SHA-256 manifest digest of all objects in the source revision.
    pub async fn calculate_source_inventory_manifest(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        source_contract_revision: u64,
    ) -> Result<(String, u64), ArtifactDataObjectMigrationError> {
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT object_name, digest_sha256, size_bytes \
                     FROM module_artifact_data_objects \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                     ORDER BY object_name ASC",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    module_slug.to_string().into(),
                    revision_value(source_contract_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;

        let mut hasher = Sha256::new();
        hasher.update(tenant_id.as_bytes());
        hasher.update(module_slug.as_bytes());
        hasher.update(&source_contract_revision.to_be_bytes());

        let count = rows.len() as u64;
        for row in rows {
            let object_name: String = row.try_get("", "object_name").map_err(storage_error)?;
            let digest_sha256: String = row.try_get("", "digest_sha256").map_err(storage_error)?;
            let size_bytes: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;

            hasher.update(object_name.as_bytes());
            hasher.update(digest_sha256.as_bytes());
            hasher.update(&size_bytes.to_be_bytes());
        }

        let digest = format!("sha256:{}", hex::encode(hasher.finalize()));
        Ok((digest, count))
    }

    /// Counts live objects in source revision that are not yet migrated to target revision.
    pub async fn count_unmigrated_live_objects(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
        source_contract_revision: u64,
        target_contract_revision: u64,
    ) -> Result<u64, ArtifactDataObjectMigrationError> {
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT COUNT(*) AS count \
                     FROM module_artifact_data_objects src \
                     WHERE src.tenant_id = {} AND src.module_slug = {} AND src.data_contract_revision = {} \
                       AND NOT EXISTS ( \
                           SELECT 1 FROM module_artifact_data_objects tgt \
                           WHERE tgt.tenant_id = src.tenant_id AND tgt.module_slug = src.module_slug \
                             AND tgt.data_contract_revision = {} AND tgt.object_name = src.object_name \
                             AND tgt.digest_sha256 = src.digest_sha256 \
                       )",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(tenant_id, backend),
                    module_slug.to_string().into(),
                    revision_value(source_contract_revision)?,
                    revision_value(target_contract_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| ArtifactDataObjectMigrationError::Storage("query returned no row".to_string()))?;

        let count: i64 = row.try_get("", "count").map_err(storage_error)?;
        Ok(count as u64)
    }

    /// Migrates objects from source contract revision to target contract revision under maintenance authority.
    pub async fn migrate_objects(
        &self,
        request: ArtifactDataObjectMigrationRequest,
    ) -> Result<ArtifactDataObjectMigrationReceipt, ArtifactDataObjectMigrationError> {
        if request.source_contract_revision == request.target_contract_revision {
            return Err(ArtifactDataObjectMigrationError::SameRevision);
        }
        if request.context.tenant_id != Some(request.tenant_id) {
            return Err(ArtifactDataObjectMigrationError::TenantMismatch);
        }
        if request.reason.trim().is_empty() {
            return Err(ArtifactDataObjectMigrationError::EmptyReason);
        }

        let backend = self.db.get_database_backend();
        let (inventory_manifest_digest, source_count) = self
            .calculate_source_inventory_manifest(
                request.tenant_id,
                &request.module_slug,
                request.source_contract_revision,
            )
            .await?;

        let operation_id = Uuid::new_v4();

        // If source has 0 objects, it trivially succeeds with empty manifest
        if source_count == 0 {
            return Ok(ArtifactDataObjectMigrationReceipt {
                operation_id,
                tenant_id: request.tenant_id,
                module_slug: request.module_slug,
                source_contract_revision: request.source_contract_revision,
                target_contract_revision: request.target_contract_revision,
                inventory_manifest_digest,
                objects_migrated: 0,
                accepted: true,
            });
        }

        // Fetch all source objects
        let source_rows = self
            .db
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT object_name, storage_key, content_type, size_bytes, digest_sha256 \
                     FROM module_artifact_data_objects \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                     ORDER BY object_name ASC",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    request.module_slug.clone().into(),
                    revision_value(request.source_contract_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;

        let mut objects_migrated = 0u64;

        for row in source_rows {
            let object_name: String = row.try_get("", "object_name").map_err(storage_error)?;
            let storage_key: String = row.try_get("", "storage_key").map_err(storage_error)?;
            let content_type: String = row.try_get("", "content_type").map_err(storage_error)?;
            let size_bytes: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
            let digest_sha256: String = row.try_get("", "digest_sha256").map_err(storage_error)?;

            let item_op_id = Uuid::new_v4();

            // 1. Reserve per-copy intent
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_data_object_copy_operations (\
                            operation_id, tenant_id, module_slug, source_contract_revision, target_contract_revision, \
                            inventory_manifest_digest, object_name, storage_key, digest_sha256, size_bytes, status, \
                            actor_id, trace_id, correlation_id, idempotency_key, reason, created_at\
                         ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'intent', {}, {}, {}, {}, {}, {})",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                        placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                        placeholder(backend, 7), placeholder(backend, 8), placeholder(backend, 9),
                        placeholder(backend, 10), placeholder(backend, 11), placeholder(backend, 12),
                        placeholder(backend, 13), placeholder(backend, 14), placeholder(backend, 15),
                        now_expression(backend),
                    ),
                    vec![
                        uuid_value(item_op_id, backend),
                        uuid_value(request.tenant_id, backend),
                        request.module_slug.clone().into(),
                        revision_value(request.source_contract_revision)?,
                        revision_value(request.target_contract_revision)?,
                        inventory_manifest_digest.clone().into(),
                        object_name.clone().into(),
                        storage_key.clone().into(),
                        digest_sha256.clone().into(),
                        size_bytes.into(),
                        uuid_value(request.context.actor_id, backend),
                        request.context.trace_id.clone().into(),
                        uuid_value(request.context.correlation_id, backend),
                        uuid_value(request.context.idempotency_key, backend),
                        request.reason.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;

            // 2. Check if target object already exists
            let target_existing = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT digest_sha256, size_bytes \
                         FROM module_artifact_data_objects \
                         WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND object_name = {}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                    ),
                    vec![
                        uuid_value(request.tenant_id, backend),
                        request.module_slug.clone().into(),
                        revision_value(request.target_contract_revision)?,
                        object_name.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;

            if let Some(target_row) = target_existing {
                let existing_digest: String = target_row.try_get("", "digest_sha256").map_err(storage_error)?;
                if existing_digest != digest_sha256 {
                    return Err(ArtifactDataObjectMigrationError::TargetObjectConflict(object_name));
                }
            } else {
                // Insert target reference with distinct target storage key
                let target_storage_key = format!("{}:r{}", storage_key, request.target_contract_revision);
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        format!(
                            "INSERT INTO module_artifact_data_objects (\
                                tenant_id, module_slug, data_contract_revision, object_name, storage_key, \
                                content_type, size_bytes, digest_sha256, revision, created_at, updated_at\
                             ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, 1, {}, {})",
                            placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                            placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                            placeholder(backend, 7), placeholder(backend, 8),
                            now_expression(backend), now_expression(backend),
                        ),
                        vec![
                            uuid_value(request.tenant_id, backend),
                            request.module_slug.clone().into(),
                            revision_value(request.target_contract_revision)?,
                            object_name.clone().into(),
                            target_storage_key.into(),
                            content_type.into(),
                            size_bytes.into(),
                            digest_sha256.into(),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?;
            }

            // 3. Mark intent as checkpointed
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_data_object_copy_operations \
                         SET status = 'checkpointed', committed_at = {} \
                         WHERE operation_id = {}",
                        now_expression(backend),
                        placeholder(backend, 1),
                    ),
                    vec![uuid_value(item_op_id, backend)],
                ))
                .await
                .map_err(storage_error)?;

            objects_migrated += 1;
        }

        // 4. Acceptance check: verify target manifest digest and count match source exactly
        let target_rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT object_name, digest_sha256, size_bytes \
                     FROM module_artifact_data_objects \
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} \
                     ORDER BY object_name ASC",
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

        let target_count = target_rows.len() as u64;
        if target_count != source_count {
            return Err(ArtifactDataObjectMigrationError::CountMismatch {
                source_count,
                target_count,
            });
        }

        let mut target_hasher = Sha256::new();
        target_hasher.update(request.tenant_id.as_bytes());
        target_hasher.update(request.module_slug.as_bytes());
        target_hasher.update(&request.source_contract_revision.to_be_bytes());

        for row in target_rows {
            let object_name: String = row.try_get("", "object_name").map_err(storage_error)?;
            let digest_sha256: String = row.try_get("", "digest_sha256").map_err(storage_error)?;
            let size_bytes: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;

            target_hasher.update(object_name.as_bytes());
            target_hasher.update(digest_sha256.as_bytes());
            target_hasher.update(&size_bytes.to_be_bytes());
        }

        let target_manifest = format!("sha256:{}", hex::encode(target_hasher.finalize()));
        if target_manifest != inventory_manifest_digest {
            return Err(ArtifactDataObjectMigrationError::ManifestMismatch {
                source_manifest: inventory_manifest_digest,
                target_manifest: target_manifest,
            });
        }

        // Monotonically advance target namespace revision
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

        Ok(ArtifactDataObjectMigrationReceipt {
            operation_id,
            tenant_id: request.tenant_id,
            module_slug: request.module_slug,
            source_contract_revision: request.source_contract_revision,
            target_contract_revision: request.target_contract_revision,
            inventory_manifest_digest,
            objects_migrated,
            accepted: true,
        })
    }

    /// Reconciles or clears stale uncommitted intents left by crashed workers.
    pub async fn reconcile_stale_intents(
        &self,
        tenant_id: Uuid,
        module_slug: &str,
    ) -> Result<u64, ArtifactDataObjectMigrationError> {
        let backend = self.db.get_database_backend();
        let result = self
            .db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_object_copy_operations \
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
