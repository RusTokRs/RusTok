//! SeaORM implementation of ArtifactDataObjectUploadService.

use bytes::Bytes;
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::objects::*;
use super::objects_persistence::*;
use super::traits::*;
use super::types::*;
use super::upload_sessions::*;
use super::validation::*;
use super::*;

/// Owner service for large-object transfers. Each sandbox invocation carries
/// one bounded chunk, while session and chunk ordering are durable so a retry
/// can resume without granting the artifact storage access.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectUploadService<A> {
    pub(crate) db: DatabaseConnection,
    pub(crate) storage: StorageRuntime,
    pub(crate) objects: SeaOrmArtifactDataObjectBroker<A>,
    pub(crate) authorizer: A,
    pub(crate) infrastructure: ControlPlaneInfrastructure,
    pub(crate) quota: ArtifactDataQuota,
}

impl<A> SeaOrmArtifactDataObjectUploadService<A>
where
    A: ArtifactDataAuthorizer + Clone,
{
    pub fn new(db: DatabaseConnection, storage: StorageRuntime, authorizer: A) -> Self {
        Self::with_infrastructure_and_quota(
            db,
            storage,
            authorizer,
            ControlPlaneInfrastructure::default(),
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self::with_infrastructure_and_quota(
            db,
            storage,
            authorizer,
            infrastructure,
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_infrastructure_and_quota(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
        quota: ArtifactDataQuota,
    ) -> Self {
        Self {
            objects: SeaOrmArtifactDataObjectBroker::with_infrastructure_and_quota(
                db.clone(),
                storage.clone(),
                authorizer.clone(),
                infrastructure.clone(),
                quota,
            ),
            db,
            storage,
            authorizer,
            infrastructure,
            quota,
        }
    }

    pub async fn begin(
        &self,
        scope: &ArtifactDataScope,
        request: ArtifactDataObjectUploadSessionRequest,
    ) -> Result<ArtifactDataObjectUploadSession, ArtifactDataError> {
        let quota = self.quota.validate()?;
        scope.validate()?;
        validate_upload_session_request(&request)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectWrite {
                    name: request.name.clone(),
                },
            )
            .await?;
        let request_digest = upload_session_request_digest(&request)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        if let Some(row) = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT session_id, object_name, content_type, expected_revision, request_digest, CAST(expires_at AS TEXT) AS expires_at
                     FROM module_artifact_data_object_upload_sessions
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {} AND policy_revision = {} AND idempotency_key = {}",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4), placeholder(backend, 5),
                ),
                vec![uuid_value(scope.tenant_id, backend), uuid_value(scope.data_owner_id, backend), uuid_value(scope.namespace_instance_id, backend), revision_value(scope.policy_revision)?, uuid_value(request.idempotency_key, backend)],
            )).await.map_err(storage_error)? {
            let stored_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
            if stored_digest != request_digest {
                return Err(ArtifactDataError::IdempotencyConflict);
            }
            let expected_revision: Option<i64> = row.try_get("", "expected_revision").map_err(storage_error)?;
            let session = ArtifactDataObjectUploadSession {
                session_id: uuid_from_row(&row, "session_id", backend)?,
                name: row.try_get("", "object_name").map_err(storage_error)?,
                content_type: row.try_get("", "content_type").map_err(storage_error)?,
                expected_revision: expected_revision.map(|value| u64::try_from(value).map_err(|_| ArtifactDataError::RevisionConflict)).transpose()?,
                expires_at: row.try_get("", "expires_at").map_err(storage_error)?,
                completed_object: None,
            };
            transaction.commit().await.map_err(storage_error)?;
            return Ok(session);
        }
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT COUNT(*) AS session_count
                     FROM module_artifact_data_object_upload_sessions
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND status IN ('open', 'completing')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ArtifactDataError::Storage("upload session usage was not computed".to_string())
            })?;
        let session_count =
            nonnegative_usage(row.try_get("", "session_count").map_err(storage_error)?)?;
        let projected_sessions = session_count.checked_add(1).ok_or_else(|| {
            ArtifactDataError::Storage("upload session quota overflow".to_string())
        })?;
        enforce_quota(
            "upload_sessions",
            quota.max_upload_sessions,
            projected_sessions,
        )?;
        let session_id = self.infrastructure.new_id();
        transaction.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_object_upload_sessions
                 (session_id, tenant_id, data_owner_id, namespace_instance_id, policy_revision, object_name, content_type, expected_revision, idempotency_key, request_digest, status, expires_at, created_at, updated_at)
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'open', {}, {}, {})",
                placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3), placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6), placeholder(backend, 7), placeholder(backend, 8), placeholder(backend, 9), placeholder(backend, 10), upload_expiry_expression(backend), now_expression(backend), now_expression(backend),
            ),
            vec![uuid_value(session_id, backend), uuid_value(scope.tenant_id, backend), uuid_value(scope.data_owner_id, backend), uuid_value(scope.namespace_instance_id, backend), revision_value(scope.policy_revision)?, request.name.clone().into(), request.content_type.clone().into(), optional_revision_value(request.expected_revision)?, uuid_value(request.idempotency_key, backend), request_digest.into()],
        )).await.map_err(storage_error)?;
        let row = transaction.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT CAST(expires_at AS TEXT) AS expires_at FROM module_artifact_data_object_upload_sessions WHERE session_id = {}", placeholder(backend, 1)),
            vec![uuid_value(session_id, backend)],
        )).await.map_err(storage_error)?.ok_or_else(|| ArtifactDataError::Storage("upload session was not persisted".to_string()))?;
        let expires_at: String = row.try_get("", "expires_at").map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataObjectUploadSession {
            session_id,
            name: request.name,
            content_type: request.content_type,
            expected_revision: request.expected_revision,
            expires_at,
            completed_object: None,
        })
    }

    pub async fn append_chunk(
        &self,
        scope: &ArtifactDataScope,
        chunk: ArtifactDataObjectUploadChunk,
    ) -> Result<(), ArtifactDataError> {
        scope.validate()?;
        if chunk.session_id.is_nil()
            || chunk.data.is_empty()
            || chunk.data.len() > MAX_SANDBOX_ARTIFACT_OBJECT_BYTES
        {
            return Err(ArtifactDataError::InvalidObject);
        }
        let _ = revision_value(chunk.sequence)?;
        let session = self.find_open_session(scope, chunk.session_id).await?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectWrite { name: session.name },
            )
            .await?;
        let quota = self.quota.validate()?;
        let digest_sha256 = format!("sha256:{}", hex::encode(Sha256::digest(&chunk.data)));
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        if let Some(row) = transaction.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT size_bytes, digest_sha256 FROM module_artifact_data_object_upload_chunks WHERE tenant_id = {} AND session_id = {} AND sequence = {}", placeholder(backend,1), placeholder(backend,2), placeholder(backend,3)),
            vec![uuid_value(scope.tenant_id, backend), uuid_value(chunk.session_id, backend), revision_value(chunk.sequence)?],
        )).await.map_err(storage_error)? {
            let size: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
            let digest: String = row.try_get("", "digest_sha256").map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            if u64::try_from(size).ok() == Some(chunk.data.len() as u64) && digest == digest_sha256 { return Ok(()); }
            return Err(ArtifactDataError::IdempotencyConflict);
        }
        transaction.commit().await.map_err(storage_error)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        let active = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_object_upload_sessions
                     SET updated_at = updated_at
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND policy_revision = {} AND session_id = {} AND status = 'open'
                     AND expires_at > {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    revision_value(scope.policy_revision)?,
                    uuid_value(chunk.session_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if active.rows_affected() != 1 {
            let _ = transaction.rollback().await;
            return Err(ArtifactDataError::NamespacePurged);
        }
        if let Some(row) = transaction.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT size_bytes, digest_sha256 FROM module_artifact_data_object_upload_chunks WHERE tenant_id = {} AND session_id = {} AND sequence = {}", placeholder(backend,1), placeholder(backend,2), placeholder(backend,3)),
            vec![uuid_value(scope.tenant_id, backend), uuid_value(chunk.session_id, backend), revision_value(chunk.sequence)?],
        )).await.map_err(storage_error)? {
            let size: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
            let digest: String = row.try_get("", "digest_sha256").map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            if u64::try_from(size).ok() == Some(chunk.data.len() as u64) && digest == digest_sha256 {
                return Ok(());
            }
            return Err(ArtifactDataError::IdempotencyConflict);
        }
        let row = transaction.query_one_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT COALESCE(SUM(size_bytes), 0) AS total_size FROM module_artifact_data_object_upload_chunks WHERE tenant_id = {} AND session_id = {}", placeholder(backend, 1), placeholder(backend, 2)),
            vec![uuid_value(scope.tenant_id, backend), uuid_value(chunk.session_id, backend)],
        )).await.map_err(storage_error)?.ok_or_else(|| ArtifactDataError::Storage("chunk total was not computed".to_string()))?;
        let total_size: i64 = row.try_get("", "total_size").map_err(storage_error)?;
        let total_size =
            u64::try_from(total_size).map_err(|_| ArtifactDataError::ObjectIntegrity)?;
        if total_size
            .checked_add(chunk.data.len() as u64)
            .filter(|size| *size <= MAX_ARTIFACT_OBJECT_BYTES)
            .is_none()
        {
            let _ = transaction.rollback().await;
            return Err(ArtifactDataError::InvalidObject);
        }
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT COALESCE(SUM(chunk.size_bytes), 0) AS total_size
                     FROM module_artifact_data_object_upload_chunks chunk
                     INNER JOIN module_artifact_data_object_upload_sessions session
                       ON session.tenant_id = chunk.tenant_id AND session.session_id = chunk.session_id
                     WHERE session.tenant_id = {} AND session.data_owner_id = {}
                       AND session.namespace_instance_id = {}
                       AND session.status IN ('open', 'completing')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ArtifactDataError::Storage("upload staging usage was not computed".to_string())
            })?;
        let staging_bytes =
            nonnegative_usage(row.try_get("", "total_size").map_err(storage_error)?)?;
        let projected_staging_bytes = staging_bytes
            .checked_add(chunk.data.len() as u64)
            .ok_or_else(|| ArtifactDataError::Storage("staging byte quota overflow".to_string()))?;
        enforce_quota(
            "staging_bytes",
            quota.max_staging_bytes,
            projected_staging_bytes,
        )?;
        let key = ObjectKey::chronological(
            "module-artifact-data",
            ObjectZone::Staging,
            ObjectScope::Namespace {
                tenant_id: scope.tenant_id,
                owner_id: scope.data_owner_id,
                instance_id: scope.namespace_instance_id,
            },
            self.infrastructure.now(),
            self.infrastructure.new_id(),
            "chunk",
        )
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))?
        .to_string();
        let stored_path = key.clone();
        self.storage
            .objects
            .put_opts(
                &Path::from(key),
                chunk.data.clone().into(),
                self.storage.put_options("application/octet-stream"),
            )
            .await
            .map_err(storage_error)?;
        let stored_size = chunk.data.len() as u64;
        let inserted = transaction.execute_raw(Statement::from_sql_and_values(
            backend,
            format!("INSERT INTO module_artifact_data_object_upload_chunks (tenant_id, session_id, sequence, storage_key, size_bytes, digest_sha256, created_at) VALUES ({}, {}, {}, {}, {}, {}, {})", placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5), placeholder(backend,6), now_expression(backend)),
            vec![uuid_value(scope.tenant_id, backend), uuid_value(chunk.session_id, backend), revision_value(chunk.sequence)?, stored_path.clone().into(), i64::try_from(stored_size).map_err(|_| ArtifactDataError::InvalidObject)?.into(), digest_sha256.into()],
        )).await;
        if let Err(error) = inserted {
            let _ = transaction.rollback().await;
            let _ = self.storage.objects.delete(&Path::from(stored_path)).await;
            return Err(storage_error(error));
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }

    pub async fn complete(
        &self,
        scope: &ArtifactDataScope,
        request: ArtifactDataObjectUploadCompleteRequest,
    ) -> Result<ArtifactDataObject, ArtifactDataError> {
        scope.validate()?;
        if request.session_id.is_nil()
            || request.size_bytes == 0
            || request.size_bytes > MAX_ARTIFACT_OBJECT_BYTES
            || !prefixed_sha256_digest(&request.digest_sha256)
        {
            return Err(ArtifactDataError::InvalidObject);
        }
        let session = match self
            .claim_upload_completion(scope, request.session_id)
            .await?
        {
            ArtifactDataObjectUploadCompletion::Active(session) => session,
            ArtifactDataObjectUploadCompletion::Completed { name } => {
                self.authorizer
                    .authorize_data(scope, ArtifactDataAccess::ObjectWrite { name })
                    .await?;
                let object = self
                    .completed_upload_object(scope, request.session_id)
                    .await?;
                if object.size_bytes != request.size_bytes
                    || object.digest_sha256 != request.digest_sha256
                {
                    return Err(ArtifactDataError::IdempotencyConflict);
                }
                return Ok(object);
            }
        };
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectWrite {
                    name: session.name.clone(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let rows = transaction.query_all_raw(Statement::from_sql_and_values(
            backend,
            format!("SELECT sequence, storage_key, size_bytes, digest_sha256 FROM module_artifact_data_object_upload_chunks WHERE tenant_id = {} AND session_id = {} ORDER BY sequence ASC", placeholder(backend,1), placeholder(backend,2)),
            vec![uuid_value(scope.tenant_id, backend), uuid_value(request.session_id, backend)],
        )).await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        let mut payload = Vec::with_capacity(
            usize::try_from(request.size_bytes).map_err(|_| ArtifactDataError::InvalidObject)?,
        );
        for (index, row) in rows.into_iter().enumerate() {
            let sequence: i64 = row.try_get("", "sequence").map_err(storage_error)?;
            if u64::try_from(sequence).ok() != Some(index as u64) {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let key: String = row.try_get("", "storage_key").map_err(storage_error)?;
            let size: i64 = row.try_get("", "size_bytes").map_err(storage_error)?;
            let digest: String = row.try_get("", "digest_sha256").map_err(storage_error)?;
            let bytes = self
                .storage
                .objects
                .get(&Path::from(key))
                .await
                .map_err(storage_error)?
                .bytes()
                .await
                .map_err(storage_error)?;
            if u64::try_from(size).ok() != Some(bytes.len() as u64)
                || format!("sha256:{}", hex::encode(Sha256::digest(&bytes))) != digest
            {
                return Err(ArtifactDataError::ObjectIntegrity);
            }
            let remaining = request.size_bytes.saturating_sub(payload.len() as u64);
            if u64::try_from(bytes.len())
                .ok()
                .filter(|size| *size <= remaining)
                .is_none()
            {
                return Err(ArtifactDataError::ObjectIntegrity);
            }
            payload.extend_from_slice(&bytes);
        }
        if payload.len() as u64 != request.size_bytes
            || format!("sha256:{}", hex::encode(Sha256::digest(&payload))) != request.digest_sha256
        {
            return Err(ArtifactDataError::ObjectIntegrity);
        }
        let object = self
            .objects
            .put_object(
                scope,
                ArtifactDataObjectUpload {
                    name: session.name,
                    content_type: session.content_type,
                    data: Bytes::from(payload),
                    expected_revision: session.expected_revision,
                    idempotency_key: session.idempotency_key,
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let completed = transaction.execute_raw(Statement::from_sql_and_values(backend, format!("UPDATE module_artifact_data_object_upload_sessions SET status = 'completed', completed_revision = {}, completed_at = {}, updated_at = {} WHERE tenant_id = {} AND session_id = {} AND status = 'completing' AND expires_at > {}", placeholder(backend,1), now_expression(backend), now_expression(backend), placeholder(backend,2), placeholder(backend,3), now_expression(backend)), vec![revision_value(object.revision)?, uuid_value(scope.tenant_id, backend), uuid_value(request.session_id, backend)])).await.map_err(storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(ArtifactDataError::NamespacePurged);
        }
        let rows = transaction.query_all_raw(Statement::from_sql_and_values(backend, format!("SELECT storage_key FROM module_artifact_data_object_upload_chunks WHERE tenant_id = {} AND session_id = {}", placeholder(backend,1), placeholder(backend,2)), vec![uuid_value(scope.tenant_id, backend), uuid_value(request.session_id, backend)])).await.map_err(storage_error)?;
        for row in rows {
            let key: String = row.try_get("", "storage_key").map_err(storage_error)?;
            queue_artifact_data_object_gc_candidate(
                &transaction,
                &self.infrastructure,
                scope,
                &key,
            )
            .await?;
        }
        transaction.execute_raw(Statement::from_sql_and_values(backend, format!("DELETE FROM module_artifact_data_object_upload_chunks WHERE tenant_id = {} AND session_id = {}", placeholder(backend,1), placeholder(backend,2)), vec![uuid_value(scope.tenant_id, backend), uuid_value(request.session_id, backend)])).await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(object)
    }
}
