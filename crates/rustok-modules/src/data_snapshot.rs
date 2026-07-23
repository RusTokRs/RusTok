use async_trait::async_trait;
use chrono::{DateTime, Utc};
use object_store::{ObjectStoreExt, PutMode, path::Path};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

use rustok_api::manifest_hash::{canonical_manifest_snapshot_json, hash_manifest_snapshot};
use rustok_events::DomainEvent;
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone, StorageRuntime};

use crate::{
    ArtifactDataError, ArtifactDataObject, ArtifactDataRecord, ArtifactDataScope,
    ControlPlaneInfrastructure,
    data::{
        configure_tenant_scope, namespace_lock_clause, now_expression, placeholder, revision_value,
        uuid_from_row, uuid_value,
    },
};

const MAX_SNAPSHOT_RECORDS: usize = 1_000;
const MAX_SNAPSHOT_OBJECTS: usize = 64;
const MAX_SNAPSHOT_INDEX_ROWS: usize = 8_192;
const MAX_SNAPSHOT_OBJECT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_SNAPSHOT_REASON_BYTES: usize = 2_000;
const MAX_SNAPSHOT_COLLECTION_BATCH: u32 = 100;
const MAX_SNAPSHOT_COLLECTION_SCAN: u32 = 1_000;
const MAX_POLICY_SNAPSHOT_ID_BYTES: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotCreateRequest {
    pub scope: ArtifactDataScope,
    pub expected_namespace_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshot {
    pub snapshot_id: Uuid,
    pub scope: ArtifactDataScope,
    pub source_namespace_revision: u64,
    pub retention_revision: u64,
    pub manifest_digest: String,
    pub structured_record_count: u64,
    pub object_count: u64,
    pub total_object_bytes: u64,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataRestoreRequest {
    pub snapshot_id: Uuid,
    pub target: ArtifactDataScope,
    pub expected_namespace_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataRestoreResult {
    pub snapshot_id: Uuid,
    pub namespace_revision: u64,
    pub restored_records: u64,
    pub restored_objects: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotRetentionUpdateRequest {
    pub tenant_id: Uuid,
    pub snapshot_id: Uuid,
    pub expected_retention_revision: u64,
    pub extend_retain_until: Option<DateTime<Utc>>,
    pub legal_hold: Option<bool>,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotRetention {
    pub snapshot_id: Uuid,
    pub retention_revision: u64,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotCollectionRequest {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub reason: String,
    pub policy_snapshot_id: String,
    pub limit: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotCollectionResult {
    pub collected: u64,
    pub retained: u64,
    pub resumed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotCollectionCandidate {
    pub snapshot_id: Uuid,
    pub scope: ArtifactDataScope,
    pub retention_revision: u64,
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub object_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataSnapshotCollectionRule {
    pub audit_hold: bool,
    pub rollback_hold: bool,
    pub collection_approved: bool,
}

#[async_trait]
pub trait ArtifactDataSnapshotRetentionAuthorizer: Send + Sync {
    async fn authorize_retention_update(
        &self,
        request: &ArtifactDataSnapshotRetentionUpdateRequest,
    ) -> Result<(), ArtifactDataError>;
}

#[async_trait]
pub trait ArtifactDataSnapshotCollectionPolicy: Send + Sync {
    fn snapshot_id(&self) -> &str;

    async fn may_collect(
        &self,
        candidate: &ArtifactDataSnapshotCollectionCandidate,
    ) -> Result<bool, ArtifactDataError>;
}

#[async_trait]
pub trait ArtifactDataSnapshotCollectionAuthorizer: Send + Sync {
    async fn authorize_collection(
        &self,
        request: &ArtifactDataSnapshotCollectionRequest,
    ) -> Result<(), ArtifactDataError>;
}

pub struct SnapshotArtifactDataSnapshotCollectionPolicy {
    snapshot_id: String,
    rules: HashMap<Uuid, ArtifactDataSnapshotCollectionRule>,
}

impl SnapshotArtifactDataSnapshotCollectionPolicy {
    pub fn new(
        snapshot_id: String,
        rules: HashMap<Uuid, ArtifactDataSnapshotCollectionRule>,
    ) -> Result<Self, ArtifactDataError> {
        if !valid_policy_snapshot_id(&snapshot_id) {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        Ok(Self { snapshot_id, rules })
    }
}

#[async_trait]
impl ArtifactDataSnapshotCollectionPolicy for SnapshotArtifactDataSnapshotCollectionPolicy {
    fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    async fn may_collect(
        &self,
        candidate: &ArtifactDataSnapshotCollectionCandidate,
    ) -> Result<bool, ArtifactDataError> {
        Ok(self.rules.get(&candidate.snapshot_id).is_some_and(|rule| {
            rule.collection_approved && !rule.audit_hold && !rule.rollback_hold
        }))
    }
}

#[async_trait]
pub trait ArtifactDataSnapshotAuthorizer: Send + Sync {
    async fn authorize_snapshot(
        &self,
        request: &ArtifactDataSnapshotCreateRequest,
    ) -> Result<(), ArtifactDataError>;

    async fn authorize_restore(
        &self,
        request: &ArtifactDataRestoreRequest,
    ) -> Result<(), ArtifactDataError>;
}

#[derive(Clone)]
pub struct SeaOrmArtifactDataSnapshotService<A> {
    db: DatabaseConnection,
    storage: StorageRuntime,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmArtifactDataSnapshotService<A>
where
    A: ArtifactDataSnapshotAuthorizer,
{
    pub fn new(db: DatabaseConnection, storage: StorageRuntime, authorizer: A) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, storage, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            storage,
            authorizer,
            infrastructure,
        }
    }

    pub async fn create(
        &self,
        request: ArtifactDataSnapshotCreateRequest,
    ) -> Result<ArtifactDataSnapshot, ArtifactDataError> {
        validate_create_request(&request)?;
        self.authorizer.authorize_snapshot(&request).await?;
        let request_digest = digest_json(&request)?;
        let snapshot_id = self.stage_snapshot(&request, &request_digest).await?;
        self.copy_snapshot_objects(request.scope.tenant_id, snapshot_id)
            .await?;
        self.finalize_snapshot(request.scope.tenant_id, snapshot_id)
            .await
    }

    pub async fn restore(
        &self,
        request: ArtifactDataRestoreRequest,
    ) -> Result<ArtifactDataRestoreResult, ArtifactDataError> {
        validate_restore_request(&request)?;
        self.authorizer.authorize_restore(&request).await?;
        let request_digest = digest_json(&request)?;
        if let Some(result) = self
            .find_restore_operation(&request, &request_digest)
            .await?
        {
            return Ok(result);
        }
        let snapshot = self
            .load_ready_snapshot(request.target.tenant_id, request.snapshot_id)
            .await?;
        if !same_data_namespace(&snapshot.scope, &request.target) {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        let objects = self
            .load_snapshot_objects(request.target.tenant_id, request.snapshot_id)
            .await?;
        if objects.len() > MAX_SNAPSHOT_OBJECTS {
            return Err(ArtifactDataError::SnapshotLimitExceeded);
        }
        let copied = self.copy_restore_objects(&request.target, &objects).await?;
        let result = self
            .commit_restore(&request, &request_digest, &snapshot, &objects, &copied)
            .await;
        if result.is_err() {
            for storage_key in copied {
                let _ = self.storage.objects.delete(&Path::from(storage_key)).await;
            }
        }
        result
    }

    async fn stage_snapshot(
        &self,
        request: &ArtifactDataSnapshotCreateRequest,
        request_digest: &str,
    ) -> Result<Uuid, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, request.scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        if let Some(row) = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT snapshot_id, request_digest FROM module_artifact_data_snapshots
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    uuid_value(request.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?
        {
            let stored_digest: String = row
                .try_get("", "request_digest")
                .map_err(snapshot_storage_error)?;
            if stored_digest != request_digest {
                return Err(ArtifactDataError::IdempotencyConflict);
            }
            let snapshot_id = uuid_from_row(&row, "snapshot_id", backend)?;
            transaction.commit().await.map_err(snapshot_storage_error)?;
            return Ok(snapshot_id);
        }

        let namespace = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT namespace_revision, CASE WHEN purged_at IS NULL THEN 0 ELSE 1 END AS is_purged
                     FROM module_artifact_data_namespaces
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}{}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    namespace_lock_clause(backend),
                ),
                scope_values(&request.scope, backend)?,
            ))
            .await
            .map_err(snapshot_storage_error)?
            .ok_or(ArtifactDataError::SnapshotPrecondition)?;
        let namespace_revision = positive_u64(&namespace, "namespace_revision")?;
        let is_purged: i64 = namespace
            .try_get("", "is_purged")
            .map_err(snapshot_storage_error)?;
        if is_purged != 0 || namespace_revision != request.expected_namespace_revision {
            return Err(ArtifactDataError::SnapshotPrecondition);
        }

        let records = query_snapshot_records(&transaction, &request.scope).await?;
        let objects = query_source_objects(&transaction, &request.scope).await?;
        let indexes = query_snapshot_indexes(&transaction, &request.scope).await?;
        let index_contract = query_index_contract(&transaction, &request.scope).await?;
        if records.len() > MAX_SNAPSHOT_RECORDS
            || objects.len() > MAX_SNAPSHOT_OBJECTS
            || indexes.len() > MAX_SNAPSHOT_INDEX_ROWS
        {
            return Err(ArtifactDataError::SnapshotLimitExceeded);
        }
        let total_object_bytes = objects.iter().try_fold(0_u64, |total, object| {
            total
                .checked_add(object.object.size_bytes)
                .filter(|total| *total <= MAX_SNAPSHOT_OBJECT_BYTES)
                .ok_or(ArtifactDataError::SnapshotLimitExceeded)
        })?;
        let snapshot_id = self.infrastructure.new_id();
        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_snapshots
                     (snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision,
                      source_namespace_revision, status, retention_revision, request_digest, manifest_digest, actor_id,
                      reason, idempotency_key, structured_record_count, object_count,
                      total_object_bytes, retain_until, legal_hold, created_at, ready_at)
                     VALUES ({}, {}, {}, {}, {}, {}, 'staging', 1, {}, NULL, {}, {}, {}, {}, {}, {}, {}, {}, {}, NULL)
                     ON CONFLICT (tenant_id, module_slug, data_contract_revision, idempotency_key) DO NOTHING",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                    placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                    placeholder(backend, 7), placeholder(backend, 8), placeholder(backend, 9),
                    placeholder(backend, 10), placeholder(backend, 11), placeholder(backend, 12),
                    placeholder(backend, 13), placeholder(backend, 14), placeholder(backend, 15),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(snapshot_id, backend),
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    revision_value(request.scope.policy_revision)?,
                    revision_value(namespace_revision)?,
                    request_digest.to_owned().into(),
                    uuid_value(request.actor_id, backend),
                    request.reason.clone().into(),
                    uuid_value(request.idempotency_key, backend),
                    revision_value(records.len() as u64)?,
                    revision_value(objects.len() as u64)?,
                    revision_value(total_object_bytes)?,
                    datetime_value(request.retain_until, backend),
                    request.legal_hold.into(),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if inserted.rows_affected() == 0 {
            let row = transaction
                .query_one(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "SELECT snapshot_id, request_digest FROM module_artifact_data_snapshots
                         WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                        placeholder(backend, 1), placeholder(backend, 2),
                        placeholder(backend, 3), placeholder(backend, 4),
                    ),
                    vec![
                        uuid_value(request.scope.tenant_id, backend),
                        request.scope.module_slug.clone().into(),
                        revision_value(request.scope.data_contract_revision)?,
                        uuid_value(request.idempotency_key, backend),
                    ],
                ))
                .await
                .map_err(snapshot_storage_error)?
                .ok_or(ArtifactDataError::IdempotencyConflict)?;
            let stored_digest: String = row
                .try_get("", "request_digest")
                .map_err(snapshot_storage_error)?;
            if stored_digest != request_digest {
                return Err(ArtifactDataError::IdempotencyConflict);
            }
            let existing_snapshot_id = uuid_from_row(&row, "snapshot_id", backend)?;
            transaction.commit().await.map_err(snapshot_storage_error)?;
            return Ok(existing_snapshot_id);
        }
        persist_snapshot_rows(
            &transaction,
            request.scope.tenant_id,
            snapshot_id,
            &records,
            &objects,
            &indexes,
            index_contract.as_deref(),
        )
        .await?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(snapshot_id)
    }

    async fn copy_snapshot_objects(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<(), ArtifactDataError> {
        let objects = self.load_snapshot_objects(tenant_id, snapshot_id).await?;
        if objects.len() > MAX_SNAPSHOT_OBJECTS {
            return Err(ArtifactDataError::SnapshotLimitExceeded);
        }
        for object in objects {
            if object.snapshot_storage_key.is_some() {
                continue;
            }
            let bytes = self
                .storage
                .objects
                .get(&Path::from(object.source_storage_key.as_str()))
                .await
                .map_err(snapshot_storage_error)?
                .bytes()
                .await
                .map_err(snapshot_storage_error)?;
            verify_object_bytes(&object.object, &bytes)?;
            let storage_key = ObjectKey::chronological(
                "module-artifact-data-snapshot",
                ObjectZone::Objects,
                ObjectScope::Tenant(tenant_id),
                self.infrastructure.now(),
                self.infrastructure.new_id(),
                "snapshot",
            )
            .map_err(|error| ArtifactDataError::Storage(error.to_string()))?
            .to_string();
            let mut options = self.storage.put_options(&object.object.content_type);
            options.mode = PutMode::Create;
            let created = match self
                .storage
                .objects
                .put_opts(&Path::from(storage_key.as_str()), bytes.into(), options)
                .await
            {
                Ok(_) => true,
                Err(object_store::Error::AlreadyExists { .. }) => false,
                Err(error) => return Err(snapshot_storage_error(error)),
            };
            if !created {
                let stored = self
                    .storage
                    .objects
                    .get(&Path::from(storage_key.as_str()))
                    .await
                    .map_err(snapshot_storage_error)?
                    .bytes()
                    .await
                    .map_err(snapshot_storage_error)?;
                verify_object_bytes(&object.object, &stored)?;
            }
            let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
            configure_tenant_scope(&transaction, tenant_id).await?;
            let backend = transaction.get_database_backend();
            transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_data_snapshot_objects SET snapshot_storage_key = {}
                         WHERE tenant_id = {} AND snapshot_id = {} AND object_name = {} AND snapshot_storage_key IS NULL",
                        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                        placeholder(backend, 4),
                    ),
                    vec![
                        storage_key.into(),
                        uuid_value(tenant_id, backend),
                        uuid_value(snapshot_id, backend),
                        object.object.name.into(),
                    ],
                ))
                .await
                .map_err(snapshot_storage_error)?;
            transaction.commit().await.map_err(snapshot_storage_error)?;
        }
        Ok(())
    }

    async fn finalize_snapshot(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<ArtifactDataSnapshot, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend = transaction.get_database_backend();
        let row = lock_snapshot(&transaction, tenant_id, snapshot_id).await?;
        let status: String = row.try_get("", "status").map_err(snapshot_storage_error)?;
        if status == "ready" {
            let snapshot = snapshot_from_row(&row, backend)?;
            let manifest = load_manifest(&transaction, tenant_id, snapshot_id).await?;
            if !manifest_within_limits(&manifest)
                || digest_json(&manifest.logical())? != snapshot.manifest_digest
                || !manifest_matches_snapshot(&manifest, &snapshot)
            {
                return Err(ArtifactDataError::SnapshotIntegrity);
            }
            transaction.commit().await.map_err(snapshot_storage_error)?;
            return Ok(snapshot);
        }
        if status != "staging" {
            return Err(ArtifactDataError::SnapshotPrecondition);
        }
        let actor_id = uuid_from_row(&row, "actor_id", backend)?;
        let manifest = load_manifest(&transaction, tenant_id, snapshot_id).await?;
        if !manifest_within_limits(&manifest)
            || manifest
                .objects
                .iter()
                .any(|object| object.snapshot_storage_key.is_none())
        {
            return Err(ArtifactDataError::SnapshotPrecondition);
        }
        let manifest_digest = digest_json(&manifest.logical())?;
        let staged_snapshot =
            snapshot_from_row_with_digest(&row, backend, manifest_digest.clone())?;
        if !manifest_matches_snapshot(&manifest, &staged_snapshot) {
            return Err(ArtifactDataError::SnapshotIntegrity);
        }
        let finalized = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_snapshots SET status = 'ready', manifest_digest = {}, ready_at = {}
                     WHERE tenant_id = {} AND snapshot_id = {} AND status = 'staging'",
                    placeholder(backend, 1), now_expression(backend), placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    manifest_digest.clone().into(),
                    uuid_value(tenant_id, backend),
                    uuid_value(snapshot_id, backend),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if finalized.rows_affected() != 1 {
            return Err(ArtifactDataError::SnapshotPrecondition);
        }
        let snapshot = staged_snapshot;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    Some(tenant_id),
                    Some(actor_id),
                    DomainEvent::ModuleArtifactDataSnapshotCreated {
                        snapshot_id,
                        tenant_id,
                        module_slug: snapshot.scope.module_slug.clone(),
                        data_contract_revision: snapshot.scope.data_contract_revision,
                        namespace_revision: snapshot.source_namespace_revision,
                        manifest_digest,
                        structured_records: snapshot.structured_record_count,
                        objects: snapshot.object_count,
                    },
                ),
            )
            .await
            .map_err(snapshot_storage_error)?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(snapshot)
    }

    async fn load_ready_snapshot(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<ArtifactDataSnapshot, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let backend = transaction.get_database_backend();
        let row = lock_snapshot(&transaction, tenant_id, snapshot_id).await?;
        let snapshot = snapshot_from_row(&row, backend)?;
        let status: String = row.try_get("", "status").map_err(snapshot_storage_error)?;
        if status != "ready" {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        let manifest = load_manifest(&transaction, tenant_id, snapshot_id).await?;
        let digest = digest_json(&manifest.logical())?;
        if !manifest_within_limits(&manifest)
            || digest != snapshot.manifest_digest
            || !manifest_matches_snapshot(&manifest, &snapshot)
        {
            return Err(ArtifactDataError::SnapshotIntegrity);
        }
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(snapshot)
    }

    async fn load_snapshot_objects(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<Vec<SnapshotObject>, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let objects = query_stored_snapshot_objects(&transaction, tenant_id, snapshot_id).await?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(objects)
    }

    async fn copy_restore_objects(
        &self,
        scope: &ArtifactDataScope,
        objects: &[SnapshotObject],
    ) -> Result<Vec<String>, ArtifactDataError> {
        let mut copied = Vec::with_capacity(objects.len());
        for object in objects {
            let snapshot_key = object
                .snapshot_storage_key
                .as_deref()
                .ok_or(ArtifactDataError::SnapshotIntegrity)?;
            let bytes = self
                .storage
                .objects
                .get(&Path::from(snapshot_key))
                .await
                .map_err(snapshot_storage_error)?
                .bytes()
                .await
                .map_err(snapshot_storage_error)?;
            verify_object_bytes(&object.object, &bytes)?;
            let storage_key = ObjectKey::chronological(
                "module-artifact-data",
                ObjectZone::Objects,
                ObjectScope::Tenant(scope.tenant_id),
                self.infrastructure.now(),
                self.infrastructure.new_id(),
                "bin",
            )
            .map_err(|error| ArtifactDataError::Storage(error.to_string()))?
            .to_string();
            if let Err(error) = self
                .storage
                .objects
                .put_opts(
                    &Path::from(storage_key.as_str()),
                    bytes.into(),
                    self.storage.put_options(&object.object.content_type),
                )
                .await
            {
                for copied_key in copied {
                    let _ = self.storage.objects.delete(&Path::from(copied_key)).await;
                }
                return Err(snapshot_storage_error(error));
            }
            copied.push(storage_key);
        }
        Ok(copied)
    }

    async fn commit_restore(
        &self,
        request: &ArtifactDataRestoreRequest,
        request_digest: &str,
        snapshot: &ArtifactDataSnapshot,
        objects: &[SnapshotObject],
        copied: &[String],
    ) -> Result<ArtifactDataRestoreResult, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, request.target.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let (namespace_revision, namespace_purged) =
            lock_restore_namespace(&transaction, request).await?;
        if let Some(result) =
            find_restore_operation_in(&transaction, request, request_digest).await?
        {
            transaction.commit().await.map_err(snapshot_storage_error)?;
            for storage_key in copied {
                let _ = self
                    .storage
                    .objects
                    .delete(&Path::from(storage_key.as_str()))
                    .await;
            }
            return Ok(result);
        }
        if namespace_purged || namespace_revision != request.expected_namespace_revision {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        ensure_namespace_empty(&transaction, &request.target).await?;
        let snapshot_row =
            lock_snapshot(&transaction, request.target.tenant_id, request.snapshot_id).await?;
        let status: String = snapshot_row
            .try_get("", "status")
            .map_err(snapshot_storage_error)?;
        let manifest_digest: Option<String> = snapshot_row
            .try_get("", "manifest_digest")
            .map_err(snapshot_storage_error)?;
        if status != "ready"
            || manifest_digest.as_deref() != Some(snapshot.manifest_digest.as_str())
        {
            return Err(ArtifactDataError::SnapshotIntegrity);
        }
        let manifest =
            load_manifest(&transaction, request.target.tenant_id, request.snapshot_id).await?;
        if !manifest_within_limits(&manifest)
            || digest_json(&manifest.logical())? != snapshot.manifest_digest
        {
            return Err(ArtifactDataError::SnapshotIntegrity);
        }
        persist_restore_rows(&transaction, &request.target, &manifest, objects, copied).await?;
        let namespace_revision = request
            .expected_namespace_revision
            .checked_add(1)
            .ok_or(ArtifactDataError::RestorePrecondition)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_namespaces SET namespace_revision = {}, updated_at = {}
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                       AND namespace_revision = {} AND purged_at IS NULL",
                    placeholder(backend, 1), now_expression(backend), placeholder(backend, 2),
                    placeholder(backend, 3), placeholder(backend, 4), placeholder(backend, 5),
                ),
                vec![
                    revision_value(namespace_revision)?,
                    uuid_value(request.target.tenant_id, backend),
                    request.target.module_slug.clone().into(),
                    revision_value(request.target.data_contract_revision)?,
                    revision_value(request.expected_namespace_revision)?,
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        let restored_records = manifest.records.len() as u64;
        let restored_objects = manifest.objects.len() as u64;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_restore_operations
                     (tenant_id, module_slug, data_contract_revision, idempotency_key, request_digest,
                      snapshot_id, expected_namespace_revision, namespace_revision, restored_records,
                      restored_objects, actor_id, reason, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3),
                    placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6),
                    placeholder(backend, 7), placeholder(backend, 8), placeholder(backend, 9),
                    placeholder(backend, 10), placeholder(backend, 11), placeholder(backend, 12),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(request.target.tenant_id, backend),
                    request.target.module_slug.clone().into(),
                    revision_value(request.target.data_contract_revision)?,
                    uuid_value(request.idempotency_key, backend),
                    request_digest.to_owned().into(),
                    uuid_value(request.snapshot_id, backend),
                    revision_value(request.expected_namespace_revision)?,
                    revision_value(namespace_revision)?,
                    revision_value(restored_records)?,
                    revision_value(restored_objects)?,
                    uuid_value(request.actor_id, backend),
                    request.reason.clone().into(),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    Some(request.target.tenant_id),
                    Some(request.actor_id),
                    DomainEvent::ModuleArtifactDataSnapshotRestored {
                        snapshot_id: request.snapshot_id,
                        tenant_id: request.target.tenant_id,
                        module_slug: request.target.module_slug.clone(),
                        data_contract_revision: request.target.data_contract_revision,
                        namespace_revision,
                        restored_records,
                        restored_objects,
                    },
                ),
            )
            .await
            .map_err(snapshot_storage_error)?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(ArtifactDataRestoreResult {
            snapshot_id: request.snapshot_id,
            namespace_revision,
            restored_records,
            restored_objects,
        })
    }

    async fn find_restore_operation(
        &self,
        request: &ArtifactDataRestoreRequest,
        request_digest: &str,
    ) -> Result<Option<ArtifactDataRestoreResult>, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, request.target.tenant_id).await?;
        let result = find_restore_operation_in(&transaction, request, request_digest).await?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(result)
    }
}

#[derive(Clone)]
pub struct SeaOrmArtifactDataSnapshotRetentionService<A> {
    db: DatabaseConnection,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmArtifactDataSnapshotRetentionService<A>
where
    A: ArtifactDataSnapshotRetentionAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn update(
        &self,
        request: ArtifactDataSnapshotRetentionUpdateRequest,
    ) -> Result<ArtifactDataSnapshotRetention, ArtifactDataError> {
        validate_retention_update_request(&request)?;
        self.authorizer.authorize_retention_update(&request).await?;
        let request_digest = digest_json(&request)?;
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id).await?;
        let backend = transaction.get_database_backend();
        if let Some(result) =
            find_retention_operation(&transaction, &request, &request_digest).await?
        {
            transaction.commit().await.map_err(snapshot_storage_error)?;
            return Ok(result);
        }
        let row = lock_snapshot(&transaction, request.tenant_id, request.snapshot_id).await?;
        if let Some(result) =
            find_retention_operation(&transaction, &request, &request_digest).await?
        {
            transaction.commit().await.map_err(snapshot_storage_error)?;
            return Ok(result);
        }
        let status: String = row.try_get("", "status").map_err(snapshot_storage_error)?;
        let current_revision = positive_u64(&row, "retention_revision")?;
        let current_retain_until = datetime_from_row(&row, "retain_until", backend)?;
        let current_legal_hold = bool_from_row(&row, "legal_hold", backend)?;
        if !matches!(status.as_str(), "staging" | "ready")
            || current_revision != request.expected_retention_revision
        {
            return Err(ArtifactDataError::SnapshotRetentionPrecondition);
        }
        let retain_until = request
            .extend_retain_until
            .unwrap_or_else(|| current_retain_until.to_owned());
        if retain_until < current_retain_until {
            return Err(ArtifactDataError::SnapshotRetentionPrecondition);
        }
        let legal_hold = request.legal_hold.unwrap_or(current_legal_hold);
        if retain_until == current_retain_until && legal_hold == current_legal_hold {
            return Err(ArtifactDataError::SnapshotRetentionPrecondition);
        }
        let retention_revision = current_revision
            .checked_add(1)
            .ok_or(ArtifactDataError::SnapshotRetentionPrecondition)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_snapshots
                     SET retention_revision = {}, retain_until = {}, legal_hold = {}
                     WHERE tenant_id = {} AND snapshot_id = {} AND retention_revision = {}
                       AND status IN ('staging', 'ready')",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    revision_value(retention_revision)?,
                    datetime_value(retain_until.to_owned(), backend),
                    legal_hold.into(),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.snapshot_id, backend),
                    revision_value(current_revision)?,
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataError::SnapshotRetentionPrecondition);
        }
        let completed = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_snapshot_retention_operations
                     (tenant_id, snapshot_id, idempotency_key, request_digest,
                      expected_retention_revision, retention_revision, retain_until,
                      legal_hold, actor_id, reason, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.snapshot_id, backend),
                    uuid_value(request.idempotency_key, backend),
                    request_digest.into(),
                    revision_value(request.expected_retention_revision)?,
                    revision_value(retention_revision)?,
                    datetime_value(retain_until.to_owned(), backend),
                    legal_hold.into(),
                    uuid_value(request.actor_id, backend),
                    request.reason.into(),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(ArtifactDataError::SnapshotRetentionPrecondition);
        }
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    Some(request.tenant_id),
                    Some(request.actor_id),
                    DomainEvent::ModuleArtifactDataSnapshotRetentionUpdated {
                        snapshot_id: request.snapshot_id,
                        tenant_id: request.tenant_id,
                        retention_revision,
                        retain_until: retain_until.to_owned(),
                        legal_hold,
                    },
                ),
            )
            .await
            .map_err(snapshot_storage_error)?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(ArtifactDataSnapshotRetention {
            snapshot_id: request.snapshot_id,
            retention_revision,
            retain_until,
            legal_hold,
        })
    }
}

#[derive(Clone)]
pub struct SeaOrmArtifactDataSnapshotCollectionService<A> {
    db: DatabaseConnection,
    storage: StorageRuntime,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<A> SeaOrmArtifactDataSnapshotCollectionService<A>
where
    A: ArtifactDataSnapshotCollectionAuthorizer,
{
    pub fn new(db: DatabaseConnection, storage: StorageRuntime, authorizer: A) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, storage, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            db,
            storage,
            authorizer,
            infrastructure,
        }
    }

    pub async fn collect(
        &self,
        request: ArtifactDataSnapshotCollectionRequest,
        policy: &dyn ArtifactDataSnapshotCollectionPolicy,
    ) -> Result<ArtifactDataSnapshotCollectionResult, ArtifactDataError> {
        validate_collection_request(&request)?;
        self.authorizer.authorize_collection(&request).await?;
        if policy.snapshot_id() != request.policy_snapshot_id {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        let candidates = self.load_collection_candidates(&request).await?;
        let now = self.infrastructure.now();
        let mut result = ArtifactDataSnapshotCollectionResult::default();
        for candidate in candidates {
            if result.collected >= u64::from(request.limit) {
                break;
            }
            let work = if candidate.status == "collecting" {
                result.resumed = result
                    .resumed
                    .checked_add(1)
                    .ok_or(ArtifactDataError::SnapshotCollectionPrecondition)?;
                self.load_collection_work(request.tenant_id, candidate.snapshot.snapshot_id)
                    .await?
            } else {
                if candidate.snapshot.legal_hold
                    || candidate.snapshot.retain_until > now
                    || !policy.may_collect(&candidate.snapshot).await?
                {
                    result.retained = result
                        .retained
                        .checked_add(1)
                        .ok_or(ArtifactDataError::SnapshotCollectionPrecondition)?;
                    continue;
                }
                self.start_collection(&request, &candidate.snapshot, now.to_owned())
                    .await?
            };
            let objects = self
                .load_snapshot_storage_keys(request.tenant_id, work.snapshot_id)
                .await?;
            for storage_key in &objects {
                self.storage
                    .objects
                    .delete(&Path::from(storage_key.as_str()))
                    .await
                    .map_err(snapshot_storage_error)?;
            }
            self.finish_collection(&work, objects.len() as u64).await?;
            result.collected = result
                .collected
                .checked_add(1)
                .ok_or(ArtifactDataError::SnapshotCollectionPrecondition)?;
        }
        Ok(result)
    }

    async fn load_collection_candidates(
        &self,
        request: &ArtifactDataSnapshotCollectionRequest,
    ) -> Result<Vec<CollectionCandidate>, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let rows = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT snapshot_id, tenant_id, module_slug, data_contract_revision,
                            policy_revision, retention_revision, retain_until, legal_hold,
                            object_count, status
                     FROM module_artifact_data_snapshots
                     WHERE tenant_id = {} AND status IN ('ready', 'collecting')
                     ORDER BY CASE WHEN status = 'collecting' THEN 0 ELSE 1 END,
                              created_at ASC, snapshot_id ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    i64::from(MAX_SNAPSHOT_COLLECTION_SCAN).into(),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        let candidates = rows
            .into_iter()
            .map(|row| collection_candidate_from_row(row, backend))
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(candidates)
    }

    async fn start_collection(
        &self,
        request: &ArtifactDataSnapshotCollectionRequest,
        candidate: &ArtifactDataSnapshotCollectionCandidate,
        now: DateTime<Utc>,
    ) -> Result<CollectionWork, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let row = lock_snapshot(&transaction, request.tenant_id, candidate.snapshot_id).await?;
        let status: String = row.try_get("", "status").map_err(snapshot_storage_error)?;
        if status == "collecting" {
            let work =
                collection_work_in(&transaction, request.tenant_id, candidate.snapshot_id).await?;
            transaction.commit().await.map_err(snapshot_storage_error)?;
            return Ok(work);
        }
        let retention_revision = positive_u64(&row, "retention_revision")?;
        let retain_until = datetime_from_row(&row, "retain_until", backend)?;
        let legal_hold = bool_from_row(&row, "legal_hold", backend)?;
        if status != "ready"
            || retention_revision != candidate.retention_revision
            || legal_hold
            || retain_until > now
        {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        let collection_id = self.infrastructure.new_id();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_snapshot_collections
                     (collection_id, tenant_id, snapshot_id, module_slug,
                      data_contract_revision, policy_snapshot_id, actor_id, reason,
                      object_count, collecting_at, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, NULL)",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(collection_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(candidate.snapshot_id, backend),
                    candidate.scope.module_slug.clone().into(),
                    revision_value(candidate.scope.data_contract_revision)?,
                    request.policy_snapshot_id.clone().into(),
                    uuid_value(request.actor_id, backend),
                    request.reason.clone().into(),
                    revision_value(candidate.object_count)?,
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_snapshots SET status = 'collecting'
                     WHERE tenant_id = {} AND snapshot_id = {} AND status = 'ready'
                       AND retention_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(candidate.snapshot_id, backend),
                    revision_value(candidate.retention_revision)?,
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(CollectionWork {
            collection_id,
            tenant_id: request.tenant_id,
            snapshot_id: candidate.snapshot_id,
            module_slug: candidate.scope.module_slug.clone(),
            data_contract_revision: candidate.scope.data_contract_revision,
            policy_snapshot_id: request.policy_snapshot_id.clone(),
            actor_id: request.actor_id,
            object_count: candidate.object_count,
        })
    }

    async fn load_collection_work(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<CollectionWork, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, tenant_id).await?;
        let work = collection_work_in(&transaction, tenant_id, snapshot_id).await?;
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(work)
    }

    async fn load_snapshot_storage_keys(
        &self,
        tenant_id: Uuid,
        snapshot_id: Uuid,
    ) -> Result<Vec<String>, ArtifactDataError> {
        let objects = {
            let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
            configure_tenant_scope(&transaction, tenant_id).await?;
            let objects =
                query_stored_snapshot_objects(&transaction, tenant_id, snapshot_id).await?;
            transaction.commit().await.map_err(snapshot_storage_error)?;
            objects
        };
        if objects.len() > MAX_SNAPSHOT_OBJECTS {
            return Err(ArtifactDataError::SnapshotLimitExceeded);
        }
        objects
            .into_iter()
            .map(|object| {
                object
                    .snapshot_storage_key
                    .ok_or(ArtifactDataError::SnapshotIntegrity)
            })
            .collect()
    }

    async fn finish_collection(
        &self,
        work: &CollectionWork,
        deleted_objects: u64,
    ) -> Result<(), ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(snapshot_storage_error)?;
        configure_tenant_scope(&transaction, work.tenant_id).await?;
        let backend = transaction.get_database_backend();
        if deleted_objects != work.object_count {
            return Err(ArtifactDataError::SnapshotIntegrity);
        }
        let row = lock_snapshot(&transaction, work.tenant_id, work.snapshot_id).await?;
        let status: String = row.try_get("", "status").map_err(snapshot_storage_error)?;
        if status != "collecting" {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        let collection = collection_work_in(&transaction, work.tenant_id, work.snapshot_id).await?;
        if collection != *work {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        let completed = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_snapshot_collections SET completed_at = {}
                     WHERE tenant_id = {} AND snapshot_id = {} AND collection_id = {}
                       AND completed_at IS NULL",
                    now_expression(backend),
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(work.tenant_id, backend),
                    uuid_value(work.snapshot_id, backend),
                    uuid_value(work.collection_id, backend),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope(
                    Some(work.tenant_id),
                    Some(work.actor_id),
                    DomainEvent::ModuleArtifactDataSnapshotCollected {
                        collection_id: work.collection_id,
                        snapshot_id: work.snapshot_id,
                        tenant_id: work.tenant_id,
                        module_slug: work.module_slug.clone(),
                        data_contract_revision: work.data_contract_revision,
                        policy_snapshot_id: work.policy_snapshot_id.clone(),
                        deleted_objects,
                    },
                ),
            )
            .await
            .map_err(snapshot_storage_error)?;
        let deleted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_snapshots
                     WHERE tenant_id = {} AND snapshot_id = {} AND status = 'collecting'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(work.tenant_id, backend),
                    uuid_value(work.snapshot_id, backend),
                ],
            ))
            .await
            .map_err(snapshot_storage_error)?;
        if deleted.rows_affected() != 1 {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        transaction.commit().await.map_err(snapshot_storage_error)?;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct CollectionCandidate {
    snapshot: ArtifactDataSnapshotCollectionCandidate,
    status: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CollectionWork {
    collection_id: Uuid,
    tenant_id: Uuid,
    snapshot_id: Uuid,
    module_slug: String,
    data_contract_revision: u64,
    policy_snapshot_id: String,
    actor_id: Uuid,
    object_count: u64,
}

#[derive(Clone, Debug, Serialize)]
struct LogicalSnapshotManifest {
    scope: ArtifactDataScope,
    source_namespace_revision: u64,
    records: Vec<ArtifactDataRecord>,
    objects: Vec<ArtifactDataObject>,
    indexes: Vec<SnapshotIndex>,
    index_contract_digest: Option<String>,
}

#[derive(Clone, Debug)]
struct StoredSnapshotManifest {
    scope: ArtifactDataScope,
    source_namespace_revision: u64,
    records: Vec<ArtifactDataRecord>,
    objects: Vec<SnapshotObject>,
    indexes: Vec<SnapshotIndex>,
    index_contract_digest: Option<String>,
}

impl StoredSnapshotManifest {
    fn logical(&self) -> LogicalSnapshotManifest {
        LogicalSnapshotManifest {
            scope: self.scope.clone(),
            source_namespace_revision: self.source_namespace_revision,
            records: self.records.clone(),
            objects: self
                .objects
                .iter()
                .map(|item| item.object.clone())
                .collect(),
            indexes: self.indexes.clone(),
            index_contract_digest: self.index_contract_digest.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SnapshotIndex {
    index_name: String,
    index_value: String,
    data_key: String,
}

#[derive(Clone, Debug)]
struct SnapshotObject {
    object: ArtifactDataObject,
    source_storage_key: String,
    snapshot_storage_key: Option<String>,
}

async fn query_snapshot_records<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT data_key, value, revision FROM module_artifact_data
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                 ORDER BY data_key ASC LIMIT {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                MAX_SNAPSHOT_RECORDS + 1,
            ),
            scope_values(scope, backend)?,
        ))
        .await
        .map_err(snapshot_storage_error)?
        .into_iter()
        .map(|row| {
            Ok(ArtifactDataRecord {
                key: row
                    .try_get("", "data_key")
                    .map_err(snapshot_storage_error)?,
                value: row.try_get("", "value").map_err(snapshot_storage_error)?,
                revision: positive_u64(&row, "revision")?,
            })
        })
        .collect()
}

async fn query_source_objects<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
) -> Result<Vec<SnapshotObject>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                 FROM module_artifact_data_objects
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                 ORDER BY object_name ASC LIMIT {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                MAX_SNAPSHOT_OBJECTS + 1,
            ),
            scope_values(scope, backend)?,
        ))
        .await
        .map_err(snapshot_storage_error)?
        .into_iter()
        .map(snapshot_object_from_source_row)
        .collect()
}

async fn query_snapshot_indexes<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
) -> Result<Vec<SnapshotIndex>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT index_name, index_value, data_key FROM module_artifact_data_indexes
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                 ORDER BY index_name ASC, index_value ASC, data_key ASC LIMIT {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                MAX_SNAPSHOT_INDEX_ROWS + 1,
            ),
            scope_values(scope, backend)?,
        ))
        .await
        .map_err(snapshot_storage_error)?
        .into_iter()
        .map(|row| {
            Ok(SnapshotIndex {
                index_name: row
                    .try_get("", "index_name")
                    .map_err(snapshot_storage_error)?,
                index_value: row
                    .try_get("", "index_value")
                    .map_err(snapshot_storage_error)?,
                data_key: row
                    .try_get("", "data_key")
                    .map_err(snapshot_storage_error)?,
            })
        })
        .collect()
}

async fn query_index_contract<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
) -> Result<Option<String>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT contract_digest FROM module_artifact_data_index_contracts
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            scope_values(scope, backend)?,
        ))
        .await
        .map_err(snapshot_storage_error)?
        .map(|row| {
            row.try_get("", "contract_digest")
                .map_err(snapshot_storage_error)
        })
        .transpose()
}

async fn persist_snapshot_rows(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    snapshot_id: Uuid,
    records: &[ArtifactDataRecord],
    objects: &[SnapshotObject],
    indexes: &[SnapshotIndex],
    index_contract: Option<&str>,
) -> Result<(), ArtifactDataError> {
    let backend = transaction.get_database_backend();
    for record in records {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_snapshot_records (tenant_id, snapshot_id, data_key, value, revision) VALUES ({}, {}, {}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5)), vec![
            uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend), record.key.clone().into(), record.value.clone().into(), revision_value(record.revision)?])).await.map_err(snapshot_storage_error)?;
    }
    for object in objects {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_snapshot_objects (tenant_id, snapshot_id, object_name, content_type, size_bytes, digest_sha256, revision, source_storage_key, snapshot_storage_key) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, NULL)",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5), placeholder(backend,6), placeholder(backend,7), placeholder(backend,8)), vec![
            uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend), object.object.name.clone().into(), object.object.content_type.clone().into(), revision_value(object.object.size_bytes)?, object.object.digest_sha256.clone().into(), revision_value(object.object.revision)?, object.source_storage_key.clone().into()])).await.map_err(snapshot_storage_error)?;
    }
    for index in indexes {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_snapshot_indexes (tenant_id, snapshot_id, index_name, index_value, data_key) VALUES ({}, {}, {}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5)), vec![
            uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend), index.index_name.clone().into(), index.index_value.clone().into(), index.data_key.clone().into()])).await.map_err(snapshot_storage_error)?;
    }
    if let Some(digest) = index_contract {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_snapshot_index_contracts (tenant_id, snapshot_id, contract_digest) VALUES ({}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3)), vec![uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend), digest.to_owned().into()])).await.map_err(snapshot_storage_error)?;
    }
    Ok(())
}

async fn load_manifest<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    snapshot_id: Uuid,
) -> Result<StoredSnapshotManifest, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let snapshot_row = lock_snapshot(connection, tenant_id, snapshot_id).await?;
    let scope = snapshot_scope_from_row(&snapshot_row, backend)?;
    let source_namespace_revision = positive_u64(&snapshot_row, "source_namespace_revision")?;
    let records = connection.query_all(Statement::from_sql_and_values(backend, format!(
        "SELECT data_key, value, revision FROM module_artifact_data_snapshot_records WHERE tenant_id = {} AND snapshot_id = {} ORDER BY data_key ASC LIMIT {}",
        placeholder(backend,1), placeholder(backend,2), MAX_SNAPSHOT_RECORDS + 1), vec![uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend)])).await.map_err(snapshot_storage_error)?.into_iter().map(|row| Ok(ArtifactDataRecord { key: row.try_get("", "data_key").map_err(snapshot_storage_error)?, value: row.try_get("", "value").map_err(snapshot_storage_error)?, revision: positive_u64(&row, "revision")? })).collect::<Result<Vec<_>, ArtifactDataError>>()?;
    let objects = query_stored_snapshot_objects(connection, tenant_id, snapshot_id).await?;
    let indexes = connection.query_all(Statement::from_sql_and_values(backend, format!(
        "SELECT index_name, index_value, data_key FROM module_artifact_data_snapshot_indexes WHERE tenant_id = {} AND snapshot_id = {} ORDER BY index_name ASC, index_value ASC, data_key ASC LIMIT {}",
        placeholder(backend,1), placeholder(backend,2), MAX_SNAPSHOT_INDEX_ROWS + 1), vec![uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend)])).await.map_err(snapshot_storage_error)?.into_iter().map(|row| Ok(SnapshotIndex { index_name: row.try_get("", "index_name").map_err(snapshot_storage_error)?, index_value: row.try_get("", "index_value").map_err(snapshot_storage_error)?, data_key: row.try_get("", "data_key").map_err(snapshot_storage_error)? })).collect::<Result<Vec<_>, ArtifactDataError>>()?;
    let index_contract_digest = connection.query_one(Statement::from_sql_and_values(backend, format!(
        "SELECT contract_digest FROM module_artifact_data_snapshot_index_contracts WHERE tenant_id = {} AND snapshot_id = {}",
        placeholder(backend,1), placeholder(backend,2)), vec![uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend)])).await.map_err(snapshot_storage_error)?.map(|row| row.try_get("", "contract_digest").map_err(snapshot_storage_error)).transpose()?;
    Ok(StoredSnapshotManifest {
        scope,
        source_namespace_revision,
        records,
        objects,
        indexes,
        index_contract_digest,
    })
}

async fn query_stored_snapshot_objects<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    snapshot_id: Uuid,
) -> Result<Vec<SnapshotObject>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection.query_all(Statement::from_sql_and_values(backend, format!(
        "SELECT object_name, content_type, size_bytes, digest_sha256, revision, source_storage_key, snapshot_storage_key FROM module_artifact_data_snapshot_objects WHERE tenant_id = {} AND snapshot_id = {} ORDER BY object_name ASC LIMIT {}",
        placeholder(backend,1), placeholder(backend,2), MAX_SNAPSHOT_OBJECTS + 1), vec![uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend)])).await.map_err(snapshot_storage_error)?.into_iter().map(snapshot_object_from_snapshot_row).collect()
}

async fn lock_snapshot<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    snapshot_id: Uuid,
) -> Result<sea_orm::QueryResult, ArtifactDataError> {
    let backend = connection.get_database_backend();
    connection.query_one(Statement::from_sql_and_values(backend, format!(
        "SELECT snapshot_id, tenant_id, module_slug, data_contract_revision, policy_revision, source_namespace_revision, status, retention_revision, manifest_digest, structured_record_count, object_count, total_object_bytes, retain_until, legal_hold, actor_id FROM module_artifact_data_snapshots WHERE tenant_id = {} AND snapshot_id = {}{}",
        placeholder(backend,1), placeholder(backend,2), namespace_lock_clause(backend)), vec![uuid_value(tenant_id, backend), uuid_value(snapshot_id, backend)])).await.map_err(snapshot_storage_error)?.ok_or(ArtifactDataError::SnapshotPrecondition)
}

async fn lock_restore_namespace(
    transaction: &DatabaseTransaction,
    request: &ArtifactDataRestoreRequest,
) -> Result<(u64, bool), ArtifactDataError> {
    let backend = transaction.get_database_backend();
    transaction.execute(Statement::from_sql_and_values(backend, format!(
        "INSERT INTO module_artifact_data_namespaces (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at) VALUES ({}, {}, {}, 1, {}, {}) ON CONFLICT DO NOTHING",
        placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), now_expression(backend), now_expression(backend)), scope_values(&request.target, backend)?)).await.map_err(snapshot_storage_error)?;
    let row = transaction.query_one(Statement::from_sql_and_values(backend, format!(
        "SELECT namespace_revision, CASE WHEN purged_at IS NULL THEN 0 ELSE 1 END AS is_purged FROM module_artifact_data_namespaces WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}{}",
        placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), namespace_lock_clause(backend)), scope_values(&request.target, backend)?)).await.map_err(snapshot_storage_error)?.ok_or(ArtifactDataError::RestorePrecondition)?;
    let is_purged: i64 = row
        .try_get("", "is_purged")
        .map_err(snapshot_storage_error)?;
    Ok((positive_u64(&row, "namespace_revision")?, is_purged != 0))
}

async fn ensure_namespace_empty(
    transaction: &DatabaseTransaction,
    scope: &ArtifactDataScope,
) -> Result<(), ArtifactDataError> {
    let backend = transaction.get_database_backend();
    for table in [
        "module_artifact_data",
        "module_artifact_data_objects",
        "module_artifact_data_indexes",
        "module_artifact_data_index_contracts",
    ] {
        let row = transaction.query_one(Statement::from_sql_and_values(backend, format!(
            "SELECT COUNT(*) AS row_count FROM {table} WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3)), scope_values(scope, backend)?)).await.map_err(snapshot_storage_error)?.ok_or(ArtifactDataError::RestorePrecondition)?;
        let count: i64 = row
            .try_get("", "row_count")
            .map_err(snapshot_storage_error)?;
        if count != 0 {
            return Err(ArtifactDataError::RestorePrecondition);
        }
    }
    Ok(())
}

async fn persist_restore_rows(
    transaction: &DatabaseTransaction,
    scope: &ArtifactDataScope,
    manifest: &StoredSnapshotManifest,
    objects: &[SnapshotObject],
    copied: &[String],
) -> Result<(), ArtifactDataError> {
    if objects.len() != copied.len() || manifest.objects.len() != objects.len() {
        return Err(ArtifactDataError::SnapshotIntegrity);
    }
    if !manifest
        .objects
        .iter()
        .zip(objects)
        .all(|(manifest_object, copied_object)| manifest_object.object == copied_object.object)
    {
        return Err(ArtifactDataError::SnapshotIntegrity);
    }
    let backend = transaction.get_database_backend();
    for record in &manifest.records {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data (tenant_id, module_slug, data_contract_revision, data_key, value, revision, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5), placeholder(backend,6), now_expression(backend)), vec![uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(), revision_value(scope.data_contract_revision)?, record.key.clone().into(), record.value.clone().into(), revision_value(record.revision)?])).await.map_err(snapshot_storage_error)?;
    }
    for (object, storage_key) in objects.iter().zip(copied) {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_objects (tenant_id, module_slug, data_contract_revision, object_name, storage_key, content_type, size_bytes, digest_sha256, revision, created_at, updated_at) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5), placeholder(backend,6), placeholder(backend,7), placeholder(backend,8), placeholder(backend,9), now_expression(backend), now_expression(backend)), vec![uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(), revision_value(scope.data_contract_revision)?, object.object.name.clone().into(), storage_key.clone().into(), object.object.content_type.clone().into(), revision_value(object.object.size_bytes)?, object.object.digest_sha256.clone().into(), revision_value(object.object.revision)?])).await.map_err(snapshot_storage_error)?;
    }
    for index in &manifest.indexes {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_indexes (tenant_id, module_slug, data_contract_revision, index_name, index_value, data_key) VALUES ({}, {}, {}, {}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), placeholder(backend,5), placeholder(backend,6)), vec![uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(), revision_value(scope.data_contract_revision)?, index.index_name.clone().into(), index.index_value.clone().into(), index.data_key.clone().into()])).await.map_err(snapshot_storage_error)?;
    }
    if let Some(digest) = &manifest.index_contract_digest {
        transaction.execute(Statement::from_sql_and_values(backend, format!(
            "INSERT INTO module_artifact_data_index_contracts (tenant_id, module_slug, data_contract_revision, contract_digest, bound_at) VALUES ({}, {}, {}, {}, {})",
            placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4), now_expression(backend)), vec![uuid_value(scope.tenant_id, backend), scope.module_slug.clone().into(), revision_value(scope.data_contract_revision)?, digest.clone().into()])).await.map_err(snapshot_storage_error)?;
    }
    Ok(())
}

async fn find_restore_operation_in<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactDataRestoreRequest,
    request_digest: &str,
) -> Result<Option<ArtifactDataRestoreResult>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection.query_one(Statement::from_sql_and_values(backend, format!(
        "SELECT request_digest, snapshot_id, namespace_revision, restored_records, restored_objects FROM module_artifact_data_restore_operations WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
        placeholder(backend,1), placeholder(backend,2), placeholder(backend,3), placeholder(backend,4)), vec![uuid_value(request.target.tenant_id, backend), request.target.module_slug.clone().into(), revision_value(request.target.data_contract_revision)?, uuid_value(request.idempotency_key, backend)])).await.map_err(snapshot_storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_digest: String = row
        .try_get("", "request_digest")
        .map_err(snapshot_storage_error)?;
    let snapshot_id = uuid_from_row(&row, "snapshot_id", backend)?;
    if stored_digest != request_digest || snapshot_id != request.snapshot_id {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    Ok(Some(ArtifactDataRestoreResult {
        snapshot_id,
        namespace_revision: positive_u64(&row, "namespace_revision")?,
        restored_records: nonnegative_u64(&row, "restored_records")?,
        restored_objects: nonnegative_u64(&row, "restored_objects")?,
    }))
}

async fn find_retention_operation<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactDataSnapshotRetentionUpdateRequest,
    request_digest: &str,
) -> Result<Option<ArtifactDataSnapshotRetention>, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_digest, retention_revision, retain_until, legal_hold
                 FROM module_artifact_data_snapshot_retention_operations
                 WHERE tenant_id = {} AND snapshot_id = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(request.tenant_id, backend),
                uuid_value(request.snapshot_id, backend),
                uuid_value(request.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(snapshot_storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_digest: String = row
        .try_get("", "request_digest")
        .map_err(snapshot_storage_error)?;
    if stored_digest != request_digest {
        return Err(ArtifactDataError::IdempotencyConflict);
    }
    Ok(Some(ArtifactDataSnapshotRetention {
        snapshot_id: request.snapshot_id,
        retention_revision: positive_u64(&row, "retention_revision")?,
        retain_until: datetime_from_row(&row, "retain_until", backend)?,
        legal_hold: bool_from_row(&row, "legal_hold", backend)?,
    }))
}

fn collection_candidate_from_row(
    row: sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<CollectionCandidate, ArtifactDataError> {
    let snapshot_id = uuid_from_row(&row, "snapshot_id", backend)?;
    let scope = ArtifactDataScope {
        tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
        module_slug: row
            .try_get("", "module_slug")
            .map_err(snapshot_storage_error)?,
        data_contract_revision: positive_u64(&row, "data_contract_revision")?,
        policy_revision: positive_u64(&row, "policy_revision")?,
    };
    scope.validate()?;
    Ok(CollectionCandidate {
        snapshot: ArtifactDataSnapshotCollectionCandidate {
            snapshot_id,
            scope,
            retention_revision: positive_u64(&row, "retention_revision")?,
            retain_until: datetime_from_row(&row, "retain_until", backend)?,
            legal_hold: bool_from_row(&row, "legal_hold", backend)?,
            object_count: nonnegative_u64(&row, "object_count")?,
        },
        status: row.try_get("", "status").map_err(snapshot_storage_error)?,
    })
}

async fn collection_work_in<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    snapshot_id: Uuid,
) -> Result<CollectionWork, ArtifactDataError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT collection_id, tenant_id, snapshot_id, module_slug,
                        data_contract_revision, policy_snapshot_id, actor_id, object_count
                 FROM module_artifact_data_snapshot_collections
                 WHERE tenant_id = {} AND snapshot_id = {} AND completed_at IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(snapshot_id, backend),
            ],
        ))
        .await
        .map_err(snapshot_storage_error)?
        .ok_or(ArtifactDataError::SnapshotCollectionPrecondition)?;
    Ok(CollectionWork {
        collection_id: uuid_from_row(&row, "collection_id", backend)?,
        tenant_id: uuid_from_row(&row, "tenant_id", backend)?,
        snapshot_id: uuid_from_row(&row, "snapshot_id", backend)?,
        module_slug: row
            .try_get("", "module_slug")
            .map_err(snapshot_storage_error)?,
        data_contract_revision: positive_u64(&row, "data_contract_revision")?,
        policy_snapshot_id: row
            .try_get("", "policy_snapshot_id")
            .map_err(snapshot_storage_error)?,
        actor_id: uuid_from_row(&row, "actor_id", backend)?,
        object_count: nonnegative_u64(&row, "object_count")?,
    })
}

fn snapshot_scope_from_row(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<ArtifactDataScope, ArtifactDataError> {
    Ok(ArtifactDataScope {
        tenant_id: uuid_from_row(row, "tenant_id", backend)?,
        module_slug: row
            .try_get("", "module_slug")
            .map_err(snapshot_storage_error)?,
        data_contract_revision: positive_u64(row, "data_contract_revision")?,
        policy_revision: positive_u64(row, "policy_revision")?,
    })
}

fn snapshot_from_row(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<ArtifactDataSnapshot, ArtifactDataError> {
    let digest: Option<String> = row
        .try_get("", "manifest_digest")
        .map_err(snapshot_storage_error)?;
    snapshot_from_row_with_digest(
        row,
        backend,
        digest.ok_or(ArtifactDataError::SnapshotPrecondition)?,
    )
}

fn snapshot_from_row_with_digest(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
    manifest_digest: String,
) -> Result<ArtifactDataSnapshot, ArtifactDataError> {
    Ok(ArtifactDataSnapshot {
        snapshot_id: uuid_from_row(row, "snapshot_id", backend)?,
        scope: snapshot_scope_from_row(row, backend)?,
        source_namespace_revision: positive_u64(row, "source_namespace_revision")?,
        retention_revision: positive_u64(row, "retention_revision")?,
        manifest_digest,
        structured_record_count: nonnegative_u64(row, "structured_record_count")?,
        object_count: nonnegative_u64(row, "object_count")?,
        total_object_bytes: nonnegative_u64(row, "total_object_bytes")?,
        retain_until: datetime_from_row(row, "retain_until", backend)?,
        legal_hold: bool_from_row(row, "legal_hold", backend)?,
    })
}

fn snapshot_object_from_source_row(
    row: sea_orm::QueryResult,
) -> Result<SnapshotObject, ArtifactDataError> {
    let object = object_from_row(&row)?;
    Ok(SnapshotObject {
        object,
        source_storage_key: row
            .try_get("", "storage_key")
            .map_err(snapshot_storage_error)?,
        snapshot_storage_key: None,
    })
}

fn snapshot_object_from_snapshot_row(
    row: sea_orm::QueryResult,
) -> Result<SnapshotObject, ArtifactDataError> {
    let object = object_from_row(&row)?;
    Ok(SnapshotObject {
        object,
        source_storage_key: row
            .try_get("", "source_storage_key")
            .map_err(snapshot_storage_error)?,
        snapshot_storage_key: row
            .try_get("", "snapshot_storage_key")
            .map_err(snapshot_storage_error)?,
    })
}

fn object_from_row(row: &sea_orm::QueryResult) -> Result<ArtifactDataObject, ArtifactDataError> {
    let object = ArtifactDataObject {
        name: row
            .try_get("", "object_name")
            .map_err(snapshot_storage_error)?,
        content_type: row
            .try_get("", "content_type")
            .map_err(snapshot_storage_error)?,
        size_bytes: positive_u64(row, "size_bytes")?,
        digest_sha256: row
            .try_get("", "digest_sha256")
            .map_err(snapshot_storage_error)?,
        revision: positive_u64(row, "revision")?,
    };
    object.validate()?;
    Ok(object)
}

fn validate_create_request(
    request: &ArtifactDataSnapshotCreateRequest,
) -> Result<(), ArtifactDataError> {
    request.scope.validate()?;
    if request.expected_namespace_revision == 0
        || request.actor_id.is_nil()
        || request.idempotency_key.is_nil()
        || !valid_reason(&request.reason)
    {
        return Err(ArtifactDataError::SnapshotPrecondition);
    }
    Ok(())
}

fn validate_restore_request(request: &ArtifactDataRestoreRequest) -> Result<(), ArtifactDataError> {
    request.target.validate()?;
    if request.snapshot_id.is_nil()
        || request.expected_namespace_revision == 0
        || request.actor_id.is_nil()
        || request.idempotency_key.is_nil()
        || !valid_reason(&request.reason)
    {
        return Err(ArtifactDataError::RestorePrecondition);
    }
    Ok(())
}

fn validate_retention_update_request(
    request: &ArtifactDataSnapshotRetentionUpdateRequest,
) -> Result<(), ArtifactDataError> {
    if request.tenant_id.is_nil()
        || request.snapshot_id.is_nil()
        || request.expected_retention_revision == 0
        || request.actor_id.is_nil()
        || request.idempotency_key.is_nil()
        || !valid_reason(&request.reason)
        || (request.extend_retain_until.is_none() && request.legal_hold.is_none())
    {
        return Err(ArtifactDataError::SnapshotRetentionPrecondition);
    }
    Ok(())
}

fn validate_collection_request(
    request: &ArtifactDataSnapshotCollectionRequest,
) -> Result<(), ArtifactDataError> {
    if request.tenant_id.is_nil()
        || request.actor_id.is_nil()
        || !valid_reason(&request.reason)
        || !valid_policy_snapshot_id(&request.policy_snapshot_id)
        || request.limit == 0
        || request.limit > MAX_SNAPSHOT_COLLECTION_BATCH
    {
        return Err(ArtifactDataError::SnapshotCollectionPrecondition);
    }
    Ok(())
}

fn valid_policy_snapshot_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_POLICY_SNAPSHOT_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn valid_reason(reason: &str) -> bool {
    !reason.trim().is_empty()
        && reason.trim() == reason
        && reason.len() <= MAX_SNAPSHOT_REASON_BYTES
}

fn same_data_namespace(left: &ArtifactDataScope, right: &ArtifactDataScope) -> bool {
    left.tenant_id == right.tenant_id
        && left.module_slug == right.module_slug
        && left.data_contract_revision == right.data_contract_revision
}

fn manifest_matches_snapshot(
    manifest: &StoredSnapshotManifest,
    snapshot: &ArtifactDataSnapshot,
) -> bool {
    let total_object_bytes = manifest.objects.iter().try_fold(0_u64, |total, object| {
        total.checked_add(object.object.size_bytes)
    });
    same_data_namespace(&manifest.scope, &snapshot.scope)
        && manifest.scope.policy_revision == snapshot.scope.policy_revision
        && manifest.source_namespace_revision == snapshot.source_namespace_revision
        && u64::try_from(manifest.records.len()).ok() == Some(snapshot.structured_record_count)
        && u64::try_from(manifest.objects.len()).ok() == Some(snapshot.object_count)
        && total_object_bytes == Some(snapshot.total_object_bytes)
}

fn manifest_within_limits(manifest: &StoredSnapshotManifest) -> bool {
    manifest.records.len() <= MAX_SNAPSHOT_RECORDS
        && manifest.objects.len() <= MAX_SNAPSHOT_OBJECTS
        && manifest.indexes.len() <= MAX_SNAPSHOT_INDEX_ROWS
        && manifest
            .objects
            .iter()
            .try_fold(0_u64, |total, object| {
                total.checked_add(object.object.size_bytes)
            })
            .is_some_and(|total| total <= MAX_SNAPSHOT_OBJECT_BYTES)
}

fn digest_json(value: &impl Serialize) -> Result<String, ArtifactDataError> {
    let canonical = canonical_manifest_snapshot_json(value).map_err(snapshot_storage_error)?;
    Ok(format!("sha256:{}", hash_manifest_snapshot(&canonical)))
}

fn verify_object_bytes(object: &ArtifactDataObject, bytes: &[u8]) -> Result<(), ArtifactDataError> {
    if u64::try_from(bytes.len()).ok() != Some(object.size_bytes)
        || format!("sha256:{}", hex::encode(Sha256::digest(bytes))) != object.digest_sha256
    {
        return Err(ArtifactDataError::SnapshotIntegrity);
    }
    Ok(())
}

fn scope_values(
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
    ])
}

fn positive_u64(row: &sea_orm::QueryResult, column: &str) -> Result<u64, ArtifactDataError> {
    let value: i64 = row.try_get("", column).map_err(snapshot_storage_error)?;
    u64::try_from(value)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(ArtifactDataError::SnapshotIntegrity)
}

fn nonnegative_u64(row: &sea_orm::QueryResult, column: &str) -> Result<u64, ArtifactDataError> {
    let value: i64 = row.try_get("", column).map_err(snapshot_storage_error)?;
    u64::try_from(value).map_err(|_| ArtifactDataError::SnapshotIntegrity)
}

fn bool_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<bool, ArtifactDataError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(snapshot_storage_error),
        _ => Ok(row
            .try_get::<i64>("", column)
            .map_err(snapshot_storage_error)?
            != 0),
    }
}

fn datetime_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<DateTime<Utc>, ArtifactDataError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(snapshot_storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(snapshot_storage_error)
            .and_then(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|timestamp| timestamp.with_timezone(&Utc))
                    .map_err(snapshot_storage_error)
            }),
    }
}

fn datetime_value(value: DateTime<Utc>, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(Box::new(value))),
        _ => value.to_rfc3339().into(),
    }
}

fn snapshot_storage_error(error: impl std::fmt::Display) -> ArtifactDataError {
    ArtifactDataError::Storage(error.to_string())
}
