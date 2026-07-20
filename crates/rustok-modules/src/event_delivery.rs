//! Durable owner-owned delivery state for artifact Event bindings.
//!
//! `sys_events` remains the platform event journal. This projection records
//! one at-least-once execution state machine per `(source event, immutable
//! installation, binding)`, so it cannot be mistaken for a second event log.

use std::{any::Any, collections::BTreeMap, sync::Arc};

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{
    manifest_hash::hash_manifest_snapshot, HostRuntimeContext, ModuleWorkError, ModuleWorkHandler,
    ModuleWorkItem, ModuleWorkOutcome, ModuleWorkSource,
};
use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_runtime::{ModuleWorkRegistration, ModuleWorkScheduler};
use rustok_sandbox::ExecutionPhase;

use crate::{
    artifact::{event_topic_matches, valid_event_topic},
    data::{configure_tenant_scope, now_expression, placeholder, uuid_from_row, uuid_value},
    ArtifactBindingDispatch, ArtifactInstallationTarget, ArtifactReleaseRef,
    ModuleArtifactDescriptor, ModuleRuntimeBinding, ModuleRuntimeBindingKind,
    SharedArtifactBindingExecutor, SharedArtifactDeliveryTenantSource,
};

const MAX_EVENT_TYPE_BYTES: usize = 128;
const MAX_BINDING_ID_BYTES: usize = 128;
const MAX_WORKER_ID_BYTES: usize = 128;
const MAX_ERROR_CODE_BYTES: usize = 96;

/// Stable generic-scheduler identity for all artifact Event delivery work.
pub const ARTIFACT_EVENT_DELIVERY_WORKER: &str = "artifact_event_delivery";

/// Immutable identity and envelope of the already durable platform event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactEventDeliverySource {
    pub event_id: Uuid,
    pub event_type: String,
    pub schema_version: u16,
    pub payload: Value,
}

/// One binding-specific projection request. The caller must have selected the
/// effective artifact subscription before it reaches this owner service.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactEventDeliveryRequest {
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub binding_id: String,
    pub source: ArtifactEventDeliverySource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEventDeliveryReceipt {
    pub delivery_id: Uuid,
    pub created: bool,
}

/// Work claimed by one host worker. The release and binding are read from the
/// immutable installed descriptor, not from a registry tag or mutable catalog.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactEventDeliveryWorkItem {
    pub delivery_id: Uuid,
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub attempt: u32,
    pub release: ArtifactReleaseRef,
    pub binding: ModuleRuntimeBinding,
    pub source: ArtifactEventDeliverySource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactEventDeliveryOutcome {
    Succeeded,
    Retryable { error_code: String },
    DeadLetter { error_code: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEventDeliveryCompletion {
    pub delivery_id: Uuid,
    pub attempt: u32,
    pub outcome: ArtifactEventDeliveryOutcome,
}

/// Queue-owned limits. The artifact never chooses a lease or retry delay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEventDeliveryConfig {
    pub max_attempts: u32,
    pub lease_seconds: u32,
    pub retry_base_seconds: u32,
    pub retry_max_seconds: u32,
}

impl Default for ArtifactEventDeliveryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            lease_seconds: 60,
            retry_base_seconds: 5,
            retry_max_seconds: 300,
        }
    }
}

impl ArtifactEventDeliveryConfig {
    fn validate(&self) -> Result<(), ArtifactEventDeliveryError> {
        if self.max_attempts == 0
            || self.max_attempts > 100
            || self.lease_seconds == 0
            || self.lease_seconds > 3_600
            || self.retry_base_seconds == 0
            || self.retry_max_seconds < self.retry_base_seconds
            || self.retry_max_seconds > 86_400
        {
            return Err(ArtifactEventDeliveryError::InvalidConfiguration);
        }
        Ok(())
    }

    fn retry_delay_seconds(&self, attempt: u32) -> u32 {
        let exponent = attempt.saturating_sub(1).min(16);
        self.retry_base_seconds
            .saturating_mul(2_u32.pow(exponent))
            .min(self.retry_max_seconds)
    }
}

/// Durable delivery projection implemented by the module owner.
#[derive(Clone)]
pub struct SeaOrmArtifactEventDeliveryQueue {
    db: DatabaseConnection,
    config: ArtifactEventDeliveryConfig,
}

/// Projects a platform event from `sys_events` into every effective immutable
/// artifact Event binding. It is deliberately a durable relay target rather
/// than a `ModuleEventListenerRegistry` callback.
#[derive(Clone)]
pub struct SeaOrmArtifactEventSubscriptionProjector {
    db: DatabaseConnection,
    deliveries: SeaOrmArtifactEventDeliveryQueue,
}

impl SeaOrmArtifactEventSubscriptionProjector {
    pub fn new(
        db: DatabaseConnection,
        config: ArtifactEventDeliveryConfig,
    ) -> Result<Self, ArtifactEventDeliveryError> {
        let deliveries = SeaOrmArtifactEventDeliveryQueue::new(db.clone(), config)?;
        Ok(Self { db, deliveries })
    }

    /// Materializes every effective subscribed binding before the outbox relay
    /// may acknowledge its source event. Retrying this call is safe because
    /// `enqueue` owns the binding-scoped source identity.
    pub async fn project(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<usize, ArtifactEventDeliveryError> {
        // Global platform events do not have a tenant artifact composition and
        // therefore cannot produce a tenant-scoped artifact delivery record.
        if envelope.tenant_id.is_nil() {
            return Ok(0);
        }
        if !valid_delivered_event_type(&envelope.event_type) {
            return Err(ArtifactEventDeliveryError::InvalidSourceEvent);
        }
        let payload = serde_json::to_value(&envelope.event)
            .map_err(|error| ArtifactEventDeliveryError::Storage(error.to_string()))?;
        let subscriptions = self
            .effective_subscriptions(envelope.tenant_id, &envelope.event_type)
            .await?;
        let mut projected = 0;
        for subscription in subscriptions {
            let receipt = self
                .deliveries
                .enqueue(ArtifactEventDeliveryRequest {
                    tenant_id: envelope.tenant_id,
                    installation_id: subscription.installation_id,
                    binding_id: subscription.binding_id,
                    source: ArtifactEventDeliverySource {
                        event_id: envelope.id,
                        event_type: envelope.event_type.clone(),
                        schema_version: envelope.schema_version,
                        payload: payload.clone(),
                    },
                })
                .await?;
            if receipt.created {
                projected += 1;
            }
        }
        Ok(projected)
    }

    async fn effective_subscriptions(
        &self,
        tenant_id: Uuid,
        event_type: &str,
    ) -> Result<Vec<ArtifactEventSubscription>, ArtifactEventDeliveryError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let enabled = match backend {
            DbBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
            _ => "COALESCE(lifecycle.enabled, 1) = 1",
        };
        let rows = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.installation_id, installation.slug, installation.scope_kind, \
                            CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     LEFT JOIN module_artifact_tenant_lifecycle lifecycle \
                       ON lifecycle.installation_id = installation.installation_id \
                      AND lifecycle.tenant_id = {} \
                     WHERE admission.status = 'active' \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id) \
                       AND {enabled} \
                       AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                            OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL))",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![uuid_value(tenant_id, backend), uuid_value(tenant_id, backend)],
            ))
            .await
            .map_err(storage_error)?;
        let mut effective = BTreeMap::<String, ArtifactSubscriptionCandidate>::new();
        for row in rows {
            let candidate = subscription_candidate_from_row(&row, backend)?;
            match effective.get(&candidate.slug) {
                None => {
                    effective.insert(candidate.slug.clone(), candidate);
                }
                Some(existing) if candidate.tenant_scoped && !existing.tenant_scoped => {
                    effective.insert(candidate.slug.clone(), candidate);
                }
                Some(existing) if !candidate.tenant_scoped && existing.tenant_scoped => {}
                Some(_) => {
                    return Err(ArtifactEventDeliveryError::AmbiguousSubscription(
                        candidate.slug,
                    ));
                }
            }
        }
        transaction.commit().await.map_err(storage_error)?;

        let mut subscriptions = Vec::new();
        for candidate in effective.into_values() {
            for binding in candidate.descriptor.bindings {
                if binding.kind == ModuleRuntimeBindingKind::Event
                    && binding
                        .event_topics
                        .iter()
                        .any(|topic| event_topic_matches(topic, event_type))
                {
                    subscriptions.push(ArtifactEventSubscription {
                        installation_id: candidate.installation_id,
                        binding_id: binding.id,
                    });
                }
            }
        }
        Ok(subscriptions)
    }
}

/// Relay target decorator that makes durable artifact projection a prerequisite
/// for downstream publication and outbox acknowledgement.
#[derive(Clone)]
pub struct ArtifactEventProjectionTransport {
    projector: SeaOrmArtifactEventSubscriptionProjector,
    downstream: Arc<dyn EventTransport>,
}

impl ArtifactEventProjectionTransport {
    pub fn new(
        projector: SeaOrmArtifactEventSubscriptionProjector,
        downstream: Arc<dyn EventTransport>,
    ) -> Self {
        Self {
            projector,
            downstream,
        }
    }
}

#[async_trait]
impl EventTransport for ArtifactEventProjectionTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        self.projector.project(&envelope).await.map_err(|error| {
            rustok_core::Error::External(format!("artifact event projection failed: {error}"))
        })?;
        self.downstream.publish(envelope).await
    }

    async fn acknowledge(&self, event_id: Uuid) -> rustok_core::Result<()> {
        self.downstream.acknowledge(event_id).await
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        self.downstream.reliability_level()
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Clone)]
struct ArtifactSubscriptionCandidate {
    installation_id: Uuid,
    slug: String,
    tenant_scoped: bool,
    descriptor: ModuleArtifactDescriptor,
}

struct ArtifactEventSubscription {
    installation_id: Uuid,
    binding_id: String,
}

fn subscription_candidate_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ArtifactSubscriptionCandidate, ArtifactEventDeliveryError> {
    let slug: String = row.try_get("", "slug").map_err(storage_error)?;
    let scope_kind: String = row.try_get("", "scope_kind").map_err(storage_error)?;
    let tenant_scoped = match scope_kind.as_str() {
        "tenant" => true,
        "platform" => false,
        _ => return Err(ArtifactEventDeliveryError::SubscriptionStateInvalid),
    };
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactEventDeliveryError::SubscriptionStateInvalid)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactEventDeliveryError::SubscriptionStateInvalid)?;
    if descriptor.slug != slug {
        return Err(ArtifactEventDeliveryError::SubscriptionStateInvalid);
    }
    Ok(ArtifactSubscriptionCandidate {
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(storage_error)?,
        slug,
        tenant_scoped,
        descriptor,
    })
}

/// Adapter between the owner queue and the platform `ModuleWorkScheduler`.
/// It owns no event subscription: the durable `sys_events` projector feeds
/// this queue before downstream publication is acknowledged.
#[derive(Clone)]
pub struct ArtifactEventDeliveryWorkAdapter {
    queue: SeaOrmArtifactEventDeliveryQueue,
    executor: SharedArtifactBindingExecutor,
    tenants: SharedArtifactDeliveryTenantSource,
    worker_id: String,
}

impl ArtifactEventDeliveryWorkAdapter {
    pub fn new(
        queue: SeaOrmArtifactEventDeliveryQueue,
        executor: SharedArtifactBindingExecutor,
        tenants: SharedArtifactDeliveryTenantSource,
        worker_id: String,
    ) -> Result<Self, ArtifactEventDeliveryError> {
        validate_worker_id(&worker_id)?;
        Ok(Self {
            queue,
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

    fn payload(item: &ModuleWorkItem) -> Result<ArtifactEventDeliveryWorkItem, ModuleWorkError> {
        serde_json::from_value(item.payload.clone()).map_err(|_| {
            ModuleWorkError::Source("artifact event work item payload is invalid".to_string())
        })
    }
}

/// Registration requires explicit host handles and fails closed if either the
/// sandbox-backed executor or tenant enumerator has not been composed.
#[derive(Clone, Debug)]
pub struct ArtifactEventDeliveryWorkRegistration {
    pub config: ArtifactEventDeliveryConfig,
    pub worker_id: String,
}

impl Default for ArtifactEventDeliveryWorkRegistration {
    fn default() -> Self {
        Self {
            config: ArtifactEventDeliveryConfig::default(),
            worker_id: ARTIFACT_EVENT_DELIVERY_WORKER.to_string(),
        }
    }
}

#[async_trait]
impl ModuleWorkRegistration for ArtifactEventDeliveryWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let executor = host
            .shared_get::<SharedArtifactBindingExecutor>()
            .ok_or_else(|| {
                ModuleWorkError::Handler("artifact event executor handle is missing".to_string())
            })?;
        let tenants = host
            .shared_get::<SharedArtifactDeliveryTenantSource>()
            .ok_or_else(|| {
                ModuleWorkError::Handler(
                    "artifact event tenant source handle is missing".to_string(),
                )
            })?;
        let queue = SeaOrmArtifactEventDeliveryQueue::new(host.db_clone(), self.config.clone())
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        ArtifactEventDeliveryWorkAdapter::new(queue, executor, tenants, self.worker_id.clone())
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for ArtifactEventDeliveryWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != ARTIFACT_EVENT_DELIVERY_WORKER {
            return Ok(None);
        }
        let tenant_ids = self
            .tenants
            .tenant_ids()
            .await
            .map_err(ModuleWorkError::Source)?;
        for tenant_id in tenant_ids {
            if tenant_id.is_nil() {
                return Err(ModuleWorkError::Source(
                    "artifact event tenant source returned a nil tenant id".to_string(),
                ));
            }
            if let Some(item) = self
                .queue
                .claim_next(tenant_id, &self.worker_id)
                .await
                .map_err(|error| ModuleWorkError::Source(error.to_string()))?
            {
                return Ok(Some(ModuleWorkItem {
                    id: item.delivery_id,
                    tenant_id: item.tenant_id,
                    worker_slug: ARTIFACT_EVENT_DELIVERY_WORKER.to_string(),
                    lease_token: Uuid::new_v4().to_string(),
                    payload: serde_json::to_value(item).expect("event work item must serialize"),
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
        if item.worker_slug != ARTIFACT_EVENT_DELIVERY_WORKER {
            return Err(ModuleWorkError::Source(
                "artifact event completion has the wrong worker slug".to_string(),
            ));
        }
        let payload = Self::payload(item)?;
        if payload.delivery_id != item.id || payload.tenant_id != item.tenant_id {
            return Err(ModuleWorkError::Source(
                "artifact event completion identity does not match its payload".to_string(),
            ));
        }
        let outcome = match outcome {
            ModuleWorkOutcome::Completed => ArtifactEventDeliveryOutcome::Succeeded,
            ModuleWorkOutcome::Retryable { .. } => ArtifactEventDeliveryOutcome::Retryable {
                error_code: "execution_failed".to_string(),
            },
            ModuleWorkOutcome::Rejected { .. } => ArtifactEventDeliveryOutcome::DeadLetter {
                error_code: "execution_rejected".to_string(),
            },
            ModuleWorkOutcome::Cancelled => ArtifactEventDeliveryOutcome::DeadLetter {
                error_code: "execution_cancelled".to_string(),
            },
        };
        self.queue
            .complete(
                payload.tenant_id,
                &self.worker_id,
                ArtifactEventDeliveryCompletion {
                    delivery_id: payload.delivery_id,
                    attempt: payload.attempt,
                    outcome,
                },
            )
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }
}

#[async_trait]
impl ModuleWorkHandler for ArtifactEventDeliveryWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        ARTIFACT_EVENT_DELIVERY_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        if item.worker_slug != ARTIFACT_EVENT_DELIVERY_WORKER {
            return Err(ModuleWorkError::Handler(
                "artifact event execution has the wrong worker slug".to_string(),
            ));
        }
        let payload = Self::payload(&item)?;
        if payload.delivery_id != item.id || payload.tenant_id != item.tenant_id {
            return Err(ModuleWorkError::Handler(
                "artifact event execution identity does not match its payload".to_string(),
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
                    "event": {
                        "id": payload.source.event_id,
                        "type": payload.source.event_type.clone(),
                        "schema_version": payload.source.schema_version,
                        "payload": payload.source.payload.clone(),
                    },
                }),
                phase: ExecutionPhase::Event,
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

impl SeaOrmArtifactEventDeliveryQueue {
    pub fn new(
        db: DatabaseConnection,
        config: ArtifactEventDeliveryConfig,
    ) -> Result<Self, ArtifactEventDeliveryError> {
        config.validate()?;
        Ok(Self { db, config })
    }

    pub fn config(&self) -> &ArtifactEventDeliveryConfig {
        &self.config
    }

    /// Creates the binding-scoped projection or returns the original receipt
    /// for an identical source retry. A changed source under the same unique
    /// identity is rejected rather than overwriting audit evidence.
    pub async fn enqueue(
        &self,
        request: ArtifactEventDeliveryRequest,
    ) -> Result<ArtifactEventDeliveryReceipt, ArtifactEventDeliveryError> {
        validate_delivery_request(&request)?;
        let source_digest = source_digest(&request.source);
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
        let binding = descriptor
            .bindings
            .iter()
            .find(|binding| binding.id == request.binding_id)
            .ok_or(ArtifactEventDeliveryError::BindingUnavailable)?;
        if binding.kind != ModuleRuntimeBindingKind::Event
            || !binding
                .event_topics
                .iter()
                .any(|topic| event_topic_matches(topic, &request.source.event_type))
        {
            return Err(ArtifactEventDeliveryError::BindingUnavailable);
        }

        let delivery_id = Uuid::new_v4();
        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_event_deliveries \
                     (delivery_id, tenant_id, source_event_id, installation_id, binding_id, event_type, \
                      schema_version, payload, source_digest, attempt, status, available_at, created_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, 0, 'pending', {}, {}) \
                     ON CONFLICT (source_event_id, installation_id, binding_id) DO NOTHING",
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
                    now_expression(backend),
                ),
                vec![
                    uuid_value(delivery_id, backend),
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.source.event_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.clone().into(),
                    request.source.event_type.clone().into(),
                    i32::from(request.source.schema_version).into(),
                    SqlValue::Json(Some(Box::new(request.source.payload.clone()))),
                    source_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if inserted.rows_affected() == 1 {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactEventDeliveryReceipt {
                delivery_id,
                created: true,
            });
        }

        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT delivery_id, source_digest FROM module_artifact_event_deliveries \
                     WHERE source_event_id = {} AND installation_id = {} AND binding_id = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                vec![
                    uuid_value(request.source.event_id, backend),
                    uuid_value(request.installation_id, backend),
                    request.binding_id.into(),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                ArtifactEventDeliveryError::Storage(
                    "delivery insert conflict was not readable".to_string(),
                )
            })?;
        let existing_digest: String = existing
            .try_get("", "source_digest")
            .map_err(storage_error)?;
        if existing_digest != source_digest {
            return Err(ArtifactEventDeliveryError::IdempotencyConflict);
        }
        let delivery_id =
            uuid_from_row(&existing, "delivery_id", backend).map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactEventDeliveryReceipt {
            delivery_id,
            created: false,
        })
    }

    /// Reclaims expired leases, then atomically claims one due item for the
    /// supplied tenant. The worker must later complete using the same attempt
    /// and worker identity; stale completions cannot mutate new claims.
    pub async fn claim_next(
        &self,
        tenant_id: Uuid,
        worker_id: &str,
    ) -> Result<Option<ArtifactEventDeliveryWorkItem>, ArtifactEventDeliveryError> {
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
                    "SELECT delivery_id FROM module_artifact_event_deliveries \
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
        let lease_until = lease_expression(backend, 2);
        let claimed = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_event_deliveries \
                     SET status = 'running', attempt = attempt + 1, claimed_by = {}, \
                         claimed_until = {lease_until}, last_error_code = NULL \
                     WHERE delivery_id = {} AND tenant_id = {} AND status = 'pending' \
                       AND available_at <= {}",
                    placeholder(backend, 1),
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
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT delivery.delivery_id, delivery.tenant_id, delivery.installation_id, \
                            delivery.binding_id, delivery.source_event_id, delivery.event_type, delivery.schema_version, \
                            delivery.payload, delivery.attempt, CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_event_deliveries delivery \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = delivery.installation_id \
                     WHERE delivery.delivery_id = {} AND delivery.tenant_id = {} \
                       AND delivery.status = 'running' AND delivery.claimed_by = {}",
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
            .ok_or(ArtifactEventDeliveryError::ClaimLost)?;
        let item = work_item_from_row(&row, backend)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(Some(item))
    }

    /// Completes a claim. Retryable results use deterministic queue-owned
    /// exponential backoff; a final failed attempt is retained as dead-letter
    /// evidence with its stable error code.
    pub async fn complete(
        &self,
        tenant_id: Uuid,
        worker_id: &str,
        completion: ArtifactEventDeliveryCompletion,
    ) -> Result<(), ArtifactEventDeliveryError> {
        validate_worker_id(worker_id)?;
        if completion.delivery_id.is_nil() || completion.attempt == 0 {
            return Err(ArtifactEventDeliveryError::InvalidCompletion);
        }
        if let ArtifactEventDeliveryOutcome::Retryable { error_code }
        | ArtifactEventDeliveryOutcome::DeadLetter { error_code } = &completion.outcome
        {
            validate_error_code(error_code)?;
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        let affected = match completion.outcome {
            ArtifactEventDeliveryOutcome::Succeeded => {
                complete_succeeded(
                    &transaction,
                    backend,
                    tenant_id,
                    worker_id,
                    completion.delivery_id,
                    completion.attempt,
                )
                .await?
            }
            ArtifactEventDeliveryOutcome::DeadLetter { error_code } => {
                complete_dead_letter(
                    &transaction,
                    backend,
                    tenant_id,
                    worker_id,
                    completion.delivery_id,
                    completion.attempt,
                    error_code,
                )
                .await?
            }
            ArtifactEventDeliveryOutcome::Retryable { error_code } => {
                if completion.attempt >= self.config.max_attempts {
                    complete_dead_letter(
                        &transaction,
                        backend,
                        tenant_id,
                        worker_id,
                        completion.delivery_id,
                        completion.attempt,
                        error_code,
                    )
                    .await?
                } else {
                    complete_retryable(
                        &transaction,
                        backend,
                        tenant_id,
                        worker_id,
                        completion.delivery_id,
                        completion.attempt,
                        error_code,
                        self.config.retry_delay_seconds(completion.attempt),
                    )
                    .await?
                }
            }
        };
        if affected != 1 {
            return Err(ArtifactEventDeliveryError::ClaimLost);
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(())
    }
}

async fn load_admitted_descriptor<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<ModuleArtifactDescriptor, ArtifactEventDeliveryError> {
    let enabled = match backend {
        DbBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
        _ => "COALESCE(lifecycle.enabled, 1) = 1",
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
                 FROM module_artifact_installations installation \
                 JOIN module_artifact_admissions admission \
                   ON admission.installation_id = installation.installation_id \
                 LEFT JOIN module_artifact_tenant_lifecycle lifecycle \
                   ON lifecycle.installation_id = installation.installation_id \
                  AND lifecycle.tenant_id = {} \
                 WHERE installation.installation_id = {} AND admission.status = 'active' \
                   AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                   WHERE uninstall.installation_id = installation.installation_id) \
                   AND {enabled} \
                   AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                        OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL))",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(installation_id, backend),
                uuid_value(tenant_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ArtifactEventDeliveryError::InstallationUnavailable)?;
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactEventDeliveryError::InstallationUnavailable)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactEventDeliveryError::InstallationUnavailable)?;
    Ok(descriptor)
}

async fn expire_claims<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    max_attempts: u32,
) -> Result<(), ArtifactEventDeliveryError> {
    connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_event_deliveries \
                 SET status = CASE WHEN attempt >= {} THEN 'dead_letter' ELSE 'pending' END, \
                     available_at = {}, claimed_by = NULL, claimed_until = NULL, \
                     last_error_code = 'lease_expired', \
                     dead_lettered_at = CASE WHEN attempt >= {} THEN {} ELSE NULL END \
                 WHERE tenant_id = {} AND status = 'running' AND claimed_until < {}",
                placeholder(backend, 1),
                now_expression(backend),
                placeholder(backend, 2),
                now_expression(backend),
                placeholder(backend, 3),
                now_expression(backend),
            ),
            vec![
                i32::try_from(max_attempts)
                    .map_err(|_| ArtifactEventDeliveryError::InvalidConfiguration)?
                    .into(),
                i32::try_from(max_attempts)
                    .map_err(|_| ArtifactEventDeliveryError::InvalidConfiguration)?
                    .into(),
                uuid_value(tenant_id, backend),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn complete_succeeded<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    worker_id: &str,
    delivery_id: Uuid,
    attempt: u32,
) -> Result<u64, ArtifactEventDeliveryError> {
    update_completion(
        connection,
        backend,
        tenant_id,
        worker_id,
        delivery_id,
        attempt,
        "status = 'succeeded', claimed_by = NULL, claimed_until = NULL, last_error_code = NULL, completed_at = ",
        None,
        now_expression(backend),
    )
    .await
}

async fn complete_dead_letter<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    worker_id: &str,
    delivery_id: Uuid,
    attempt: u32,
    error_code: String,
) -> Result<u64, ArtifactEventDeliveryError> {
    update_completion(
        connection,
        backend,
        tenant_id,
        worker_id,
        delivery_id,
        attempt,
        "status = 'dead_letter', claimed_by = NULL, claimed_until = NULL, last_error_code = ",
        Some(error_code),
        now_expression(backend),
    )
    .await
}

async fn complete_retryable<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    worker_id: &str,
    delivery_id: Uuid,
    attempt: u32,
    error_code: String,
    delay_seconds: u32,
) -> Result<u64, ArtifactEventDeliveryError> {
    let delay = retry_expression(backend, 2);
    let result = connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_event_deliveries \
                 SET status = 'pending', claimed_by = NULL, claimed_until = NULL, \
                     last_error_code = {}, available_at = {delay} \
                 WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' \
                   AND claimed_by = {} AND attempt = {}",
                placeholder(backend, 1),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
            ),
            vec![
                error_code.into(),
                i64::from(delay_seconds).into(),
                uuid_value(delivery_id, backend),
                uuid_value(tenant_id, backend),
                worker_id.to_owned().into(),
                i64::from(attempt).into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(result.rows_affected())
}

async fn update_completion<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    worker_id: &str,
    delivery_id: Uuid,
    attempt: u32,
    assignment: &str,
    error_code: Option<String>,
    terminal_timestamp: &str,
) -> Result<u64, ArtifactEventDeliveryError> {
    let has_error_code = error_code.is_some();
    let (assignment, values) = match error_code {
        Some(error_code) => (
            format!("{assignment}{}", placeholder(backend, 1)),
            vec![error_code.into()],
        ),
        None => (format!("{assignment}{terminal_timestamp}"), Vec::new()),
    };
    let offset = values.len();
    let result = connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_artifact_event_deliveries SET {assignment}{} \
                 WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' \
                   AND claimed_by = {} AND attempt = {}",
                if has_error_code {
                    format!(", dead_lettered_at = {terminal_timestamp}")
                } else {
                    String::new()
                },
                placeholder(backend, offset + 1),
                placeholder(backend, offset + 2),
                placeholder(backend, offset + 3),
                placeholder(backend, offset + 4),
            ),
            {
                let mut values = values;
                values.extend([
                    uuid_value(delivery_id, backend),
                    uuid_value(tenant_id, backend),
                    worker_id.to_owned().into(),
                    i64::from(attempt).into(),
                ]);
                values
            },
        ))
        .await
        .map_err(storage_error)?;
    Ok(result.rows_affected())
}

fn work_item_from_row(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<ArtifactEventDeliveryWorkItem, ArtifactEventDeliveryError> {
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(storage_error)?,
    )
    .map_err(|_| ArtifactEventDeliveryError::InstallationUnavailable)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactEventDeliveryError::InstallationUnavailable)?;
    let binding_id: String = row.try_get("", "binding_id").map_err(storage_error)?;
    let binding = descriptor
        .bindings
        .iter()
        .find(|binding| binding.id == binding_id && binding.kind == ModuleRuntimeBindingKind::Event)
        .cloned()
        .ok_or(ArtifactEventDeliveryError::BindingUnavailable)?;
    let event_type: String = row.try_get("", "event_type").map_err(storage_error)?;
    if !binding
        .event_topics
        .iter()
        .any(|topic| event_topic_matches(topic, &event_type))
    {
        return Err(ArtifactEventDeliveryError::BindingUnavailable);
    }
    let attempt = match backend {
        DbBackend::Postgres => i64::from(row.try_get::<i32>("", "attempt").map_err(storage_error)?),
        _ => row.try_get::<i64>("", "attempt").map_err(storage_error)?,
    };
    let schema_version = match backend {
        DbBackend::Postgres => i64::from(
            row.try_get::<i16>("", "schema_version")
                .map_err(storage_error)?,
        ),
        _ => row
            .try_get::<i64>("", "schema_version")
            .map_err(storage_error)?,
    };
    Ok(ArtifactEventDeliveryWorkItem {
        delivery_id: uuid_from_row(row, "delivery_id", backend).map_err(storage_error)?,
        tenant_id: uuid_from_row(row, "tenant_id", backend).map_err(storage_error)?,
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(storage_error)?,
        attempt: u32::try_from(attempt).map_err(|_| ArtifactEventDeliveryError::ClaimLost)?,
        release: descriptor.release_ref(),
        binding,
        source: ArtifactEventDeliverySource {
            event_id: uuid_from_row(row, "source_event_id", backend).map_err(storage_error)?,
            event_type,
            schema_version: u16::try_from(schema_version)
                .map_err(|_| ArtifactEventDeliveryError::InstallationUnavailable)?,
            payload: row.try_get("", "payload").map_err(storage_error)?,
        },
    })
}

fn validate_delivery_request(
    request: &ArtifactEventDeliveryRequest,
) -> Result<(), ArtifactEventDeliveryError> {
    if request.tenant_id.is_nil()
        || request.installation_id.is_nil()
        || request.source.event_id.is_nil()
        || request.source.schema_version == 0
        || !valid_delivered_event_type(&request.source.event_type)
        || request.binding_id.is_empty()
        || request.binding_id.len() > MAX_BINDING_ID_BYTES
    {
        return Err(ArtifactEventDeliveryError::InvalidRequest);
    }
    Ok(())
}

fn validate_worker_id(worker_id: &str) -> Result<(), ArtifactEventDeliveryError> {
    if worker_id.is_empty() || worker_id.len() > MAX_WORKER_ID_BYTES {
        return Err(ArtifactEventDeliveryError::InvalidWorker);
    }
    Ok(())
}

fn validate_error_code(error_code: &str) -> Result<(), ArtifactEventDeliveryError> {
    if error_code.is_empty()
        || error_code.len() > MAX_ERROR_CODE_BYTES
        || !error_code.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
    {
        return Err(ArtifactEventDeliveryError::InvalidCompletion);
    }
    Ok(())
}

fn valid_delivered_event_type(value: &str) -> bool {
    value.len() <= MAX_EVENT_TYPE_BYTES && valid_event_topic(value) && !value.ends_with(".*")
}

fn source_digest(source: &ArtifactEventDeliverySource) -> String {
    format!(
        "sha256:{}",
        hash_manifest_snapshot(&serde_json::json!({
            "event_id": source.event_id,
            "event_type": source.event_type,
            "schema_version": source.schema_version,
            "payload": source.payload,
        }))
    )
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

fn retry_expression(backend: DbBackend, parameter: usize) -> String {
    lease_expression(backend, parameter)
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactEventDeliveryError {
    ArtifactEventDeliveryError::Storage(error.to_string())
}

#[derive(Debug, Error)]
pub enum ArtifactEventDeliveryError {
    #[error("artifact event-delivery queue configuration is invalid")]
    InvalidConfiguration,
    #[error("artifact event-delivery request is invalid")]
    InvalidRequest,
    #[error("artifact event-delivery source event is invalid")]
    InvalidSourceEvent,
    #[error("artifact event-delivery worker identity is invalid")]
    InvalidWorker,
    #[error("artifact event-delivery completion is invalid")]
    InvalidCompletion,
    #[error("artifact event-delivery idempotency source differs from the original request")]
    IdempotencyConflict,
    #[error("artifact event-delivery installation is not active for the tenant")]
    InstallationUnavailable,
    #[error("artifact event-delivery binding is not admitted for the source event")]
    BindingUnavailable,
    #[error("artifact event subscription state is invalid")]
    SubscriptionStateInvalid,
    #[error("artifact event subscription for module `{0}` is ambiguous")]
    AmbiguousSubscription(String),
    #[error("artifact event-delivery claim was lost or expired")]
    ClaimLost,
    #[error("artifact event-delivery storage failed: {0}")]
    Storage(String),
}
