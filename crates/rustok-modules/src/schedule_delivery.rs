//! Durable owner-owned delivery state for artifact Schedule bindings.
//!
//! A clock/materializer computes due slots and calls `enqueue`; this module
//! owns the immutable slot queue and adapts it to the platform scheduler. It
//! never creates a module-local timer or executes a mutable catalog release.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{
    HostRuntimeContext, ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome,
    ModuleWorkSource,
};
use rustok_runtime::{ModuleWorkRegistration, ModuleWorkScheduler};
use rustok_sandbox::ExecutionPhase;

use crate::{
    data::{configure_tenant_scope, now_expression, placeholder, uuid_from_row, uuid_value},
    schedule_binding_digest, ArtifactBindingDispatch, ArtifactInstallationTarget,
    ArtifactReleaseRef, ArtifactScheduleMaterializationConfig, ArtifactScheduleMaterializer,
    ControlPlaneInfrastructure, ModuleArtifactDescriptor, ModuleRuntimeBinding,
    ModuleRuntimeBindingKind, SharedArtifactBindingExecutor, SharedArtifactDeliveryTenantSource,
};

const MAX_BINDING_ID_BYTES: usize = 128;
const MAX_WORKER_ID_BYTES: usize = 128;
const MAX_ERROR_CODE_BYTES: usize = 96;

/// Stable generic-scheduler identity for all artifact Schedule slot work.
pub const ARTIFACT_SCHEDULE_DELIVERY_WORKER: &str = "artifact_schedule_delivery";

/// One immutable UTC slot supplied by the owner clock/materializer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactScheduleDeliveryRequest {
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub binding_id: String,
    pub scheduled_for: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactScheduleDeliveryReceipt {
    pub delivery_id: Uuid,
    pub created: bool,
}

/// The persisted descriptor is revalidated after claim. This item never
/// contains a catalog identity or a mutable effective-release lookup.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactScheduleDeliveryWorkItem {
    pub delivery_id: Uuid,
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub attempt: u32,
    pub scheduled_for: DateTime<Utc>,
    pub schedule_digest: String,
    pub release: ArtifactReleaseRef,
    pub binding: ModuleRuntimeBinding,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactScheduleDeliveryOutcome {
    Succeeded,
    Retryable { error_code: String },
    DeadLetter { error_code: String },
    Cancelled { error_code: String },
}

/// Queue-owned execution limits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactScheduleDeliveryConfig {
    pub max_attempts: u32,
    pub lease_seconds: u32,
    pub retry_base_seconds: u32,
    pub retry_max_seconds: u32,
}

impl Default for ArtifactScheduleDeliveryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            lease_seconds: 60,
            retry_base_seconds: 5,
            retry_max_seconds: 300,
        }
    }
}

impl ArtifactScheduleDeliveryConfig {
    fn validate(&self) -> Result<(), ArtifactScheduleDeliveryError> {
        if self.max_attempts == 0
            || self.max_attempts > 100
            || self.lease_seconds == 0
            || self.lease_seconds > 3_600
            || self.retry_base_seconds == 0
            || self.retry_max_seconds < self.retry_base_seconds
            || self.retry_max_seconds > 86_400
        {
            return Err(ArtifactScheduleDeliveryError::InvalidConfiguration);
        }
        Ok(())
    }

    fn retry_delay_seconds(&self, attempt: u32) -> u32 {
        self.retry_base_seconds
            .saturating_mul(2_u32.pow(attempt.saturating_sub(1).min(16)))
            .min(self.retry_max_seconds)
    }
}

#[derive(Clone)]
pub struct SeaOrmArtifactScheduleDeliveryQueue {
    db: DatabaseConnection,
    config: ArtifactScheduleDeliveryConfig,
    infrastructure: ControlPlaneInfrastructure,
}

impl SeaOrmArtifactScheduleDeliveryQueue {
    pub fn new(
        db: DatabaseConnection,
        config: ArtifactScheduleDeliveryConfig,
    ) -> Result<Self, ArtifactScheduleDeliveryError> {
        Self::with_infrastructure(db, config, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        config: ArtifactScheduleDeliveryConfig,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Result<Self, ArtifactScheduleDeliveryError> {
        config.validate()?;
        Ok(Self {
            db,
            config,
            infrastructure,
        })
    }

    pub fn config(&self) -> &ArtifactScheduleDeliveryConfig {
        &self.config
    }

    /// Creates one idempotent slot record. The queue derives and stores the
    /// Schedule binding digest in the same tenant-scoped transaction.
    pub async fn enqueue(
        &self,
        request: ArtifactScheduleDeliveryRequest,
    ) -> Result<ArtifactScheduleDeliveryReceipt, ArtifactScheduleDeliveryError> {
        validate_request(&request)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let descriptor = load_admitted_descriptor(
            &transaction,
            backend,
            request.tenant_id,
            request.installation_id,
        )
        .await?;
        let schedule_digest = schedule_digest_for_binding(&descriptor, &request.binding_id)?;
        let delivery_id = self.infrastructure.new_id();
        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_schedule_deliveries \
                     (delivery_id, tenant_id, installation_id, binding_id, schedule_digest, scheduled_for, \
                      attempt, status, available_at, created_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, 0, 'pending', {}, {}) \
                     ON CONFLICT (tenant_id, installation_id, binding_id, scheduled_for) DO NOTHING",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    now_expression(backend),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(delivery_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.clone().into(),
                    schedule_digest.clone().into(),
                    datetime_value(request.scheduled_for, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if inserted.rows_affected() == 1 {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactScheduleDeliveryReceipt {
                delivery_id,
                created: true,
            });
        }

        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT delivery_id, schedule_digest FROM module_artifact_schedule_deliveries \
                     WHERE tenant_id = {} AND installation_id = {} AND binding_id = {} \
                       AND scheduled_for = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.into(),
                    datetime_value(request.scheduled_for, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ArtifactScheduleDeliveryError::Storage("slot conflict was not readable".to_string())
            })?;
        let existing_digest: String = existing
            .try_get("", "schedule_digest")
            .map_err(storage_error)?;
        if existing_digest != schedule_digest {
            return Err(ArtifactScheduleDeliveryError::ScheduleChanged);
        }
        let delivery_id =
            uuid_from_row(&existing, "delivery_id", backend).map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactScheduleDeliveryReceipt {
            delivery_id,
            created: false,
        })
    }

    /// Reclaims abandoned leases then claims one due slot for one tenant. A
    /// disabled, uninstalled, or changed slot is cancelled before dispatch.
    pub async fn claim_next(
        &self,
        tenant_id: Uuid,
        worker_id: &str,
    ) -> Result<Option<ArtifactScheduleDeliveryWorkItem>, ArtifactScheduleDeliveryError> {
        validate_worker_id(worker_id)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        expire_claims(&transaction, backend, tenant_id, self.config.max_attempts).await?;
        let lock = if backend == DbBackend::Postgres {
            " FOR UPDATE SKIP LOCKED"
        } else {
            ""
        };
        let candidate = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT delivery_id, installation_id, binding_id, schedule_digest, scheduled_for \
                     FROM module_artifact_schedule_deliveries \
                     WHERE tenant_id = {} AND status = 'pending' AND available_at <= {} \
                     ORDER BY available_at, delivery_id LIMIT 1{lock}",
                    placeholder(backend, 1),
                    now_expression(backend),
                ),
                vec![uuid_value(tenant_id, backend)],
            ))
            .await
            .map_err(storage_error)?;
        let Some(candidate) = candidate else {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(None);
        };
        let delivery_id =
            uuid_from_row(&candidate, "delivery_id", backend).map_err(storage_error)?;
        let installation_id =
            uuid_from_row(&candidate, "installation_id", backend).map_err(storage_error)?;
        let binding_id: String = candidate.try_get("", "binding_id").map_err(storage_error)?;
        let persisted_digest: String = candidate
            .try_get("", "schedule_digest")
            .map_err(storage_error)?;
        let scheduled_for = datetime_from_row(&candidate, "scheduled_for", backend)?;
        let descriptor =
            match load_admitted_descriptor(&transaction, backend, tenant_id, installation_id).await
            {
                Ok(descriptor) => descriptor,
                Err(ArtifactScheduleDeliveryError::InstallationUnavailable) => {
                    cancel_unavailable(&transaction, backend, tenant_id, delivery_id).await?;
                    transaction.commit().await.map_err(storage_error)?;
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
        let schedule_digest = match schedule_digest_for_binding(&descriptor, &binding_id) {
            Ok(digest) => digest,
            Err(ArtifactScheduleDeliveryError::BindingUnavailable) => {
                cancel_unavailable(&transaction, backend, tenant_id, delivery_id).await?;
                transaction.commit().await.map_err(storage_error)?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        if schedule_digest != persisted_digest {
            cancel_unavailable(&transaction, backend, tenant_id, delivery_id).await?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(None);
        }
        let claimed = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_schedule_deliveries \
                     SET status = 'running', attempt = attempt + 1, claimed_by = {}, \
                         claimed_until = {}, last_error_code = NULL \
                     WHERE delivery_id = {} AND tenant_id = {} AND status = 'pending' \
                       AND available_at <= {}",
                    placeholder(backend, 1),
                    lease_expression(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    now_expression(backend),
                ),
                vec![
                    worker_id.to_owned().into(),
                    i64::from(self.config.lease_seconds).into(),
                    uuid_value(delivery_id, backend),
                    uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if claimed.rows_affected() != 1 {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(None);
        }
        let attempt = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT attempt FROM module_artifact_schedule_deliveries \
                     WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' AND claimed_by = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(delivery_id, backend),
                    uuid_value(tenant_id, backend),
                    worker_id.to_owned().into(),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactScheduleDeliveryError::ClaimLost)
            .and_then(|row| attempt_from_row(&row, backend))?;
        let release = descriptor.release_ref();
        let binding = descriptor
            .bindings
            .into_iter()
            .find(|binding| {
                binding.id == binding_id && binding.kind == ModuleRuntimeBindingKind::Schedule
            })
            .ok_or(ArtifactScheduleDeliveryError::BindingUnavailable)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(Some(ArtifactScheduleDeliveryWorkItem {
            delivery_id,
            tenant_id,
            installation_id,
            attempt,
            scheduled_for,
            schedule_digest,
            release,
            binding,
        }))
    }

    pub async fn complete(
        &self,
        tenant_id: Uuid,
        worker_id: &str,
        delivery_id: Uuid,
        attempt: u32,
        outcome: ArtifactScheduleDeliveryOutcome,
    ) -> Result<(), ArtifactScheduleDeliveryError> {
        validate_worker_id(worker_id)?;
        if delivery_id.is_nil() || attempt == 0 {
            return Err(ArtifactScheduleDeliveryError::InvalidCompletion);
        }
        let (status, error_code, terminal_column, retryable) = match outcome {
            ArtifactScheduleDeliveryOutcome::Succeeded => {
                ("succeeded", None, Some("completed_at"), false)
            }
            ArtifactScheduleDeliveryOutcome::Cancelled { error_code } => {
                validate_error_code(&error_code)?;
                ("cancelled", Some(error_code), Some("cancelled_at"), false)
            }
            ArtifactScheduleDeliveryOutcome::DeadLetter { error_code } => {
                validate_error_code(&error_code)?;
                (
                    "dead_letter",
                    Some(error_code),
                    Some("dead_lettered_at"),
                    false,
                )
            }
            ArtifactScheduleDeliveryOutcome::Retryable { error_code } => {
                validate_error_code(&error_code)?;
                ("pending", Some(error_code), None, true)
            }
        };
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let affected = if retryable && attempt < self.config.max_attempts {
            complete_retryable(
                &transaction,
                backend,
                tenant_id,
                worker_id,
                delivery_id,
                attempt,
                error_code
                    .as_deref()
                    .expect("retryable outcome has an error code"),
                self.config.retry_delay_seconds(attempt),
            )
            .await?
        } else {
            let status = if retryable { "dead_letter" } else { status };
            let terminal_column = if retryable {
                "dead_lettered_at"
            } else {
                terminal_column.expect("terminal outcome has a timestamp")
            };
            complete_terminal(
                &transaction,
                backend,
                tenant_id,
                worker_id,
                delivery_id,
                attempt,
                status,
                error_code.as_deref(),
                terminal_column,
            )
            .await?
        };
        if affected != 1 {
            return Err(ArtifactScheduleDeliveryError::ClaimLost);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }
}

/// The adapter delegates scheduling and shutdown to `ModuleWorkScheduler`; it
/// has no competing `tokio::spawn` loop or module-local timer.
#[derive(Clone)]
pub struct ArtifactScheduleDeliveryWorkAdapter {
    queue: SeaOrmArtifactScheduleDeliveryQueue,
    materializer: ArtifactScheduleMaterializer,
    executor: SharedArtifactBindingExecutor,
    tenants: SharedArtifactDeliveryTenantSource,
    worker_id: String,
}

impl ArtifactScheduleDeliveryWorkAdapter {
    pub fn new(
        queue: SeaOrmArtifactScheduleDeliveryQueue,
        executor: SharedArtifactBindingExecutor,
        tenants: SharedArtifactDeliveryTenantSource,
        worker_id: String,
    ) -> Result<Self, ArtifactScheduleDeliveryError> {
        let materializer = ArtifactScheduleMaterializer::new(
            queue.db.clone(),
            queue.clone(),
            ArtifactScheduleMaterializationConfig::default(),
        )
        .map_err(|error| ArtifactScheduleDeliveryError::Storage(error.to_string()))?;
        Self::with_materializer(queue, materializer, executor, tenants, worker_id)
    }

    pub fn with_materializer(
        queue: SeaOrmArtifactScheduleDeliveryQueue,
        materializer: ArtifactScheduleMaterializer,
        executor: SharedArtifactBindingExecutor,
        tenants: SharedArtifactDeliveryTenantSource,
        worker_id: String,
    ) -> Result<Self, ArtifactScheduleDeliveryError> {
        validate_worker_id(&worker_id)?;
        Ok(Self {
            queue,
            materializer,
            executor,
            tenants,
            worker_id,
        })
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    fn payload(item: &ModuleWorkItem) -> Result<ArtifactScheduleDeliveryWorkItem, ModuleWorkError> {
        serde_json::from_value(item.payload.clone()).map_err(|_| {
            ModuleWorkError::Source("artifact schedule work item payload is invalid".to_string())
        })
    }
}

/// Registration requires explicit host handles and fails closed if either the
/// executor or tenant enumerator has not been composed.
#[derive(Clone, Debug)]
pub struct ArtifactScheduleDeliveryWorkRegistration {
    pub config: ArtifactScheduleDeliveryConfig,
    pub materialization: ArtifactScheduleMaterializationConfig,
    pub worker_id: String,
}

impl Default for ArtifactScheduleDeliveryWorkRegistration {
    fn default() -> Self {
        Self {
            config: ArtifactScheduleDeliveryConfig::default(),
            materialization: ArtifactScheduleMaterializationConfig::default(),
            worker_id: ARTIFACT_SCHEDULE_DELIVERY_WORKER.to_string(),
        }
    }
}

#[async_trait]
impl ModuleWorkRegistration for ArtifactScheduleDeliveryWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let executor = host
            .shared_get::<SharedArtifactBindingExecutor>()
            .ok_or_else(|| {
                ModuleWorkError::Handler("artifact schedule executor handle is missing".to_string())
            })?;
        let tenants = host
            .shared_get::<SharedArtifactDeliveryTenantSource>()
            .ok_or_else(|| {
                ModuleWorkError::Handler(
                    "artifact schedule tenant source handle is missing".to_string(),
                )
            })?;
        let queue = SeaOrmArtifactScheduleDeliveryQueue::new(host.db_clone(), self.config.clone())
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        let materializer = ArtifactScheduleMaterializer::new(
            host.db_clone(),
            queue.clone(),
            self.materialization.clone(),
        )
        .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        ArtifactScheduleDeliveryWorkAdapter::with_materializer(
            queue,
            materializer,
            executor,
            tenants,
            self.worker_id.clone(),
        )
        .map_err(|error| ModuleWorkError::Handler(error.to_string()))?
        .register_with(scheduler)
        .await
    }
}

#[async_trait]
impl ModuleWorkSource for ArtifactScheduleDeliveryWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != ARTIFACT_SCHEDULE_DELIVERY_WORKER {
            return Ok(None);
        }
        let tenant_ids = self
            .tenants
            .tenant_ids()
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        for tenant_id in tenant_ids {
            if tenant_id.is_nil() {
                return Err(ModuleWorkError::Source(
                    "artifact schedule tenant source returned a nil tenant id".to_string(),
                ));
            }
            self.materializer
                .materialize_tenant(tenant_id, self.queue.infrastructure.now())
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
            if let Some(item) = self
                .queue
                .claim_next(tenant_id, &self.worker_id)
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?
            {
                return Ok(Some(ModuleWorkItem {
                    id: item.delivery_id,
                    tenant_id: item.tenant_id,
                    worker_slug: ARTIFACT_SCHEDULE_DELIVERY_WORKER.to_string(),
                    lease_token: self.queue.infrastructure.new_id().to_string(),
                    payload: serde_json::to_value(item).expect("schedule work item must serialize"),
                }));
            }
        }
        Ok(None)
    }

    async fn complete(
        &self,
        item: &ModuleWorkItem,
        outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        if item.worker_slug != ARTIFACT_SCHEDULE_DELIVERY_WORKER {
            return Err(ModuleWorkError::Source(
                "artifact schedule completion has the wrong worker slug".to_string(),
            ));
        }
        let payload = Self::payload(item)?;
        if payload.delivery_id != item.id || payload.tenant_id != item.tenant_id {
            return Err(ModuleWorkError::Source(
                "artifact schedule completion identity does not match its payload".to_string(),
            ));
        }
        let outcome = match outcome {
            ModuleWorkOutcome::Completed => ArtifactScheduleDeliveryOutcome::Succeeded,
            ModuleWorkOutcome::Retryable { .. } => ArtifactScheduleDeliveryOutcome::Retryable {
                error_code: "execution_failed".to_string(),
            },
            ModuleWorkOutcome::Rejected { .. } => ArtifactScheduleDeliveryOutcome::DeadLetter {
                error_code: "execution_rejected".to_string(),
            },
            ModuleWorkOutcome::Cancelled => ArtifactScheduleDeliveryOutcome::Cancelled {
                error_code: "execution_cancelled".to_string(),
            },
        };
        self.queue
            .complete(
                payload.tenant_id,
                &self.worker_id,
                payload.delivery_id,
                payload.attempt,
                outcome,
            )
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }
}

#[async_trait]
impl ModuleWorkHandler for ArtifactScheduleDeliveryWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        ARTIFACT_SCHEDULE_DELIVERY_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        if item.worker_slug != ARTIFACT_SCHEDULE_DELIVERY_WORKER {
            return Err(ModuleWorkError::Handler(
                "artifact schedule execution has the wrong worker slug".to_string(),
            ));
        }
        let payload = Self::payload(&item)?;
        if payload.delivery_id != item.id || payload.tenant_id != item.tenant_id {
            return Err(ModuleWorkError::Handler(
                "artifact schedule execution identity does not match its payload".to_string(),
            ));
        }
        match self
            .executor
            .dispatch_binding(ArtifactBindingDispatch {
                release: &payload.release,
                binding: &payload.binding,
                target: ArtifactInstallationTarget::ExactInstallation {
                    installation_id: payload.installation_id,
                },
                tenant_id: payload.tenant_id,
                input: serde_json::json!({
                    "binding_id": payload.binding.id.clone(),
                    "scheduled_for": payload.scheduled_for,
                    "schedule_digest": payload.schedule_digest.clone(),
                }),
                phase: ExecutionPhase::Scheduled,
                context: crate::ArtifactBindingExecutionContext::default(),
            })
            .await
        {
            Ok(_) => Ok(ModuleWorkOutcome::Completed),
            Err(_) => Ok(ModuleWorkOutcome::Retryable {
                message: "execution_failed".to_string(),
            }),
        }
    }
}

fn validate_request(
    request: &ArtifactScheduleDeliveryRequest,
) -> Result<(), ArtifactScheduleDeliveryError> {
    if request.tenant_id.is_nil()
        || request.installation_id.is_nil()
        || request.binding_id.is_empty()
        || request.binding_id.len() > MAX_BINDING_ID_BYTES
        || request.scheduled_for.timestamp_subsec_nanos() != 0
    {
        return Err(ArtifactScheduleDeliveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_worker_id(worker_id: &str) -> Result<(), ArtifactScheduleDeliveryError> {
    if worker_id.is_empty() || worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(ArtifactScheduleDeliveryError::InvalidWorker);
    }
    Ok(())
}

fn validate_error_code(error_code: &str) -> Result<(), ArtifactScheduleDeliveryError> {
    if error_code.is_empty()
        || error_code.len() > MAX_ERROR_CODE_BYTES
        || !error_code.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(ArtifactScheduleDeliveryError::InvalidCompletion);
    }
    Ok(())
}

fn schedule_digest_for_binding(
    descriptor: &ModuleArtifactDescriptor,
    binding_id: &str,
) -> Result<String, ArtifactScheduleDeliveryError> {
    descriptor
        .bindings
        .iter()
        .find(|binding| {
            binding.id == binding_id && binding.kind == ModuleRuntimeBindingKind::Schedule
        })
        .and_then(|binding| binding.schedule.as_ref())
        .map(schedule_binding_digest)
        .ok_or(ArtifactScheduleDeliveryError::BindingUnavailable)
}

async fn load_admitted_descriptor<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<ModuleArtifactDescriptor, ArtifactScheduleDeliveryError> {
    let enabled = match backend {
        DbBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
        _ => "COALESCE(lifecycle.enabled, 1) = 1",
    };
    let row = connection.query_one(Statement::from_sql_and_values(backend, format!(
        "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
         FROM module_artifact_installations installation \
         JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
         LEFT JOIN module_artifact_tenant_lifecycle lifecycle ON lifecycle.installation_id = installation.installation_id AND lifecycle.tenant_id = {} \
         WHERE installation.installation_id = {} AND admission.status = 'active' \
           AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall WHERE uninstall.installation_id = installation.installation_id) \
           AND {enabled} \
           AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL))",
        placeholder(backend, 1), placeholder(backend, 2), placeholder(backend, 3)),
        vec![uuid_value(tenant_id, backend), uuid_value(installation_id, backend), uuid_value(tenant_id, backend)])).await.map_err(storage_error)?
        .ok_or(ArtifactScheduleDeliveryError::InstallationUnavailable)?;
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactScheduleDeliveryError::InstallationUnavailable)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactScheduleDeliveryError::InstallationUnavailable)?;
    Ok(descriptor)
}

async fn expire_claims<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    max_attempts: u32,
) -> Result<(), ArtifactScheduleDeliveryError> {
    connection.execute(Statement::from_sql_and_values(backend, format!(
        "UPDATE module_artifact_schedule_deliveries \
         SET status = CASE WHEN attempt >= {} THEN 'dead_letter' ELSE 'pending' END, \
             available_at = {}, claimed_by = NULL, claimed_until = NULL, last_error_code = 'lease_expired', \
             dead_lettered_at = CASE WHEN attempt >= {} THEN {} ELSE NULL END \
         WHERE tenant_id = {} AND status = 'running' AND claimed_until < {}",
        placeholder(backend, 1), now_expression(backend), placeholder(backend, 2), now_expression(backend), placeholder(backend, 3), now_expression(backend)),
        vec![i32::try_from(max_attempts).map_err(|_| ArtifactScheduleDeliveryError::InvalidConfiguration)?.into(), i32::try_from(max_attempts).map_err(|_| ArtifactScheduleDeliveryError::InvalidConfiguration)?.into(), uuid_value(tenant_id, backend)])).await.map_err(storage_error)?;
    Ok(())
}

async fn cancel_unavailable<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    delivery_id: Uuid,
) -> Result<(), ArtifactScheduleDeliveryError> {
    connection.execute(Statement::from_sql_and_values(backend, format!(
        "UPDATE module_artifact_schedule_deliveries \
         SET status = 'cancelled', claimed_by = NULL, claimed_until = NULL, last_error_code = 'schedule_unavailable', cancelled_at = {} \
         WHERE delivery_id = {} AND tenant_id = {} AND status = 'pending'",
        now_expression(backend), placeholder(backend, 1), placeholder(backend, 2)),
        vec![uuid_value(delivery_id, backend), uuid_value(tenant_id, backend)])).await.map_err(storage_error)?;
    Ok(())
}

async fn complete_terminal<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    worker_id: &str,
    delivery_id: Uuid,
    attempt: u32,
    status: &str,
    error_code: Option<&str>,
    timestamp_column: &str,
) -> Result<u64, ArtifactScheduleDeliveryError> {
    let (error_assignment, values) = match error_code {
        Some(error_code) => (
            format!("last_error_code = {}, ", placeholder(backend, 1)),
            vec![error_code.to_owned().into()],
        ),
        None => ("last_error_code = NULL, ".to_string(), Vec::new()),
    };
    let offset = values.len();
    let result = connection.execute(Statement::from_sql_and_values(backend, format!(
        "UPDATE module_artifact_schedule_deliveries \
         SET status = '{status}', claimed_by = NULL, claimed_until = NULL, {error_assignment}{timestamp_column} = {} \
         WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' AND claimed_by = {} AND attempt = {}",
        now_expression(backend), placeholder(backend, offset + 1), placeholder(backend, offset + 2), placeholder(backend, offset + 3), placeholder(backend, offset + 4)), {
        let mut values = values;
        values.extend([uuid_value(delivery_id, backend), uuid_value(tenant_id, backend), worker_id.to_owned().into(), i64::from(attempt).into()]);
        values
    })).await.map_err(storage_error)?;
    Ok(result.rows_affected())
}

async fn complete_retryable<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    worker_id: &str,
    delivery_id: Uuid,
    attempt: u32,
    error_code: &str,
    delay_seconds: u32,
) -> Result<u64, ArtifactScheduleDeliveryError> {
    let result = connection.execute(Statement::from_sql_and_values(backend, format!(
        "UPDATE module_artifact_schedule_deliveries \
         SET status = 'pending', claimed_by = NULL, claimed_until = NULL, last_error_code = {}, available_at = {} \
         WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' AND claimed_by = {} AND attempt = {}",
        placeholder(backend, 1), lease_expression(backend, 2), placeholder(backend, 3), placeholder(backend, 4), placeholder(backend, 5), placeholder(backend, 6)),
        vec![error_code.to_owned().into(), i64::from(delay_seconds).into(), uuid_value(delivery_id, backend), uuid_value(tenant_id, backend), worker_id.to_owned().into(), i64::from(attempt).into()])).await.map_err(storage_error)?;
    Ok(result.rows_affected())
}

fn attempt_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<u32, ArtifactScheduleDeliveryError> {
    let attempt = match backend {
        DbBackend::Postgres => i64::from(row.try_get::<i32>("", "attempt").map_err(storage_error)?),
        _ => row.try_get::<i64>("", "attempt").map_err(storage_error)?,
    };
    u32::try_from(attempt).map_err(|_| ArtifactScheduleDeliveryError::ClaimLost)
}

fn datetime_from_row(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<DateTime<Utc>, ArtifactScheduleDeliveryError> {
    match backend {
        DbBackend::Postgres => row
            .try_get::<DateTime<Utc>>("", column)
            .map_err(storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(storage_error)
            .and_then(|value| {
                DateTime::parse_from_rfc3339(&value)
                    .map(|timestamp| timestamp.with_timezone(&Utc))
                    .map_err(storage_error)
            }),
    }
}

fn datetime_value(value: DateTime<Utc>, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(Box::new(value))),
        _ => value.to_rfc3339().into(),
    }
}

fn lease_expression(backend: DbBackend, parameter: usize) -> String {
    match backend {
        DbBackend::Postgres => format!(
            "NOW() + ({} * INTERVAL '1 second')",
            placeholder(backend, parameter)
        ),
        _ => format!(
            "datetime('now', '+' || {} || ' seconds')",
            placeholder(backend, parameter)
        ),
    }
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactScheduleDeliveryError {
    ArtifactScheduleDeliveryError::Storage(error.to_string())
}

#[derive(Debug, Error)]
pub enum ArtifactScheduleDeliveryError {
    #[error("artifact schedule-delivery queue configuration is invalid")]
    InvalidConfiguration,
    #[error("artifact schedule-delivery request is invalid")]
    InvalidRequest,
    #[error("artifact schedule-delivery worker identity is invalid")]
    InvalidWorker,
    #[error("artifact schedule-delivery completion is invalid")]
    InvalidCompletion,
    #[error("artifact schedule-delivery installation is not active for the tenant")]
    InstallationUnavailable,
    #[error("artifact schedule-delivery binding is not admitted")]
    BindingUnavailable,
    #[error("artifact schedule-delivery slot conflicts with a changed schedule contract")]
    ScheduleChanged,
    #[error("artifact schedule-delivery claim was lost or expired")]
    ClaimLost,
    #[error("artifact schedule-delivery storage failed: {0}")]
    Storage(String),
}
