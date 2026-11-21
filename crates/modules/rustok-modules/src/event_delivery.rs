//! Durable owner-owned delivery state for artifact Event bindings.
//!
//! `sys_events` remains the platform event journal. This projection records
//! one at-least-once execution state machine per `(source event, immutable
//! installation, binding)`, so it cannot be mistaken for a second event log.

use std::{any::Any, sync::Arc};

use async_trait::async_trait;
use rustok_events::{ContractEventEnvelope, ContractEventPayload};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, QueryResult, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{
    HostRuntimeContext, ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome,
    ModuleWorkSource, manifest_hash::hash_manifest_snapshot,
};
use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_runtime::{ModuleWorkRegistration, ModuleWorkScheduler};
use rustok_sandbox::ExecutionPhase;

use crate::backend::SupportedBackend;
use crate::{
    ArtifactBindingDispatch, ArtifactInstallationTarget, ArtifactReleaseRef,
    ControlPlaneInfrastructure, ModuleArtifactDescriptor, ModuleRuntimeBinding,
    ModuleRuntimeBindingKind, SharedArtifactBindingExecutor, SharedArtifactDeliveryTenantSource,
    artifact::{event_topic_matches, valid_event_topic},
    data::{configure_tenant_scope, retry_available_at_expression, uuid_from_row},
};

const MAX_EVENT_TYPE_BYTES: usize = 128;
const MAX_BINDING_ID_BYTES: usize = 128;
const MAX_WORKER_ID_BYTES: usize = 128;
const MAX_ERROR_CODE_BYTES: usize = 96;
/// Maximum eligible installation rows in one complete tenant projection. The
/// query reads one extra sentinel row; over the limit it fails rather than
/// dispatching an event with an arbitrarily truncated subscription set.
const MAX_EVENT_CANDIDATES_PER_PROJECTION: usize = 10_000;
/// Upper bound on poison/lost-race candidates one `claim_next` call will
/// skip before reporting no work, so a single poll stays time-bounded.
const MAX_CANDIDATE_SKIPS_PER_CLAIM: usize = 16;

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
    pub claim_token: Uuid,
    pub release: ArtifactReleaseRef,
    pub binding: ModuleRuntimeBinding,
    pub source: ArtifactEventDeliverySource,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArtifactEventDeliveryOutcome {
    Succeeded,
    Retryable {
        error_code: String,
    },
    /// The binding was denied by effective policy before any guest
    /// execution. The delivery is parked (returned to `pending` with its
    /// claim's attempt increment reverted) so a later re-enable can still
    /// deliver it; only genuine execution failures spend the
    /// `max_attempts` dead-letter budget.
    Held {
        error_code: String,
    },
    DeadLetter {
        error_code: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactEventDeliveryCompletion {
    pub delivery_id: Uuid,
    pub attempt: u32,
    pub claim_token: Uuid,
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
    infrastructure: ControlPlaneInfrastructure,
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
        Self::with_infrastructure(db, config, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        config: ArtifactEventDeliveryConfig,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Result<Self, ArtifactEventDeliveryError> {
        let deliveries = SeaOrmArtifactEventDeliveryQueue::with_infrastructure(
            db.clone(),
            config,
            infrastructure,
        )?;
        Ok(Self { db, deliveries })
    }

    /// Materializes every effective subscribed binding before the outbox relay
    /// may acknowledge its source event. Retrying this call is safe because
    /// `enqueue` owns the binding-scoped source identity.
    pub async fn project(
        &self,
        envelope: &EventEnvelope,
    ) -> Result<usize, ArtifactEventDeliveryError> {
        let payload = serde_json::to_value(&envelope.event)
            .map_err(|error| ArtifactEventDeliveryError::Storage(error.to_string()))?;
        self.project_source(
            envelope.tenant_id,
            ArtifactEventDeliverySource {
                event_id: envelope.id,
                event_type: envelope.event_type.clone(),
                schema_version: envelope.schema_version,
                payload,
            },
        )
        .await
    }

    /// Materializes bounded-family subscriptions without reinterpreting them as root events.
    /// Guest payloads preserve the sealed family wrapper; dedupe uses the original envelope ID.
    pub async fn project_contract(
        &self,
        envelope: &ContractEventEnvelope,
    ) -> Result<usize, ArtifactEventDeliveryError> {
        let payload = envelope
            .payload()
            .map_err(|_| ArtifactEventDeliveryError::InvalidSourceEvent)?;
        if matches!(payload, ContractEventPayload::Root(_)) {
            let root = envelope
                .clone()
                .into_root_envelope()
                .map_err(|_| ArtifactEventDeliveryError::InvalidSourceEvent)?;
            return self.project(&root).await;
        }
        let payload = serde_json::to_value(payload)
            .map_err(|error| ArtifactEventDeliveryError::Storage(error.to_string()))?;
        self.project_source(
            envelope.tenant_id(),
            ArtifactEventDeliverySource {
                event_id: envelope.id(),
                event_type: envelope.event_type().to_owned(),
                schema_version: envelope.schema_version(),
                payload,
            },
        )
        .await
    }

    async fn project_source(
        &self,
        tenant_id: Uuid,
        source: ArtifactEventDeliverySource,
    ) -> Result<usize, ArtifactEventDeliveryError> {
        // Global platform events have no tenant artifact composition.
        if tenant_id.is_nil() {
            return Ok(0);
        }
        if !valid_delivered_event_type(&source.event_type) {
            return Err(ArtifactEventDeliveryError::InvalidSourceEvent);
        }
        let subscriptions = self
            .effective_subscriptions(tenant_id, &source.event_type)
            .await?;
        let mut projected = 0;
        for subscription in subscriptions {
            let receipt = self
                .deliveries
                .enqueue(ArtifactEventDeliveryRequest {
                    tenant_id,
                    installation_id: subscription.installation_id,
                    binding_id: subscription.binding_id,
                    source: source.clone(),
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
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        let enabled = match backend {
            SupportedBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
            SupportedBackend::Sqlite => "COALESCE(lifecycle.enabled, 1) = 1",
        };
        let rows = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.installation_id, installation.slug, installation.scope_kind, \
                            installation.version, installation.payload_digest, \
                            admission.payload_digest AS admission_payload_digest, \
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
                            OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL)) \
                     LIMIT {candidate_limit}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    candidate_limit = MAX_EVENT_CANDIDATES_PER_PROJECTION + 1,
                ),
                vec![backend.uuid_value(tenant_id), backend.uuid_value(tenant_id)],
            ))
            .await
            .map_err(storage_error)?;
        if rows.len() > MAX_EVENT_CANDIDATES_PER_PROJECTION {
            return Err(ArtifactEventDeliveryError::SubscriptionCandidateLimitExceeded);
        }
        // Selection precedes descriptor parsing: a corrupt tenant winner
        // cannot resurrect its platform fallback. An incomplete selection is
        // an error, not an empty subscription: otherwise the relay can mark
        // the source dispatched and permanently lose potential deliveries.
        let selected = crate::installation_selection::select_projection_rows(rows)
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        if selected.ambiguous_count != 0 {
            tracing::warn!(%tenant_id, ambiguous_count = selected.ambiguous_count,
                "artifact event subscription selection is ambiguous");
            return Err(ArtifactEventDeliveryError::SubscriptionStateInvalid);
        }

        let mut subscriptions = Vec::new();
        for row in selected.rows {
            let candidate = match subscription_candidate_from_row(&row, backend) {
                Err(ArtifactEventDeliveryError::SubscriptionStateInvalid) => {
                    tracing::warn!(%tenant_id, "invalid artifact event subscription descriptor or release identity");
                    return Err(ArtifactEventDeliveryError::SubscriptionStateInvalid);
                }
                result => result?,
            };
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

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
        if matches!(
            envelope
                .payload()
                .map_err(|error| rustok_core::Error::Validation(error.to_string()))?,
            ContractEventPayload::Root(_)
        ) {
            let root = envelope
                .into_root_envelope()
                .map_err(|error| rustok_core::Error::Validation(error.to_string()))?;
            return self.publish(root).await;
        }
        self.projector
            .project_contract(&envelope)
            .await
            .map_err(|error| {
                rustok_core::Error::External(format!(
                    "artifact contract event projection failed: {error}"
                ))
            })?;
        self.downstream.publish_contract(envelope).await
    }

    async fn acknowledge(&self, event_id: Uuid) -> rustok_core::Result<()> {
        self.downstream.acknowledge(event_id).await
    }

    // Transactional appends bypass delivery-time projection (projection
    // happens when the relay publishes), so they forward verbatim.
    fn supports_transactional_writes(&self) -> bool {
        self.downstream.supports_transactional_writes()
    }

    async fn write_root_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> rustok_core::Result<()> {
        self.downstream
            .write_root_in_transaction(transaction, envelope)
            .await
    }

    async fn write_contract_in_transaction(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        envelope: ContractEventEnvelope,
    ) -> rustok_core::Result<()> {
        self.downstream
            .write_contract_in_transaction(transaction, envelope)
            .await
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
    descriptor: ModuleArtifactDescriptor,
}

struct ArtifactEventSubscription {
    installation_id: Uuid,
    binding_id: String,
}

fn subscription_candidate_from_row(
    row: &QueryResult,
    backend: SupportedBackend,
) -> Result<ArtifactSubscriptionCandidate, ArtifactEventDeliveryError> {
    let descriptor = crate::installation_selection::descriptor_with_identity(row)
        .map_err(storage_error)?
        .ok_or(ArtifactEventDeliveryError::SubscriptionStateInvalid)?;
    Ok(ArtifactSubscriptionCandidate {
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(storage_error)?,
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
    tenant_cursor: Arc<std::sync::atomic::AtomicUsize>,
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
            tenant_cursor: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
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
        let mut tenant_ids = self
            .tenants
            .tenant_ids()
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        if tenant_ids.iter().any(Uuid::is_nil) {
            return Err(ModuleWorkError::Source(
                "artifact event tenant source returned a nil tenant id".to_string(),
            ));
        }
        tenant_ids.sort_unstable();
        tenant_ids.dedup();
        if !tenant_ids.is_empty() {
            let start = self.tenant_cursor.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % tenant_ids.len();
            tenant_ids.rotate_left(start);
        }
        let mut first_error = None;
        for tenant_id in tenant_ids {
            let item = match self.queue.claim_next(tenant_id, &self.worker_id).await {
                Ok(item) => item,
                Err(error) => {
                    tracing::error!(%tenant_id, %error, "artifact event delivery claim failed");
                    if first_error.is_none() { first_error = Some(error.to_string()); }
                    continue;
                }
            };
            if let Some(item) = item {
                return Ok(Some(ModuleWorkItem {
                    id: item.delivery_id,
                    tenant_id: item.tenant_id,
                    worker_slug: ARTIFACT_EVENT_DELIVERY_WORKER.to_string(),
                    lease_token: item.claim_token.to_string(),
                    // INVARIANT: EventDeliveryWorkItem contains only serializable fields
                    payload: serde_json::to_value(item).expect("event work item must serialize"),
                }));
            }
        }
        match first_error {
            Some(error) => Err(ModuleWorkError::Source(error)),
            None => Ok(None),
        }
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
        if payload.delivery_id != item.id || payload.tenant_id != item.tenant_id
            || payload.claim_token.to_string() != item.lease_token {
            return Err(ModuleWorkError::Source(
                "artifact event completion identity does not match its payload".to_string(),
            ));
        }
        let outcome = match outcome {
            ModuleWorkOutcome::Completed => ArtifactEventDeliveryOutcome::Succeeded,
            // A policy denial never executed the guest; park the delivery
            // instead of spending its failure-attempt budget.
            ModuleWorkOutcome::Held { message } => ArtifactEventDeliveryOutcome::Held {
                error_code: crate::dispatcher::delivery_error_code(
                    &message,
                    crate::dispatcher::DELIVERY_CODE_DENIED_BY_POLICY,
                ),
            },
            ModuleWorkOutcome::Retryable { message } => ArtifactEventDeliveryOutcome::Retryable {
                error_code: crate::dispatcher::delivery_error_code(&message, "execution_failed"),
            },
            ModuleWorkOutcome::Rejected { message } => ArtifactEventDeliveryOutcome::DeadLetter {
                error_code: crate::dispatcher::delivery_error_code(&message, "execution_rejected"),
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
                    claim_token: payload.claim_token,
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
        if payload.delivery_id != item.id || payload.tenant_id != item.tenant_id
            || payload.claim_token.to_string() != item.lease_token {
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
                admission: crate::ArtifactExecutionAdmission::owner_revalidation(
                    payload.tenant_id,
                    &payload.release,
                    ArtifactInstallationTarget::ExactInstallation {
                        installation_id: payload.installation_id,
                    },
                ),
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
            Err(error) if error.is_policy_denied() => Ok(ModuleWorkOutcome::Held {
                message: error.code().to_string(),
            }),
            Err(error) if error.is_terminal_for(payload.binding.idempotency) => {
                Ok(ModuleWorkOutcome::Rejected {
                    message: error.code().to_string(),
                })
            }
            Err(error) => Ok(ModuleWorkOutcome::Retryable {
                message: error.code().to_string(),
            }),
        }
    }
}

impl SeaOrmArtifactEventDeliveryQueue {
    pub fn new(
        db: DatabaseConnection,
        config: ArtifactEventDeliveryConfig,
    ) -> Result<Self, ArtifactEventDeliveryError> {
        Self::with_infrastructure(db, config, ControlPlaneInfrastructure::default())
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        config: ArtifactEventDeliveryConfig,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Result<Self, ArtifactEventDeliveryError> {
        config.validate()?;
        Ok(Self {
            db,
            config,
            infrastructure,
        })
    }

    pub fn config(&self) -> &ArtifactEventDeliveryConfig {
        &self.config
    }

    /// Creates the binding-scoped projection or returns the original receipt
    /// for an identical source retry in the same tenant. A changed source or
    /// an identity belonging to another tenant is never acknowledged as a
    /// replay, even where the database role bypasses tenant RLS.
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
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
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

        let delivery_id = self.infrastructure.new_id();
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_event_deliveries \
                     (delivery_id, tenant_id, source_event_id, installation_id, binding_id, event_type, \
                      schema_version, payload, source_digest, attempt, status, available_at, created_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, 0, 'pending', {}, {}) \
                     ON CONFLICT (source_event_id, installation_id, binding_id) DO NOTHING",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                    backend.placeholder(7),
                    backend.placeholder(8),
                    backend.placeholder(9),
                    backend.now_sql(),
                    backend.now_sql(),
                ),
                vec![
                    backend.uuid_value(delivery_id),
                    backend.uuid_value(request.tenant_id),
                    backend.uuid_value(request.source.event_id),
                    backend.uuid_value(request.installation_id),
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
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT delivery_id, source_digest FROM module_artifact_event_deliveries \
                     WHERE source_event_id = {} AND installation_id = {} AND binding_id = {} \
                       AND tenant_id = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                vec![
                    backend.uuid_value(request.source.event_id),
                    backend.uuid_value(request.installation_id),
                    request.binding_id.into(),
                    backend.uuid_value(request.tenant_id),
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
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        expire_claims(
            &transaction,
            backend,
            tenant_id,
            self.config.max_attempts,
            self.config.retry_base_seconds,
            self.config.retry_max_seconds,
        )
        .await?;
        let lock = if backend == SupportedBackend::Postgres {
            " FOR UPDATE SKIP LOCKED"
        } else {
            ""
        };
        // A poison candidate (revoked, quarantined, uninstalled, or
        // inactive) is dead-lettered and the scan moves on within the
        // same claim call, bounded so one poll cannot spin forever on
        // a poisoned backlog. (Previously each poison item ended the
        // claim with `Ok(None)`, draining one item per poll and
        // starving the valid work queued behind it.)
        for _skip in 0..MAX_CANDIDATE_SKIPS_PER_CLAIM {
            let candidate = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "SELECT delivery_id, installation_id FROM module_artifact_event_deliveries \
                         WHERE tenant_id = {} AND status = 'pending' AND available_at <= {} \
                         ORDER BY available_at, delivery_id LIMIT 1{lock}",
                        backend.placeholder(1),
                        backend.now_sql(),
                    ),
                    vec![backend.uuid_value(tenant_id)],
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

            // Revalidate security, quarantine, revocation, and uninstallation state before claim
            let validity = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "SELECT \
                            i.slug, i.version, i.payload_digest, \
                            admission.payload_digest AS admission_payload_digest, \
                            CAST(i.descriptor AS TEXT) AS descriptor, \
                            admission.status AS admission_status, \
                            (EXISTS (SELECT 1 FROM module_artifact_uninstall_operations u WHERE u.installation_id = i.installation_id)) AS is_uninstalled, \
                            (EXISTS (SELECT 1 FROM module_artifact_security_states s WHERE s.module_slug = i.slug AND s.module_version = i.version AND s.payload_digest = i.payload_digest AND s.status IN ('quarantined', 'revoked'))) AS is_revoked \
                         FROM module_artifact_installations i \
                         JOIN module_artifact_admissions admission ON admission.installation_id = i.installation_id \
                         WHERE i.installation_id = {}",
                        backend.placeholder(1),
                    ),
                    vec![backend.uuid_value(installation_id)],
                ))
                .await
                .map_err(storage_error)?;

            if let Some(row) = validity {
                let descriptor = crate::installation_selection::descriptor_with_identity(&row)
                    .map_err(storage_error)?;
                if descriptor.is_none() {
                    let error = ArtifactEventDeliveryError::InvalidInstallationIdentity;
                    transaction.rollback().await.map_err(|rollback_error| {
                        storage_error(format!(
                            "{error}; additionally, rolling back the event claim failed: {rollback_error}"
                        ))
                    })?;
                    return Err(error);
                }
                let is_revoked: bool = row.try_get("", "is_revoked").map_err(storage_error)?;
                let is_uninstalled: bool =
                    row.try_get("", "is_uninstalled").map_err(storage_error)?;
                let admission_status: String =
                    row.try_get("", "admission_status").map_err(storage_error)?;

                if is_revoked {
                    transaction
                        .execute_raw(Statement::from_sql_and_values(
                            backend.db_backend(),
                            format!(
                                "UPDATE module_artifact_event_deliveries \
                                 SET status = 'dead_letter', dead_lettered_at = {}, last_error_code = 'revoked_or_quarantined', \
                                     claimed_by = NULL, claimed_until = NULL \
                                 WHERE delivery_id = {} AND tenant_id = {}",
                                backend.now_sql(),
                                backend.placeholder(1),
                                backend.placeholder(2),
                            ),
                            vec![backend.uuid_value(delivery_id), backend.uuid_value(tenant_id)],
                        ))
                        .await
                        .map_err(storage_error)?;
                    continue;
                }

                if is_uninstalled || admission_status != "active" {
                    transaction
                        .execute_raw(Statement::from_sql_and_values(
                            backend.db_backend(),
                            format!(
                                "UPDATE module_artifact_event_deliveries \
                                 SET status = 'dead_letter', dead_lettered_at = {}, last_error_code = 'installation_unavailable', \
                                     claimed_by = NULL, claimed_until = NULL \
                                 WHERE delivery_id = {} AND tenant_id = {}",
                                backend.now_sql(),
                                backend.placeholder(1),
                                backend.placeholder(2),
                            ),
                            vec![backend.uuid_value(delivery_id), backend.uuid_value(tenant_id)],
                        ))
                        .await
                        .map_err(storage_error)?;
                    continue;
                }
            } else {
                // A missing installation or admission cannot establish the
                // immutable identity needed for a terminal security decision.
                transaction.rollback().await.map_err(storage_error)?;
                return Err(ArtifactEventDeliveryError::InvalidInstallationIdentity);
            }

            // This fence is independent of both the attempt budget and any
            // injected deterministic ID source used for durable entity IDs.
            let claim_token = Uuid::new_v4();
            let lease_until = lease_expression(backend, 2);
            let claimed = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "UPDATE module_artifact_event_deliveries \
                         SET status = 'running', attempt = attempt + 1, claimed_by = {}, \
                             claimed_until = {lease_until}, claim_token = {}, last_error_code = NULL \
                         WHERE delivery_id = {} AND tenant_id = {} AND status = 'pending' \
                           AND available_at <= {}",
                        backend.placeholder(1),
                        backend.placeholder(5),
                        backend.placeholder(3),
                        backend.placeholder(4),
                        backend.now_sql(),
                    ),
                    vec![
                        worker_id.to_owned().into(),
                        i64::from(self.config.lease_seconds).into(),
                        backend.uuid_value(delivery_id),
                        backend.uuid_value(tenant_id),
                        backend.uuid_value(claim_token),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if claimed.rows_affected() != 1 {
                // Lost the claim race for this candidate; try the next one.
                continue;
            }
            let row = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "SELECT delivery.delivery_id, delivery.tenant_id, delivery.installation_id, \
                                delivery.binding_id, delivery.source_event_id, delivery.event_type, delivery.schema_version, \
                                delivery.payload, delivery.attempt, installation.slug, installation.version, \
                                installation.payload_digest, admission.payload_digest AS admission_payload_digest, \
                                CAST(installation.descriptor AS TEXT) AS descriptor \
                         FROM module_artifact_event_deliveries delivery \
                         JOIN module_artifact_installations installation \
                           ON installation.installation_id = delivery.installation_id \
                         JOIN module_artifact_admissions admission \
                           ON admission.installation_id = installation.installation_id \
                         WHERE delivery.delivery_id = {} AND delivery.tenant_id = {} \
                           AND delivery.status = 'running' AND delivery.claimed_by = {}",
                        backend.placeholder(1),
                        backend.placeholder(2),
                        backend.placeholder(3),
                    ),
                    vec![
                        backend.uuid_value(delivery_id),
                        backend.uuid_value(tenant_id),
                        worker_id.to_owned().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?
                .ok_or(ArtifactEventDeliveryError::ClaimLost)?;
            let item = match work_item_from_row(&row, backend, claim_token) {
                Ok(item) => item,
                Err(error) => {
                    transaction.rollback().await.map_err(|rollback_error| {
                        storage_error(format!(
                            "{error}; additionally, rolling back the event claim failed: {rollback_error}"
                        ))
                    })?;
                    return Err(error);
                }
            };
            transaction.commit().await.map_err(storage_error)?;
            return Ok(Some(item));
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(None)
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
        if completion.delivery_id.is_nil() || completion.claim_token.is_nil() || completion.attempt == 0 {
            return Err(ArtifactEventDeliveryError::InvalidCompletion);
        }
        if let ArtifactEventDeliveryOutcome::Retryable { error_code }
        | ArtifactEventDeliveryOutcome::Held { error_code }
        | ArtifactEventDeliveryOutcome::DeadLetter { error_code } = &completion.outcome
        {
            validate_error_code(error_code)?;
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        if !crate::delivery_lease::lock_for_completion(
            &transaction, backend, crate::delivery_lease::DeliveryQueue::Event,
            tenant_id, completion.delivery_id,
        ).await.map_err(storage_error)? {
            return Err(ArtifactEventDeliveryError::ClaimLost);
        }
        let claim = EventDeliveryClaim {
            backend,
            tenant_id,
            worker_id,
            delivery_id: completion.delivery_id,
            attempt: completion.attempt,
            claim_token: completion.claim_token,
        };
        let affected = match completion.outcome {
            ArtifactEventDeliveryOutcome::Succeeded => {
                complete_succeeded(&transaction, claim).await?
            }
            ArtifactEventDeliveryOutcome::DeadLetter { error_code } => {
                complete_dead_letter(&transaction, claim, error_code).await?
            }
            ArtifactEventDeliveryOutcome::Held { error_code } => {
                complete_held(
                    &transaction,
                    claim,
                    error_code,
                    self.config.retry_delay_seconds(completion.attempt),
                ).await?
            }
            ArtifactEventDeliveryOutcome::Retryable { error_code } => {
                if completion.attempt >= self.config.max_attempts {
                    complete_dead_letter(&transaction, claim, error_code).await?
                } else {
                    complete_retryable(
                        &transaction,
                        claim,
                        error_code,
                        self.config.retry_delay_seconds(completion.attempt),
                    ).await?
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
    backend: SupportedBackend,
    tenant_id: Uuid,
    installation_id: Uuid,
) -> Result<ModuleArtifactDescriptor, ArtifactEventDeliveryError> {
    // Single canonical implementation shared with the schedule queue; see
    // `delivery_descriptor` for the admissibility contract. (This queue's
    // former copy lacked the retired-work-generation and
    // quarantine/revocation checks.)
    crate::delivery_descriptor::load_admitted_descriptor(
        connection,
        backend,
        tenant_id,
        installation_id,
    )
    .await
    .map_err(|error| match error {
        crate::delivery_descriptor::LoadAdmittedDescriptorError::Unavailable => {
            ArtifactEventDeliveryError::InstallationUnavailable
        }
        crate::delivery_descriptor::LoadAdmittedDescriptorError::InvalidIdentity => {
            ArtifactEventDeliveryError::InvalidInstallationIdentity
        }
        crate::delivery_descriptor::LoadAdmittedDescriptorError::Storage(message) => {
            ArtifactEventDeliveryError::Storage(message)
        }
    })
}

async fn expire_claims<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    tenant_id: Uuid,
    max_attempts: u32,
    retry_base_seconds: u32,
    retry_max_seconds: u32,
) -> Result<(), ArtifactEventDeliveryError> {
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_artifact_event_deliveries \
                 SET status = CASE WHEN attempt >= {} THEN 'dead_letter' ELSE 'pending' END, \
                     available_at = {}, claimed_by = NULL, claimed_until = NULL, \
                     last_error_code = 'lease_expired', \
                     dead_lettered_at = CASE WHEN attempt >= {} THEN {} ELSE NULL END \
                 WHERE tenant_id = {} AND status = 'running' AND claimed_until <= {}",
                backend.placeholder(1),
                retry_available_at_expression(backend, 4, 5),
                backend.placeholder(2),
                backend.now_sql(),
                backend.placeholder(3),
                backend.lease_now_sql(),
            ),
            vec![
                i32::try_from(max_attempts)
                    .map_err(|_| ArtifactEventDeliveryError::InvalidConfiguration)?
                    .into(),
                i32::try_from(max_attempts)
                    .map_err(|_| ArtifactEventDeliveryError::InvalidConfiguration)?
                    .into(),
                backend.uuid_value(tenant_id),
                i64::from(retry_base_seconds).into(),
                i64::from(retry_max_seconds).into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct EventDeliveryClaim<'a> {
    backend: SupportedBackend,
    tenant_id: Uuid,
    worker_id: &'a str,
    delivery_id: Uuid,
    attempt: u32,
    claim_token: Uuid,
}

async fn complete_succeeded<C: ConnectionTrait>(
    connection: &C,
    claim: EventDeliveryClaim<'_>,
) -> Result<u64, ArtifactEventDeliveryError> {
    update_completion(
        connection,
        claim,
        "status = 'succeeded', claimed_by = NULL, claimed_until = NULL, last_error_code = NULL, completed_at = ",
        None,
        claim.backend.now_sql(),
    )
    .await
}

async fn complete_dead_letter<C: ConnectionTrait>(
    connection: &C,
    claim: EventDeliveryClaim<'_>,
    error_code: String,
) -> Result<u64, ArtifactEventDeliveryError> {
    update_completion(
        connection,
        claim,
        "status = 'dead_letter', claimed_by = NULL, claimed_until = NULL, last_error_code = ",
        Some(error_code),
        claim.backend.now_sql(),
    )
    .await
}

async fn complete_retryable<C: ConnectionTrait>(
    connection: &C,
    claim: EventDeliveryClaim<'_>,
    error_code: String,
    delay_seconds: u32,
) -> Result<u64, ArtifactEventDeliveryError> {
    let EventDeliveryClaim {
        backend,
        tenant_id,
        worker_id,
        delivery_id,
        attempt,
        claim_token,
    } = claim;
    let delay = retry_expression(backend, 2);
    let result = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_artifact_event_deliveries \
                 SET status = 'pending', claimed_by = NULL, claimed_until = NULL, \
                     last_error_code = {}, available_at = {delay} \
                 WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' \
                   AND claimed_by = {} AND attempt = {} AND claim_token = {} AND claimed_until > {}",
                backend.placeholder(1),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.lease_now_sql(),
            ),
            vec![
                error_code.into(),
                i64::from(delay_seconds).into(),
                backend.uuid_value(delivery_id),
                backend.uuid_value(tenant_id),
                worker_id.to_owned().into(),
                i64::from(attempt).into(),
                backend.uuid_value(claim_token),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(result.rows_affected())
}

/// Parks a policy-denied delivery: identical requeue to a retryable
/// failure except the claim's attempt increment is reverted, because the
/// guest never executed. `attempt` in the guard is the claimed attempt
/// (>= 1, validated by `complete`), so the decrement cannot underflow.
async fn complete_held<C: ConnectionTrait>(
    connection: &C,
    claim: EventDeliveryClaim<'_>,
    error_code: String,
    delay_seconds: u32,
) -> Result<u64, ArtifactEventDeliveryError> {
    let EventDeliveryClaim {
        backend,
        tenant_id,
        worker_id,
        delivery_id,
        attempt,
        claim_token,
    } = claim;
    let delay = retry_expression(backend, 2);
    let result = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_artifact_event_deliveries \
                 SET status = 'pending', attempt = attempt - 1, claimed_by = NULL, claimed_until = NULL, \
                     last_error_code = {}, available_at = {delay} \
                 WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' \
                   AND claimed_by = {} AND attempt = {} AND claim_token = {} AND claimed_until > {}",
                backend.placeholder(1),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.lease_now_sql(),
            ),
            vec![
                error_code.into(),
                i64::from(delay_seconds).into(),
                backend.uuid_value(delivery_id),
                backend.uuid_value(tenant_id),
                worker_id.to_owned().into(),
                i64::from(attempt).into(),
                backend.uuid_value(claim_token),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(result.rows_affected())
}

async fn update_completion<C: ConnectionTrait>(
    connection: &C,
    claim: EventDeliveryClaim<'_>,
    assignment: &str,
    error_code: Option<String>,
    terminal_timestamp: &str,
) -> Result<u64, ArtifactEventDeliveryError> {
    let EventDeliveryClaim {
        backend,
        tenant_id,
        worker_id,
        delivery_id,
        attempt,
        claim_token,
    } = claim;
    let has_error_code = error_code.is_some();
    let (assignment, values) = match error_code {
        Some(error_code) => (
            format!("{assignment}{}", backend.placeholder(1)),
            vec![error_code.into()],
        ),
        None => (format!("{assignment}{terminal_timestamp}"), Vec::new()),
    };
    let offset = values.len();
    let result = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_artifact_event_deliveries SET {assignment}{} \
                 WHERE delivery_id = {} AND tenant_id = {} AND status = 'running' \
                   AND claimed_by = {} AND attempt = {} AND claim_token = {} AND claimed_until > {}",
                if has_error_code {
                    format!(", dead_lettered_at = {terminal_timestamp}")
                } else {
                    String::new()
                },
                backend.placeholder(offset + 1),
                backend.placeholder(offset + 2),
                backend.placeholder(offset + 3),
                backend.placeholder(offset + 4),
                backend.placeholder(offset + 5),
                backend.lease_now_sql(),
            ),
            {
                let mut values = values;
                values.extend([
                    backend.uuid_value(delivery_id),
                    backend.uuid_value(tenant_id),
                    worker_id.to_owned().into(),
                    i64::from(attempt).into(),
                    backend.uuid_value(claim_token),
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
    backend: SupportedBackend,
    claim_token: Uuid,
) -> Result<ArtifactEventDeliveryWorkItem, ArtifactEventDeliveryError> {
    // A malformed descriptor or identity mismatch is not proof that this
    // delivery is terminally unavailable. Abort the claim transaction so the
    // item stays pending for repair rather than dispatching a different release.
    let descriptor = crate::installation_selection::descriptor_with_identity(row)
        .map_err(storage_error)?
        .ok_or(ArtifactEventDeliveryError::InvalidInstallationIdentity)?;
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
        SupportedBackend::Postgres => {
            i64::from(row.try_get::<i32>("", "attempt").map_err(storage_error)?)
        }
        SupportedBackend::Sqlite => row.try_get::<i64>("", "attempt").map_err(storage_error)?,
    };
    let schema_version = match backend {
        SupportedBackend::Postgres => i64::from(
            row.try_get::<i16>("", "schema_version")
                .map_err(storage_error)?,
        ),
        SupportedBackend::Sqlite => row
            .try_get::<i64>("", "schema_version")
            .map_err(storage_error)?,
    };
    Ok(ArtifactEventDeliveryWorkItem {
        delivery_id: uuid_from_row(row, "delivery_id", backend).map_err(storage_error)?,
        tenant_id: uuid_from_row(row, "tenant_id", backend).map_err(storage_error)?,
        installation_id: uuid_from_row(row, "installation_id", backend).map_err(storage_error)?,
        attempt: u32::try_from(attempt).map_err(|_| ArtifactEventDeliveryError::ClaimLost)?,
        claim_token,
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

fn lease_expression(backend: SupportedBackend, parameter: usize) -> String {
    match backend {
        SupportedBackend::Postgres => format!(
            "clock_timestamp() + ({} * INTERVAL '1 second')",
            backend.placeholder(parameter)
        ),
        SupportedBackend::Sqlite => format!(
            "strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now', '+' || {} || ' seconds')",
            backend.placeholder(parameter)
        ),
    }
}

fn retry_expression(backend: SupportedBackend, parameter: usize) -> String {
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
    #[error("artifact event-delivery installation identity is inconsistent")]
    InvalidInstallationIdentity,
    #[error("artifact event-delivery binding is not admitted for the source event")]
    BindingUnavailable,
    #[error("artifact event subscription state is invalid")]
    SubscriptionStateInvalid,
    #[error("artifact event subscription candidate limit exceeded")]
    SubscriptionCandidateLimitExceeded,
    #[error("artifact event-delivery claim was lost or expired")]
    ClaimLost,
    #[error("artifact event-delivery storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod contract_projection_tests {
    use super::*;
    use rustok_events::InventoryReservationEvent;
    use sea_orm::{Database, DbBackend};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestTenants(Vec<Uuid>);

    #[async_trait]
    impl crate::ArtifactDeliveryTenantSource for TestTenants {
        async fn tenant_ids(&self) -> Result<Vec<Uuid>, crate::ArtifactDeliveryTenantSourceError> {
            Ok(self.0.clone())
        }
    }

    struct FailingTenants;

    #[async_trait]
    impl crate::ArtifactDeliveryTenantSource for FailingTenants {
        async fn tenant_ids(&self) -> Result<Vec<Uuid>, crate::ArtifactDeliveryTenantSourceError> {
            Err(crate::ArtifactDeliveryTenantSourceError::Store("tenant enumeration failed".into()))
        }
    }

    struct ClaimOnlyExecutor;

    #[async_trait]
    impl crate::ArtifactBindingExecutor for ClaimOnlyExecutor {
        fn supports_payload_kind(&self, _: crate::ArtifactPayloadKind) -> bool { true }
        async fn dispatch_binding(&self, _: ArtifactBindingDispatch<'_>) -> Result<Value, crate::ArtifactBindingDispatchError> {
            panic!("claim-only adapter test must not dispatch guest work")
        }
    }

    struct CountingTransport(AtomicUsize);
    #[async_trait]
    impl EventTransport for CountingTransport {
        async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn publish_contract(&self, _envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn reliability_level(&self) -> ReliabilityLevel { ReliabilityLevel::InMemory }
        fn as_any(&self) -> &dyn Any { self }
    }

    fn envelope() -> ContractEventEnvelope {
        ContractEventEnvelope::new(
            Uuid::new_v4(),
            None,
            InventoryReservationEvent::Changed {
                product_id: Uuid::new_v4(),
                variant_id: Uuid::new_v4(),
            },
        )
        .unwrap()
    }

    #[test]
    fn postgres_event_lease_and_retry_use_wall_clock() {
        assert!(lease_expression(SupportedBackend::Postgres, 1).contains("clock_timestamp()"));
        assert!(retry_available_at_expression(SupportedBackend::Postgres, 1, 2).contains("clock_timestamp()"));
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL"]
    async fn postgres_event_lease_is_live_after_transaction_wait() {
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL").expect("test database URL");
        let db = Database::connect(url).await.unwrap();
        let tx = db.begin().await.unwrap();
        tx.execute_unprepared("SELECT pg_sleep(1.2)").await.unwrap();
        let row = tx.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            format!("SELECT {} > clock_timestamp() AS live", lease_expression(SupportedBackend::Postgres, 1)),
            vec![1_i64.into()])).await.unwrap().unwrap();
        assert!(row.try_get::<bool>("", "live").unwrap());
        let row = tx.query_one_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            format!("SELECT {} > clock_timestamp() AS delayed FROM (SELECT 1 AS attempt) candidate", retry_available_at_expression(SupportedBackend::Postgres, 1, 2)),
            vec![1_i64.into(), 1_i64.into()])).await.unwrap().unwrap();
        assert!(row.try_get::<bool>("", "delayed").unwrap());
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn delivery_adapters_rotate_and_isolate_poisoned_tenants() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use crate::{ArtifactScheduleDeliveryConfig, ArtifactScheduleDeliveryRequest,
            ArtifactScheduleDeliveryWorkAdapter, SeaOrmArtifactScheduleDeliveryQueue,
            ARTIFACT_SCHEDULE_DELIVERY_WORKER};
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let fixture = ProjectionFixture::new().await;
        fixture.set_active(fixture.platform_duplicate, false).await;
        fixture.set_active(fixture.tenant_winner, false).await;
        for sql in [
            "CREATE TABLE module_artifact_work_generations (installation_id TEXT, retired INTEGER)",
            "CREATE TABLE module_artifact_security_states (module_slug TEXT, module_version TEXT, payload_digest TEXT, status TEXT)",
            "CREATE TABLE module_artifact_event_deliveries (delivery_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, source_event_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, event_type TEXT NOT NULL, schema_version INTEGER NOT NULL, payload JSON NOT NULL, source_digest TEXT NOT NULL, attempt INTEGER NOT NULL, status TEXT NOT NULL, available_at TEXT NOT NULL, created_at TEXT NOT NULL, claimed_by TEXT, claimed_until TEXT, claim_token TEXT, last_error_code TEXT, completed_at TEXT, dead_lettered_at TEXT, UNIQUE (source_event_id, installation_id, binding_id))",
            "CREATE TABLE module_artifact_schedule_deliveries (delivery_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, schedule_digest TEXT NOT NULL, scheduled_for TEXT NOT NULL, attempt INTEGER NOT NULL, status TEXT NOT NULL, available_at TEXT NOT NULL, created_at TEXT NOT NULL, claimed_by TEXT, claimed_until TEXT, claim_token TEXT, last_error_code TEXT, completed_at TEXT, cancelled_at TEXT, dead_lettered_at TEXT, UNIQUE (tenant_id, installation_id, binding_id, scheduled_for))",
            "CREATE TABLE module_artifact_schedule_cursors (tenant_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, schedule_digest TEXT NOT NULL, materialized_through TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY (tenant_id, installation_id, binding_id))",
        ] { fixture.db.execute_unprepared(sql).await.unwrap(); }
        let event_queue = SeaOrmArtifactEventDeliveryQueue::new(fixture.db.clone(), ArtifactEventDeliveryConfig::default()).unwrap();
        let schedule_queue = SeaOrmArtifactScheduleDeliveryQueue::new(fixture.db.clone(), ArtifactScheduleDeliveryConfig::default()).unwrap();
        let poison = Uuid::from_u128(1);
        let healthy_a = Uuid::from_u128(2);
        let healthy_b = Uuid::from_u128(3);
        for (tenant, installation, count) in [(poison, fixture.other, 1), (healthy_a, fixture.platform, 2), (healthy_b, fixture.platform, 2)] {
            for index in 0..count {
                event_queue.enqueue(ArtifactEventDeliveryRequest {
                    tenant_id: tenant, installation_id: installation, binding_id: "event_handler".into(),
                    source: ArtifactEventDeliverySource { event_id: Uuid::new_v4(), event_type: "sample.event".into(), schema_version: 1, payload: serde_json::json!({}) },
                }).await.unwrap();
                schedule_queue.enqueue(ArtifactScheduleDeliveryRequest {
                    tenant_id: tenant, installation_id: installation, binding_id: "schedule_handler".into(),
                    scheduled_for: chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z").unwrap().with_timezone(&chrono::Utc) + chrono::Duration::minutes(10 * index),
                }).await.unwrap();
            }
        }
        // Leave an already-enqueued poison item: a missing admission must
        // fail only its tenant, without turning that item terminal.
        fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "DELETE FROM module_artifact_admissions WHERE installation_id = ?1", vec![fixture.other.to_string().into()])).await.unwrap();
        let tenants: SharedArtifactDeliveryTenantSource = Arc::new(TestTenants(vec![healthy_b, healthy_a, poison, healthy_a]));
        let executor: SharedArtifactBindingExecutor = Arc::new(ClaimOnlyExecutor);
        let event = ArtifactEventDeliveryWorkAdapter::new(event_queue, executor.clone(), tenants.clone(), "worker".into()).unwrap();
        let schedule = ArtifactScheduleDeliveryWorkAdapter::new(schedule_queue.clone(), executor, tenants, "worker".into()).unwrap();

        async fn exercise(adapter: &dyn ModuleWorkSource, clone: &dyn ModuleWorkSource, worker: &str, a: Uuid, b: Uuid) {
            // Rotation is over sorted/deduplicated tenants. Clones share the
            // cursor, so a continuously busy tenant cannot win every poll.
            for (source, expected) in [(adapter, a), (clone, a), (adapter, b), (clone, b)] {
                let item = source.claim(worker).await.unwrap().expect("healthy tenant work");
                assert_eq!(item.tenant_id, expected);
                source.complete(&item, ModuleWorkOutcome::Completed).await.unwrap();
            }
            let error = adapter.claim(worker).await.expect_err("poison must remain observable when no work is available");
            assert!(matches!(error, ModuleWorkError::Source(_)));
        }
        let event_clone = event.clone();
        exercise(&event, &event_clone, ARTIFACT_EVENT_DELIVERY_WORKER, healthy_a, healthy_b).await;
        let schedule_clone = schedule.clone();
        exercise(&schedule, &schedule_clone, ARTIFACT_SCHEDULE_DELIVERY_WORKER, healthy_a, healthy_b).await;

        // Independently exercise materializer isolation, not just the queue's
        // missing-admission error. Poison cursor initialization for one
        // tenant while another still has a valid durable slot.
        fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "DELETE FROM module_artifact_schedule_cursors WHERE tenant_id = ?1", vec![poison.to_string().into()])).await.unwrap();
        fixture.db.execute_unprepared(&format!(
            "CREATE TRIGGER fail_poison_cursor BEFORE INSERT ON module_artifact_schedule_cursors WHEN NEW.tenant_id = '{poison}' BEGIN SELECT RAISE(ABORT, 'poison materializer'); END"
        )).await.unwrap();
        schedule_queue.enqueue(ArtifactScheduleDeliveryRequest {
            tenant_id: healthy_a, installation_id: fixture.platform, binding_id: "schedule_handler".into(),
            scheduled_for: chrono::DateTime::parse_from_rfc3339("2020-01-02T00:00:00Z").unwrap().with_timezone(&chrono::Utc),
        }).await.unwrap();
        let item = schedule.claim(ARTIFACT_SCHEDULE_DELIVERY_WORKER).await.unwrap().unwrap();
        assert_eq!(item.tenant_id, healthy_a);
        schedule.complete(&item, ModuleWorkOutcome::Completed).await.unwrap();
        let failure = schedule.claim(ARTIFACT_SCHEDULE_DELIVERY_WORKER).await.expect_err("materialization failure must remain observable");
        assert!(failure.to_string().contains("poison materializer"));
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            let row = fixture.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
                format!("SELECT status, attempt FROM {table} WHERE tenant_id = ?1"), vec![poison.to_string().into()])).await.unwrap().unwrap();
            assert_eq!(row.try_get::<String>("", "status").unwrap(), "pending");
            assert_eq!(row.try_get::<i64>("", "attempt").unwrap(), 0);
        }
    }

    #[tokio::test]
    async fn delivery_adapters_reject_invalid_or_unavailable_tenant_enumeration() {
        use crate::{ArtifactScheduleDeliveryConfig, ArtifactScheduleDeliveryWorkAdapter,
            SeaOrmArtifactScheduleDeliveryQueue, ARTIFACT_SCHEDULE_DELIVERY_WORKER};
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let executor: SharedArtifactBindingExecutor = Arc::new(ClaimOnlyExecutor);
        for tenants in [Arc::new(TestTenants(vec![Uuid::nil()])) as SharedArtifactDeliveryTenantSource,
            Arc::new(FailingTenants) as SharedArtifactDeliveryTenantSource] {
            let event = ArtifactEventDeliveryWorkAdapter::new(
                SeaOrmArtifactEventDeliveryQueue::new(db.clone(), ArtifactEventDeliveryConfig::default()).unwrap(),
                executor.clone(), tenants.clone(), "worker".into()).unwrap();
            let schedule = ArtifactScheduleDeliveryWorkAdapter::new(
                SeaOrmArtifactScheduleDeliveryQueue::new(db.clone(), ArtifactScheduleDeliveryConfig::default()).unwrap(),
                executor.clone(), tenants, "worker".into()).unwrap();
            assert!(matches!(event.claim(ARTIFACT_EVENT_DELIVERY_WORKER).await, Err(ModuleWorkError::Source(_))));
            assert!(matches!(schedule.claim(ARTIFACT_SCHEDULE_DELIVERY_WORKER).await, Err(ModuleWorkError::Source(_))));
            // Irrelevant worker names must not enumerate tenants at all.
            assert!(event.claim("other").await.unwrap().is_none());
            assert!(schedule.claim("other").await.unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn artifact_contract_projection_validates_scope_before_subscription_storage() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            db,
            ArtifactEventDeliveryConfig::default(),
        )
        .unwrap();
        let mut wire = serde_json::to_value(envelope()).unwrap();
        wire["tenant_id"] = serde_json::json!(Uuid::nil());
        let invalid: ContractEventEnvelope = serde_json::from_value(wire).unwrap();
        assert!(matches!(
            projector.project_contract(&invalid).await,
            Err(ArtifactEventDeliveryError::InvalidSourceEvent)
        ));
    }

    #[tokio::test]
    async fn event_projection_rejects_incomplete_selection_and_recovers() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use sea_orm::ConnectionTrait;
        use std::collections::BTreeSet;

        async fn ids(
            projector: &SeaOrmArtifactEventSubscriptionProjector,
            tenant: Uuid,
        ) -> BTreeSet<Uuid> {
            projector
                .effective_subscriptions(tenant, "sample.event")
                .await
                .unwrap()
                .into_iter()
                .map(|subscription| subscription.installation_id)
                .collect()
        }

        let fixture = ProjectionFixture::new().await;
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            fixture.db.clone(),
            ArtifactEventDeliveryConfig::default(),
        )
        .unwrap();
        // A corrupt tenant winner cannot become an empty successful projection.
        assert!(matches!(projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionStateInvalid)));
        fixture.repair_tenant().await;
        assert_eq!(ids(&projector, fixture.tenant).await, BTreeSet::from([fixture.other, fixture.tenant_winner]));
        fixture.set_active(fixture.tenant_duplicate, true).await;
        assert!(matches!(projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionStateInvalid)));
        fixture.set_active(fixture.tenant_winner, false).await;
        fixture.set_active(fixture.tenant_duplicate, false).await;
        assert!(matches!(projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionStateInvalid)));
        fixture.set_active(fixture.platform_duplicate, false).await;
        assert_eq!(ids(&projector, fixture.tenant).await, BTreeSet::from([fixture.other, fixture.platform]));
        fixture.set_active(fixture.tenant_winner, true).await;
        fixture.set_tenant_enabled(false).await;
        assert_eq!(ids(&projector, fixture.tenant).await, BTreeSet::from([fixture.other, fixture.platform]));
        fixture.set_tenant_enabled(true).await;
        assert_eq!(ids(&projector, fixture.tenant).await, BTreeSet::from([fixture.other, fixture.tenant_winner]));
        fixture.db.execute_raw(sea_orm::Statement::from_sql_and_values(
            sea_orm::DbBackend::Sqlite,
            "INSERT INTO module_artifact_uninstall_operations VALUES (?1)",
            vec![fixture.tenant_winner.to_string().into()],
        )).await.unwrap();
        assert_eq!(ids(&projector, fixture.tenant).await, BTreeSet::from([fixture.other, fixture.platform]));
    }

    #[tokio::test]
    async fn event_projection_rejects_mismatched_release_identity_before_forwarding() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use rustok_events::DomainEvent;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let fixture = ProjectionFixture::new().await;
        fixture.repair_tenant().await;
        let downstream = Arc::new(CountingTransport(AtomicUsize::new(0)));
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        let transport = ArtifactEventProjectionTransport::new(projector, downstream.clone());
        let root = ControlPlaneInfrastructure::default().event_envelope(
            Some(fixture.tenant), None, DomainEvent::TenantUpdated { tenant_id: fixture.tenant },
        );
        let good_digest = format!("sha256:{}", "a".repeat(64));
        let bad_digest = format!("sha256:{}", "b".repeat(64));
        for (table, column, bad, good) in [
            ("module_artifact_installations", "version", "2.0.0".to_string(), "1.0.0".to_string()),
            ("module_artifact_installations", "payload_digest", bad_digest.clone(), good_digest.clone()),
            ("module_artifact_admissions", "payload_digest", bad_digest.clone(), good_digest.clone()),
        ] {
            let update = format!("UPDATE {table} SET {column} = ?1 WHERE installation_id = ?2");
            fixture.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite, update.clone(),
                vec![bad.into(), fixture.tenant_winner.to_string().into()],
            )).await.unwrap();
            // This event has no matching binding. Identity validation still
            // precedes filtering, so it cannot silently miss a subscription.
            assert!(transport.publish(root.clone()).await.is_err());
            assert_eq!(downstream.0.load(Ordering::SeqCst), 0);
            fixture.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite, update,
                vec![good.into(), fixture.tenant_winner.to_string().into()],
            )).await.unwrap();
        }
        transport.publish(root).await.unwrap();
        assert_eq!(downstream.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn event_delivery_replay_never_acknowledges_another_tenant() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let fixture = ProjectionFixture::new().await;
        // Add the delivery-loader dependencies not needed for selection.
        for sql in [
            "CREATE TABLE module_artifact_work_generations (installation_id TEXT, retired INTEGER)",
            "CREATE TABLE module_artifact_security_states (module_slug TEXT, module_version TEXT, payload_digest TEXT, status TEXT)",
            "CREATE TABLE module_artifact_event_deliveries (delivery_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, source_event_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, event_type TEXT NOT NULL, schema_version INTEGER NOT NULL, payload JSON NOT NULL, source_digest TEXT NOT NULL, attempt INTEGER NOT NULL, status TEXT NOT NULL, available_at TEXT NOT NULL, created_at TEXT NOT NULL, UNIQUE (source_event_id, installation_id, binding_id))",
        ] {
            fixture.db.execute_unprepared(sql).await.unwrap();
        }
        fixture.set_active(fixture.platform_duplicate, false).await;
        fixture.set_active(fixture.tenant_winner, false).await;
        fixture.set_active(fixture.other, false).await;
        let queue = SeaOrmArtifactEventDeliveryQueue::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        let source = ArtifactEventDeliverySource {
            event_id: Uuid::new_v4(), event_type: "sample.event".into(),
            schema_version: 1, payload: serde_json::json!({"value": 1}),
        };
        let request = ArtifactEventDeliveryRequest {
            tenant_id: fixture.tenant, installation_id: fixture.platform,
            binding_id: "event_handler".into(), source: source.clone(),
        };
        let first = queue.enqueue(request.clone()).await.unwrap();
        assert!(first.created);
        let replay = queue.enqueue(request.clone()).await.unwrap();
        assert!(!replay.created);
        assert_eq!(first.delivery_id, replay.delivery_id);
        let mut changed = request.clone();
        changed.source.payload = serde_json::json!({"value": 2});
        assert!(matches!(queue.enqueue(changed).await, Err(ArtifactEventDeliveryError::IdempotencyConflict)));
        let other_tenant = Uuid::new_v4();
        let mut foreign = request;
        foreign.tenant_id = other_tenant;
        assert!(queue.enqueue(foreign).await.is_err());
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        assert!(projector.project_source(other_tenant, source).await.is_err(),
            "a foreign delivery must not make the relay acknowledge this tenant's event");
        let count: i64 = fixture.db.query_one_raw(Statement::from_string(
            DbBackend::Sqlite, "SELECT COUNT(*) AS count FROM module_artifact_event_deliveries",
        )).await.unwrap().unwrap().try_get("", "count").unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn delivery_queues_reject_inconsistent_release_identity_without_losing_work() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use crate::{
            ArtifactScheduleDeliveryConfig, ArtifactScheduleDeliveryError, ArtifactScheduleDeliveryOutcome,
            ArtifactScheduleDeliveryRequest, SeaOrmArtifactScheduleDeliveryQueue,
        };
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let fixture = ProjectionFixture::new().await;
        for sql in [
            "CREATE TABLE module_artifact_work_generations (installation_id TEXT, retired INTEGER)",
            "CREATE TABLE module_artifact_security_states (module_slug TEXT, module_version TEXT, payload_digest TEXT, status TEXT)",
            "CREATE TABLE module_artifact_event_deliveries (delivery_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, source_event_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, event_type TEXT NOT NULL, schema_version INTEGER NOT NULL, payload JSON NOT NULL, source_digest TEXT NOT NULL, attempt INTEGER NOT NULL, status TEXT NOT NULL, available_at TEXT NOT NULL, created_at TEXT NOT NULL, claimed_by TEXT, claimed_until TEXT, claim_token TEXT, last_error_code TEXT, completed_at TEXT, dead_lettered_at TEXT, UNIQUE (source_event_id, installation_id, binding_id))",
            "CREATE TABLE module_artifact_schedule_deliveries (delivery_id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, installation_id TEXT NOT NULL, binding_id TEXT NOT NULL, schedule_digest TEXT NOT NULL, scheduled_for TEXT NOT NULL, attempt INTEGER NOT NULL, status TEXT NOT NULL, available_at TEXT NOT NULL, created_at TEXT NOT NULL, claimed_by TEXT, claimed_until TEXT, claim_token TEXT, last_error_code TEXT, completed_at TEXT, cancelled_at TEXT, dead_lettered_at TEXT, UNIQUE (tenant_id, installation_id, binding_id, scheduled_for))",
        ] {
            fixture.db.execute_unprepared(sql).await.unwrap();
        }
        let event_queue = SeaOrmArtifactEventDeliveryQueue::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        let schedule_queue = SeaOrmArtifactScheduleDeliveryQueue::new(
            fixture.db.clone(), ArtifactScheduleDeliveryConfig::default(),
        ).unwrap();
        let event = ArtifactEventDeliveryRequest {
            tenant_id: fixture.tenant, installation_id: fixture.platform,
            binding_id: "event_handler".into(), source: ArtifactEventDeliverySource {
                event_id: Uuid::new_v4(), event_type: "sample.event".into(),
                schema_version: 1, payload: serde_json::json!({"value": 1}),
            },
        };
        let schedule = ArtifactScheduleDeliveryRequest {
            tenant_id: fixture.tenant, installation_id: fixture.platform,
            binding_id: "schedule_handler".into(),
            scheduled_for: chrono::DateTime::parse_from_rfc3339("2020-01-01T00:00:00Z")
                .unwrap().with_timezone(&chrono::Utc),
        };
        event_queue.enqueue(event.clone()).await.unwrap();
        schedule_queue.enqueue(schedule.clone()).await.unwrap();
        let good_digest = format!("sha256:{}", "a".repeat(64));
        let bad_digest = format!("sha256:{}", "b".repeat(64));
        // The inconsistent version happens to name a quarantined release.
        // Its security status must not terminalize this different delivery.
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_security_states VALUES (?1, ?2, ?3, 'quarantined')",
            vec!["sample_module".into(), "2.0.0".into(), good_digest.clone().into()],
        )).await.unwrap();
        let descriptor: String = fixture.db.query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT descriptor FROM module_artifact_installations WHERE installation_id = ?1",
            vec![fixture.platform.to_string().into()],
        )).await.unwrap().unwrap().try_get("", "descriptor").unwrap();
        for (table, column, bad, good) in [
            ("module_artifact_installations", "descriptor", "invalid JSON".to_string(), descriptor),
            ("module_artifact_installations", "slug", "wrong_module".to_string(), "sample_module".to_string()),
            ("module_artifact_installations", "version", "2.0.0".to_string(), "1.0.0".to_string()),
            ("module_artifact_installations", "payload_digest", bad_digest.clone(), good_digest.clone()),
            ("module_artifact_admissions", "payload_digest", bad_digest.clone(), good_digest.clone()),
        ] {
            let update = format!("UPDATE {table} SET {column} = ?1 WHERE installation_id = ?2");
            fixture.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite, update.clone(), vec![bad.into(), fixture.platform.to_string().into()],
            )).await.unwrap();
            assert!(matches!(event_queue.enqueue(event.clone()).await, Err(ArtifactEventDeliveryError::InvalidInstallationIdentity)));
            assert!(matches!(schedule_queue.enqueue(schedule.clone()).await, Err(ArtifactScheduleDeliveryError::InvalidInstallationIdentity)));
            assert!(matches!(event_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactEventDeliveryError::InvalidInstallationIdentity)));
            assert!(matches!(schedule_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactScheduleDeliveryError::InvalidInstallationIdentity)));
            for deliveries in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
                let row = fixture.db.query_one_raw(Statement::from_string(
                    DbBackend::Sqlite, format!("SELECT status, attempt FROM {deliveries}"),
                )).await.unwrap().unwrap();
                assert_eq!(row.try_get::<String>("", "status").unwrap(), "pending");
                assert_eq!(row.try_get::<i64>("", "attempt").unwrap(), 0);
            }
            fixture.db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite, update, vec![good.into(), fixture.platform.to_string().into()],
            )).await.unwrap();
        }
        // An absent admission is not an inactive installation: both queues
        // must preserve the pending work until its identity can be repaired.
        fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "DELETE FROM module_artifact_admissions WHERE installation_id = ?1",
            vec![fixture.platform.to_string().into()])).await.unwrap();
        assert!(matches!(event_queue.enqueue(event.clone()).await, Err(ArtifactEventDeliveryError::InvalidInstallationIdentity)));
        assert!(matches!(schedule_queue.enqueue(schedule.clone()).await, Err(ArtifactScheduleDeliveryError::InvalidInstallationIdentity)));
        assert!(matches!(event_queue.claim_next(fxture.tenant, "worker").await, Err(ArtifactEventDeliveryError::InvalidInstallationIdentity)));
        assert!(matches!(schedule_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactScheduleDeliveryError::InvalidInstallationIdentity)));
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            let row = fixture.db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
                format!("SELECT status, attempt FROM {table}"))).await.unwrap().unwrap();
            assert_eq!(row.try_get::<String>("", "status").unwrap(), "pending");
            assert_eq!(row.try_get::<i64>("", "attempt").unwrap(), 0);
        }
        fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO module_artifact_admissions (installation_id, status, payload_digest) VALUES (?1, 'active', ?2)",
            vec![fixture.platform.to_string().into(), good_digest.clone().into()])).await.unwrap();
        // Missing installation identity is also repairable corruption, not
        // evidence that the schedule may be terminally cancelled.
        fixture.db.execute_unprepared("CREATE TABLE fixture_saved_installation AS SELECT * FROM module_artifact_installations WHERE 0").await.unwrap();
        fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "INSERT INTO fixture_saved_installation SELECT * FROM module_artifact_installations WHERE installation_id = ?1",
            vec![fixture.platform.to_string().into()])).await.unwrap();
        fixture.db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "DELETE FROM module_artifact_installations WHERE installation_id = ?1",
            vec![fixture.platform.to_string().into()])).await.unwrap();
        assert!(matches!(event_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactEventDeliveryError::InvalidInstallationIdentity)));
        assert!(matches!(schedule_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactScheduleDeliveryError::InvalidInstallationIdentity)));
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            let row = fixture.db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
                format!("SELECT status, attempt FROM {table}"))).await.unwrap().unwrap();
            assert_eq!(row.try_get::<String>("", "status").unwrap(), "pending");
            assert_eq!(row.try_get::<i64>("", "attempt").unwrap(), 0);
        }
        fixture.db.execute_unprepared("INSERT INTO module_artifact_installations SELECT * FROM fixture_saved_installation").await.unwrap();
        // An uninstall marker also cannot turn a corrupt identity into a
        // terminal queue decision.
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_uninstall_operations VALUES (?1)",
            vec![fixture.platform.to_string().into()],
        )).await.unwrap();
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_installations SET version = '2.0.0' WHERE installation_id = ?1",
            vec![fixture.platform.to_string().into()],
        )).await.unwrap();
        assert!(matches!(event_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactEventDeliveryError::InvalidInstallationIdentity)));
        assert!(matches!(schedule_queue.claim_next(fixture.tenant, "worker").await, Err(ArtifactScheduleDeliveryError::InvalidInstallationIdentity)));
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            let row = fixture.db.query_one_raw(Statement::from_string(
                DbBackend::Sqlite, format!("SELECT status, attempt FROM {table}"),
            )).await.unwrap().unwrap();
            assert_eq!(row.try_get::<String>("", "status").unwrap(), "pending");
            assert_eq!(row.try_get::<i64>("", "attempt").unwrap(), 0);
        }
        fixture.db.execute_unprepared("DELETE FROM module_artifact_uninstall_operations").await.unwrap();
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_installations SET version = '1.0.0' WHERE installation_id = ?1",
            vec![fixture.platform.to_string().into()],
        )).await.unwrap();
        let old_event = event_queue.claim_next(fixture.tenant, "worker").await.unwrap().unwrap();
        let old_schedule = schedule_queue.claim_next(fixture.tenant, "worker").await.unwrap().unwrap();
        event_queue.complete(fixture.tenant, "worker", ArtifactEventDeliveryCompletion {
            delivery_id: old_event.delivery_id, attempt: old_event.attempt, claim_token: old_event.claim_token,
            outcome: ArtifactEventDeliveryOutcome::Held { error_code: "held".into() },
        }).await.unwrap();
        schedule_queue.complete(fixture.tenant, "worker", old_schedule.delivery_id, old_schedule.attempt,
            old_schedule.claim_token, ArtifactScheduleDeliveryOutcome::Held { error_code: "held".into() }).await.unwrap();
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            fixture.db.execute_unprepared(&format!("UPDATE {table} SET available_at = '2000-01-01T00:00:00.000+00:00'")).await.unwrap();
        }
        let event_item = event_queue.claim_next(fixture.tenant, "worker").await.unwrap().unwrap();
        let schedule_item = schedule_queue.claim_next(fixture.tenant, "worker").await.unwrap().unwrap();
        assert_eq!(old_event.attempt, event_item.attempt);
        assert_eq!(old_schedule.attempt, schedule_item.attempt);
        assert_ne!(old_event.claim_token, event_item.claim_token);
        assert_ne!(old_schedule.claim_token, schedule_item.claim_token);
        for outcome in [
            ArtifactEventDeliveryOutcome::Succeeded,
            ArtifactEventDeliveryOutcome::DeadLetter { error_code: "failed".into() },
            ArtifactEventDeliveryOutcome::Retryable { error_code: "failed".into() },
            ArtifactEventDeliveryOutcome::Held { error_code: "held".into() },
        ] {
            assert!(matches!(event_queue.complete(fixture.tenant, "worker", ArtifactEventDeliveryCompletion {
                delivery_id: old_event.delivery_id, attempt: old_event.attempt, claim_token: old_event.claim_token, outcome,
            }).await, Err(ArtifactEventDeliveryError::ClaimLost)));
        }
        for outcome in [
            ArtifactScheduleDeliveryOutcome::Succeeded,
            ArtifactScheduleDeliveryOutcome::Cancelled { error_code: "failed".into() },
            ArtifactScheduleDeliveryOutcome::DeadLetter { error_code: "failed".into() },
            ArtifactScheduleDeliveryOutcome::Retryable { error_code: "failed".into() },
            ArtifactScheduleDeliveryOutcome::Held { error_code: "held".into() },
        ] {
            assert!(matches!(schedule_queue.complete(fixture.tenant, "worker", old_schedule.delivery_id,
                old_schedule.attempt, old_schedule.claim_token, outcome).await, Err(ArtifactScheduleDeliveryError::ClaimLost)));
        }
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            fixture.db.execute_unprepared(&format!("UPDATE {table} SET claimed_until = '2000-01-01T00:00:00.000+00:00'")).await.unwrap();
        }
        for outcome in [
            ArtifactEventDeliveryOutcome::Succeeded,
            ArtifactEventDeliveryOutcome::DeadLetter { error_code: "failed".into() },
            ArtifactEventDeliveryOutcome::Retryable { error_code: "failed".into() },
            ArtifactEventDeliveryOutcome::Held { error_code: "held".into() },
        ] {
            assert!(matches!(event_queue.complete(fixture.tenant, "worker", ArtifactEventDeliveryCompletion {
                delivery_id: event_item.delivery_id, attempt: event_item.attempt, claim_token: event_item.claim_token, outcome,
            }).await, Err(ArtifactEventDeliveryError::ClaimLost)));
        }
        for outcome in [
            ArtifactScheduleDeliveryOutcome::Succeeded,
            ArtifactScheduleDeliveryOutcome::Cancelled { error_code: "failed".into() },
            ArtifactScheduleDeliveryOutcome::DeadLetter { error_code: "failed".into() },
            ArtifactScheduleDeliveryOutcome::Retryable { error_code: "failed".into() },
            ArtifactScheduleDeliveryOutcome::Held { error_code: "held".into() },
        ] {
            assert!(matches!(schedule_queue.complete(fixture.tenant, "worker", schedule_item.delivery_id,
                schedule_item.attempt, schedule_item.claim_token, outcome).await, Err(ArtifactScheduleDeliveryError::ClaimLost)));
        }
        for table in ["module_artifact_event_deliveries", "module_artifact_schedule_deliveries"] {
            let row = fixture.db.query_one_raw(Statement::from_string(
                DbBackend::Sqlite, format!("SELECT status, attempt FROM {table}"),
            )).await.unwrap().unwrap();
            assert_eq!(row.try_get::<String>("", "status").unwrap(), "running");
            assert_eq!(row.try_get::<i64>("", "attempt").unwrap(), 1);
            fixture.db.execute_unprepared(&format!("UPDATE {table} SET claimed_until = '2999-01-01T00:00:00.000+00:00'")).await.unwrap();
        }
        event_queue.complete(fixture.tenant, "worker", ArtifactEventDeliveryCompletion {
            delivery_id: event_item.delivery_id, attempt: event_item.attempt, claim_token: event_item.claim_token,
            outcome: ArtifactEventDeliveryOutcome::Succeeded,
        }).await.unwrap();
        schedule_queue.complete(fixture.tenant, "worker", schedule_item.delivery_id,
            schedule_item.attempt, schedule_item.claim_token, ArtifactScheduleDeliveryOutcome::Succeeded).await.unwrap();
    }

    #[tokio::test]
    async fn event_projection_rejects_limit_plus_one_before_publishing() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use rustok_events::DomainEvent;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let fixture = ProjectionFixture::new().await;
        fixture.repair_tenant().await;
        // Four fixture installations are eligible. At the limit the duplicate
        // slug is an invariant error; one more row must report the limit.
        fixture.add_limit_candidates(MAX_EVENT_CANDIDATES_PER_PROJECTION - 4).await;
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        assert!(matches!(projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionStateInvalid)));
        fixture.set_active(fixture.tenant_duplicate, true).await;
        assert!(matches!(projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionCandidateLimitExceeded)));

        let downstream = Arc::new(CountingTransport(AtomicUsize::new(0)));
        let transport = ArtifactEventProjectionTransport::new(projector, downstream.clone());
        let root = ControlPlaneInfrastructure::default().event_envelope(
            Some(fixture.tenant), None, DomainEvent::TenantUpdated { tenant_id: fixture.tenant },
        );
        assert!(transport.publish(root.clone()).await.is_err());
        assert_eq!(downstream.0.load(Ordering::SeqCst), 0);
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_tenant_lifecycle VALUES (?1, ?2, 0)",
            vec![fixture.tenant_duplicate.to_string().into(), fixture.tenant.to_string().into()],
        )).await.unwrap();
        assert!(matches!(transport.projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionStateInvalid)));
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM module_artifact_tenant_lifecycle WHERE installation_id = ?1",
            vec![fixture.tenant_duplicate.to_string().into()],
        )).await.unwrap();
        fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_uninstall_operations VALUES (?1)",
            vec![fixture.tenant_duplicate.to_string().into()],
        )).await.unwrap();
        assert!(matches!(transport.projector.effective_subscriptions(fixture.tenant, "sample.event").await,
            Err(ArtifactEventDeliveryError::SubscriptionStateInvalid)));
        fixture.set_active(fixture.tenant_duplicate, false).await;
        fixture.db.execute_raw(Statement::from_string(DbBackend::Sqlite,
            "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id LIKE 'limit-%'".to_string(),
        )).await.unwrap();
        transport.publish(root).await.unwrap();
        assert_eq!(downstream.0.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn incomplete_projection_blocks_both_transport_paths_until_repaired() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use rustok_events::DomainEvent;

        let fixture = ProjectionFixture::new().await;
        let downstream = Arc::new(CountingTransport(AtomicUsize::new(0)));
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        let transport = ArtifactEventProjectionTransport::new(projector, downstream.clone());
        let root = ControlPlaneInfrastructure::default().event_envelope(Some(fixture.tenant), None, DomainEvent::TenantUpdated { tenant_id: fixture.tenant });
        let contract = ContractEventEnvelope::new(fixture.tenant, None, InventoryReservationEvent::Changed {
            product_id: Uuid::new_v4(), variant_id: Uuid::new_v4(),
        }).unwrap();

        assert!(transport.publish(root.clone()).await.is_err());
        assert!(transport.publish_contract(contract.clone()).await.is_err());
        assert_eq!(downstream.0.load(Ordering::SeqCst), 0);
        fixture.repair_tenant().await;
        // Neither event type matches the fixture bindings, so no queue tables
        // are needed to verify forwarding after repair.
        transport.publish(root.clone()).await.unwrap();
        transport.publish_contract(contract.clone()).await.unwrap();
        assert_eq!(downstream.0.load(Ordering::SeqCst), 2);
        fixture.set_active(fixture.tenant_duplicate, true).await;
        assert!(transport.publish(root).await.is_err());
        assert!(transport.publish_contract(contract).await.is_err());
        assert_eq!(downstream.0.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn relay_dead_letters_incomplete_projection_instead_of_dispatching() {
        use crate::installation_selection::test_support::ProjectionFixture;
        use rustok_events::DomainEvent;
        use rustok_outbox::{entity, OutboxRelay, RelayConfig, SysEventsMigration};
        use sea_orm::{EntityTrait, Set};
        use sea_orm_migration::{MigrationTrait, SchemaManager};

        let fixture = ProjectionFixture::new().await;
        SysEventsMigration.up(&SchemaManager::new(&fixture.db)).await.unwrap();
        let root = ControlPlaneInfrastructure::default().event_envelope(Some(fixture.tenant), None, DomainEvent::TenantUpdated { tenant_id: fixture.tenant });
        let event_id = root.id;
        entity::Entity::insert(entity::ActiveModel {
            id: Set(event_id),
            event_type: Set(root.event_type.clone()),
            schema_version: Set(root.schema_version as i16),
            payload: Set(serde_json::to_value(root).unwrap()),
            status: Set(entity::SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(chrono::Utc::now()),
            dispatched_at: Set(None),
        }).exec_without_returning(&fixture.db).await.unwrap();
        let downstream = Arc::new(CountingTransport(AtomicUsize::new(0)));
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            fixture.db.clone(), ArtifactEventDeliveryConfig::default(),
        ).unwrap();
        let transport = Arc::new(ArtifactEventProjectionTransport::new(projector, downstream.clone()));
        let relay = OutboxRelay::new(fixture.db.clone(), transport).with_config(RelayConfig {
            max_attempts: 1,
            worker_id: "projection-test".to_string(),
            ..RelayConfig::default()
        });
        assert_eq!(relay.process_pending_once(Some(1)).await.unwrap(), 1);
        let event = entity::Entity::find_by_id(event_id).one(&fixture.db).await.unwrap().unwrap();
        assert_eq!(event.status, entity::SysEventStatus::Failed);
        assert_eq!(event.retry_count, 1);
        assert!(event.last_error.as_deref().is_some_and(|message| message.contains("artifact event subscription state is invalid")));
        assert!(event.dispatched_at.is_none());
        assert_eq!(downstream.0.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn bounded_fact_does_not_skip_the_durable_artifact_subscription_projection() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let projector = SeaOrmArtifactEventSubscriptionProjector::new(
            db,
            ArtifactEventDeliveryConfig::default(),
        )
        .unwrap();
        // No artifact tables exist: the real subscription read must refuse, not acknowledge
        // an unwired bounded family by returning zero subscriptions without a read.
        assert!(matches!(
            projector.project_contract(&envelope()).await,
            Err(ArtifactEventDeliveryError::Storage(_))
        ));
    }
}
