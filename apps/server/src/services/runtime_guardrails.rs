use crate::services::cache_redis_status_monitor::CacheRedisStatusMonitorHandle;
use crate::services::channel_cache_invalidation::ChannelCacheInvalidationListenerHandle;
use crate::services::event_bus::EventForwarderHandle;
use crate::services::field_definition_cache::{
    FieldDefinitionCacheGenerationReconciliationHandle, FieldDefinitionCacheInvalidationHandle,
};
use crate::services::rbac_cache_invalidation::RbacCacheInvalidationListenerHandle;
use crate::services::rbac_invalidation_generation::RbacInvalidationGenerationWatchdogHandle;
#[cfg(feature = "mod-seo")]
use crate::services::seo_redirect_cache_reconciliation::{
    SeoRedirectCacheReconciliationHandle, seo_redirect_cache_reconciliation_required,
};
use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::tenant_locale_generation::TenantLocaleGenerationListenerHandle;

mod base {
    include!("runtime_guardrails_base.rs");
}

pub use base::{
    EventBusGuardrailSnapshot, EventTransportGuardrailSnapshot, RateLimitGuardrailSnapshot,
    RateLimitPolicySnapshot, RemoteExecutorGuardrailSnapshot, RuntimeGuardrailRollout,
    RuntimeGuardrailSnapshot, RuntimeGuardrailStatus,
};

pub async fn collect_runtime_guardrail_snapshot(
    ctx: &ServerRuntimeContext,
) -> RuntimeGuardrailSnapshot {
    let mut snapshot = base::collect_runtime_guardrail_snapshot(ctx).await;
    if !snapshot.runtime_dependencies_enabled {
        return snapshot;
    }

    observe_worker(
        &mut snapshot,
        "event bus transport forwarder",
        ctx.shared_get::<EventForwarderHandle>()
            .map(|handle| handle.is_running()),
        RuntimeGuardrailStatus::Critical,
    );
    observe_worker(
        &mut snapshot,
        "tenant locale durable generation runtime",
        ctx.shared_get::<TenantLocaleGenerationListenerHandle>()
            .map(|handle| handle.is_ready()),
        RuntimeGuardrailStatus::Critical,
    );
    observe_worker(
        &mut snapshot,
        "channel resolution durable invalidation runtime",
        ctx.shared_get::<ChannelCacheInvalidationListenerHandle>()
            .map(|handle| handle.is_ready()),
        RuntimeGuardrailStatus::Critical,
    );
    #[cfg(feature = "mod-seo")]
    if seo_redirect_cache_reconciliation_required(ctx) {
        observe_worker(
            &mut snapshot,
            "SEO redirect durable cache reconciliation",
            ctx.shared_get::<SeoRedirectCacheReconciliationHandle>()
                .map(|handle| handle.is_ready()),
            RuntimeGuardrailStatus::Critical,
        );
    }
    observe_worker(
        &mut snapshot,
        "Flex field-definition durable cache reconciliation",
        ctx.shared_get::<FieldDefinitionCacheGenerationReconciliationHandle>()
            .map(|handle| handle.is_ready()),
        RuntimeGuardrailStatus::Critical,
    );
    observe_worker(
        &mut snapshot,
        "RBAC cache invalidation runtime",
        ctx.shared_get::<RbacCacheInvalidationListenerHandle>()
            .map(|handle| handle.is_running()),
        RuntimeGuardrailStatus::Critical,
    );
    observe_worker(
        &mut snapshot,
        "RBAC durable generation watchdog",
        ctx.shared_get::<RbacInvalidationGenerationWatchdogHandle>()
            .map(|handle| handle.is_running()),
        RuntimeGuardrailStatus::Critical,
    );
    observe_worker(
        &mut snapshot,
        "cache Redis status monitor",
        ctx.shared_get::<CacheRedisStatusMonitorHandle>()
            .map(|handle| handle.is_running()),
        RuntimeGuardrailStatus::Degraded,
    );
    observe_worker(
        &mut snapshot,
        "field definition cache invalidation consumer",
        ctx.shared_get::<FieldDefinitionCacheInvalidationHandle>()
            .map(|handle| handle.is_running()),
        RuntimeGuardrailStatus::Degraded,
    );
    apply_rollout_status(&mut snapshot);
    snapshot
}

fn observe_worker(
    snapshot: &mut RuntimeGuardrailSnapshot,
    name: &'static str,
    running: Option<bool>,
    severity: RuntimeGuardrailStatus,
) {
    match running {
        Some(true) => {}
        Some(false) => escalate_snapshot(snapshot, severity, format!("{name} task has stopped")),
        None => escalate_snapshot(snapshot, severity, format!("{name} handle is missing")),
    }
}

fn escalate_snapshot(
    snapshot: &mut RuntimeGuardrailSnapshot,
    severity: RuntimeGuardrailStatus,
    reason: String,
) {
    if guardrail_rank(severity) > guardrail_rank(snapshot.observed_status) {
        snapshot.observed_status = severity;
    }
    snapshot.reasons.push(reason);
}

fn apply_rollout_status(snapshot: &mut RuntimeGuardrailSnapshot) {
    snapshot.status = match snapshot.rollout {
        RuntimeGuardrailRollout::Enforce => snapshot.observed_status,
        RuntimeGuardrailRollout::Observe => {
            if snapshot.observed_status == RuntimeGuardrailStatus::Ok {
                RuntimeGuardrailStatus::Ok
            } else {
                RuntimeGuardrailStatus::Degraded
            }
        }
    };
}

fn guardrail_rank(status: RuntimeGuardrailStatus) -> u8 {
    match status {
        RuntimeGuardrailStatus::Ok => 0,
        RuntimeGuardrailStatus::Degraded => 1,
        RuntimeGuardrailStatus::Critical => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(rollout: RuntimeGuardrailRollout) -> RuntimeGuardrailSnapshot {
        RuntimeGuardrailSnapshot {
            status: RuntimeGuardrailStatus::Ok,
            observed_status: RuntimeGuardrailStatus::Ok,
            rollout,
            host_mode: "full".to_string(),
            runtime_dependencies_enabled: true,
            reasons: Vec::new(),
            rate_limits: Vec::new(),
            event_bus: EventBusGuardrailSnapshot {
                backpressure_enabled: false,
                current_depth: 0,
                max_depth: 0,
                state: RuntimeGuardrailStatus::Ok,
                events_rejected: 0,
                warning_count: 0,
                critical_count: 0,
            },
            event_transport: EventTransportGuardrailSnapshot {
                relay_fallback_active: false,
                channel_capacity: 0,
            },
            remote_executor: RemoteExecutorGuardrailSnapshot {
                enabled: false,
                token_configured: false,
                lease_ttl_ms: 0,
                requeue_scan_interval_ms: 0,
                active_claims: 0,
                expired_claims: 0,
                state: RuntimeGuardrailStatus::Ok,
            },
        }
    }

    #[test]
    fn terminal_critical_worker_respects_rollout_mode() {
        let mut enforce = snapshot(RuntimeGuardrailRollout::Enforce);
        observe_worker(
            &mut enforce,
            "RBAC cache invalidation runtime",
            Some(false),
            RuntimeGuardrailStatus::Critical,
        );
        apply_rollout_status(&mut enforce);
        assert_eq!(enforce.observed_status, RuntimeGuardrailStatus::Critical);
        assert_eq!(enforce.status, RuntimeGuardrailStatus::Critical);

        let mut observe = snapshot(RuntimeGuardrailRollout::Observe);
        observe_worker(
            &mut observe,
            "RBAC cache invalidation runtime",
            None,
            RuntimeGuardrailStatus::Critical,
        );
        apply_rollout_status(&mut observe);
        assert_eq!(observe.observed_status, RuntimeGuardrailStatus::Critical);
        assert_eq!(observe.status, RuntimeGuardrailStatus::Degraded);
    }

    #[test]
    fn noncritical_worker_does_not_lower_existing_severity() {
        let mut snapshot = snapshot(RuntimeGuardrailRollout::Enforce);
        escalate_snapshot(
            &mut snapshot,
            RuntimeGuardrailStatus::Critical,
            "existing critical condition".to_string(),
        );
        observe_worker(
            &mut snapshot,
            "cache Redis status monitor",
            Some(false),
            RuntimeGuardrailStatus::Degraded,
        );
        apply_rollout_status(&mut snapshot);
        assert_eq!(snapshot.observed_status, RuntimeGuardrailStatus::Critical);
        assert_eq!(snapshot.status, RuntimeGuardrailStatus::Critical);
        assert_eq!(snapshot.reasons.len(), 2);
    }
}
