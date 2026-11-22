use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
use rustok_modules::ModuleControlPlane;
use rustok_notifications::{
    DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE, NotificationCandidatePolicyDeferral,
    NotificationCandidateWorkItem, NotificationCandidateWorker, NotificationError,
    NotificationRecipientPolicyRuntime, NotificationTenantCapabilityCommitDecision,
    NotificationTenantCapabilityCommitError, NotificationTenantCapabilityCommitGuard,
    NotificationTenantCapabilityCommitRequest,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::DirectWorkerRegistrationLock;
use crate::services::server_runtime_context::published_worker_running;
use crate::services::stop_handle::{StopHandle, may_admit_next_worker_operation};
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::server_runtime_context::{ServerRuntimeContext, ServerRuntimeStartReservation};

const CANDIDATE_POLL_INTERVAL: Duration = Duration::from_millis(500);
const NOTIFICATIONS_MODULE_SLUG: &str = "notifications";
const MODULE_LIFECYCLE_POLICY_CONSUMER: &str = "module.lifecycle";
static NOTIFICATION_CANDIDATE_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, PartialEq)]
struct TenantPolicyObservation {
    policy_revision: String,
    default_enabled_modules: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TenantNotificationPolicy {
    Enabled(TenantPolicyObservation),
    Disabled,
    Unavailable,
}

#[derive(Clone)]
struct ServerNotificationTenantCapabilityCommitGuard {
    db: DatabaseConnection,
    module_registry: ModuleRegistry,
}

#[async_trait]
impl NotificationTenantCapabilityCommitGuard for ServerNotificationTenantCapabilityCommitGuard {
    async fn evaluate(
        &self,
        transaction: &DatabaseTransaction,
        request: NotificationTenantCapabilityCommitRequest,
    ) -> Result<NotificationTenantCapabilityCommitDecision, NotificationTenantCapabilityCommitError>
    {
        if request.tenant_id.is_nil() || request.module_slug != NOTIFICATIONS_MODULE_SLUG {
            return Err(NotificationTenantCapabilityCommitError::permanent());
        }

        // Manifest defaults were captured before candidate claim. Keeping them in
        // the observation avoids opening another pool connection while the final
        // transaction is active. Manifest mutation is intentionally outside the
        // lifecycle cursor guarantee and remains a separate rollout gate.
        let policy = ModuleControlPlane::new(self.db.clone())
            .policy_revision_consumer()
            .lock_and_resolve_static_policy_in_transaction(
                transaction,
                request.tenant_id,
                MODULE_LIFECYCLE_POLICY_CONSUMER,
                &self.module_registry,
                request.observed_default_enabled_modules,
            )
            .await
            .map_err(|_| NotificationTenantCapabilityCommitError::retryable())?;

        if !policy.contains(request.module_slug.as_str()) {
            return Ok(NotificationTenantCapabilityCommitDecision::Disabled);
        }
        if policy.policy_revision() != request.observed_policy_revision {
            return Ok(NotificationTenantCapabilityCommitDecision::RevisionChanged);
        }
        Ok(NotificationTenantCapabilityCommitDecision::Allow)
    }
}

// Marker owns the entire local bootstrap window, not just the JoinHandle slot.
// It is distinct from the published worker handle so callers cannot race
// while dependencies are being materialized.
struct NotificationCandidateWorkerStartupReservation;

pub struct NotificationCandidateWorkerHandle {
    instance_id: u64,
    _handle: JoinHandle<()>,
}

impl NotificationCandidateWorkerHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self._handle.is_finished()
    }

    /// Await a durable worker's current operation after the shutdown signal.
    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self._handle.await
    }

    pub(crate) fn into_task(self) -> JoinHandle<()> {
        self._handle
    }
}

pub fn start_notification_candidate_worker_if_ready(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers() {
        return Ok(());
    }
    if published_worker_running::<NotificationCandidateWorkerHandle>(
        ctx,
        "Notification candidate worker",
        NotificationCandidateWorkerHandle::is_finished,
    )? {
        return Ok(());
    }

    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| Error::Message("module runtime extensions are unavailable".to_string()))?;
    let Some(policy_runtime) = extensions
        .get::<NotificationRecipientPolicyRuntime>()
        .cloned()
    else {
        tracing::info!(
            "Notification candidate worker disabled: recipient policy runtime is absent"
        );
        return Ok(());
    };

    if !policy_runtime.candidate_worker_enabled() {
        tracing::info!("Notification candidate worker disabled by explicit runtime flag");
        return Ok(());
    }
    if !policy_runtime.relation_ports_ready() {
        tracing::warn!(
            "Notification candidate worker not started: recipient relation ports are not ready"
        );
        return Ok(());
    }
    if !policy_runtime.candidate_worker_ready() {
        return Ok(());
    }

    let registry = rustok_notifications::api::notification_source_registry_from_extensions(
        extensions.as_ref(),
    )
    .ok_or_else(|| {
        Error::Message(
            "notification source registry is unavailable for candidate worker".to_string(),
        )
    })?;
    let module_registry = ctx
        .shared_get::<ModuleRegistry>()
        .ok_or_else(|| Error::Message("module registry is unavailable".to_string()))?;

    let stop = StopHandle::ensure(ctx);
    if stop.is_stopping() {
        return Err(Error::Message(
            "notification candidate worker cannot start during server shutdown".to_string(),
        ));
    }
    let Some(reservation) =
        ServerRuntimeStartReservation::try_claim(ctx, NotificationCandidateWorkerStartupReservation)
    else {
        // Another caller is still preparing this worker; a successful return
        // would falsely tell bootstrap it is running if that starter fails.
        if published_worker_running::<NotificationCandidateWorkerHandle>(
            ctx,
            "Notification candidate worker",
            NotificationCandidateWorkerHandle::is_finished,
        )? {
            return Ok(());
        }
        return Err(Error::Message(
            "Notification candidate worker startup is already in progress; retry".to_string(),
        ));
    };
    // Another bootstrap may have committed the worker before this reservation
    // was created by an older caller. Never overwrite an existing JoinHandle.
    if published_worker_running::<NotificationCandidateWorkerHandle>(
        ctx,
        "Notification candidate worker",
        NotificationCandidateWorkerHandle::is_finished,
    )? {
        return Ok(());
    }
    let stop_rx = stop.subscribe();

    let instance_id = NOTIFICATION_CANDIDATE_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let worker_id = notification_candidate_worker_id(instance_id);
    let commit_guard = Arc::new(ServerNotificationTenantCapabilityCommitGuard {
        db: ctx.db_clone(),
        module_registry: module_registry.clone(),
    });
    let worker = NotificationCandidateWorker::new_with_commit_guard(
        ctx.db_clone(),
        registry,
        policy_runtime.policy_arc(),
        commit_guard,
        worker_id,
        DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE,
    )
    .map_err(|error| {
        Error::Message(format!("notification candidate worker is invalid: {error}"))
    })?;

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        "Starting notification candidate worker"
    );
    // The startup reservation fences competing initializers. This separate
    // registration gate closes the last stop-check -> spawn -> publish race
    // against shutdown taking the JoinHandle.
    let registration = DirectWorkerRegistrationLock::ensure(ctx);
    let _registration_guard = registration
        .0
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if stop.is_stopping() {
        return Err(Error::Message(
            "notification candidate worker startup interrupted by server shutdown".to_string(),
        ));
    }
    ctx.shared_insert(NotificationCandidateWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_candidate_worker_loop(
            worker,
            ctx.db_clone(),
            module_registry,
            stop_rx,
        )),
    });
    reservation.commit();
    Ok(())
}

fn notification_candidate_worker_id(instance_id: u64) -> String {
    format!("notification-candidate-{instance_id}-{}", uuid::Uuid::new_v4())
}

async fn notification_candidate_worker_loop(
    worker: NotificationCandidateWorker,
    db: DatabaseConnection,
    module_registry: ModuleRegistry,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(
                worker_id = worker.worker_id(),
                "Notification candidate worker stopped"
            );
            return;
        }

        let work_items = match worker.claimable_candidate_work().await {
            Ok(work_items) => work_items,
            Err(error) => {
                tracing::error!(
                    worker_id = worker.worker_id(),
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    "Notification candidate worker failed to select claimable items"
                );
                Vec::new()
            }
        };

        for work in work_items {
            if *stop_rx.borrow() {
                tracing::info!(
                    worker_id = worker.worker_id(),
                    "Notification candidate worker stopped before next claim"
                );
                return;
            }

            let observation = candidate_work_policy_observation(
                &worker,
                &db,
                &module_registry,
                work,
                &stop_rx,
            )
            .await;
            if !may_admit_next_worker_operation(&stop_rx) {
                tracing::info!(
                    worker_id = worker.worker_id(),
                    "Notification candidate worker stopped after policy observation"
                );
                return;
            }
            let Some(observation) = observation else {
                continue;
            };

            match worker
                .process_candidate_with_policy_revision(
                    work.item_id,
                    observation.policy_revision.as_str(),
                    observation.default_enabled_modules.as_slice(),
                )
                .await
            {
                Ok(result) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    item_id = %result.item_id,
                    status = ?result.status,
                    replayed = result.replayed,
                    policy_revision = observation.policy_revision,
                    "Notification candidate processed under commit-time tenant policy guard"
                ),
                Err(NotificationError::LeaseUnavailable) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    item_id = %work.item_id,
                    "Notification candidate claim lost to another worker"
                ),
                Err(error) => tracing::warn!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    item_id = %work.item_id,
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    "Notification candidate processing completed with durable failure state"
                ),
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(CANDIDATE_POLL_INTERVAL) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    tracing::info!(worker_id = worker.worker_id(), "Notification candidate worker received shutdown signal");
                    return;
                }
            }
        }
    }
}

async fn candidate_work_policy_observation(
    worker: &NotificationCandidateWorker,
    db: &DatabaseConnection,
    module_registry: &ModuleRegistry,
    work: NotificationCandidateWorkItem,
    stop_rx: &tokio::sync::watch::Receiver<bool>,
) -> Option<TenantPolicyObservation> {
    let policy = tenant_notification_policy(db, module_registry, work.tenant_id).await;
    // A disabled/unavailable policy normally causes a durable deferral. Do
    // not start that transition if the policy read crossed host shutdown.
    if !may_admit_next_worker_operation(stop_rx) {
        return None;
    }
    let reason = match policy {
        TenantNotificationPolicy::Enabled(observation) => return Some(observation),
        TenantNotificationPolicy::Disabled => NotificationCandidatePolicyDeferral::TenantDisabled,
        TenantNotificationPolicy::Unavailable => {
            NotificationCandidatePolicyDeferral::PolicyUnavailable
        }
    };

    match worker.defer_candidate(work, reason).await {
        Ok(()) => {
            tracing::debug!(
                worker_id = worker.worker_id(),
                tenant_id = %work.tenant_id,
                item_id = %work.item_id,
                reason = ?reason,
                "Notification candidate deferred before recipient policy evaluation"
            );
        }
        Err(NotificationError::LeaseUnavailable) => {
            tracing::debug!(
                worker_id = worker.worker_id(),
                tenant_id = %work.tenant_id,
                item_id = %work.item_id,
                "Notification candidate tenant-policy deferral lost to another claim"
            );
        }
        Err(error) => {
            tracing::warn!(
                worker_id = worker.worker_id(),
                tenant_id = %work.tenant_id,
                item_id = %work.item_id,
                error_code = error.stable_code(),
                retryable = error.is_retryable(),
                "Notification candidate tenant-policy deferral failed"
            );
        }
    }
    None
}

async fn tenant_notification_policy(
    db: &DatabaseConnection,
    module_registry: &ModuleRegistry,
    tenant_id: uuid::Uuid,
) -> TenantNotificationPolicy {
    match EffectiveModulePolicyService::resolve_snapshot(db, module_registry, tenant_id).await {
        Ok(snapshot) if snapshot.policy.contains(NOTIFICATIONS_MODULE_SLUG) => {
            TenantNotificationPolicy::Enabled(TenantPolicyObservation {
                policy_revision: snapshot.policy.policy_revision().to_string(),
                default_enabled_modules: snapshot.default_enabled_modules,
            })
        }
        Ok(_) => {
            tracing::debug!(
                tenant_id = %tenant_id,
                module_slug = NOTIFICATIONS_MODULE_SLUG,
                "Notification candidate skipped because tenant capability is disabled"
            );
            TenantNotificationPolicy::Disabled
        }
        Err(_error) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                module_slug = NOTIFICATIONS_MODULE_SLUG,
                error_code = "notification_tenant_policy_lookup_failed",
                "Notification candidate policy lookup failed closed"
            );
            TenantNotificationPolicy::Unavailable
        }
    }
}

#[cfg(test)]
mod worker_identity_tests {
    use super::notification_candidate_worker_id;

    #[test]
    fn candidate_lease_owner_ids_are_unique_even_when_local_instance_ids_match() {
        let first = notification_candidate_worker_id(1);
        let second = notification_candidate_worker_id(1);

        assert_ne!(first, second);
        assert!(first.starts_with("notification-candidate-1-"));
        assert!(first.len() <= 191, "owner identity must fit the module contract");
    }
}
