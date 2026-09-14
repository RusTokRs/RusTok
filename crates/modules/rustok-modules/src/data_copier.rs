//! Authorized atomic record copying between owner namespace instances.
//! Source facts and records are read under root locks; exact replay precedes
//! mutable preconditions. A page receipt never authorizes serving selection.

use crate::{
    ArtifactDataNamespace, ArtifactDataQuota, ArtifactDataRecord, ArtifactDataScope,
    ModuleArtifactDescriptor, ModuleCommandContext,
    artifact_schema::ArtifactSchemaValidatorCache,
    data::{
        configure_tenant_scope, ensure_namespace_not_migration_held_on, index_contract_digest,
        lock_artifact_data_namespace_on, now_expression, placeholder, revision_value,
        synchronize_artifact_data_indexes, uuid_from_row, uuid_value,
        validate_artifact_data_index_contract, validate_artifact_data_key,
    },
    promotion::{digest_json, valid_digest},
};
use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataCopyError {
    #[error("Record copy requires distinct namespace instances")]
    SameNamespace,
    #[error("Record copy command is invalid")]
    InvalidCommand,
    #[error("Command tenant does not match record-copy tenant")]
    TenantMismatch,
    #[error("Record-copy policy denied the command")]
    PolicyDenied,
    #[error("Record-copy namespace preconditions changed")]
    NamespacePrecondition,
    #[error("Record-copy identity was reused with different evidence")]
    IdempotencyConflict,
    #[error("Target key '{0}' conflicts with the exact source record")]
    TargetKeyConflict(String),
    #[error("Record-copy metadata or receipt failed integrity validation")]
    Integrity,
    #[error("Record does not satisfy the admitted target schema")]
    SchemaViolation,
    #[error("Record copy exceeds the namespace quota")]
    QuotaExceeded,
    #[error("Record-copy storage error: {0}")]
    Storage(String),
}
fn storage_error(error: impl std::fmt::Display) -> ArtifactDataCopyError {
    ArtifactDataCopyError::Storage(error.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDataCopyRequest {
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub source_namespace_instance_id: Uuid,
    pub target_namespace_instance_id: Uuid,
    pub expected_source_namespace_revision: u64,
    pub expected_target_namespace_revision: u64,
    pub page_size: u32,
    pub page_cursor: Option<String>,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDataCopyReceipt {
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub source_namespace_instance_id: Uuid,
    pub target_namespace_instance_id: Uuid,
    pub source_namespace_revision: u64,
    pub policy_revision: u64,
    pub target_namespace_revision: u64,
    pub page_cursor: Option<String>,
    pub next_page_cursor: Option<String>,
    pub page_digest: String,
    pub items_copied: u64,
    pub is_terminal_page: bool,
}

/// The host supplies actual admitted target evidence and current policy limits.
/// This is mandatory composition; no default maintenance or fence policy exists.
pub struct ArtifactDataCopyAuthorization {
    pub target_scope: ArtifactDataScope,
    pub target_descriptor: ModuleArtifactDescriptor,
    pub quota: ArtifactDataQuota,
}

#[async_trait]
pub trait ArtifactDataCopyAuthorizer: Send + Sync {
    async fn authorize_copy_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &ArtifactDataCopyRequest,
        source: &ArtifactDataNamespace,
        target: &ArtifactDataNamespace,
    ) -> Result<ArtifactDataCopyAuthorization, ArtifactDataCopyError>;
}

pub struct ArtifactDataCopier<A> {
    db: DatabaseConnection,
    authorizer: A,
    schemas: ArtifactSchemaValidatorCache,
}

/// Durable admission of a frozen page; it cannot authorize serving selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDataCopyReservation {
    pub operation_id: Uuid,
    pub tenant_id: Uuid,
    pub data_owner_id: Uuid,
    pub request_digest: String,
    pub page_digest: String,
    pub is_committed: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FrozenPage {
    source: ArtifactDataNamespace,
    target: ArtifactDataNamespace,
    records: Vec<ArtifactDataRecord>,
    descriptor_digest: String,
    reservation_scope: ArtifactDataScope,
    reservation_quota: ArtifactDataQuota,
    page_digest: String,
    next_page_cursor: Option<String>,
    is_terminal_page: bool,
}
struct CopyOperation {
    operation_id: Uuid,
    request: ArtifactDataCopyRequest,
    request_digest: String,
    frozen: FrozenPage,
    frozen_digest: String,
    receipt: Option<ArtifactDataCopyReceipt>,
}
impl CopyOperation {
    fn reservation(&self) -> ArtifactDataCopyReservation {
        ArtifactDataCopyReservation {
            operation_id: self.operation_id,
            tenant_id: self.request.tenant_id,
            data_owner_id: self.request.data_owner_id,
            request_digest: self.request_digest.clone(),
            page_digest: self.frozen.page_digest.clone(),
            is_committed: self.receipt.is_some(),
        }
    }
}
impl<A: ArtifactDataCopyAuthorizer> ArtifactDataCopier<A> {
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        Self {
            db,
            authorizer,
            schemas: ArtifactSchemaValidatorCache::default(),
        }
    }
    /// Commit exact request and frozen source records before any record write.
    pub async fn reserve_page(
        &self,
        request: ArtifactDataCopyRequest,
    ) -> Result<ArtifactDataCopyReservation, ArtifactDataCopyError> {
        validate_request(&request)?;
        let request_digest = digest_json(&request).map_err(storage_error)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        if let Some(operation) = load_operation(&transaction, &request).await?
            && operation.receipt.is_some()
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(operation.reservation());
        }
        let (source, target) = locked_namespaces(&transaction, &request).await?;
        let existing = load_operation(&transaction, &request).await?;
        if let Some(operation) = &existing
            && operation.receipt.is_some()
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(operation.reservation());
        }
        let previous = validate_continuation(
            &transaction,
            &request,
            existing.as_ref().map(|o| o.operation_id),
        )
        .await?;
        let authority = self
            .authorizer
            .authorize_copy_on(&transaction, &request, &source, &target)
            .await?;
        validate_authority(&authority, &target)?;
        let descriptor_digest = digest_json(&authority.target_descriptor).map_err(storage_error)?;
        if previous
            .as_ref()
            .is_some_and(|o| o.frozen.descriptor_digest != descriptor_digest)
        {
            return Err(ArtifactDataCopyError::PolicyDenied);
        }
        if let Some(operation) = existing {
            ensure_frozen_facts(&operation.frozen, &source, &target, &descriptor_digest)?;
            self.validate_records(&authority, &operation.frozen.records)?;
            validate_projected_quota(
                &transaction,
                &request,
                &operation.frozen.records,
                authority.quota,
            )
            .await?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(operation.reservation());
        }
        let backend = transaction.get_database_backend();
        let mut values = vec![
            uuid_value(request.tenant_id, backend),
            uuid_value(request.data_owner_id, backend),
            uuid_value(source.namespace_instance_id, backend),
        ];
        let after = if let Some(cursor) = &request.page_cursor {
            values.push(cursor.clone().into());
            " AND data_key>{4}"
        } else {
            ""
        };
        let limit_position = values.len() + 1;
        values.push(i64::from(request.page_size + 1).into());
        let rows=transaction.query_all_raw(statement(backend,&format!(
            "SELECT data_key,CAST(value AS TEXT) AS value_text,value_size_bytes,revision FROM module_artifact_data
             WHERE tenant_id={{1}} AND data_owner_id={{2}} AND namespace_instance_id={{3}}{after}
             ORDER BY data_key ASC LIMIT {{{limit_position}}}"),values)).await.map_err(storage_error)?;
        let is_terminal_page =
            rows.len() <= usize::try_from(request.page_size).map_err(storage_error)?;
        let mut records = Vec::new();
        for row in rows
            .into_iter()
            .take(usize::try_from(request.page_size).map_err(storage_error)?)
        {
            let key: String = row.try_get("", "data_key").map_err(storage_error)?;
            let value: serde_json::Value = serde_json::from_str(
                &row.try_get::<String>("", "value_text")
                    .map_err(storage_error)?,
            )
            .map_err(|_| ArtifactDataCopyError::Integrity)?;
            let size = u64::try_from(
                row.try_get::<i64>("", "value_size_bytes")
                    .map_err(storage_error)?,
            )
            .map_err(storage_error)?;
            if size != record_size(&value)? {
                return Err(ArtifactDataCopyError::Integrity);
            }
            records.push(ArtifactDataRecord {
                key,
                value,
                revision: positive(row.try_get("", "revision").map_err(storage_error)?)?,
            });
        }
        self.validate_records(&authority, &records)?;
        validate_projected_quota(&transaction, &request, &records, authority.quota).await?;
        let frozen = FrozenPage {
            page_digest: digest_json(&(&source, &target, &records)).map_err(storage_error)?,
            descriptor_digest,
            reservation_scope: authority.target_scope,
            reservation_quota: authority.quota,
            next_page_cursor: if is_terminal_page {
                None
            } else {
                records.last().map(|r| r.key.clone())
            },
            source,
            target,
            records,
            is_terminal_page,
        };
        validate_frozen(&frozen, &request)?;
        let operation = CopyOperation {
            operation_id: Uuid::new_v4(),
            request,
            request_digest,
            frozen_digest: digest_json(&frozen).map_err(storage_error)?,
            frozen,
            receipt: None,
        };
        transaction.execute_raw(statement(backend,&format!(
            "INSERT INTO module_artifact_data_copy_operations
             (operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,
              expected_source_namespace_revision,expected_target_namespace_revision,idempotency_key,request_digest,
              request_json,frozen_page_json,frozen_page_digest,status,actor_id,trace_id,correlation_id,reason,created_at)
             VALUES ({{1}},{{2}},{{3}},{{4}},{{5}},{{6}},{{7}},{{8}},{{9}},{{10}},{{11}},{{12}},'preparing',{{13}},{{14}},{{15}},{{16}},{})",now_expression(backend)),
            vec![uuid_value(operation.operation_id,backend),uuid_value(operation.request.tenant_id,backend),uuid_value(operation.request.data_owner_id,backend),
                uuid_value(operation.request.source_namespace_instance_id,backend),uuid_value(operation.request.target_namespace_instance_id,backend),
                revision_value(operation.request.expected_source_namespace_revision).map_err(storage_error)?,
                revision_value(operation.request.expected_target_namespace_revision).map_err(storage_error)?,
                uuid_value(operation.request.context.idempotency_key,backend),operation.request_digest.clone().into(),json_value(&operation.request)?,
                json_value(&operation.frozen)?,operation.frozen_digest.clone().into(),uuid_value(operation.request.context.actor_id,backend),
                operation.request.context.trace_id.clone().into(),uuid_value(operation.request.context.correlation_id,backend),operation.request.reason.clone().into()]))
            .await.map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(operation.reservation())
    }
    /// Resume durable admission under current authority. Record/index writes,
    /// namespace CAS and the immutable exact response share the commit.
    pub async fn copy_page(
        &self,
        request: ArtifactDataCopyRequest,
    ) -> Result<ArtifactDataCopyReceipt, ArtifactDataCopyError> {
        let reservation = self.reserve_page(request.clone()).await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        let operation = load_operation(&transaction, &request)
            .await?
            .ok_or(ArtifactDataCopyError::Integrity)?;
        if let Some(receipt) = operation.receipt {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(receipt);
        }
        let (source, target) = locked_namespaces(&transaction, &request).await?;
        let operation = load_operation(&transaction, &request)
            .await?
            .ok_or(ArtifactDataCopyError::Integrity)?;
        if let Some(receipt) = operation.receipt {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(receipt);
        }
        if operation.operation_id != reservation.operation_id {
            return Err(ArtifactDataCopyError::Integrity);
        }
        validate_continuation(&transaction, &request, Some(operation.operation_id)).await?;
        let authority = self
            .authorizer
            .authorize_copy_on(&transaction, &request, &source, &target)
            .await?;
        validate_authority(&authority, &target)?;
        ensure_frozen_facts(
            &operation.frozen,
            &source,
            &target,
            &digest_json(&authority.target_descriptor).map_err(storage_error)?,
        )?;
        self.validate_records(&authority, &operation.frozen.records)?;
        validate_projected_quota(
            &transaction,
            &request,
            &operation.frozen.records,
            authority.quota,
        )
        .await?;
        let backend = transaction.get_database_backend();
        let committing=transaction.execute_raw(statement(backend,
            "UPDATE module_artifact_data_copy_operations SET status='committing'
             WHERE operation_id={1} AND request_digest={2} AND frozen_page_digest={3} AND status='preparing'",
            vec![uuid_value(operation.operation_id,backend),operation.request_digest.clone().into(),operation.frozen_digest.clone().into()]))
            .await.map_err(storage_error)?;
        if committing.rows_affected() != 1 {
            return Err(ArtifactDataCopyError::NamespacePrecondition);
        }
        write_records(
            &transaction,
            &request,
            &target,
            &authority,
            &operation.frozen.records,
        )
        .await?;
        validate_projected_quota(&transaction, &request, &[], authority.quota).await?;
        let revision = target
            .namespace_revision
            .checked_add(1)
            .filter(|value| *value <= i64::MAX as u64)
            .ok_or(ArtifactDataCopyError::NamespacePrecondition)?;
        let updated = transaction
            .execute_raw(statement(
                backend,
                &format!(
            "UPDATE module_artifact_data_namespaces SET namespace_revision={{1}},updated_at={}
             WHERE tenant_id={{2}} AND data_owner_id={{3}} AND namespace_instance_id={{4}}
              AND state='staging' AND namespace_revision={{5}}",now_expression(backend)),
                vec![
                    revision_value(revision).map_err(storage_error)?,
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.data_owner_id, backend),
                    uuid_value(target.namespace_instance_id, backend),
                    revision_value(target.namespace_revision).map_err(storage_error)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataCopyError::NamespacePrecondition);
        }
        let receipt = ArtifactDataCopyReceipt {
            operation_id: operation.operation_id,
            tenant_id: request.tenant_id,
            data_owner_id: request.data_owner_id,
            source_namespace_instance_id: source.namespace_instance_id,
            target_namespace_instance_id: target.namespace_instance_id,
            source_namespace_revision: source.namespace_revision,
            target_namespace_revision: revision,
            policy_revision: authority.target_scope.policy_revision,
            page_cursor: request.page_cursor,
            next_page_cursor: operation.frozen.next_page_cursor,
            page_digest: operation.frozen.page_digest,
            items_copied: u64::try_from(operation.frozen.records.len()).map_err(storage_error)?,
            is_terminal_page: operation.frozen.is_terminal_page,
        };
        let committed=transaction.execute_raw(statement(backend,&format!(
            "UPDATE module_artifact_data_copy_operations SET status='committed',receipt_json={{1}},receipt_digest={{2}},committed_at={}
             WHERE operation_id={{3}} AND request_digest={{4}} AND frozen_page_digest={{5}} AND status='committing'",now_expression(backend)),
            vec![json_value(&receipt)?,digest_json(&receipt).map_err(storage_error)?.into(),uuid_value(operation.operation_id,backend),
                operation.request_digest.into(),operation.frozen_digest.into()])).await.map_err(storage_error)?;
        if committed.rows_affected() != 1 {
            return Err(ArtifactDataCopyError::NamespacePrecondition);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(receipt)
    }
    /// Reconcile original requests through the real owner operation, never by
    /// resetting status or fabricating a context.
    pub async fn reconcile_pending_pages(
        &self,
        tenant_id: Uuid,
        data_owner_id: Uuid,
    ) -> Result<Vec<Result<ArtifactDataCopyReceipt, ArtifactDataCopyError>>, ArtifactDataCopyError>
    {
        if tenant_id.is_nil() || data_owner_id.is_nil() {
            return Err(ArtifactDataCopyError::InvalidCommand);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let rows=transaction.query_all_raw(statement(backend,&format!(
            "SELECT {OPERATION_COLUMNS} FROM module_artifact_data_copy_operations WHERE tenant_id={{1}} AND data_owner_id={{2}}
             AND status='preparing' ORDER BY created_at,operation_id LIMIT 100"),
            vec![uuid_value(tenant_id,backend),uuid_value(data_owner_id,backend)])).await.map_err(storage_error)?;
        let requests = rows
            .iter()
            .map(|row| {
                decode_operation(row, backend).and_then(|operation| {
                    if operation.request.tenant_id != tenant_id
                        || operation.request.data_owner_id != data_owner_id
                    {
                        return Err(ArtifactDataCopyError::Integrity);
                    }
                    Ok(operation.request)
                })
            })
            .collect::<Vec<_>>();
        transaction.commit().await.map_err(storage_error)?;
        let mut results = Vec::new();
        for request in requests {
            results.push(match request {
                Ok(request) => self.copy_page(request).await,
                Err(error) => Err(error),
            });
        }
        Ok(results)
    }
    fn validate_records(
        &self,
        authority: &ArtifactDataCopyAuthorization,
        records: &[ArtifactDataRecord],
    ) -> Result<(), ArtifactDataCopyError> {
        let contract = authority
            .target_descriptor
            .persistence_contract
            .as_ref()
            .ok_or(ArtifactDataCopyError::Integrity)?;
        let schema = authority
            .target_descriptor
            .schema_documents
            .iter()
            .find(|schema| schema.digest == contract.schema_digest)
            .ok_or(ArtifactDataCopyError::Integrity)?;
        for record in records {
            validate_artifact_data_key(&record.key)
                .map_err(|_| ArtifactDataCopyError::Integrity)?;
            record_size(&record.value)?;
            if record.revision == 0 || record.revision > i64::MAX as u64 {
                return Err(ArtifactDataCopyError::Integrity);
            }
            self.schemas
                .validate(&schema.digest, &schema.document, &record.value)
                .map_err(|_| ArtifactDataCopyError::SchemaViolation)?;
        }
        Ok(())
    }
}

async fn locked_namespaces(
    db: &DatabaseTransaction,
    request: &ArtifactDataCopyRequest,
) -> Result<(ArtifactDataNamespace, ArtifactDataNamespace), ArtifactDataCopyError> {
    let mut ids = [
        request.source_namespace_instance_id,
        request.target_namespace_instance_id,
    ];
    ids.sort();
    let mut namespaces = Vec::new();
    for id in ids {
        namespaces.push(
            lock_artifact_data_namespace_on(db, request.tenant_id, request.data_owner_id, id)
                .await
                .map_err(storage_error)?
                .ok_or(ArtifactDataCopyError::NamespacePrecondition)?,
        );
    }
    let (source, source_state) = namespaces
        .iter()
        .find(|(n, _)| n.namespace_instance_id == request.source_namespace_instance_id)
        .ok_or(ArtifactDataCopyError::Integrity)?;
    let (target, target_state) = namespaces
        .iter()
        .find(|(n, _)| n.namespace_instance_id == request.target_namespace_instance_id)
        .ok_or(ArtifactDataCopyError::Integrity)?;
    if source_state != "verified"
        || target_state != "staging"
        || source.namespace_revision != request.expected_source_namespace_revision
        || target.namespace_revision != request.expected_target_namespace_revision
    {
        return Err(ArtifactDataCopyError::NamespacePrecondition);
    }
    let backend = db.get_database_backend();
    if db.query_one_raw(statement(backend,
        "SELECT 1 FROM module_artifact_data_owner_references WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id IN ({3},{4})",
        vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),
            uuid_value(source.namespace_instance_id,backend),uuid_value(target.namespace_instance_id,backend)]))
        .await.map_err(storage_error)?.is_some(){return Err(ArtifactDataCopyError::NamespacePrecondition);}
    for namespace in [source, target] {
        ensure_namespace_not_migration_held_on(
            db,
            request.tenant_id,
            request.data_owner_id,
            namespace.namespace_instance_id,
            Some(request.target_namespace_instance_id),
        )
        .await
        .map_err(|error| match error {
            crate::ArtifactDataError::Storage(message) => ArtifactDataCopyError::Storage(message),
            _ => ArtifactDataCopyError::NamespacePrecondition,
        })?;
    }
    Ok((source.clone(), target.clone()))
}
async fn validate_continuation(
    db: &DatabaseTransaction,
    request: &ArtifactDataCopyRequest,
    own_pending: Option<Uuid>,
) -> Result<Option<CopyOperation>, ArtifactDataCopyError> {
    let backend = db.get_database_backend();
    let pending=db.query_one_raw(statement(backend,
        "SELECT operation_id FROM module_artifact_data_copy_operations
         WHERE tenant_id={1} AND data_owner_id={2} AND target_namespace_instance_id={3} AND status='preparing'",
        vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.target_namespace_instance_id,backend)]))
        .await.map_err(storage_error)?;
    if let Some(row) = pending
        && own_pending != Some(uuid_from_row(&row, "operation_id", backend).map_err(storage_error)?)
    {
        return Err(ArtifactDataCopyError::NamespacePrecondition);
    }
    let previous=db.query_one_raw(statement(backend,&format!(
        "SELECT {OPERATION_COLUMNS} FROM module_artifact_data_copy_operations
         WHERE tenant_id={{1}} AND data_owner_id={{2}} AND target_namespace_instance_id={{3}} AND status='committed'
         ORDER BY expected_target_namespace_revision DESC LIMIT 1"),
        vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.target_namespace_instance_id,backend)]))
        .await.map_err(storage_error)?;
    if let Some(row) = previous {
        let previous = decode_operation(&row, backend)?;
        let receipt = previous
            .receipt
            .as_ref()
            .ok_or(ArtifactDataCopyError::Integrity)?;
        if previous.request.source_namespace_instance_id != request.source_namespace_instance_id
            || previous.request.expected_source_namespace_revision
                != request.expected_source_namespace_revision
            || receipt.is_terminal_page
            || receipt.target_namespace_revision != request.expected_target_namespace_revision
            || receipt.next_page_cursor != request.page_cursor
        {
            return Err(ArtifactDataCopyError::NamespacePrecondition);
        }
        Ok(Some(previous))
    } else {
        if request.page_cursor.is_some() {
            return Err(ArtifactDataCopyError::NamespacePrecondition);
        }
        if db.query_one_raw(statement(backend,
            "SELECT 1 FROM module_artifact_data WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id={3} LIMIT 1",
            vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.target_namespace_instance_id,backend)]))
            .await.map_err(storage_error)?.is_some(){return Err(ArtifactDataCopyError::NamespacePrecondition);}
        Ok(None)
    }
}
fn ensure_frozen_facts(
    frozen: &FrozenPage,
    source: &ArtifactDataNamespace,
    target: &ArtifactDataNamespace,
    descriptor_digest: &str,
) -> Result<(), ArtifactDataCopyError> {
    if frozen.source != *source || frozen.target != *target {
        return Err(ArtifactDataCopyError::NamespacePrecondition);
    }
    if frozen.descriptor_digest != descriptor_digest {
        return Err(ArtifactDataCopyError::PolicyDenied);
    }
    Ok(())
}
fn record_size(value: &serde_json::Value) -> Result<u64, ArtifactDataCopyError> {
    let size = u64::try_from(serde_json::to_vec(value).map_err(storage_error)?.len())
        .map_err(storage_error)?;
    if size == 0 || size > 64 * 1024 {
        return Err(ArtifactDataCopyError::Integrity);
    }
    Ok(size)
}
async fn validate_projected_quota(
    db: &DatabaseTransaction,
    request: &ArtifactDataCopyRequest,
    records: &[ArtifactDataRecord],
    quota: ArtifactDataQuota,
) -> Result<(), ArtifactDataCopyError> {
    let backend = db.get_database_backend();
    let row=db.query_one_raw(statement(backend,
        "SELECT COUNT(*) AS records,CAST(COALESCE(SUM(value_size_bytes),0) AS BIGINT) AS bytes FROM module_artifact_data
         WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id={3}",
        vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.target_namespace_instance_id,backend)]))
        .await.map_err(storage_error)?.ok_or(ArtifactDataCopyError::Integrity)?;
    let mut count = u64::try_from(row.try_get::<i64>("", "records").map_err(storage_error)?)
        .map_err(storage_error)?;
    let mut bytes = u64::try_from(row.try_get::<i64>("", "bytes").map_err(storage_error)?)
        .map_err(storage_error)?;
    for record in records {
        let existing=db.query_one_raw(statement(backend,
            "SELECT CAST(value AS TEXT) AS value_text,value_size_bytes,revision FROM module_artifact_data
             WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id={3} AND data_key={4}",
            vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),
                uuid_value(request.target_namespace_instance_id,backend),record.key.clone().into()])).await.map_err(storage_error)?;
        if let Some(row) = existing {
            let value: serde_json::Value = serde_json::from_str(
                &row.try_get::<String>("", "value_text")
                    .map_err(storage_error)?,
            )
            .map_err(|_| ArtifactDataCopyError::Integrity)?;
            if value != record.value
                || positive(row.try_get("", "revision").map_err(storage_error)?)? != record.revision
                || u64::try_from(
                    row.try_get::<i64>("", "value_size_bytes")
                        .map_err(storage_error)?,
                )
                .map_err(storage_error)?
                    != record_size(&record.value)?
            {
                return Err(ArtifactDataCopyError::TargetKeyConflict(record.key.clone()));
            }
        } else {
            count = count
                .checked_add(1)
                .ok_or(ArtifactDataCopyError::QuotaExceeded)?;
            bytes = bytes
                .checked_add(record_size(&record.value)?)
                .ok_or(ArtifactDataCopyError::QuotaExceeded)?;
        }
    }
    if count > quota.max_structured_records || bytes > quota.max_structured_bytes {
        return Err(ArtifactDataCopyError::QuotaExceeded);
    }
    Ok(())
}
async fn write_records(
    transaction: &DatabaseTransaction,
    request: &ArtifactDataCopyRequest,
    target: &ArtifactDataNamespace,
    authority: &ArtifactDataCopyAuthorization,
    records: &[ArtifactDataRecord],
) -> Result<(), ArtifactDataCopyError> {
    let backend = transaction.get_database_backend();
    let contract = authority
        .target_descriptor
        .persistence_contract
        .as_ref()
        .ok_or(ArtifactDataCopyError::Integrity)?;
    if let Some(index_digest) = index_contract_digest(&contract.indexes) {
        validate_artifact_data_index_contract(
            transaction,
            &authority.target_scope,
            backend,
            &index_digest,
            true,
        )
        .await
        .map_err(storage_error)?;
    }
    for record in records {
        let existing = transaction.query_one_raw(statement(backend,
                "SELECT CAST(value AS TEXT) AS value_text,value_size_bytes,revision FROM module_artifact_data
                 WHERE tenant_id={1} AND data_owner_id={2} AND namespace_instance_id={3} AND data_key={4}",
                vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),
                    uuid_value(target.namespace_instance_id,backend),record.key.clone().into()]))
                .await.map_err(storage_error)?;
        let encoded = serde_json::to_vec(&record.value).map_err(storage_error)?;
        let size = u64::try_from(encoded.len()).map_err(storage_error)?;
        if let Some(existing) = existing {
            let text: String = existing.try_get("", "value_text").map_err(storage_error)?;
            let value: serde_json::Value =
                serde_json::from_str(&text).map_err(|_| ArtifactDataCopyError::Integrity)?;
            let stored_size = u64::try_from(
                existing
                    .try_get::<i64>("", "value_size_bytes")
                    .map_err(storage_error)?,
            )
            .map_err(storage_error)?;
            let revision = positive(existing.try_get("", "revision").map_err(storage_error)?)?;
            if value != record.value || stored_size != size || revision != record.revision {
                return Err(ArtifactDataCopyError::TargetKeyConflict(record.key.clone()));
            }
        } else {
            transaction.execute_raw(statement(backend,
                    "INSERT INTO module_artifact_data (tenant_id,data_owner_id,namespace_instance_id,
                     data_key,value,value_size_bytes,revision,updated_at) VALUES ({1},{2},{3},{4},{5},{6},{7},{8})",
                    vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),
                        uuid_value(target.namespace_instance_id,backend),record.key.clone().into(),
                        SqlValue::Json(Some(Box::new(record.value.clone()))),
                        revision_value(size).map_err(storage_error)?,
                        revision_value(record.revision).map_err(storage_error)?,chrono::Utc::now().into()]))
                    .await.map_err(storage_error)?;
        }
        synchronize_artifact_data_indexes(
            transaction,
            &authority.target_scope,
            record,
            &contract.indexes,
        )
        .await
        .map_err(storage_error)?;
    }

    Ok(())
}
fn positive(value: i64) -> Result<u64, ArtifactDataCopyError> {
    u64::try_from(value)
        .ok()
        .filter(|n| *n > 0)
        .ok_or(ArtifactDataCopyError::Integrity)
}
fn validate_request(request: &ArtifactDataCopyRequest) -> Result<(), ArtifactDataCopyError> {
    if request.context.tenant_id != Some(request.tenant_id) {
        return Err(ArtifactDataCopyError::TenantMismatch);
    }
    if request.source_namespace_instance_id == request.target_namespace_instance_id {
        return Err(ArtifactDataCopyError::SameNamespace);
    }
    if request.tenant_id.is_nil()
        || request.data_owner_id.is_nil()
        || request.source_namespace_instance_id.is_nil()
        || request.target_namespace_instance_id.is_nil()
        || request.context.validate().is_err()
        || request.reason.trim().is_empty()
        || request.reason.len() > 2000
        || request.page_size == 0
        || request.page_size > 100
        || request.expected_source_namespace_revision == 0
        || request.expected_source_namespace_revision > i64::MAX as u64
        || request.expected_target_namespace_revision == 0
        || request.expected_target_namespace_revision > i64::MAX as u64
        || request
            .page_cursor
            .as_ref()
            .is_some_and(|key| validate_artifact_data_key(key).is_err())
    {
        return Err(ArtifactDataCopyError::InvalidCommand);
    }
    Ok(())
}
fn validate_authority(
    authority: &ArtifactDataCopyAuthorization,
    target: &ArtifactDataNamespace,
) -> Result<(), ArtifactDataCopyError> {
    authority
        .target_scope
        .validate()
        .map_err(|_| ArtifactDataCopyError::PolicyDenied)?;
    authority
        .quota
        .validate()
        .map_err(|_| ArtifactDataCopyError::PolicyDenied)?;
    authority
        .target_descriptor
        .validate()
        .map_err(|_| ArtifactDataCopyError::PolicyDenied)?;
    let scope = &authority.target_scope;
    if scope.policy_revision > i64::MAX as u64 {
        return Err(ArtifactDataCopyError::PolicyDenied);
    }
    let contract = authority
        .target_descriptor
        .persistence_contract
        .as_ref()
        .ok_or(ArtifactDataCopyError::PolicyDenied)?;
    if scope.tenant_id != target.tenant_id
        || scope.data_owner_id != target.data_owner_id
        || scope.namespace_instance_id != target.namespace_instance_id
        || scope.module_slug != target.module_slug
        || scope.data_contract_revision != target.data_contract_revision
        || scope.data_contract_digest != target.data_contract_digest
        || authority.target_descriptor.slug != target.module_slug
        || contract.revision != target.data_contract_revision
        || digest_json(contract).map_err(storage_error)? != target.data_contract_digest
    {
        return Err(ArtifactDataCopyError::PolicyDenied);
    }
    Ok(())
}

const OPERATION_COLUMNS:&str="operation_id,tenant_id,data_owner_id,source_namespace_instance_id,target_namespace_instance_id,
    expected_source_namespace_revision,expected_target_namespace_revision,idempotency_key,request_digest,
    CAST(request_json AS TEXT) AS request_json,CAST(frozen_page_json AS TEXT) AS frozen_page_json,frozen_page_digest,status,
    CAST(receipt_json AS TEXT) AS receipt_json,receipt_digest,actor_id,trace_id,correlation_id,reason";
async fn load_operation<C: ConnectionTrait>(
    db: &C,
    request: &ArtifactDataCopyRequest,
) -> Result<Option<CopyOperation>, ArtifactDataCopyError> {
    let backend = db.get_database_backend();
    let row=db.query_one_raw(statement(backend,&format!(
        "SELECT {OPERATION_COLUMNS} FROM module_artifact_data_copy_operations WHERE tenant_id={{1}} AND data_owner_id={{2}} AND idempotency_key={{3}}"),
        vec![uuid_value(request.tenant_id,backend),uuid_value(request.data_owner_id,backend),uuid_value(request.context.idempotency_key,backend)]))
        .await.map_err(storage_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    let saved_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
    if saved_digest != digest_json(request).map_err(storage_error)? {
        return Err(ArtifactDataCopyError::IdempotencyConflict);
    }
    let operation = decode_operation(&row, backend)?;
    if operation.request != *request {
        return Err(ArtifactDataCopyError::IdempotencyConflict);
    }
    Ok(Some(operation))
}
fn decode_operation(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<CopyOperation, ArtifactDataCopyError> {
    let request: ArtifactDataCopyRequest = serde_json::from_str(
        &row.try_get::<String>("", "request_json")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactDataCopyError::Integrity)?;
    validate_request(&request).map_err(|_| ArtifactDataCopyError::Integrity)?;
    let request_digest: String = row.try_get("", "request_digest").map_err(storage_error)?;
    if request_digest != digest_json(&request).map_err(storage_error)? {
        return Err(ArtifactDataCopyError::Integrity);
    }
    for (column, value) in [
        ("tenant_id", request.tenant_id),
        ("data_owner_id", request.data_owner_id),
        (
            "source_namespace_instance_id",
            request.source_namespace_instance_id,
        ),
        (
            "target_namespace_instance_id",
            request.target_namespace_instance_id,
        ),
        ("idempotency_key", request.context.idempotency_key),
        ("actor_id", request.context.actor_id),
        ("correlation_id", request.context.correlation_id),
    ] {
        if uuid_from_row(row, column, backend).map_err(storage_error)? != value {
            return Err(ArtifactDataCopyError::Integrity);
        }
    }
    if positive(
        row.try_get("", "expected_source_namespace_revision")
            .map_err(storage_error)?,
    )? != request.expected_source_namespace_revision
        || positive(
            row.try_get("", "expected_target_namespace_revision")
                .map_err(storage_error)?,
        )? != request.expected_target_namespace_revision
        || row
            .try_get::<String>("", "trace_id")
            .map_err(storage_error)?
            != request.context.trace_id
        || row.try_get::<String>("", "reason").map_err(storage_error)? != request.reason
    {
        return Err(ArtifactDataCopyError::Integrity);
    }
    let frozen: FrozenPage = serde_json::from_str(
        &row.try_get::<String>("", "frozen_page_json")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactDataCopyError::Integrity)?;
    let frozen_digest: String = row
        .try_get("", "frozen_page_digest")
        .map_err(storage_error)?;
    if frozen_digest != digest_json(&frozen).map_err(storage_error)? {
        return Err(ArtifactDataCopyError::Integrity);
    }
    validate_frozen(&frozen, &request)?;
    let operation_id = uuid_from_row(row, "operation_id", backend).map_err(storage_error)?;
    if operation_id.is_nil() {
        return Err(ArtifactDataCopyError::Integrity);
    }
    let receipt_json: Option<String> = row.try_get("", "receipt_json").map_err(storage_error)?;
    let receipt_digest: Option<String> =
        row.try_get("", "receipt_digest").map_err(storage_error)?;
    let status: String = row.try_get("", "status").map_err(storage_error)?;
    let receipt = match status.as_str() {
        "preparing" if receipt_json.is_none() && receipt_digest.is_none() => None,
        "committed" => {
            let receipt: ArtifactDataCopyReceipt = serde_json::from_str(
                receipt_json
                    .as_deref()
                    .ok_or(ArtifactDataCopyError::Integrity)?,
            )
            .map_err(|_| ArtifactDataCopyError::Integrity)?;
            if receipt_digest.as_deref()
                != Some(digest_json(&receipt).map_err(storage_error)?.as_str())
            {
                return Err(ArtifactDataCopyError::Integrity);
            }
            validate_receipt(&receipt, operation_id, &request, &frozen)?;
            Some(receipt)
        }
        _ => return Err(ArtifactDataCopyError::Integrity),
    };
    Ok(CopyOperation {
        operation_id,
        request,
        request_digest,
        frozen,
        frozen_digest,
        receipt,
    })
}
fn validate_frozen(
    frozen: &FrozenPage,
    request: &ArtifactDataCopyRequest,
) -> Result<(), ArtifactDataCopyError> {
    if frozen.source.tenant_id != request.tenant_id
        || frozen.target.tenant_id != request.tenant_id
        || frozen.source.data_owner_id != request.data_owner_id
        || frozen.target.data_owner_id != request.data_owner_id
        || frozen.source.namespace_instance_id != request.source_namespace_instance_id
        || frozen.target.namespace_instance_id != request.target_namespace_instance_id
        || frozen.source.namespace_revision != request.expected_source_namespace_revision
        || frozen.target.namespace_revision != request.expected_target_namespace_revision
        || frozen.records.len() > usize::try_from(request.page_size).map_err(storage_error)?
        || !valid_digest(&frozen.descriptor_digest)
        || digest_json(&(&frozen.source, &frozen.target, &frozen.records)).map_err(storage_error)?
            != frozen.page_digest
        || frozen.next_page_cursor
            != if frozen.is_terminal_page {
                None
            } else {
                frozen.records.last().map(|r| r.key.clone())
            }
        || (!frozen.is_terminal_page
            && frozen.records.len() != usize::try_from(request.page_size).map_err(storage_error)?)
    {
        return Err(ArtifactDataCopyError::Integrity);
    }
    frozen
        .reservation_scope
        .validate()
        .map_err(|_| ArtifactDataCopyError::Integrity)?;
    frozen
        .reservation_quota
        .validate()
        .map_err(|_| ArtifactDataCopyError::Integrity)?;
    let scope = &frozen.reservation_scope;
    if scope.tenant_id != frozen.target.tenant_id
        || scope.data_owner_id != frozen.target.data_owner_id
        || scope.namespace_instance_id != frozen.target.namespace_instance_id
        || scope.module_slug != frozen.target.module_slug
        || scope.data_contract_revision != frozen.target.data_contract_revision
        || scope.data_contract_digest != frozen.target.data_contract_digest
        || scope.policy_revision > i64::MAX as u64
    {
        return Err(ArtifactDataCopyError::Integrity);
    }
    let mut keys = std::collections::HashSet::new();
    for record in &frozen.records {
        validate_artifact_data_key(&record.key).map_err(|_| ArtifactDataCopyError::Integrity)?;
        if !keys.insert(&record.key) || record.revision == 0 || record.revision > i64::MAX as u64 {
            return Err(ArtifactDataCopyError::Integrity);
        }
        record_size(&record.value)?;
    }
    Ok(())
}
fn validate_receipt(
    receipt: &ArtifactDataCopyReceipt,
    operation_id: Uuid,
    request: &ArtifactDataCopyRequest,
    frozen: &FrozenPage,
) -> Result<(), ArtifactDataCopyError> {
    if receipt.operation_id != operation_id
        || receipt.tenant_id != request.tenant_id
        || receipt.data_owner_id != request.data_owner_id
        || receipt.source_namespace_instance_id != request.source_namespace_instance_id
        || receipt.target_namespace_instance_id != request.target_namespace_instance_id
        || receipt.source_namespace_revision != request.expected_source_namespace_revision
        || receipt.target_namespace_revision
            != request
                .expected_target_namespace_revision
                .checked_add(1)
                .ok_or(ArtifactDataCopyError::Integrity)?
        || receipt.target_namespace_revision > i64::MAX as u64
        || receipt.policy_revision == 0
        || receipt.policy_revision > i64::MAX as u64
        || receipt.page_cursor != request.page_cursor
        || receipt.next_page_cursor != frozen.next_page_cursor
        || receipt.page_digest != frozen.page_digest
        || receipt.items_copied != u64::try_from(frozen.records.len()).map_err(storage_error)?
        || receipt.is_terminal_page != frozen.is_terminal_page
        || receipt.is_terminal_page != receipt.next_page_cursor.is_none()
    {
        return Err(ArtifactDataCopyError::Integrity);
    }
    Ok(())
}
fn json_value(value: &impl Serialize) -> Result<SqlValue, ArtifactDataCopyError> {
    Ok(SqlValue::Json(Some(Box::new(
        serde_json::to_value(value).map_err(storage_error)?,
    ))))
}
fn statement(backend: DbBackend, template: &str, values: Vec<SqlValue>) -> Statement {
    let mut sql = template.to_string();
    for index in 1..=values.len() {
        sql = sql.replace(&format!("{{{index}}}"), &placeholder(backend, index));
    }
    Statement::from_sql_and_values(backend, sql, values)
}
