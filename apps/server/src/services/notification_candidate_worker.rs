use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustok_core::{ModuleRegistry, ModuleRuntimeExtensions};
use rustok_notifications::{
    DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE, NotificationCandidatePolicyDeferral,
    NotificationCandidateWorkItem, NotificationCandidateWorker, NotificationError,
    NotificationRecipientPolicyRuntime,
};
use sea_orm::DatabaseConnection;
use tokio::task::JoinHandle;

use crate::error::{Error, Result};
use crate::services::app_lifecycle::StopHandle;
use crate::services::effective_module_policy::EffectiveModulePolicyService;
use crate::services::server_runtime_context::ServerRuntimeContext;

const CANDIDATE_POLL_INTERVAL: Duration = Duration::from_millis(500);
const NOTIFICATIONS_MODULE_SLUG: &str = "notifications";
static NOTIFICATION_CANDIDATE_WORKER_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TenantNotificationPolicy {
    Enabled,
    Disabled,
    Unavailable,
}

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
}

pub fn start_notification_candidate_worker_if_ready(ctx: &ServerRuntimeContext) -> Result<()> {
    if !ctx.settings().runtime.runs_background_workers()
        || ctx.shared_contains::<NotificationCandidateWorkerHandle>()
    {
        return Ok(());
    }

    let extensions = ctx
        .shared_get::<Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| Error::Message("module runtime extensions are unavailable".to_string()))?;
    let Some(policy_runtime) = extensions
        .get::<NotificationRecipientPolicyRuntime>()
        .cloned()
    else {
        tracing::info!("Notification candidate worker disabled: recipient policy runtime is absent");
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
        Error::Message("notification source registry is unavailable for candidate worker".to_string())
    })?;
    let module_registry = ctx
        .shared_get::<ModuleRegistry>()
        .ok_or_else(|| Error::Message("module registry is unavailable".to_string()))?;

    if !ctx.shared_contains::<StopHandle>() {
        let (stop_handle, _stop_rx) = StopHandle::new();
        ctx.shared_insert(stop_handle);
    }
    let stop_rx = ctx
        .shared_get::<StopHandle>()
        .expect("StopHandle must be registered before notification candidate worker startup")
        .subscribe();

    let instance_id = NOTIFICATION_CANDIDATE_WORKER_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let worker_id = format!("notification-candidate-{instance_id}");
    let worker = NotificationCandidateWorker::new(
        ctx.db_clone(),
        registry,
        policy_runtime.policy_arc(),
        worker_id,
        DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE,
    )
    .map_err(|error| Error::Message(format!("notification candidate worker is invalid: {error}")))?;

    tracing::info!(
        instance_id,
        batch_size = worker.batch_size(),
        "Starting notification candidate worker"
    );
    ctx.shared_insert(NotificationCandidateWorkerHandle {
        instance_id,
        _handle: tokio::spawn(notification_candidate_worker_loop(
            worker,
            ctx.db_clone(),
            module_registry,
            stop_rx,
        )),
    });
    Ok(())
}

async fn notification_candidate_worker_loop(
    worker: NotificationCandidateWorker,
    db: DatabaseConnection,
    module_registry: ModuleRegistry,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *stop_rx.borrow() {
            tracing::info!(worker_id = worker.worker_id(), "Notification candidate worker stopped");
            return;
        }

        let work_items = match worker.claimable_candidate_work().await {
            Ok(work_items) => work_items,
            Err(error) => {
                tracing::error!(
                    worker_id = worker.worker_id(),
                    error_code = error.stable_code(),
                    retryable = error.is_retryable(),
                    error = %error,
                    "Notification candidate worker failed to select claimable items"
                );
                Vec::new()
            }
        };

        for work in work_items {
            // A shutdown signal prevents future claims. A candidate already being
            // processed is allowed to finish its lease/CAS completion path.
            if *stop_rx.borrow() {
                tracing::info!(worker_id = worker.worker_id(), "Notification candidate worker stopped before next claim");
                return;
            }

            if !candidate_work_is_enabled(&worker, &db, &module_registry, work).await {
                continue;
            }

            match worker.process_candidate(work.item_id).await {
                Ok(result) => tracing::debug!(
                    worker_id = worker.worker_id(),
                    tenant_id = %work.tenant_id,
                    item_id = %result.item_id,
                    status = ?result.status,
                    replayed = result.replayed,
                    "Notification candidate processed"
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
                    error = %error,
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

async fn candidate_work_is_enabled(
    worker: &NotificationCandidateWorker,
    db: &DatabaseConnection,
    module_registry: &ModuleRegistry,
    work: NotificationCandidateWorkItem,
) -> bool {
    let reason = match tenant_notification_policy(db, module_registry, work.tenant_id).await {
        TenantNotificationPolicy::Enabled => return true,
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
                error = %error,
                "Notification candidate tenant-policy deferral failed"
            );
        }
    }
    false
}

async fn tenant_notification_policy(
    db: &DatabaseConnection,
    module_registry: &ModuleRegistry,
    tenant_id: uuid::Uuid,
) -> TenantNotificationPolicy {
    match EffectiveModulePolicyService::is_enabled(
        db,
        module_registry,
        tenant_id,
        NOTIFICATIONS_MODULE_SLUG,
    )
    .await
    {
        Ok(true) => TenantNotificationPolicy::Enabled,
        Ok(false) => {
            tracing::debug!(
                tenant_id = %tenant_id,
                module_slug = NOTIFICATIONS_MODULE_SLUG,
                "Notification candidate skipped because tenant capability is disabled"
            );
            TenantNotificationPolicy::Disabled
        }
        Err(error) => {
            tracing::warn!(
                tenant_id = %tenant_id,
                module_slug = NOTIFICATIONS_MODULE_SLUG,
                error = %error,
                "Notification candidate policy lookup failed closed"
            );
            TenantNotificationPolicy::Unavailable
        }
    }
}
