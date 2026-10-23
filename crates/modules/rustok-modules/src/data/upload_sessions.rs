//! Upload session management, reaping, validation, and completion.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
    Value as SqlValue,
};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::objects::*;
use super::objects_persistence::*;
use super::traits::*;
use super::types::*;
use super::upload::*;
use super::validation::*;
use super::*;

impl<A> SeaOrmArtifactDataObjectUploadService<A>
where
    A: ArtifactDataAuthorizer + Clone,
{
    /// Retires expired sessions for one tenant. A deployment scheduler invokes
    /// this owner operation; deletion remains delegated to the retention-aware
    /// object GC service after the chunk keys have been recorded durably.
    pub async fn reap_expired_tenant(
        &self,
        tenant_id: Uuid,
        limit: u32,
    ) -> Result<ArtifactDataObjectUploadReapResult, ArtifactDataError> {
        if tenant_id.is_nil() || limit == 0 || limit > MAX_ARTIFACT_OBJECT_GC_BATCH_SIZE {
            return Err(ArtifactDataError::InvalidObject);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let sessions = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT session.session_id, session.data_owner_id, session.namespace_instance_id,
                            session.policy_revision, namespace.module_slug, namespace.data_contract_revision,
                            namespace.data_contract_digest
                     FROM module_artifact_data_object_upload_sessions session
                     JOIN module_artifact_data_namespaces namespace
                       ON namespace.tenant_id = session.tenant_id AND namespace.data_owner_id = session.data_owner_id
                      AND namespace.namespace_instance_id = session.namespace_instance_id
                     WHERE session.tenant_id = {} AND session.status IN ('open', 'completing') AND session.expires_at <= {}
                     ORDER BY session.expires_at ASC, session.session_id ASC LIMIT {}",
                    backend.placeholder(1),
                    backend.now_sql(),
                    backend.placeholder(2),
                ),
                vec![backend.uuid_value(tenant_id), i64::from(limit).into()],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        let mut result = ArtifactDataObjectUploadReapResult::default();
        for row in sessions {
            let session_id = uuid_from_row(&row, "session_id", backend)?;
            let data_contract_revision: i64 = row
                .try_get("", "data_contract_revision")
                .map_err(storage_error)?;
            let policy_revision: i64 = row.try_get("", "policy_revision").map_err(storage_error)?;
            let scope = ArtifactDataScope {
                tenant_id,
                data_owner_id: uuid_from_row(&row, "data_owner_id", backend)?,
                namespace_instance_id: uuid_from_row(&row, "namespace_instance_id", backend)?,
                data_contract_digest: row
                    .try_get("", "data_contract_digest")
                    .map_err(storage_error)?,
                module_slug: row.try_get("", "module_slug").map_err(storage_error)?,
                data_contract_revision: u64::try_from(data_contract_revision)
                    .map_err(|_| ArtifactDataError::RevisionConflict)?,
                policy_revision: u64::try_from(policy_revision)
                    .map_err(|_| ArtifactDataError::RevisionConflict)?,
            };
            let transaction = self.db.begin().await.map_err(storage_error)?;
            configure_tenant_scope(&transaction, tenant_id).await?;
            let tx_backend =
                SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
            let abandoned = transaction
                .execute_raw(Statement::from_sql_and_values(
                    tx_backend.db_backend(),
                    format!(
                        "UPDATE module_artifact_data_object_upload_sessions
                         SET status = 'abandoned', updated_at = {}
                         WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                         AND policy_revision = {} AND session_id = {}
                         AND status IN ('open', 'completing') AND expires_at <= {}",
                        tx_backend.now_sql(),
                        tx_backend.placeholder(1),
                        tx_backend.placeholder(2),
                        tx_backend.placeholder(3),
                        tx_backend.placeholder(4),
                        tx_backend.placeholder(5),
                        tx_backend.now_sql(),
                    ),
                    vec![
                        tx_backend.uuid_value(tenant_id),
                        backend.uuid_value(scope.data_owner_id),
                        backend.uuid_value(scope.namespace_instance_id),
                        revision_value(scope.policy_revision)?,
                        tx_backend.uuid_value(session_id),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if abandoned.rows_affected() != 1 {
                transaction.commit().await.map_err(storage_error)?;
                continue;
            }
            let chunks = transaction
                .query_all_raw(Statement::from_sql_and_values(
                    tx_backend.db_backend(),
                    format!(
                        "SELECT storage_key FROM module_artifact_data_object_upload_chunks
                         WHERE tenant_id = {} AND session_id = {}",
                        tx_backend.placeholder(1),
                        tx_backend.placeholder(2),
                    ),
                    vec![
                        tx_backend.uuid_value(tenant_id),
                        tx_backend.uuid_value(session_id),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            let mut queued_chunks = 0_u64;
            for chunk in chunks {
                let storage_key: String =
                    chunk.try_get("", "storage_key").map_err(storage_error)?;
                queue_artifact_data_object_gc_candidate(
                    &transaction,
                    &self.infrastructure,
                    &scope,
                    &storage_key,
                )
                .await?;
                queued_chunks = queued_chunks.checked_add(1).ok_or_else(|| {
                    ArtifactDataError::Storage("upload reaper overflow".to_string())
                })?;
            }
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    tx_backend.db_backend(),
                    format!(
                        "DELETE FROM module_artifact_data_object_upload_chunks
                         WHERE tenant_id = {} AND session_id = {}",
                        tx_backend.placeholder(1),
                        tx_backend.placeholder(2),
                    ),
                    vec![
                        tx_backend.uuid_value(tenant_id),
                        tx_backend.uuid_value(session_id),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            result.queued_chunks = result
                .queued_chunks
                .checked_add(queued_chunks)
                .ok_or_else(|| ArtifactDataError::Storage("upload reaper overflow".to_string()))?;
            result.abandoned_sessions = result
                .abandoned_sessions
                .checked_add(1)
                .ok_or_else(|| ArtifactDataError::Storage("upload reaper overflow".to_string()))?;
        }
        Ok(result)
    }

    pub(crate) async fn find_open_session(
        &self,
        scope: &ArtifactDataScope,
        session_id: Uuid,
    ) -> Result<StoredArtifactDataObjectUploadSession, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let row = transaction.query_one_raw(Statement::from_sql_and_values(backend.db_backend(), format!("SELECT object_name, content_type, expected_revision, idempotency_key FROM module_artifact_data_object_upload_sessions WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {} AND policy_revision = {} AND session_id = {} AND status = 'open' AND expires_at > {}", backend.placeholder(1), backend.placeholder(2), backend.placeholder(3), backend.placeholder(4), backend.placeholder(5), backend.now_sql()), vec![backend.uuid_value(scope.tenant_id), backend.uuid_value(scope.data_owner_id), backend.uuid_value(scope.namespace_instance_id), revision_value(scope.policy_revision)?, backend.uuid_value(session_id)])).await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        let row = row.ok_or(ArtifactDataError::NamespacePurged)?;
        let expected_revision: Option<i64> = row
            .try_get("", "expected_revision")
            .map_err(storage_error)?;
        Ok(StoredArtifactDataObjectUploadSession {
            name: row.try_get("", "object_name").map_err(storage_error)?,
            content_type: row.try_get("", "content_type").map_err(storage_error)?,
            expected_revision: expected_revision
                .map(|value| u64::try_from(value).map_err(|_| ArtifactDataError::RevisionConflict))
                .transpose()?,
            idempotency_key: uuid_from_row(&row, "idempotency_key", backend)?,
        })
    }

    pub(crate) async fn claim_upload_completion(
        &self,
        scope: &ArtifactDataScope,
        session_id: Uuid,
    ) -> Result<ArtifactDataObjectUploadCompletion, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT object_name, content_type, expected_revision, idempotency_key, status
                     FROM module_artifact_data_object_upload_sessions
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND policy_revision = {} AND session_id = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    backend.uuid_value(scope.tenant_id),
                    backend.uuid_value(scope.data_owner_id),
                    backend.uuid_value(scope.namespace_instance_id),
                    revision_value(scope.policy_revision)?,
                    backend.uuid_value(session_id),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::NamespacePurged)?;
        let name: String = row.try_get("", "object_name").map_err(storage_error)?;
        let status: String = row.try_get("", "status").map_err(storage_error)?;
        if status == "completed" {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactDataObjectUploadCompletion::Completed { name });
        }
        if status != "open" && status != "completing" {
            return Err(ArtifactDataError::NamespacePurged);
        }
        let claimed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_data_object_upload_sessions
                     SET status = 'completing', expires_at = {}, updated_at = {}
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND policy_revision = {} AND session_id = {}
                     AND status IN ('open', 'completing') AND expires_at > {}",
                    upload_expiry_expression(backend),
                    backend.now_sql(),
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.now_sql(),
                ),
                vec![
                    backend.uuid_value(scope.tenant_id),
                    backend.uuid_value(scope.data_owner_id),
                    backend.uuid_value(scope.namespace_instance_id),
                    revision_value(scope.policy_revision)?,
                    backend.uuid_value(session_id),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if claimed.rows_affected() != 1 {
            return Err(ArtifactDataError::NamespacePurged);
        }
        let expected_revision: Option<i64> = row
            .try_get("", "expected_revision")
            .map_err(storage_error)?;
        let session = StoredArtifactDataObjectUploadSession {
            name,
            content_type: row.try_get("", "content_type").map_err(storage_error)?,
            expected_revision: expected_revision
                .map(|value| u64::try_from(value).map_err(|_| ArtifactDataError::RevisionConflict))
                .transpose()?,
            idempotency_key: uuid_from_row(&row, "idempotency_key", backend)?,
        };
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataObjectUploadCompletion::Active(session))
    }

    pub(crate) async fn completed_upload_object(
        &self,
        scope: &ArtifactDataScope,
        session_id: Uuid,
    ) -> Result<ArtifactDataObject, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let operation =
            find_artifact_data_object_operation(&transaction, scope, session_id).await?;
        transaction.commit().await.map_err(storage_error)?;
        operation.map(|(stored, _)| stored.object).ok_or_else(|| {
            ArtifactDataError::Storage("completed upload result is unavailable".to_string())
        })
    }
}

pub(crate) struct StoredArtifactDataObjectUploadSession {
    pub(crate) name: String,
    pub(crate) content_type: String,
    pub(crate) expected_revision: Option<u64>,
    pub(crate) idempotency_key: Uuid,
}

pub(crate) enum ArtifactDataObjectUploadCompletion {
    Active(StoredArtifactDataObjectUploadSession),
    Completed { name: String },
}

pub(crate) fn validate_upload_session_request(
    request: &ArtifactDataObjectUploadSessionRequest,
) -> Result<(), ArtifactDataError> {
    if request.idempotency_key.is_nil() || request.expected_revision == Some(0) {
        return Err(ArtifactDataError::InvalidIdempotencyKey);
    }
    ArtifactDataObject {
        name: request.name.clone(),
        content_type: request.content_type.clone(),
        size_bytes: 1,
        digest_sha256: format!("sha256:{}", "0".repeat(64)),
        revision: 1,
    }
    .validate()
}

pub(crate) fn upload_session_request_digest(
    request: &ArtifactDataObjectUploadSessionRequest,
) -> Result<String, ArtifactDataError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    Ok(crate::installation::sha256_digest(&bytes))
}

pub(crate) fn upload_expiry_expression(backend: SupportedBackend) -> &'static str {
    match backend {
        SupportedBackend::Postgres => "NOW() + INTERVAL '3600 seconds'",
        SupportedBackend::Sqlite => "strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now', '+3600 seconds')",
    }
}
