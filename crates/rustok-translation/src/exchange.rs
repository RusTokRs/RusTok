use std::sync::Arc;

use bytes::Bytes;
use chrono::{DateTime, Duration as ChronoDuration, FixedOffset, Utc};
use rustok_api::{Action, PortCallPolicy, PortContext, Resource, manifest_hash::hash_manifest};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_storage::{
    ObjectKey, ObjectScope, ObjectZone, StorageRuntime,
    object_store::{self, ObjectStore, ObjectStoreExt, path::Path},
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
    sea_query::{Condition, Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    ImportTranslationItemInput, ProposalValue, TranslationError, TranslationInterchangeDocument,
    TranslationInterchangeService, TranslationResult,
    entities::{exchange_job, job},
    interchange::INTERCHANGE_SCHEMA_VERSION,
    observability::{self, InterchangeOperation},
    workflow::{actor_kind_value, validate_workflow_actor},
};

pub const MAX_INTERCHANGE_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_INTERCHANGE_ARTIFACT_LIST_LIMIT: u16 = 200;
pub const MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS: u32 = 300;
pub const MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS: u32 = 7 * 24 * 60 * 60;
pub const DEFAULT_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS: u32 = 24 * 60 * 60;
const EXPIRED_ARTIFACTS_PER_OPERATION: u64 = 50;
const PROCESSING_LEASE_SECONDS: i64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationInterchangeDirection {
    Export,
    Import,
}

impl TranslationInterchangeDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Import => "import",
        }
    }

    fn parse(value: &str) -> TranslationResult<Self> {
        match value {
            "export" => Ok(Self::Export),
            "import" => Ok(Self::Import),
            _ => Err(TranslationError::InvalidRequest(
                "translation interchange artifact has an invalid direction".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationInterchangeArtifactStatus {
    Writing,
    Ready,
    Processing,
    Completed,
    Failed,
    Expired,
}

impl TranslationInterchangeArtifactStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Writing => "writing",
            Self::Ready => "ready",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }

    fn parse(value: &str) -> TranslationResult<Self> {
        match value {
            "writing" => Ok(Self::Writing),
            "ready" => Ok(Self::Ready),
            "processing" => Ok(Self::Processing),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "expired" => Ok(Self::Expired),
            _ => Err(TranslationError::InvalidRequest(
                "translation interchange artifact has an invalid status".to_string(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateInterchangeExportArtifactInput {
    pub job_id: Uuid,
    pub max_items: u16,
    pub expires_in_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreInterchangeImportArtifactInput {
    pub job_id: Uuid,
    pub document: TranslationInterchangeDocument,
    pub expires_in_seconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListInterchangeArtifactsInput {
    pub job_id: Option<Uuid>,
    pub include_expired: bool,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadInterchangeArtifactInput {
    pub artifact_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInterchangeImportArtifactInput {
    pub artifact_id: Uuid,
}

struct PersistArtifactInput<T> {
    job_id: Uuid,
    direction: TranslationInterchangeDirection,
    expires_in_seconds: u32,
    request: T,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationInterchangeItemOutcome {
    pub item_id: Uuid,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslationInterchangeConflictReport {
    pub total_items: u16,
    pub accepted_items: u16,
    pub conflict_items: u16,
    pub rejected_items: u16,
    pub outcomes: Vec<TranslationInterchangeItemOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationInterchangeArtifactRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub direction: TranslationInterchangeDirection,
    pub status: TranslationInterchangeArtifactStatus,
    pub content_length: u64,
    pub checksum_sha256: String,
    pub expires_at: DateTime<FixedOffset>,
    pub processed_at: Option<DateTime<FixedOffset>>,
    pub report: Option<TranslationInterchangeConflictReport>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranslationInterchangeArtifactContent {
    pub artifact: TranslationInterchangeArtifactRecord,
    pub document: TranslationInterchangeDocument,
}

enum ImportArtifactClaim {
    Completed(exchange_job::Model),
    Processing {
        model: exchange_job::Model,
        lease_token: Uuid,
    },
}

/// Translation-owned lifecycle for bounded interchange artifacts.
///
/// Database rows carry authorization, expiry, idempotency, and aggregate import
/// evidence. The interchange document itself exists only at a private,
/// tenant-scoped object-storage key.
pub struct TranslationExchangeService {
    database: DatabaseConnection,
    interchange: TranslationInterchangeService,
    storage: StorageRuntime,
}

impl TranslationExchangeService {
    pub fn new(
        database: DatabaseConnection,
        providers: Arc<rustok_translation_targets::TranslationTargetRegistry>,
        tenant_locale_policies: Arc<dyn rustok_tenant::TenantLocalePolicyPort>,
        event_bus: rustok_outbox::TransactionalEventBus,
        storage: StorageRuntime,
    ) -> Self {
        Self {
            interchange: TranslationInterchangeService::new(
                database.clone(),
                providers,
                tenant_locale_policies,
                event_bus,
            ),
            database,
            storage,
        }
    }

    pub async fn create_export_artifact(
        &self,
        context: PortContext,
        input: CreateInterchangeExportArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        observability::observe_interchange_operation(
            InterchangeOperation::ExportCreate,
            self.create_export_artifact_inner(context, input),
        )
        .await
    }

    async fn create_export_artifact_inner(
        &self,
        context: PortContext,
        input: CreateInterchangeExportArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        let tenant_id = authorize(&context, Action::Export, PortCallPolicy::write())?;
        validate_workflow_actor(&context.actor)?;
        validate_expiry(input.expires_in_seconds)?;
        self.expire_due_artifacts(tenant_id).await?;
        let document = self
            .interchange
            .export_job(
                context.clone(),
                crate::ExportTranslationJobInput {
                    job_id: input.job_id,
                    max_items: input.max_items,
                },
            )
            .await?;
        let bytes = serialize_document(&document)?;
        self.persist_artifact(
            context,
            tenant_id,
            PersistArtifactInput {
                job_id: input.job_id,
                direction: TranslationInterchangeDirection::Export,
                expires_in_seconds: input.expires_in_seconds,
                request: input,
                bytes,
            },
        )
        .await
    }

    pub async fn store_import_artifact(
        &self,
        context: PortContext,
        input: StoreInterchangeImportArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        observability::observe_interchange_operation(
            InterchangeOperation::ImportStore,
            self.store_import_artifact_inner(context, input),
        )
        .await
    }

    async fn store_import_artifact_inner(
        &self,
        context: PortContext,
        input: StoreInterchangeImportArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        let tenant_id = authorize(&context, Action::Import, PortCallPolicy::write())?;
        validate_workflow_actor(&context.actor)?;
        validate_expiry(input.expires_in_seconds)?;
        validate_import_document(&input.document, input.job_id)?;
        self.expire_due_artifacts(tenant_id).await?;
        ensure_job(&self.database, tenant_id, input.job_id).await?;
        let bytes = serialize_document(&input.document)?;
        self.persist_artifact(
            context,
            tenant_id,
            PersistArtifactInput {
                job_id: input.job_id,
                direction: TranslationInterchangeDirection::Import,
                expires_in_seconds: input.expires_in_seconds,
                request: input,
                bytes,
            },
        )
        .await
    }

    pub async fn list_artifacts(
        &self,
        context: PortContext,
        input: ListInterchangeArtifactsInput,
    ) -> TranslationResult<Vec<TranslationInterchangeArtifactRecord>> {
        let tenant_id = authorize(&context, Action::Read, PortCallPolicy::read())?;
        if input.limit == 0 || input.limit > MAX_INTERCHANGE_ARTIFACT_LIST_LIMIT {
            return Err(TranslationError::InvalidRequest(format!(
                "translation interchange artifact limit must be between 1 and {MAX_INTERCHANGE_ARTIFACT_LIST_LIMIT}",
            )));
        }
        self.expire_due_artifacts(tenant_id).await?;
        let now = Utc::now().fixed_offset();
        let mut query =
            exchange_job::Entity::find().filter(exchange_job::Column::TenantId.eq(tenant_id));
        if let Some(job_id) = input.job_id {
            query = query.filter(exchange_job::Column::JobId.eq(job_id));
        }
        if !input.include_expired {
            query = query
                .filter(
                    exchange_job::Column::Status
                        .ne(TranslationInterchangeArtifactStatus::Expired.as_str()),
                )
                .filter(exchange_job::Column::ExpiresAt.gt(now));
        }
        query
            .order_by_desc(exchange_job::Column::CreatedAt)
            .order_by_desc(exchange_job::Column::Id)
            .limit(u64::from(input.limit))
            .all(&self.database)
            .await?
            .into_iter()
            .map(artifact_record)
            .collect()
    }

    pub async fn read_artifact(
        &self,
        context: PortContext,
        input: ReadInterchangeArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactContent> {
        let tenant_id = authorize(&context, Action::Read, PortCallPolicy::read())?;
        self.expire_due_artifacts(tenant_id).await?;
        let model = self
            .load_active_artifact(tenant_id, input.artifact_id)
            .await?;
        let artifact = artifact_record(model.clone())?;
        let document = self.load_document(&model).await?;
        Ok(TranslationInterchangeArtifactContent { artifact, document })
    }

    pub async fn process_import_artifact(
        &self,
        context: PortContext,
        input: ProcessInterchangeImportArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        observability::observe_interchange_operation(
            InterchangeOperation::ImportProcess,
            self.process_import_artifact_inner(context, input),
        )
        .await
    }

    async fn process_import_artifact_inner(
        &self,
        context: PortContext,
        input: ProcessInterchangeImportArtifactInput,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        let tenant_id = authorize(&context, Action::Import, PortCallPolicy::write())?;
        validate_workflow_actor(&context.actor)?;
        let idempotency_key = idempotency_key(&context)?;
        let request_hash = hash_manifest(&input)?;
        self.expire_due_artifacts(tenant_id).await?;
        let model = self
            .load_active_artifact(tenant_id, input.artifact_id)
            .await?;
        if TranslationInterchangeDirection::parse(&model.direction)?
            != TranslationInterchangeDirection::Import
        {
            return Err(TranslationError::InvalidRequest(
                "translation interchange artifact is not an import document".to_string(),
            ));
        }
        let claimed = self
            .claim_import_processing(model, &context, &idempotency_key, &request_hash)
            .await?;
        let (claimed, lease_token) = match claimed {
            ImportArtifactClaim::Completed(model) => return artifact_record(model),
            ImportArtifactClaim::Processing { model, lease_token } => (model, lease_token),
        };
        let document = match self.load_document(&claimed).await {
            Ok(document) => document,
            Err(error) => {
                self.mark_import_failed(tenant_id, claimed.id, lease_token)
                    .await?;
                return Err(error);
            }
        };
        if let Err(error) = validate_import_document(&document, claimed.job_id) {
            self.mark_import_failed(tenant_id, claimed.id, lease_token)
                .await?;
            return Err(error);
        }
        let report = self
            .process_import_document(&context, claimed.id, document)
            .await;
        let artifact = self
            .complete_import_processing(tenant_id, claimed.id, lease_token, report)
            .await?;
        if let Some(report) = artifact.report.as_ref() {
            observability::record_interchange_import_report(report);
        }
        Ok(artifact)
    }

    async fn persist_artifact<T: Serialize>(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        input: PersistArtifactInput<T>,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        let idempotency_key = idempotency_key(&context)?;
        let request_hash = hash_manifest(&input.request)?;
        if let Some(existing) =
            find_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            verify_creation_replay(&existing, &context, &request_hash)?;
            return self.finalize_artifact_write(existing, input.bytes).await;
        }
        let id = generate_id();
        let now = Utc::now();
        let expires_at =
            (now + ChronoDuration::seconds(i64::from(input.expires_in_seconds))).fixed_offset();
        let key = ObjectKey::chronological(
            "translation",
            ObjectZone::Objects,
            ObjectScope::Tenant(tenant_id),
            now,
            id,
            "json",
        )
        .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let checksum_sha256 = hex::encode(Sha256::digest(&input.bytes));
        exchange_job::Entity::insert(exchange_job::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            job_id: Set(input.job_id),
            direction: Set(input.direction.as_str().to_string()),
            status: Set(TranslationInterchangeArtifactStatus::Writing
                .as_str()
                .to_string()),
            object_key: Set(key.to_string()),
            content_length: Set(i64::try_from(input.bytes.len()).map_err(|_| {
                TranslationError::InvalidRequest(
                    "translation interchange artifact is too large".to_string(),
                )
            })?),
            checksum_sha256: Set(checksum_sha256),
            created_by_actor_kind: Set(actor_kind_value(&context.actor.kind).to_string()),
            created_by_actor_id: Set(context.actor.id.clone()),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            processing_idempotency_key: Set(None),
            processing_request_hash: Set(None),
            processed_by_actor_kind: Set(None),
            processed_by_actor_id: Set(None),
            processing_lease_token: Set(None),
            processing_lease_expires_at: Set(None),
            processed_at: Set(None),
            report: Set(serde_json::Value::Null),
            expires_at: Set(expires_at),
            storage_deleted_at: Set(None),
            created_at: Set(now.fixed_offset()),
            updated_at: Set(now.fixed_offset()),
        })
        .on_conflict(
            OnConflict::columns([
                exchange_job::Column::TenantId,
                exchange_job::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&self.database)
        .await?;
        let persisted = find_by_idempotency(&self.database, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        verify_creation_replay(&persisted, &context, &request_hash)?;
        self.finalize_artifact_write(persisted, input.bytes).await
    }

    async fn finalize_artifact_write(
        &self,
        model: exchange_job::Model,
        bytes: Vec<u8>,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        let status = TranslationInterchangeArtifactStatus::parse(&model.status)?;
        if status == TranslationInterchangeArtifactStatus::Expired
            || model.expires_at <= Utc::now().fixed_offset()
        {
            expire_interchange_artifact(&self.database, &self.storage, &model).await?;
            return Err(TranslationError::InterchangeArtifactExpired);
        }
        if matches!(
            status,
            TranslationInterchangeArtifactStatus::Ready
                | TranslationInterchangeArtifactStatus::Completed
        ) {
            return artifact_record(model);
        }
        if let Err(error) = self
            .storage
            .objects
            .put_opts(
                &Path::from(model.object_key.as_str()),
                Bytes::from(bytes).into(),
                self.storage.put_options("application/json"),
            )
            .await
        {
            self.mark_artifact_write_failed(model.tenant_id, model.id)
                .await?;
            return Err(storage_error(error));
        }
        let now = Utc::now().fixed_offset();
        let update = exchange_job::Entity::update_many()
            .col_expr(
                exchange_job::Column::Status,
                Expr::value(TranslationInterchangeArtifactStatus::Ready.as_str()),
            )
            .col_expr(exchange_job::Column::UpdatedAt, Expr::value(now))
            .filter(exchange_job::Column::TenantId.eq(model.tenant_id))
            .filter(exchange_job::Column::Id.eq(model.id))
            .filter(exchange_job::Column::Status.eq(model.status.clone()))
            .exec(&self.database)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let persisted = exchange_job::Entity::find_by_id(model.id)
            .filter(exchange_job::Column::TenantId.eq(model.tenant_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::InterchangeArtifactNotFound)?;
        artifact_record(persisted)
    }

    async fn claim_import_processing(
        &self,
        model: exchange_job::Model,
        context: &PortContext,
        idempotency_key: &str,
        request_hash: &str,
    ) -> TranslationResult<ImportArtifactClaim> {
        let status = TranslationInterchangeArtifactStatus::parse(&model.status)?;
        let now = Utc::now().fixed_offset();
        if let Some(existing_key) = model.processing_idempotency_key.as_deref() {
            if model.processing_request_hash.as_deref() != Some(request_hash) {
                return Err(TranslationError::IdempotencyConflict);
            }
            if existing_key != idempotency_key
                || model.processed_by_actor_kind.as_deref()
                    != Some(actor_kind_value(&context.actor.kind))
                || model.processed_by_actor_id.as_deref() != Some(context.actor.id.as_str())
            {
                return Err(TranslationError::IdempotencyActorMismatch);
            }
            if status == TranslationInterchangeArtifactStatus::Completed {
                return Ok(ImportArtifactClaim::Completed(model));
            }
            if status == TranslationInterchangeArtifactStatus::Processing
                && processing_lease_active(&model, now)
            {
                return Err(TranslationError::InterchangeArtifactInProgress);
            }
        } else if status == TranslationInterchangeArtifactStatus::Completed {
            return Err(TranslationError::InterchangeArtifactAlreadyProcessed);
        }
        if !matches!(
            status,
            TranslationInterchangeArtifactStatus::Ready
                | TranslationInterchangeArtifactStatus::Failed
        ) && !(status == TranslationInterchangeArtifactStatus::Processing
            && model.processing_idempotency_key.is_some())
        {
            return Err(TranslationError::InterchangeArtifactNotReady);
        }
        let lease_expires_at = now + ChronoDuration::seconds(PROCESSING_LEASE_SECONDS);
        if model.expires_at <= lease_expires_at {
            return Err(TranslationError::InterchangeArtifactNotReady);
        }
        let lease_token = generate_id();
        let update = exchange_job::Entity::update_many()
            .col_expr(
                exchange_job::Column::Status,
                Expr::value(TranslationInterchangeArtifactStatus::Processing.as_str()),
            )
            .col_expr(
                exchange_job::Column::ProcessingIdempotencyKey,
                Expr::value(idempotency_key.to_string()),
            )
            .col_expr(
                exchange_job::Column::ProcessingRequestHash,
                Expr::value(request_hash.to_string()),
            )
            .col_expr(
                exchange_job::Column::ProcessedByActorKind,
                Expr::value(actor_kind_value(&context.actor.kind)),
            )
            .col_expr(
                exchange_job::Column::ProcessedByActorId,
                Expr::value(context.actor.id.clone()),
            )
            .col_expr(
                exchange_job::Column::ProcessingLeaseToken,
                Expr::value(Some(lease_token)),
            )
            .col_expr(
                exchange_job::Column::ProcessingLeaseExpiresAt,
                Expr::value(Some(lease_expires_at)),
            )
            .col_expr(exchange_job::Column::UpdatedAt, Expr::value(now))
            .filter(exchange_job::Column::TenantId.eq(model.tenant_id))
            .filter(exchange_job::Column::Id.eq(model.id))
            .filter(exchange_job::Column::Status.eq(model.status))
            .filter(
                Condition::any()
                    .add(exchange_job::Column::ProcessingLeaseExpiresAt.is_null())
                    .add(exchange_job::Column::ProcessingLeaseExpiresAt.lte(now)),
            )
            .exec(&self.database)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::InterchangeArtifactInProgress);
        }
        let claimed = exchange_job::Entity::find_by_id(model.id)
            .filter(exchange_job::Column::TenantId.eq(model.tenant_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::InterchangeArtifactNotFound)?;
        Ok(ImportArtifactClaim::Processing {
            model: claimed,
            lease_token,
        })
    }

    async fn complete_import_processing(
        &self,
        tenant_id: Uuid,
        artifact_id: Uuid,
        lease_token: Uuid,
        report: TranslationInterchangeConflictReport,
    ) -> TranslationResult<TranslationInterchangeArtifactRecord> {
        let now = Utc::now().fixed_offset();
        let report_json = serde_json::to_value(report)?;
        let update = exchange_job::Entity::update_many()
            .col_expr(
                exchange_job::Column::Status,
                Expr::value(TranslationInterchangeArtifactStatus::Completed.as_str()),
            )
            .col_expr(exchange_job::Column::Report, Expr::value(report_json))
            .col_expr(
                exchange_job::Column::ProcessingLeaseToken,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                exchange_job::Column::ProcessingLeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(exchange_job::Column::ProcessedAt, Expr::value(Some(now)))
            .col_expr(exchange_job::Column::UpdatedAt, Expr::value(now))
            .filter(exchange_job::Column::TenantId.eq(tenant_id))
            .filter(exchange_job::Column::Id.eq(artifact_id))
            .filter(
                exchange_job::Column::Status
                    .eq(TranslationInterchangeArtifactStatus::Processing.as_str()),
            )
            .filter(exchange_job::Column::ProcessingLeaseToken.eq(lease_token))
            .filter(exchange_job::Column::ExpiresAt.gt(now))
            .exec(&self.database)
            .await?;
        if update.rows_affected != 1 {
            let current = exchange_job::Entity::find_by_id(artifact_id)
                .filter(exchange_job::Column::TenantId.eq(tenant_id))
                .one(&self.database)
                .await?
                .ok_or(TranslationError::InterchangeArtifactNotFound)?;
            if current.expires_at <= now {
                expire_interchange_artifact(&self.database, &self.storage, &current).await?;
                return Err(TranslationError::InterchangeArtifactExpired);
            }
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let completed = exchange_job::Entity::find_by_id(artifact_id)
            .filter(exchange_job::Column::TenantId.eq(tenant_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::InterchangeArtifactNotFound)?;
        artifact_record(completed)
    }

    async fn process_import_document(
        &self,
        context: &PortContext,
        artifact_id: Uuid,
        document: TranslationInterchangeDocument,
    ) -> TranslationInterchangeConflictReport {
        let mut report = TranslationInterchangeConflictReport {
            total_items: u16::try_from(document.items.len()).unwrap_or(u16::MAX),
            accepted_items: 0,
            conflict_items: 0,
            rejected_items: 0,
            outcomes: Vec::with_capacity(document.items.len()),
        };
        for item in document.items {
            let item_id = item.item_id;
            let values = item
                .fields
                .into_iter()
                .filter_map(|field| {
                    field.proposed_value.map(|value| ProposalValue {
                        key: field.key,
                        value,
                    })
                })
                .collect::<Vec<_>>();
            if values.is_empty() {
                report.rejected_items += 1;
                report.outcomes.push(TranslationInterchangeItemOutcome {
                    item_id,
                    status: "missing_proposed_values".to_string(),
                });
                continue;
            }
            let child_context = match child_context(context, artifact_id, item_id) {
                Ok(context) => context,
                Err(_) => {
                    report.rejected_items += 1;
                    report.outcomes.push(TranslationInterchangeItemOutcome {
                        item_id,
                        status: "import_rejected".to_string(),
                    });
                    continue;
                }
            };
            match self
                .interchange
                .import_item(
                    child_context,
                    ImportTranslationItemInput {
                        schema_version: document.schema_version,
                        job_id: document.job_id,
                        item_id,
                        identity: item.identity,
                        source_digest: item.source_digest,
                        values,
                    },
                )
                .await
            {
                Ok(_) => {
                    report.accepted_items += 1;
                    report.outcomes.push(TranslationInterchangeItemOutcome {
                        item_id,
                        status: "imported".to_string(),
                    });
                }
                Err(TranslationError::WorkflowRevisionConflict) => {
                    report.conflict_items += 1;
                    report.outcomes.push(TranslationInterchangeItemOutcome {
                        item_id,
                        status: "source_conflict".to_string(),
                    });
                }
                Err(TranslationError::Provider {
                    retryable: true, ..
                }) => {
                    report.rejected_items += 1;
                    report.outcomes.push(TranslationInterchangeItemOutcome {
                        item_id,
                        status: "temporarily_unavailable".to_string(),
                    });
                }
                Err(_) => {
                    report.rejected_items += 1;
                    report.outcomes.push(TranslationInterchangeItemOutcome {
                        item_id,
                        status: "rejected".to_string(),
                    });
                }
            }
        }
        report
    }

    async fn load_active_artifact(
        &self,
        tenant_id: Uuid,
        artifact_id: Uuid,
    ) -> TranslationResult<exchange_job::Model> {
        let model = exchange_job::Entity::find_by_id(artifact_id)
            .filter(exchange_job::Column::TenantId.eq(tenant_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::InterchangeArtifactNotFound)?;
        if TranslationInterchangeArtifactStatus::parse(&model.status)?
            == TranslationInterchangeArtifactStatus::Expired
            || model.expires_at <= Utc::now().fixed_offset()
        {
            expire_interchange_artifact(&self.database, &self.storage, &model).await?;
            return Err(TranslationError::InterchangeArtifactExpired);
        }
        Ok(model)
    }

    async fn load_document(
        &self,
        model: &exchange_job::Model,
    ) -> TranslationResult<TranslationInterchangeDocument> {
        let path = Path::from(model.object_key.as_str());
        let metadata = self
            .storage
            .objects
            .head(&path)
            .await
            .map_err(storage_error)?;
        if metadata.size > MAX_INTERCHANGE_ARTIFACT_BYTES as u64
            || i64::try_from(metadata.size).ok() != Some(model.content_length)
        {
            return Err(TranslationError::Provider {
                code: "translation.interchange.artifact_integrity".to_string(),
                message: "translation interchange artifact integrity validation failed".to_string(),
                retryable: false,
            });
        }
        let bytes = self
            .storage
            .objects
            .get(&path)
            .await
            .map_err(storage_error)?
            .bytes()
            .await
            .map_err(storage_error)?;
        if bytes.len() > MAX_INTERCHANGE_ARTIFACT_BYTES
            || hex::encode(Sha256::digest(&bytes)) != model.checksum_sha256
        {
            return Err(TranslationError::Provider {
                code: "translation.interchange.artifact_integrity".to_string(),
                message: "translation interchange artifact integrity validation failed".to_string(),
                retryable: false,
            });
        }
        serde_json::from_slice(&bytes).map_err(TranslationError::from)
    }

    async fn mark_import_failed(
        &self,
        tenant_id: Uuid,
        artifact_id: Uuid,
        lease_token: Uuid,
    ) -> TranslationResult<()> {
        exchange_job::Entity::update_many()
            .col_expr(
                exchange_job::Column::Status,
                Expr::value(TranslationInterchangeArtifactStatus::Failed.as_str()),
            )
            .col_expr(
                exchange_job::Column::ProcessingLeaseToken,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                exchange_job::Column::ProcessingLeaseExpiresAt,
                Expr::value(Option::<DateTime<FixedOffset>>::None),
            )
            .col_expr(
                exchange_job::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(exchange_job::Column::TenantId.eq(tenant_id))
            .filter(exchange_job::Column::Id.eq(artifact_id))
            .filter(
                exchange_job::Column::Status
                    .eq(TranslationInterchangeArtifactStatus::Processing.as_str()),
            )
            .filter(exchange_job::Column::ProcessingLeaseToken.eq(lease_token))
            .exec(&self.database)
            .await?;
        Ok(())
    }

    async fn mark_artifact_write_failed(
        &self,
        tenant_id: Uuid,
        artifact_id: Uuid,
    ) -> TranslationResult<()> {
        exchange_job::Entity::update_many()
            .col_expr(
                exchange_job::Column::Status,
                Expr::value(TranslationInterchangeArtifactStatus::Failed.as_str()),
            )
            .col_expr(
                exchange_job::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(exchange_job::Column::TenantId.eq(tenant_id))
            .filter(exchange_job::Column::Id.eq(artifact_id))
            .filter(
                exchange_job::Column::Status
                    .eq(TranslationInterchangeArtifactStatus::Writing.as_str()),
            )
            .exec(&self.database)
            .await?;
        Ok(())
    }

    async fn expire_due_artifacts(&self, tenant_id: Uuid) -> TranslationResult<()> {
        let due = due_interchange_artifacts(
            &self.database,
            Some(tenant_id),
            EXPIRED_ARTIFACTS_PER_OPERATION,
        )
        .await?;
        for model in due {
            expire_interchange_artifact(&self.database, &self.storage, &model).await?;
        }
        Ok(())
    }
}

#[cfg(feature = "runtime")]
pub(crate) async fn next_expired_interchange_artifact(
    database: &DatabaseConnection,
) -> TranslationResult<Option<exchange_job::Model>> {
    Ok(due_interchange_artifacts(database, None, 1)
        .await?
        .into_iter()
        .next())
}

pub(crate) async fn expire_interchange_artifact(
    database: &DatabaseConnection,
    storage: &StorageRuntime,
    model: &exchange_job::Model,
) -> TranslationResult<()> {
    let now = Utc::now().fixed_offset();
    exchange_job::Entity::update_many()
        .col_expr(
            exchange_job::Column::Status,
            Expr::value(TranslationInterchangeArtifactStatus::Expired.as_str()),
        )
        .col_expr(
            exchange_job::Column::ProcessingLeaseToken,
            Expr::value(Option::<Uuid>::None),
        )
        .col_expr(
            exchange_job::Column::ProcessingLeaseExpiresAt,
            Expr::value(Option::<DateTime<FixedOffset>>::None),
        )
        .col_expr(exchange_job::Column::UpdatedAt, Expr::value(now))
        .filter(exchange_job::Column::TenantId.eq(model.tenant_id))
        .filter(exchange_job::Column::Id.eq(model.id))
        .exec(database)
        .await?;
    match storage
        .objects
        .delete(&Path::from(model.object_key.as_str()))
        .await
    {
        Ok(()) | Err(object_store::Error::NotFound { .. }) => {
            exchange_job::Entity::update_many()
                .col_expr(
                    exchange_job::Column::StorageDeletedAt,
                    Expr::value(Some(now)),
                )
                .col_expr(exchange_job::Column::UpdatedAt, Expr::value(now))
                .filter(exchange_job::Column::TenantId.eq(model.tenant_id))
                .filter(exchange_job::Column::Id.eq(model.id))
                .exec(database)
                .await?;
            observability::record_interchange_expiry_cleanup(true);
        }
        Err(_) => observability::record_interchange_expiry_cleanup(false),
    }
    Ok(())
}

async fn due_interchange_artifacts(
    database: &DatabaseConnection,
    tenant_id: Option<Uuid>,
    limit: u64,
) -> TranslationResult<Vec<exchange_job::Model>> {
    let now = Utc::now().fixed_offset();
    let mut query = exchange_job::Entity::find()
        .filter(exchange_job::Column::ExpiresAt.lte(now))
        .filter(exchange_job::Column::StorageDeletedAt.is_null());
    if let Some(tenant_id) = tenant_id {
        query = query.filter(exchange_job::Column::TenantId.eq(tenant_id));
    }
    Ok(query
        .order_by_asc(exchange_job::Column::ExpiresAt)
        .order_by_asc(exchange_job::Column::Id)
        .limit(limit)
        .all(database)
        .await?)
}

fn serialize_document(document: &TranslationInterchangeDocument) -> TranslationResult<Vec<u8>> {
    let bytes = serde_json::to_vec(document)?;
    if bytes.len() > MAX_INTERCHANGE_ARTIFACT_BYTES {
        return Err(TranslationError::InvalidRequest(format!(
            "translation interchange artifact exceeds the {MAX_INTERCHANGE_ARTIFACT_BYTES}-byte bound",
        )));
    }
    Ok(bytes)
}

/// Parses a bounded JSON document supplied for a private interchange artifact.
///
/// Transport adapters use this parser before constructing the domain input so
/// malformed client payloads reach the shared public-error classifier without
/// exposing parser details.
pub fn parse_artifact_document(value: &str) -> TranslationResult<TranslationInterchangeDocument> {
    if value.len() > MAX_INTERCHANGE_ARTIFACT_BYTES {
        return Err(TranslationError::InvalidRequest(format!(
            "translation interchange artifact exceeds the {MAX_INTERCHANGE_ARTIFACT_BYTES}-byte bound",
        )));
    }
    serde_json::from_str(value).map_err(|_| {
        TranslationError::InvalidRequest(
            "translation interchange artifact document is invalid".to_string(),
        )
    })
}

fn validate_import_document(
    document: &TranslationInterchangeDocument,
    job_id: Uuid,
) -> TranslationResult<()> {
    if document.schema_version != INTERCHANGE_SCHEMA_VERSION || document.job_id != job_id {
        return Err(TranslationError::InvalidRequest(
            "translation interchange import document does not match its artifact job".to_string(),
        ));
    }
    if document.items.is_empty()
        || document.items.len() > usize::from(MAX_INTERCHANGE_ARTIFACT_LIST_LIMIT)
    {
        return Err(TranslationError::InvalidRequest(format!(
            "translation interchange import document must contain between 1 and {MAX_INTERCHANGE_ARTIFACT_LIST_LIMIT} items",
        )));
    }
    Ok(())
}

fn validate_expiry(expires_in_seconds: u32) -> TranslationResult<()> {
    if !(MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS..=MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS)
        .contains(&expires_in_seconds)
    {
        return Err(TranslationError::InvalidRequest(format!(
            "translation interchange artifact expiry must be between {MIN_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS} and {MAX_INTERCHANGE_ARTIFACT_EXPIRY_SECONDS} seconds",
        )));
    }
    Ok(())
}

fn processing_lease_active(model: &exchange_job::Model, now: DateTime<FixedOffset>) -> bool {
    model.processing_lease_token.is_some()
        && model
            .processing_lease_expires_at
            .is_some_and(|expires_at| expires_at > now)
}

fn authorize(
    context: &PortContext,
    action: Action,
    policy: PortCallPolicy,
) -> TranslationResult<Uuid> {
    context.require_policy(policy)?;
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Translations, action) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn idempotency_key(context: &PortContext) -> TranslationResult<String> {
    let key = context.idempotency_key.as_deref().unwrap_or_default();
    if key.is_empty() || key.len() > 191 || key.trim() != key {
        return Err(TranslationError::InvalidRequest(
            "translation interchange artifact idempotency key is invalid".to_string(),
        ));
    }
    Ok(key.to_string())
}

fn child_context(
    context: &PortContext,
    artifact_id: Uuid,
    item_id: Uuid,
) -> TranslationResult<PortContext> {
    let digest = hash_manifest(&(artifact_id, item_id))?;
    let mut child = context.clone();
    child.causation_id = Some(context.correlation_id.clone());
    child.idempotency_key = Some(format!("translation-interchange-item:{digest}"));
    Ok(child)
}

async fn ensure_job(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TranslationResult<job::Model> {
    job::Entity::find_by_id(job_id)
        .filter(job::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobNotFound)
}

async fn find_by_idempotency(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<exchange_job::Model>> {
    Ok(exchange_job::Entity::find()
        .filter(exchange_job::Column::TenantId.eq(tenant_id))
        .filter(exchange_job::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

fn verify_creation_replay(
    model: &exchange_job::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<()> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.created_by_actor_kind != actor_kind_value(&context.actor.kind)
        || model.created_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

fn artifact_record(
    model: exchange_job::Model,
) -> TranslationResult<TranslationInterchangeArtifactRecord> {
    let report = if model.report.is_null() {
        None
    } else {
        Some(serde_json::from_value(model.report)?)
    };
    Ok(TranslationInterchangeArtifactRecord {
        id: model.id,
        job_id: model.job_id,
        direction: TranslationInterchangeDirection::parse(&model.direction)?,
        status: TranslationInterchangeArtifactStatus::parse(&model.status)?,
        content_length: u64::try_from(model.content_length).map_err(|_| {
            TranslationError::InvalidRequest(
                "translation interchange artifact has an invalid byte length".to_string(),
            )
        })?,
        checksum_sha256: model.checksum_sha256,
        expires_at: model.expires_at,
        processed_at: model.processed_at,
        report,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn storage_error(_error: object_store::Error) -> TranslationError {
    TranslationError::Provider {
        code: "translation.interchange.storage_unavailable".to_string(),
        message: "translation interchange storage is temporarily unavailable".to_string(),
        retryable: true,
    }
}
