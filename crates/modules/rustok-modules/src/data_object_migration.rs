//! Authorized, create-only migration between opaque namespace instances.
//! Frozen requests and all copy keys commit before byte publication.
//! Exact recovery verifies actual bytes and references; uncertainty never permits GC.

use async_trait::async_trait;
use chrono::Utc;
use object_store::{ObjectStoreExt, PutMode, path::Path};
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone, StorageRuntime};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactDataNamespace, ArtifactDataObject, ArtifactDataQuota, ModuleCommandContext,
    data::{
        configure_tenant_scope, lock_artifact_data_namespace_on, placeholder, revision_value,
        uuid_from_row, uuid_value,
    },
    promotion::{digest_json, valid_digest},
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataObjectMigrationError {
    #[error("Object migration requires distinct namespace instances")]
    SameNamespace,
    #[error("Object migration command is invalid")]
    InvalidCommand,
    #[error("Command context tenant does not match request tenant")]
    TenantMismatch,
    #[error("Reason must not be empty")]
    EmptyReason,
    #[error("Object migration policy denied the command")]
    PolicyDenied,
    #[error("Object migration namespace preconditions changed")]
    NamespacePrecondition,
    #[error("Object migration identity was reused with different evidence")]
    IdempotencyConflict,
    #[error("Target object '{0}' conflicts with the reserved copy")]
    TargetObjectConflict(String),
    #[error("Object migration bytes or metadata do not match the frozen inventory")]
    Integrity,
    #[error("Object migration exceeds namespace quota")]
    QuotaExceeded,
    #[error("Object migration storage error: {0}")]
    Storage(String),
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactDataObjectMigrationError {
    ArtifactDataObjectMigrationError::Storage(error.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDataObjectMigrationRequest {
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub source_namespace_instance_id: Uuid,
    pub target_namespace_instance_id: Uuid,
    pub expected_source_namespace_revision: u64,
    pub expected_target_namespace_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataObjectMigrationReceipt {
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub source_namespace_instance_id: Uuid,
    pub target_namespace_instance_id: Uuid,
    pub inventory_manifest_digest: String,
    pub objects_migrated: u64,
    pub accepted: bool,
}

/// Implementations bind current actor authority and maintenance evidence on
/// the owner's transaction. A permission read alone is not an operational fence.
#[async_trait]
pub trait ArtifactDataObjectMigrationAuthorizer: Send + Sync {
    async fn authorize_object_migration_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &ArtifactDataObjectMigrationRequest,
        source: &ArtifactDataNamespace,
        target: &ArtifactDataNamespace,
    ) -> Result<(), ArtifactDataObjectMigrationError>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReservedObject {
    object: ArtifactDataObject,
    source_storage_key: String,
    target_storage_key: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenInventory {
    source: ArtifactDataNamespace,
    target: ArtifactDataNamespace,
    objects: Vec<ReservedObject>,
}

struct MigrationOperation {
    id: Uuid,
    request_digest: String,
    inventory_digest: String,
    inventory: FrozenInventory,
    committed: bool,
}

impl MigrationOperation {
    fn receipt(
        &self,
        request: &ArtifactDataObjectMigrationRequest,
    ) -> Result<ArtifactDataObjectMigrationReceipt, ArtifactDataObjectMigrationError> {
        Ok(ArtifactDataObjectMigrationReceipt {
            operation_id: self.id,
            tenant_id: request.tenant_id,
            data_owner_id: request.data_owner_id,
            source_namespace_instance_id: request.source_namespace_instance_id,
            target_namespace_instance_id: request.target_namespace_instance_id,
            inventory_manifest_digest: self.inventory_digest.clone(),
            objects_migrated: u64::try_from(self.inventory.objects.len()).map_err(storage_error)?,
            accepted: self.committed,
        })
    }
}

struct LockedNamespaces {
    source: ArtifactDataNamespace,
    target: ArtifactDataNamespace,
    source_state: String,
    target_state: String,
}

impl LockedNamespaces {
    async fn validate_on(
        &self,
        db: &DatabaseTransaction,
        request: &ArtifactDataObjectMigrationRequest,
    ) -> Result<(), ArtifactDataObjectMigrationError> {
        if self.source_state != "verified"
            || self.target_state != "staging"
            || self.source.namespace_revision != request.expected_source_namespace_revision
            || self.target.namespace_revision != request.expected_target_namespace_revision
        {
            return Err(ArtifactDataObjectMigrationError::NamespacePrecondition);
        }
        let backend = db.get_database_backend();
        let serving = db
            .query_one_raw(statement(
                backend,
                "SELECT 1 FROM module_artifact_data_owner_references
             WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id IN ({3},{4})",
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.data_owner_id, backend),
                    uuid_value(request.source_namespace_instance_id, backend),
                    uuid_value(request.target_namespace_instance_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if serving.is_some() {
            return Err(ArtifactDataObjectMigrationError::NamespacePrecondition);
        }
        Ok(())
    }
}

pub struct ArtifactDataObjectMigrationService<A> {
    db: DatabaseConnection,
    storage: StorageRuntime,
    authorizer: A,
}

impl<A: ArtifactDataObjectMigrationAuthorizer> ArtifactDataObjectMigrationService<A> {
    pub fn new(db: DatabaseConnection, storage: StorageRuntime, authorizer: A) -> Self {
        Self {
            db,
            storage,
            authorizer,
        }
    }

    pub async fn migrate_objects(
        &self,
        request: ArtifactDataObjectMigrationRequest,
    ) -> Result<ArtifactDataObjectMigrationReceipt, ArtifactDataObjectMigrationError> {
        validate_request(&request)?;
        let operation = self.reserve(&request).await?;
        if operation.committed {
            return operation.receipt(&request);
        }
        for object in &operation.inventory.objects {
            self.publish_copy(object).await?;
        }
        self.commit(&request, operation).await
    }

    async fn reserve(
        &self,
        request: &ArtifactDataObjectMigrationRequest,
    ) -> Result<MigrationOperation, ArtifactDataObjectMigrationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        if let Some(operation) = load_operation(&transaction, request).await?
            && operation.committed
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(operation);
        }
        let locked = lock_namespaces(&transaction, request).await?;
        // A waiter replays terminal evidence before mutable state/ref checks.
        if let Some(operation) = load_operation(&transaction, request).await? {
            if operation.committed {
                transaction.commit().await.map_err(storage_error)?;
                return Ok(operation);
            }
            locked.validate_on(&transaction, request).await?;
            if operation.inventory.source != locked.source
                || operation.inventory.target != locked.target
            {
                return Err(ArtifactDataObjectMigrationError::NamespacePrecondition);
            }
            self.authorizer
                .authorize_object_migration_on(
                    &transaction,
                    request,
                    &locked.source,
                    &locked.target,
                )
                .await?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(operation);
        }
        locked.validate_on(&transaction, request).await?;
        self.authorizer
            .authorize_object_migration_on(&transaction, request, &locked.source, &locked.target)
            .await?;
        crate::data::ensure_namespace_not_migration_held_on(
            &transaction,
            request.tenant_id,
            request.data_owner_id,
            request.source_namespace_instance_id,
            None,
        )
        .await
        .map_err(|error| match error {
            crate::ArtifactDataError::Storage(message) => {
                ArtifactDataObjectMigrationError::Storage(message)
            }
            _ => ArtifactDataObjectMigrationError::NamespacePrecondition,
        })?;
        crate::data::ensure_namespace_not_migration_held_on(
            &transaction,
            request.tenant_id,
            request.data_owner_id,
            request.target_namespace_instance_id,
            None,
        )
        .await
        .map_err(|error| match error {
            crate::ArtifactDataError::Storage(message) => {
                ArtifactDataObjectMigrationError::Storage(message)
            }
            _ => ArtifactDataObjectMigrationError::NamespacePrecondition,
        })?;
        let backend = transaction.get_database_backend();
        if transaction
            .query_one_raw(statement(
                backend,
                "SELECT 1 FROM module_artifact_data_object_migration_operations
             WHERE tenant_id={1} AND data_owner_id={2} AND target_namespace_instance_id={3}",
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.data_owner_id, backend),
                    uuid_value(request.target_namespace_instance_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
            .is_some()
        {
            return Err(ArtifactDataObjectMigrationError::NamespacePrecondition);
        }
        if let Some(object) = load_objects(&transaction, &locked.target).await?.first() {
            return Err(ArtifactDataObjectMigrationError::TargetObjectConflict(
                object.object.name.clone(),
            ));
        }
        let mut objects = load_objects(&transaction, &locked.source).await?;
        check_quota(&objects)?;
        for object in &mut objects {
            object.target_storage_key = ObjectKey::chronological(
                "module-data",
                ObjectZone::Objects,
                ObjectScope::Namespace {
                    tenant_id: request.tenant_id,
                    owner_id: request.data_owner_id,
                    instance_id: request.target_namespace_instance_id,
                },
                Utc::now(),
                Uuid::new_v4(),
                "bin",
            )
            .map_err(storage_error)?
            .to_string();
        }
        let inventory = FrozenInventory {
            source: locked.source,
            target: locked.target,
            objects,
        };
        let operation = MigrationOperation {
            id: Uuid::new_v4(),
            request_digest: digest_json(request).map_err(storage_error)?,
            inventory_digest: digest_json(&inventory).map_err(storage_error)?,
            inventory,
            committed: false,
        };
        let backend = transaction.get_database_backend();
        transaction.execute_raw(statement(backend,
            "INSERT INTO module_artifact_data_object_migration_operations
             (operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,
              idempotency_key,request_digest,request_json,inventory_manifest_digest,inventory_json,status,created_at)
             VALUES ({1},{2},{3},{4},{5},{6},{7},{8},{9},{10},'preparing',{11})",
            vec![uuid_value(operation.id,backend),uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),
                 uuid_value(request.source_namespace_instance_id,backend),uuid_value(request.target_namespace_instance_id,backend),
                 uuid_value(request.context.idempotency_key,backend),operation.request_digest.clone().into(),
                 serde_json::to_string(request).map_err(storage_error)?.into(),operation.inventory_digest.clone().into(),
                 serde_json::to_string(&operation.inventory).map_err(storage_error)?.into(),Utc::now().into()]))
            .await.map_err(storage_error)?;
        for object in &operation.inventory.objects {
            transaction.execute_raw(statement(backend,
                "INSERT INTO module_artifact_data_object_copy_operations
                 (operation_id,migration_operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,
                  inventory_manifest_digest,object_name,source_storage_key,target_storage_key,digest_sha256,size_bytes,status,
                  actor_id,trace_id,correlation_id,idempotency_key,reason,created_at)
                 VALUES ({1},{2},{3},{4},{5},{6},{7},{8},{9},{10},{11},{12},'intent',{13},{14},{15},{16},{17},{18})",
                vec![uuid_value(Uuid::new_v4(),backend),uuid_value(operation.id,backend),uuid_value(request.tenant_id,backend),
                     uuid_value(request.data_owner_id,backend),uuid_value(request.source_namespace_instance_id,backend),
                     uuid_value(request.target_namespace_instance_id,backend),operation.inventory_digest.clone().into(),
                     object.object.name.clone().into(),object.source_storage_key.clone().into(),object.target_storage_key.clone().into(),
                     object.object.digest_sha256.clone().into(),number(object.object.size_bytes)?,
                     uuid_value(request.context.actor_id,backend),request.context.trace_id.clone().into(),
                     uuid_value(request.context.correlation_id,backend),uuid_value(request.context.idempotency_key,backend),
                     request.reason.clone().into(),Utc::now().into()])).await.map_err(storage_error)?;
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(operation)
    }

    async fn publish_copy(
        &self,
        object: &ReservedObject,
    ) -> Result<(), ArtifactDataObjectMigrationError> {
        let target = Path::from(object.target_storage_key.as_str());
        match self.storage.objects.head(&target).await {
            Ok(_) => return self.verify_copy(object).await,
            Err(object_store::Error::NotFound { .. }) => {}
            Err(error) => return Err(storage_error(error)),
        }
        let bytes = self
            .storage
            .objects
            .get(&Path::from(object.source_storage_key.as_str()))
            .await
            .map_err(storage_error)?
            .bytes()
            .await
            .map_err(storage_error)?;
        verify_bytes(&object.object, &bytes)?;
        let mut options = self.storage.put_options(&object.object.content_type);
        options.mode = PutMode::Create;
        match self
            .storage
            .objects
            .put_opts(&target, bytes.into(), options)
            .await
        {
            Ok(_) | Err(object_store::Error::AlreadyExists { .. }) => {}
            Err(error) => return Err(storage_error(error)),
        }
        self.verify_copy(object).await
    }

    async fn verify_copy(
        &self,
        object: &ReservedObject,
    ) -> Result<(), ArtifactDataObjectMigrationError> {
        let bytes = self
            .storage
            .objects
            .get(&Path::from(object.target_storage_key.as_str()))
            .await
            .map_err(storage_error)?
            .bytes()
            .await
            .map_err(storage_error)?;
        verify_bytes(&object.object, &bytes)
    }

    async fn commit(
        &self,
        request: &ArtifactDataObjectMigrationRequest,
        mut operation: MigrationOperation,
    ) -> Result<ArtifactDataObjectMigrationReceipt, ArtifactDataObjectMigrationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        if let Some(existing) = load_operation(&transaction, request).await?
            && existing.committed
        {
            transaction.commit().await.map_err(storage_error)?;
            return existing.receipt(request);
        }
        let locked = lock_namespaces(&transaction, request).await?;
        let existing = load_operation(&transaction, request)
            .await?
            .ok_or(ArtifactDataObjectMigrationError::Integrity)?;
        if existing.committed {
            transaction.commit().await.map_err(storage_error)?;
            return existing.receipt(request);
        }
        locked.validate_on(&transaction, request).await?;
        if existing.id != operation.id
            || existing.inventory_digest != operation.inventory_digest
            || locked.source != operation.inventory.source
            || locked.target != operation.inventory.target
        {
            return Err(ArtifactDataObjectMigrationError::Integrity);
        }
        self.authorizer
            .authorize_object_migration_on(&transaction, request, &locked.source, &locked.target)
            .await?;
        if !load_objects(&transaction, &locked.target).await?.is_empty() {
            return Err(ArtifactDataObjectMigrationError::Integrity);
        }
        let backend = transaction.get_database_backend();
        // This transient state is visible only in this write transaction. Other
        // writers still see preparing; a crash rolls back to that held state.
        let committing = transaction
            .execute_raw(statement(
                backend,
                "UPDATE module_artifact_data_object_migration_operations SET status='committing'
             WHERE operation_id={1} AND request_digest={2} AND status='preparing'",
                vec![
                    uuid_value(operation.id, backend),
                    operation.request_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if committing.rows_affected() != 1 {
            return Err(ArtifactDataObjectMigrationError::Integrity);
        }
        for object in &operation.inventory.objects {
            self.verify_copy(object).await?;
            transaction.execute_raw(statement(backend,
                "INSERT INTO module_artifact_data_objects
                 (tenant_id,data_owner_id,namespace_instance_id,object_name,storage_key,content_type,size_bytes,digest_sha256,revision,created_at,updated_at)
                 VALUES ({1},{2},{3},{4},{5},{6},{7},{8},1,{9},{9})",
                vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.target_namespace_instance_id,backend),
                     object.object.name.clone().into(),object.target_storage_key.clone().into(),object.object.content_type.clone().into(),
                     number(object.object.size_bytes)?,object.object.digest_sha256.clone().into(),Utc::now().into()])).await.map_err(storage_error)?;
            let checkpoint = transaction.execute_raw(statement(backend,
                "UPDATE module_artifact_data_object_copy_operations SET status='checkpointed',committed_at={1}
                 WHERE migration_operation_id={2} AND tenant_id={3} AND object_name={4}
                  AND source_storage_key={5} AND target_storage_key={6} AND inventory_manifest_digest={7}
                  AND digest_sha256={8} AND size_bytes={9} AND status='intent'",
                vec![Utc::now().into(),uuid_value(operation.id,backend),uuid_value(request.tenant_id,backend),object.object.name.clone().into(),
                     object.source_storage_key.clone().into(),object.target_storage_key.clone().into(),operation.inventory_digest.clone().into(),
                     object.object.digest_sha256.clone().into(),number(object.object.size_bytes)?])).await.map_err(storage_error)?;
            if checkpoint.rows_affected() != 1 {
                return Err(ArtifactDataObjectMigrationError::Integrity);
            }
        }
        let finished = transaction.execute_raw(statement(backend,
            "UPDATE module_artifact_data_object_migration_operations SET status='committed',committed_at={1}
             WHERE operation_id={2} AND request_digest={3} AND status='committing'",
            vec![Utc::now().into(),uuid_value(operation.id,backend),operation.request_digest.clone().into()])).await.map_err(storage_error)?;
        if finished.rows_affected() != 1 {
            return Err(ArtifactDataObjectMigrationError::Integrity);
        }
        transaction.commit().await.map_err(storage_error)?;
        operation.committed = true;
        operation.receipt(request)
    }

    /// Resume exact persisted operations with current policy. Never age-delete unresolved bytes.
    pub async fn reconcile_stale_intents(
        &self,
        tenant_id: Uuid,
        data_owner_id: Uuid,
    ) -> Result<u64, ArtifactDataObjectMigrationError> {
        if tenant_id.is_nil() || data_owner_id.is_nil() {
            return Err(ArtifactDataObjectMigrationError::InvalidCommand);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let rows = transaction.query_all_raw(statement(backend,
            "SELECT request_json FROM module_artifact_data_object_migration_operations
             WHERE tenant_id={1} AND data_owner_id={2} AND status='preparing' ORDER BY created_at,operation_id LIMIT 100",
            vec![uuid_value(tenant_id,backend),uuid_value(data_owner_id,backend)])).await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        let mut resumed = 0_u64;
        for row in rows {
            let request: ArtifactDataObjectMigrationRequest = serde_json::from_str(
                &row.try_get::<String>("", "request_json")
                    .map_err(storage_error)?,
            )
            .map_err(storage_error)?;
            if request.tenant_id != tenant_id || request.data_owner_id != data_owner_id {
                return Err(ArtifactDataObjectMigrationError::Integrity);
            }
            self.migrate_objects(request).await?;
            resumed = resumed
                .checked_add(1)
                .ok_or(ArtifactDataObjectMigrationError::Integrity)?;
        }
        Ok(resumed)
    }

    pub async fn count_unmigrated_live_objects(
        &self,
        tenant_id: Uuid,
        data_owner_id: Uuid,
        source: Uuid,
        target: Uuid,
    ) -> Result<u64, ArtifactDataObjectMigrationError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let row = transaction.query_one_raw(statement(backend,
            "SELECT COUNT(*) AS count FROM module_artifact_data_objects source
             WHERE source.tenant_id={1} AND source.data_owner_id={2} AND source.namespace_instance_id={3}
              AND NOT EXISTS (SELECT 1 FROM module_artifact_data_objects target
               WHERE target.tenant_id=source.tenant_id AND target.data_owner_id=source.data_owner_id
                AND target.namespace_instance_id={4} AND target.object_name=source.object_name
                AND target.content_type=source.content_type AND target.digest_sha256=source.digest_sha256 AND target.size_bytes=source.size_bytes)",
            vec![uuid_value(tenant_id,backend),uuid_value(data_owner_id,backend),uuid_value(source,backend),uuid_value(target,backend)]))
            .await.map_err(storage_error)?.ok_or(ArtifactDataObjectMigrationError::Integrity)?;
        let count = u64::try_from(row.try_get::<i64>("", "count").map_err(storage_error)?)
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(count)
    }
}

fn validate_request(
    request: &ArtifactDataObjectMigrationRequest,
) -> Result<(), ArtifactDataObjectMigrationError> {
    if request.source_namespace_instance_id == request.target_namespace_instance_id {
        return Err(ArtifactDataObjectMigrationError::SameNamespace);
    }
    if request.context.tenant_id != Some(request.tenant_id) {
        return Err(ArtifactDataObjectMigrationError::TenantMismatch);
    }
    if request.reason.trim().is_empty() {
        return Err(ArtifactDataObjectMigrationError::EmptyReason);
    }
    if request.tenant_id.is_nil()
        || request.data_owner_id.is_nil()
        || request.source_namespace_instance_id.is_nil()
        || request.target_namespace_instance_id.is_nil()
        || request.expected_source_namespace_revision == 0
        || request.expected_target_namespace_revision == 0
        || request.expected_source_namespace_revision > i64::MAX as u64
        || request.expected_target_namespace_revision > i64::MAX as u64
        || request.context.validate().is_err()
        || request.reason.len() > 2_000
    {
        return Err(ArtifactDataObjectMigrationError::InvalidCommand);
    }
    Ok(())
}

async fn load_operation<C: ConnectionTrait>(
    db: &C,
    request: &ArtifactDataObjectMigrationRequest,
) -> Result<Option<MigrationOperation>, ArtifactDataObjectMigrationError> {
    let backend = db.get_database_backend();
    let row = db.query_one_raw(statement(backend,
        "SELECT operation_id,request_digest,inventory_manifest_digest,inventory_json,status
         FROM module_artifact_data_object_migration_operations WHERE tenant_id={1} AND data_owner_id={2} AND idempotency_key={3}",
        vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.context.idempotency_key,backend)]))
        .await.map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let request_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
    if request_digest != digest_json(request).map_err(storage_error)? {
        return Err(ArtifactDataObjectMigrationError::IdempotencyConflict);
    }
    let inventory_digest: String = row
        .try_get("", "inventory_manifest_digest")
        .map_err(storage_error)?;
    let inventory: FrozenInventory = serde_json::from_str(
        &row.try_get::<String>("", "inventory_json")
            .map_err(storage_error)?,
    )
    .map_err(storage_error)?;
    check_quota(&inventory.objects)?;
    if digest_json(&inventory).map_err(storage_error)? != inventory_digest
        || inventory.source.tenant_id != request.tenant_id
        || inventory.target.tenant_id != request.tenant_id
        || inventory.source.data_owner_id != request.data_owner_id
        || inventory.target.data_owner_id != request.data_owner_id
        || inventory.source.namespace_instance_id != request.source_namespace_instance_id
        || inventory.target.namespace_instance_id != request.target_namespace_instance_id
        || inventory.source.namespace_revision != request.expected_source_namespace_revision
        || inventory.target.namespace_revision != request.expected_target_namespace_revision
    {
        return Err(ArtifactDataObjectMigrationError::Integrity);
    }
    let status: String = row.try_get("", "status").map_err(storage_error)?;
    if !matches!(status.as_str(), "preparing" | "committed") {
        return Err(ArtifactDataObjectMigrationError::Integrity);
    }
    Ok(Some(MigrationOperation {
        id: uuid_from_row(&row, "operation_id", backend).map_err(storage_error)?,
        request_digest,
        inventory_digest,
        inventory,
        committed: status == "committed",
    }))
}

async fn lock_namespaces(
    db: &DatabaseTransaction,
    request: &ArtifactDataObjectMigrationRequest,
) -> Result<LockedNamespaces, ArtifactDataObjectMigrationError> {
    let mut ids = [
        request.source_namespace_instance_id,
        request.target_namespace_instance_id,
    ];
    ids.sort();
    let mut entries = Vec::new();
    for id in ids {
        entries.push(
            lock_artifact_data_namespace_on(db, request.tenant_id, request.data_owner_id, id)
                .await
                .map_err(storage_error)?
                .ok_or(ArtifactDataObjectMigrationError::NamespacePrecondition)?,
        );
    }
    let (source, source_state) = entries
        .iter()
        .find(|(namespace, _)| {
            namespace.namespace_instance_id == request.source_namespace_instance_id
        })
        .cloned()
        .ok_or(ArtifactDataObjectMigrationError::Integrity)?;
    let (target, target_state) = entries
        .iter()
        .find(|(namespace, _)| {
            namespace.namespace_instance_id == request.target_namespace_instance_id
        })
        .cloned()
        .ok_or(ArtifactDataObjectMigrationError::Integrity)?;
    Ok(LockedNamespaces {
        source,
        target,
        source_state,
        target_state,
    })
}

async fn load_objects<C: ConnectionTrait>(
    db: &C,
    namespace: &ArtifactDataNamespace,
) -> Result<Vec<ReservedObject>, ArtifactDataObjectMigrationError> {
    let backend = db.get_database_backend();
    let limit = ArtifactDataQuota::default()
        .max_objects
        .checked_add(1)
        .ok_or(ArtifactDataObjectMigrationError::QuotaExceeded)?;
    let rows = db.query_all_raw(statement(backend,
        "SELECT object_name,storage_key,content_type,size_bytes,digest_sha256,revision FROM module_artifact_data_objects
         WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id={3} ORDER BY object_name LIMIT {4}",
        vec![uuid_value(namespace.tenant_id,backend),uuid_value(namespace.data_owner_id,backend),uuid_value(namespace.namespace_instance_id,backend),number(limit)?]))
        .await.map_err(storage_error)?;
    rows.into_iter()
        .map(|row| {
            let object = ArtifactDataObject {
                name: row.try_get("", "object_name").map_err(storage_error)?,
                content_type: row.try_get("", "content_type").map_err(storage_error)?,
                size_bytes: u64::try_from(
                    row.try_get::<i64>("", "size_bytes")
                        .map_err(storage_error)?,
                )
                .map_err(storage_error)?,
                digest_sha256: row.try_get("", "digest_sha256").map_err(storage_error)?,
                revision: u64::try_from(row.try_get::<i64>("", "revision").map_err(storage_error)?)
                    .map_err(storage_error)?,
            };
            if !valid_digest(&object.digest_sha256) || object.revision == 0 {
                return Err(ArtifactDataObjectMigrationError::Integrity);
            }
            Ok(ReservedObject {
                object,
                source_storage_key: row.try_get("", "storage_key").map_err(storage_error)?,
                target_storage_key: String::new(),
            })
        })
        .collect()
}

fn check_quota(objects: &[ReservedObject]) -> Result<(), ArtifactDataObjectMigrationError> {
    let quota = ArtifactDataQuota::default();
    let bytes = objects
        .iter()
        .try_fold(0_u64, |total, object| {
            total.checked_add(object.object.size_bytes)
        })
        .ok_or(ArtifactDataObjectMigrationError::QuotaExceeded)?;
    if u64::try_from(objects.len()).map_err(storage_error)? > quota.max_objects
        || bytes > quota.max_object_bytes
    {
        return Err(ArtifactDataObjectMigrationError::QuotaExceeded);
    }
    Ok(())
}

fn verify_bytes(
    object: &ArtifactDataObject,
    bytes: &[u8],
) -> Result<(), ArtifactDataObjectMigrationError> {
    if u64::try_from(bytes.len()).ok() != Some(object.size_bytes)
        || format!("sha256:{}", hex::encode(Sha256::digest(bytes))) != object.digest_sha256
    {
        return Err(ArtifactDataObjectMigrationError::Integrity);
    }
    Ok(())
}

fn number(value: u64) -> Result<sea_orm::Value, ArtifactDataObjectMigrationError> {
    revision_value(value).map_err(storage_error)
}

fn statement(backend: DbBackend, template: &str, values: Vec<sea_orm::Value>) -> Statement {
    let mut sql = template.to_owned();
    for index in 1..=values.len() {
        sql = sql.replace(&format!("{{{index}}}"), &placeholder(backend, index));
    }
    Statement::from_sql_and_values(backend, sql, values)
}
