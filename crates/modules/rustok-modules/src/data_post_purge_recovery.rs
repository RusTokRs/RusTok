//! Owner-derived post-purge restore and independently authorized reference CAS.
//! Recovery keeps the source namespace permanently tombstoned. Ports must be
//! composed by the host; this owner does not invent traffic or policy fences.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_storage::StorageRuntime;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactDataPurgeAuthorizationContext, ArtifactDataRestoreRequest, ArtifactDataScope,
    ArtifactDataSnapshotAuthorizer, ControlPlaneInfrastructure, ModuleCommandContext,
    SeaOrmArtifactDataSnapshotService,
    data::{
        configure_tenant_scope, load_retired_artifact_data_authorization_on,
        lock_artifact_data_installation_on, namespace_lock_clause, placeholder, revision_value,
        uuid_from_row, uuid_value,
    },
    data_snapshot::{
        datetime_from_row, load_manifest, lock_snapshot, positive_u64, snapshot_from_row,
    },
    promotion::{digest_json, valid_digest},
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PostPurgeRecoveryError {
    #[error("Recovery storage error: {0}")]
    Storage(String),
    #[error("Post-purge recovery command is invalid")]
    InvalidCommand,
    #[error("Recovery policy denied the command")]
    AuthorizationDenied,
    #[error("Recovery identity was reused with different command evidence")]
    IdempotencyConflict,
    #[error("Recovery source, target, or reference preconditions changed")]
    CasCutoverConflict,
    #[error("Recovery snapshot is unavailable or has incompatible ownership")]
    SnapshotNotReady,
    #[error("Recovery target content or verification evidence does not match")]
    Integrity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareRecoveryRequest {
    pub installation_id: Uuid,
    pub expected_reference_revision: u64,
    pub expected_tombstone_revision: u64,
    pub source_snapshot_id: Uuid,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PostPurgeRecoveryCutoverRequest {
    pub recovery_id: Uuid,
    pub expected_reference_revision: u64,
    pub expected_tombstone_revision: u64,
    pub expected_target_namespace_revision: u64,
    pub verified_manifest_digest: String,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedRecoveryReceipt {
    pub recovery_id: Uuid,
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub data_owner_id: Uuid,
    pub source_namespace_instance_id: Uuid,
    pub namespace_instance_id: Uuid,
    pub target_namespace_revision: u64,
    pub records_restored: u64,
    pub objects_restored: u64,
    pub source_manifest_digest: String,
    pub verified_manifest_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostPurgeRecoveryCutoverReceipt {
    pub recovery_id: Uuid,
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub source_namespace_instance_id: Uuid,
    pub namespace_instance_id: Uuid,
    pub active_reference_revision: u64,
    pub active_namespace_revision: u64,
    pub records_restored: u64,
    pub objects_restored: u64,
    pub cutover_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ArtifactDataRecoveryAuthorizationContext {
    pub recovery_id: Uuid,
    pub original: ArtifactDataPurgeAuthorizationContext,
    pub target: ArtifactDataScope,
    pub source_snapshot_id: Uuid,
    pub source_manifest_digest: String,
}

/// The host must evaluate current authorization and required operational
/// fences on this transaction. There is no permissive default implementation.
#[async_trait]
pub trait ArtifactDataRecoveryAuthorizer: Send + Sync {
    async fn authorize_prepare_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &PrepareRecoveryRequest,
        owner: &ArtifactDataRecoveryAuthorizationContext,
    ) -> Result<(), PostPurgeRecoveryError>;

    async fn authorize_cutover_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &PostPurgeRecoveryCutoverRequest,
        owner: &ArtifactDataRecoveryAuthorizationContext,
    ) -> Result<(), PostPurgeRecoveryError>;
}

pub struct ArtifactDataPostPurgeRecoveryService<S, A> {
    db: DatabaseConnection,
    snapshots: SeaOrmArtifactDataSnapshotService<S>,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
}

impl<S: ArtifactDataSnapshotAuthorizer, A: ArtifactDataRecoveryAuthorizer>
    ArtifactDataPostPurgeRecoveryService<S, A>
{
    pub fn new(
        db: DatabaseConnection,
        storage: StorageRuntime,
        snapshots: S,
        authorizer: A,
    ) -> Self {
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, storage, snapshots, authorizer, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        snapshots: S,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self {
            snapshots: SeaOrmArtifactDataSnapshotService::with_infrastructure(
                db.clone(),
                storage,
                snapshots,
                infrastructure.clone(),
            ),
            db,
            authorizer,
            infrastructure,
        }
    }

    pub async fn prepare_recovery(
        &self,
        request: PrepareRecoveryRequest,
    ) -> Result<StagedRecoveryReceipt, PostPurgeRecoveryError> {
        let tenant_id = validate_prepare(&request)?;
        let request_digest = digest_json(&request).map_err(storage_error)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let existing =
            load_prepare_operation(&transaction, &request, tenant_id, &request_digest).await?;
        if let Some(operation) = &existing
            && operation.status != "staging"
        {
            let receipt = operation.staged_receipt()?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(receipt);
        }
        lock_artifact_data_installation_on(&transaction, tenant_id, request.installation_id)
            .await
            .map_err(storage_error)?;
        // Replay after serialization before reading mutable lifecycle/reference facts.
        let existing =
            load_prepare_operation(&transaction, &request, tenant_id, &request_digest).await?;
        if let Some(operation) = &existing
            && operation.status != "staging"
        {
            let receipt = operation.staged_receipt()?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(receipt);
        }
        let original = load_retired_artifact_data_authorization_on(
            &transaction,
            tenant_id,
            request.installation_id,
        )
        .await
        .map_err(storage_error)?;
        let operation = if let Some(operation) = existing {
            operation
        } else {
            self.reserve_recovery_on(&transaction, &request, &request_digest, original.clone())
                .await?
        };
        if operation.original != original {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        ensure_source_reference_on(&transaction, &operation).await?;
        ensure_recovery_hold_on(&transaction, &operation).await?;
        self.authorizer
            .authorize_prepare_on(&transaction, &request, &operation.authorization())
            .await?;
        transaction.commit().await.map_err(storage_error)?;

        let target = operation.target();
        let restored = self
            .snapshots
            .restore(ArtifactDataRestoreRequest {
                snapshot_id: operation.source_snapshot_id,
                target: target.clone(),
                expected_namespace_revision: 1,
                context: request.context.clone(),
                reason: request.reason.clone(),
            })
            .await
            .map_err(recovery_data_error)?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let operation = load_prepare_operation(&transaction, &request, tenant_id, &request_digest)
            .await?
            .ok_or(PostPurgeRecoveryError::CasCutoverConflict)?;
        if operation.status != "staging" {
            let receipt = operation.staged_receipt()?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(receipt);
        }
        lock_artifact_data_installation_on(&transaction, tenant_id, request.installation_id)
            .await
            .map_err(storage_error)?;
        let operation = load_prepare_operation(&transaction, &request, tenant_id, &request_digest)
            .await?
            .ok_or(PostPurgeRecoveryError::CasCutoverConflict)?;
        if operation.status != "staging" {
            let receipt = operation.staged_receipt()?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(receipt);
        }
        let current = load_retired_artifact_data_authorization_on(
            &transaction,
            tenant_id,
            request.installation_id,
        )
        .await
        .map_err(storage_error)?;
        if current != operation.original {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        ensure_source_reference_on(&transaction, &operation).await?;
        ensure_recovery_hold_on(&transaction, &operation).await?;
        let row = lock_verified_target_on(&transaction, &operation.target()).await?;
        let fingerprint: String = row
            .try_get("", "verified_manifest_digest")
            .map_err(storage_error)?;
        if positive_u64(&row, "namespace_revision").map_err(storage_error)?
            != restored.namespace_revision
        {
            return Err(PostPurgeRecoveryError::Integrity);
        }
        let backend = transaction.get_database_backend();
        let updated = transaction
            .execute_raw(statement(
                backend,
                "UPDATE module_artifact_data_namespace_recovery_operations SET status='verified',
             target_namespace_revision={1}, records_restored={2}, objects_restored={3},
             verified_manifest_digest={4}, verified_at={5}
             WHERE tenant_id={6} AND recovery_id={7} AND status='staging'",
                vec![
                    number(restored.namespace_revision)?,
                    number(restored.restored_records)?,
                    number(restored.restored_objects)?,
                    fingerprint.into(),
                    timestamp(self.infrastructure.now(), backend),
                    uuid_value(tenant_id, backend),
                    uuid_value(operation.recovery_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        let receipt = load_operation(&transaction, tenant_id, operation.recovery_id)
            .await?
            .staged_receipt()?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(receipt)
    }

    async fn reserve_recovery_on(
        &self,
        db: &DatabaseTransaction,
        request: &PrepareRecoveryRequest,
        request_digest: &str,
        original: ArtifactDataPurgeAuthorizationContext,
    ) -> Result<RecoveryOperation, PostPurgeRecoveryError> {
        let backend = db.get_database_backend();
        let tenant_id = original.scope.tenant_id;
        let recovery_id = self.infrastructure.new_id();
        let namespace_instance_id = self.infrastructure.new_id();
        if recovery_id.is_nil() || namespace_instance_id.is_nil() {
            return Err(PostPurgeRecoveryError::InvalidCommand);
        }
        let row = lock_snapshot(db, tenant_id, request.source_snapshot_id)
            .await
            .map_err(storage_error)?;
        let snapshot = snapshot_from_row(&row, backend).map_err(storage_error)?;
        let status: String = row.try_get("", "status").map_err(storage_error)?;
        if status != "ready"
            || snapshot.retain_until <= self.infrastructure.now()
            || snapshot.scope.tenant_id != tenant_id
            || snapshot.scope.data_owner_id != original.data_owner_id
            || snapshot.scope.namespace_instance_id != original.scope.namespace_instance_id
            || snapshot.scope.data_contract_digest != original.scope.data_contract_digest
        {
            return Err(PostPurgeRecoveryError::SnapshotNotReady);
        }
        let operation = RecoveryOperation {
            recovery_id,
            original,
            namespace_instance_id,
            source_snapshot_id: request.source_snapshot_id,
            source_retention_revision: snapshot.retention_revision,
            expected_reference_revision: request.expected_reference_revision,
            tombstone_namespace_revision: request.expected_tombstone_revision,
            target_namespace_revision: 1,
            records_restored: 0,
            objects_restored: 0,
            source_manifest_digest: snapshot.manifest_digest,
            verified_manifest_digest: None,
            request_digest: request_digest.to_owned(),
            status: "staging".into(),
            cutover_request_digest: None,
            active_reference_revision: None,
            active_namespace_revision: None,
            cutover_at: None,
        };
        ensure_source_reference_on(db, &operation).await?;
        self.authorizer
            .authorize_prepare_on(db, request, &operation.authorization())
            .await?;
        let target = operation.target();
        db.execute_raw(statement(backend,
            "INSERT INTO module_artifact_data_namespaces
             (tenant_id,data_owner_id,namespace_instance_id,module_slug,data_contract_revision,data_contract_digest,
              state,namespace_revision,verified_manifest_digest,purged_at,created_at,updated_at)
             VALUES ({1},{2},{3},{4},{5},{6},'staging',1,NULL,NULL,{7},{7})",
            vec![uuid_value(tenant_id,backend),uuid_value(target.data_owner_id,backend),uuid_value(namespace_instance_id,backend),
                 target.module_slug.clone().into(),number(target.data_contract_revision)?,target.data_contract_digest.clone().into(),
                 timestamp(self.infrastructure.now(),backend)])).await.map_err(storage_error)?;
        db.execute_raw(statement(backend,
            "INSERT INTO module_artifact_data_namespace_recovery_operations
             (recovery_id,tenant_id,installation_id,data_owner_id,source_namespace_instance_id,namespace_instance_id,
              source_snapshot_id,source_retention_revision,expected_reference_revision,tombstone_namespace_revision,
              target_namespace_revision,status,records_restored,objects_restored,source_manifest_digest,verified_manifest_digest,
              request_digest,prepare_request_json,authorization_json,idempotency_key,created_at,verified_at,cutover_at,
              cutover_request_digest,cutover_request_json,active_reference_revision,active_namespace_revision)
             VALUES ({1},{2},{3},{4},{5},{6},{7},{8},{9},{10},1,'staging',0,0,{11},NULL,{12},{13},{14},{15},{16},
                     NULL,NULL,NULL,NULL,NULL,NULL)",
            vec![uuid_value(recovery_id,backend),uuid_value(tenant_id,backend),uuid_value(request.installation_id,backend),
                 uuid_value(target.data_owner_id,backend),uuid_value(operation.original.scope.namespace_instance_id,backend),
                 uuid_value(namespace_instance_id,backend),uuid_value(request.source_snapshot_id,backend),
                 number(operation.source_retention_revision)?,number(operation.expected_reference_revision)?,
                 number(operation.tombstone_namespace_revision)?,operation.source_manifest_digest.clone().into(),
                 request_digest.to_owned().into(),serde_json::to_string(request).map_err(storage_error)?.into(),
                 serde_json::to_string(&operation.original).map_err(storage_error)?.into(),
                 uuid_value(request.context.idempotency_key,backend),timestamp(self.infrastructure.now(),backend)])
        ).await.map_err(storage_error)?;
        db.execute_raw(statement(backend,
            "INSERT INTO module_artifact_data_snapshot_holds
             (tenant_id,snapshot_id,holder_kind,holder_id,data_owner_id,namespace_instance_id,request_digest,created_at,released_at)
             VALUES ({1},{2},'recovery',{3},{4},{5},{6},{7},NULL)",
            vec![uuid_value(tenant_id,backend),uuid_value(request.source_snapshot_id,backend),uuid_value(recovery_id,backend),
                 uuid_value(target.data_owner_id,backend),uuid_value(namespace_instance_id,backend),request_digest.to_owned().into(),
                 timestamp(self.infrastructure.now(),backend)])).await.map_err(storage_error)?;
        Ok(operation)
    }

    pub async fn execute_cas_cutover(
        &self,
        request: PostPurgeRecoveryCutoverRequest,
    ) -> Result<PostPurgeRecoveryCutoverReceipt, PostPurgeRecoveryError> {
        let tenant_id = validate_cutover(&request)?;
        let request_digest = digest_json(&request).map_err(storage_error)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let operation = load_operation(&transaction, tenant_id, request.recovery_id).await?;
        if operation.status == "cutover" {
            return terminal_cutover_replay(transaction, operation, &request_digest).await;
        }
        lock_artifact_data_installation_on(
            &transaction,
            tenant_id,
            operation.original.installation_id,
        )
        .await
        .map_err(storage_error)?;
        let operation = load_operation(&transaction, tenant_id, request.recovery_id).await?;
        if operation.status == "cutover" {
            return terminal_cutover_replay(transaction, operation, &request_digest).await;
        }
        let original = load_retired_artifact_data_authorization_on(
            &transaction,
            tenant_id,
            operation.original.installation_id,
        )
        .await
        .map_err(storage_error)?;
        if operation.original != original
            || operation.status != "verified"
            || request.expected_reference_revision != operation.expected_reference_revision
            || request.expected_tombstone_revision != operation.tombstone_namespace_revision
            || request.expected_target_namespace_revision != operation.target_namespace_revision
            || operation.verified_manifest_digest.as_deref()
                != Some(request.verified_manifest_digest.as_str())
        {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        ensure_source_reference_on(&transaction, &operation).await?;
        ensure_recovery_hold_on(&transaction, &operation).await?;
        let target = operation.target();
        let row = lock_verified_target_on(&transaction, &target).await?;
        crate::data::ensure_namespace_not_migration_held_on(
            &transaction,
            target.tenant_id,
            target.data_owner_id,
            target.namespace_instance_id,
            None,
        )
        .await
        .map_err(|error| match error {
            crate::ArtifactDataError::Storage(message) => PostPurgeRecoveryError::Storage(message),
            _ => PostPurgeRecoveryError::CasCutoverConflict,
        })?;
        let fingerprint: String = row
            .try_get("", "verified_manifest_digest")
            .map_err(storage_error)?;
        if positive_u64(&row, "namespace_revision").map_err(storage_error)?
            != request.expected_target_namespace_revision
            || fingerprint != request.verified_manifest_digest
        {
            return Err(PostPurgeRecoveryError::Integrity);
        }
        self.authorizer
            .authorize_cutover_on(&transaction, &request, &operation.authorization())
            .await?;
        let snapshot_row = lock_snapshot(&transaction, tenant_id, operation.source_snapshot_id)
            .await
            .map_err(storage_error)?;
        let snapshot = snapshot_from_row(&snapshot_row, transaction.get_database_backend())
            .map_err(storage_error)?;
        let status: String = snapshot_row.try_get("", "status").map_err(storage_error)?;
        if status != "ready"
            || snapshot.manifest_digest != operation.source_manifest_digest
            || snapshot.retention_revision < operation.source_retention_revision
        {
            return Err(PostPurgeRecoveryError::SnapshotNotReady);
        }
        let manifest = load_manifest(&transaction, tenant_id, operation.source_snapshot_id)
            .await
            .map_err(storage_error)?;
        let verified = self
            .snapshots
            .verify_restored_rows_in(
                &transaction,
                &target,
                &manifest,
                operation.source_snapshot_id,
            )
            .await
            .map_err(recovery_data_error)?;
        if verified != fingerprint {
            return Err(PostPurgeRecoveryError::Integrity);
        }
        let reference_revision = request
            .expected_reference_revision
            .checked_add(1)
            .ok_or(PostPurgeRecoveryError::InvalidCommand)?;
        let namespace_revision = request
            .expected_target_namespace_revision
            .checked_add(1)
            .ok_or(PostPurgeRecoveryError::InvalidCommand)?;
        let backend = transaction.get_database_backend();
        let updated = transaction.execute_raw(statement(backend,
            "UPDATE module_artifact_data_owner_references SET namespace_instance_id={1}, reference_revision={2}
             WHERE tenant_id={3} AND data_owner_id={4} AND namespace_instance_id={5} AND reference_revision={6}",
            vec![uuid_value(target.namespace_instance_id,backend),number(reference_revision)?,uuid_value(tenant_id,backend),
                 uuid_value(target.data_owner_id,backend),uuid_value(operation.original.scope.namespace_instance_id,backend),
                 number(request.expected_reference_revision)?])).await.map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        let updated = transaction.execute_raw(statement(backend,
            "UPDATE module_artifact_data_namespaces SET state='serving',namespace_revision={1},updated_at={2}
             WHERE tenant_id={3} AND data_owner_id={4} AND namespace_instance_id={5} AND state='verified'
              AND namespace_revision={6} AND verified_manifest_digest={7} AND purged_at IS NULL",
            vec![number(namespace_revision)?,timestamp(self.infrastructure.now(),backend),uuid_value(tenant_id,backend),
                 uuid_value(target.data_owner_id,backend),uuid_value(target.namespace_instance_id,backend),
                 number(request.expected_target_namespace_revision)?,fingerprint.into()])).await.map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        let updated = transaction.execute_raw(statement(backend,
            "UPDATE module_artifact_data_namespace_recovery_operations SET status='cutover',cutover_at={1},
             cutover_request_digest={2},cutover_request_json={3},active_reference_revision={4},active_namespace_revision={5}
             WHERE tenant_id={6} AND recovery_id={7} AND status='verified'",
            vec![timestamp(self.infrastructure.now(),backend),request_digest.into(),serde_json::to_string(&request).map_err(storage_error)?.into(),
                 number(reference_revision)?,number(namespace_revision)?,uuid_value(tenant_id,backend),uuid_value(request.recovery_id,backend)])
        ).await.map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        let released = transaction
            .execute_raw(statement(
                backend,
                "UPDATE module_artifact_data_snapshot_holds SET released_at={1}
             WHERE tenant_id={2} AND snapshot_id={3} AND holder_kind='recovery' AND holder_id={4}
              AND request_digest={5} AND released_at IS NULL",
                vec![
                    timestamp(self.infrastructure.now(), backend),
                    uuid_value(tenant_id, backend),
                    uuid_value(operation.source_snapshot_id, backend),
                    uuid_value(operation.recovery_id, backend),
                    operation.request_digest.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if released.rows_affected() != 1 {
            return Err(PostPurgeRecoveryError::CasCutoverConflict);
        }
        let receipt = load_operation(&transaction, tenant_id, request.recovery_id)
            .await?
            .cutover_receipt()?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(receipt)
    }
}

struct RecoveryOperation {
    recovery_id: Uuid,
    original: ArtifactDataPurgeAuthorizationContext,
    namespace_instance_id: Uuid,
    source_snapshot_id: Uuid,
    source_retention_revision: u64,
    expected_reference_revision: u64,
    tombstone_namespace_revision: u64,
    target_namespace_revision: u64,
    records_restored: u64,
    objects_restored: u64,
    source_manifest_digest: String,
    verified_manifest_digest: Option<String>,
    request_digest: String,
    status: String,
    cutover_request_digest: Option<String>,
    active_reference_revision: Option<u64>,
    active_namespace_revision: Option<u64>,
    cutover_at: Option<DateTime<Utc>>,
}

impl RecoveryOperation {
    fn target(&self) -> ArtifactDataScope {
        ArtifactDataScope {
            namespace_instance_id: self.namespace_instance_id,
            ..self.original.scope.clone()
        }
    }
    fn authorization(&self) -> ArtifactDataRecoveryAuthorizationContext {
        ArtifactDataRecoveryAuthorizationContext {
            recovery_id: self.recovery_id,
            original: self.original.clone(),
            target: self.target(),
            source_snapshot_id: self.source_snapshot_id,
            source_manifest_digest: self.source_manifest_digest.clone(),
        }
    }
    fn staged_receipt(&self) -> Result<StagedRecoveryReceipt, PostPurgeRecoveryError> {
        Ok(StagedRecoveryReceipt {
            recovery_id: self.recovery_id,
            tenant_id: self.original.scope.tenant_id,
            installation_id: self.original.installation_id,
            data_owner_id: self.original.data_owner_id,
            source_namespace_instance_id: self.original.scope.namespace_instance_id,
            namespace_instance_id: self.namespace_instance_id,
            target_namespace_revision: self.target_namespace_revision,
            records_restored: self.records_restored,
            objects_restored: self.objects_restored,
            source_manifest_digest: self.source_manifest_digest.clone(),
            verified_manifest_digest: self
                .verified_manifest_digest
                .clone()
                .ok_or(PostPurgeRecoveryError::Integrity)?,
        })
    }
    fn cutover_receipt(&self) -> Result<PostPurgeRecoveryCutoverReceipt, PostPurgeRecoveryError> {
        Ok(PostPurgeRecoveryCutoverReceipt {
            recovery_id: self.recovery_id,
            tenant_id: self.original.scope.tenant_id,
            data_owner_id: self.original.data_owner_id,
            source_namespace_instance_id: self.original.scope.namespace_instance_id,
            namespace_instance_id: self.namespace_instance_id,
            active_reference_revision: self
                .active_reference_revision
                .ok_or(PostPurgeRecoveryError::Integrity)?,
            active_namespace_revision: self
                .active_namespace_revision
                .ok_or(PostPurgeRecoveryError::Integrity)?,
            records_restored: self.records_restored,
            objects_restored: self.objects_restored,
            cutover_at: self.cutover_at.ok_or(PostPurgeRecoveryError::Integrity)?,
        })
    }
}

async fn ensure_source_reference_on(
    db: &DatabaseTransaction,
    operation: &RecoveryOperation,
) -> Result<(), PostPurgeRecoveryError> {
    let backend = db.get_database_backend();
    let scope = &operation.original.scope;
    let row=db.query_one_raw(statement(backend,&format!(
        "SELECT namespace.namespace_revision FROM module_artifact_data_owner_references reference
         JOIN module_artifact_data_namespaces namespace USING (tenant_id,data_owner_id,namespace_instance_id)
         WHERE reference.tenant_id={{1}} AND reference.data_owner_id={{2}} AND reference.namespace_instance_id={{3}}
          AND reference.reference_revision={{4}} AND namespace.state='purged' AND namespace.purged_at IS NOT NULL
          AND namespace.namespace_revision={{5}} AND namespace.data_contract_digest={{6}}
          AND EXISTS (SELECT 1 FROM module_artifact_data_purge_operations receipt
           WHERE receipt.tenant_id=reference.tenant_id AND receipt.installation_id={{7}}
            AND receipt.data_owner_id=reference.data_owner_id AND receipt.namespace_instance_id=reference.namespace_instance_id
            AND receipt.namespace_revision=namespace.namespace_revision){}",
        namespace_lock_clause(backend)),
        vec![uuid_value(scope.tenant_id,backend),uuid_value(scope.data_owner_id,backend),uuid_value(scope.namespace_instance_id,backend),
             number(operation.expected_reference_revision)?,number(operation.tombstone_namespace_revision)?,
             scope.data_contract_digest.clone().into(),uuid_value(operation.original.installation_id,backend)]))
        .await.map_err(storage_error)?;
    if row.is_none() {
        return Err(PostPurgeRecoveryError::CasCutoverConflict);
    }
    Ok(())
}

async fn lock_verified_target_on(
    db: &DatabaseTransaction,
    scope: &ArtifactDataScope,
) -> Result<sea_orm::QueryResult, PostPurgeRecoveryError> {
    let backend = db.get_database_backend();
    db.query_one_raw(statement(backend,&format!(
        "SELECT namespace_revision,verified_manifest_digest FROM module_artifact_data_namespaces namespace
         WHERE tenant_id={{1}} AND data_owner_id={{2}} AND namespace_instance_id={{3}}
          AND state='verified' AND purged_at IS NULL AND data_contract_digest={{4}}
          AND NOT EXISTS (SELECT 1 FROM module_artifact_data_owner_references reference
           WHERE reference.tenant_id=namespace.tenant_id AND reference.data_owner_id=namespace.data_owner_id
            AND reference.namespace_instance_id=namespace.namespace_instance_id){}",
        namespace_lock_clause(backend)),
        vec![uuid_value(scope.tenant_id,backend),uuid_value(scope.data_owner_id,backend),uuid_value(scope.namespace_instance_id,backend),
             scope.data_contract_digest.clone().into()])).await.map_err(storage_error)?
        .ok_or(PostPurgeRecoveryError::Integrity)
}

async fn ensure_recovery_hold_on(
    db: &DatabaseTransaction,
    operation: &RecoveryOperation,
) -> Result<(), PostPurgeRecoveryError> {
    let backend = db.get_database_backend();
    let scope = &operation.original.scope;
    let row=db.query_one_raw(statement(backend,
        "SELECT 1 FROM module_artifact_data_snapshot_holds WHERE tenant_id={1} AND snapshot_id={2}
         AND holder_kind='recovery' AND holder_id={3} AND data_owner_id={4} AND namespace_instance_id={5}
         AND request_digest={6} AND released_at IS NULL",
        vec![uuid_value(scope.tenant_id,backend),uuid_value(operation.source_snapshot_id,backend),uuid_value(operation.recovery_id,backend),
             uuid_value(scope.data_owner_id,backend),uuid_value(operation.namespace_instance_id,backend),operation.request_digest.clone().into()]))
        .await.map_err(storage_error)?;
    if row.is_none() {
        return Err(PostPurgeRecoveryError::CasCutoverConflict);
    }
    Ok(())
}

async fn load_prepare_operation(
    db: &DatabaseTransaction,
    request: &PrepareRecoveryRequest,
    tenant_id: Uuid,
    request_digest: &str,
) -> Result<Option<RecoveryOperation>, PostPurgeRecoveryError> {
    let backend = db.get_database_backend();
    let row = db
        .query_one_raw(statement(
            backend,
            "SELECT * FROM module_artifact_data_namespace_recovery_operations
         WHERE tenant_id={1} AND installation_id={2} AND idempotency_key={3}",
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(request.installation_id, backend),
                uuid_value(request.context.idempotency_key, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let stored_request_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
    if stored_request_digest != request_digest {
        return Err(PostPurgeRecoveryError::IdempotencyConflict);
    }
    Ok(Some(operation_from_row(&row, backend)?))
}

async fn load_operation(
    db: &DatabaseTransaction,
    tenant_id: Uuid,
    recovery_id: Uuid,
) -> Result<RecoveryOperation, PostPurgeRecoveryError> {
    let backend = db.get_database_backend();
    let row=db.query_one_raw(statement(backend,
        "SELECT * FROM module_artifact_data_namespace_recovery_operations WHERE tenant_id={1} AND recovery_id={2}",
        vec![uuid_value(tenant_id,backend),uuid_value(recovery_id,backend)])).await.map_err(storage_error)?
        .ok_or(PostPurgeRecoveryError::CasCutoverConflict)?;
    operation_from_row(&row, backend)
}

fn operation_from_row(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<RecoveryOperation, PostPurgeRecoveryError> {
    let original: ArtifactDataPurgeAuthorizationContext = serde_json::from_str(
        &row.try_get::<String>("", "authorization_json")
            .map_err(storage_error)?,
    )
    .map_err(storage_error)?;
    original.scope.validate().map_err(storage_error)?;
    if uuid_from_row(row, "tenant_id", backend).map_err(storage_error)? != original.scope.tenant_id
        || uuid_from_row(row, "installation_id", backend).map_err(storage_error)?
            != original.installation_id
        || uuid_from_row(row, "data_owner_id", backend).map_err(storage_error)?
            != original.data_owner_id
        || uuid_from_row(row, "source_namespace_instance_id", backend).map_err(storage_error)?
            != original.scope.namespace_instance_id
    {
        return Err(PostPurgeRecoveryError::Integrity);
    }
    let status: String = row.try_get("", "status").map_err(storage_error)?;
    if !matches!(status.as_str(), "staging" | "verified" | "cutover") {
        return Err(PostPurgeRecoveryError::Integrity);
    }
    let counter = |field| -> Result<u64, PostPurgeRecoveryError> {
        let value: i64 = row.try_get("", field).map_err(storage_error)?;
        u64::try_from(value).map_err(storage_error)
    };
    let optional_revision = |field| -> Result<Option<u64>, PostPurgeRecoveryError> {
        let value: Option<i64> = row.try_get("", field).map_err(storage_error)?;
        value
            .map(|value| {
                u64::try_from(value)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or(PostPurgeRecoveryError::Integrity)
            })
            .transpose()
    };
    Ok(RecoveryOperation {
        recovery_id: uuid_from_row(row, "recovery_id", backend).map_err(storage_error)?,
        original,
        namespace_instance_id: uuid_from_row(row, "namespace_instance_id", backend)
            .map_err(storage_error)?,
        source_snapshot_id: uuid_from_row(row, "source_snapshot_id", backend)
            .map_err(storage_error)?,
        source_retention_revision: positive_u64(row, "source_retention_revision")
            .map_err(storage_error)?,
        expected_reference_revision: positive_u64(row, "expected_reference_revision")
            .map_err(storage_error)?,
        tombstone_namespace_revision: positive_u64(row, "tombstone_namespace_revision")
            .map_err(storage_error)?,
        target_namespace_revision: positive_u64(row, "target_namespace_revision")
            .map_err(storage_error)?,
        records_restored: counter("records_restored")?,
        objects_restored: counter("objects_restored")?,
        source_manifest_digest: row
            .try_get("", "source_manifest_digest")
            .map_err(storage_error)?,
        verified_manifest_digest: row
            .try_get("", "verified_manifest_digest")
            .map_err(storage_error)?,
        request_digest: row.try_get("", "request_digest").map_err(storage_error)?,
        status: status.clone(),
        cutover_request_digest: row
            .try_get("", "cutover_request_digest")
            .map_err(storage_error)?,
        active_reference_revision: optional_revision("active_reference_revision")?,
        active_namespace_revision: optional_revision("active_namespace_revision")?,
        cutover_at: if status == "cutover" {
            Some(datetime_from_row(row, "cutover_at", backend).map_err(storage_error)?)
        } else {
            None
        },
    })
}

async fn terminal_cutover_replay(
    transaction: DatabaseTransaction,
    operation: RecoveryOperation,
    request_digest: &str,
) -> Result<PostPurgeRecoveryCutoverReceipt, PostPurgeRecoveryError> {
    if operation.cutover_request_digest.as_deref() != Some(request_digest) {
        return Err(PostPurgeRecoveryError::IdempotencyConflict);
    }
    let receipt = operation.cutover_receipt()?;
    transaction.commit().await.map_err(storage_error)?;
    Ok(receipt)
}

fn validate_prepare(request: &PrepareRecoveryRequest) -> Result<Uuid, PostPurgeRecoveryError> {
    let tenant = validate_context(&request.context, &request.reason)?;
    if request.installation_id.is_nil() || request.source_snapshot_id.is_nil() {
        return Err(PostPurgeRecoveryError::InvalidCommand);
    }
    number(request.expected_reference_revision)?;
    number(request.expected_tombstone_revision)?;
    if request.expected_reference_revision == 0 || request.expected_tombstone_revision == 0 {
        return Err(PostPurgeRecoveryError::InvalidCommand);
    }
    Ok(tenant)
}
fn validate_cutover(
    request: &PostPurgeRecoveryCutoverRequest,
) -> Result<Uuid, PostPurgeRecoveryError> {
    let tenant = validate_context(&request.context, &request.reason)?;
    if request.recovery_id.is_nil() || !valid_digest(&request.verified_manifest_digest) {
        return Err(PostPurgeRecoveryError::InvalidCommand);
    }
    for revision in [
        request.expected_reference_revision,
        request.expected_tombstone_revision,
        request.expected_target_namespace_revision,
    ] {
        number(revision)?;
        if revision == 0 {
            return Err(PostPurgeRecoveryError::InvalidCommand);
        }
    }
    Ok(tenant)
}
fn validate_context(
    context: &ModuleCommandContext,
    reason: &str,
) -> Result<Uuid, PostPurgeRecoveryError> {
    context
        .validate()
        .map_err(|_| PostPurgeRecoveryError::InvalidCommand)?;
    let tenant = context
        .tenant_id
        .filter(|id| !id.is_nil())
        .ok_or(PostPurgeRecoveryError::InvalidCommand)?;
    if reason.trim().is_empty() || reason.len() > 2000 {
        return Err(PostPurgeRecoveryError::InvalidCommand);
    }
    Ok(tenant)
}
fn statement(backend: DbBackend, sql: &str, values: Vec<SqlValue>) -> Statement {
    let mut sql = sql.to_owned();
    for index in 1..=values.len() {
        sql = sql.replace(&format!("{{{index}}}"), &placeholder(backend, index));
    }
    Statement::from_sql_and_values(backend, sql, values)
}
fn number(value: u64) -> Result<SqlValue, PostPurgeRecoveryError> {
    revision_value(value).map_err(storage_error)
}
fn timestamp(value: DateTime<Utc>, backend: DbBackend) -> SqlValue {
    if backend == DbBackend::Postgres {
        SqlValue::ChronoDateTimeUtc(Some(value))
    } else {
        value.to_rfc3339().into()
    }
}
fn storage_error(error: impl std::fmt::Display) -> PostPurgeRecoveryError {
    PostPurgeRecoveryError::Storage(error.to_string())
}

fn recovery_data_error(error: crate::ArtifactDataError) -> PostPurgeRecoveryError {
    match error {
        crate::ArtifactDataError::SnapshotIntegrity => PostPurgeRecoveryError::Integrity,
        crate::ArtifactDataError::IdempotencyConflict => {
            PostPurgeRecoveryError::IdempotencyConflict
        }
        crate::ArtifactDataError::RestorePrecondition => PostPurgeRecoveryError::CasCutoverConflict,
        error => storage_error(error),
    }
}
