use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use rustok_cache::{
    record_tenant_generation_listener_metrics, TenantGenerationListenerMetrics,
};
use tokio::sync::RwLock;

const MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TenantCacheGenerationListenerStatus {
    Disabled,
    Starting,
    Healthy,
    Degraded,
}

impl TenantCacheGenerationListenerStatus {
    pub fn metric_value(self) -> i64 {
        match self {
            Self::Disabled => 0,
            Self::Starting => 1,
            Self::Healthy => 2,
            Self::Degraded => 3,
        }
    }

    fn from_metric_value(value: i64) -> Self {
        match value {
            1 => Self::Starting,
            2 => Self::Healthy,
            3 => Self::Degraded,
            _ => Self::Disabled,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantCacheGenerationListenerSnapshot {
    pub status: TenantCacheGenerationListenerStatus,
    pub last_error: Option<String>,
    pub redis_required: bool,
    pub local_ready: bool,
    pub subscriber_ready: bool,
    pub reconciliation_healthy: bool,
}

impl TenantCacheGenerationListenerSnapshot {
    pub fn disabled(reason: impl Into<String>) -> Self {
        Self {
            status: TenantCacheGenerationListenerStatus::Disabled,
            last_error: Some(bounded_listener_error(reason.into())),
            redis_required: false,
            local_ready: false,
            subscriber_ready: false,
            reconciliation_healthy: false,
        }
    }
}

#[derive(Debug)]
pub struct TenantCacheGenerationListenerState {
    redis_required: bool,
    local_ready: AtomicBool,
    subscriber_ready: AtomicBool,
    reconciliation_healthy: AtomicBool,
    local_degraded: AtomicBool,
    subscriber_degraded: AtomicBool,
    reconciliation_degraded: AtomicBool,
    last_error: RwLock<Option<String>>,
}

impl TenantCacheGenerationListenerState {
    pub fn new(redis_required: bool) -> Arc<Self> {
        let state = Arc::new(Self {
            redis_required,
            local_ready: AtomicBool::new(false),
            subscriber_ready: AtomicBool::new(false),
            reconciliation_healthy: AtomicBool::new(!redis_required),
            local_degraded: AtomicBool::new(false),
            subscriber_degraded: AtomicBool::new(false),
            reconciliation_degraded: AtomicBool::new(false),
            last_error: RwLock::new(None),
        });
        state.publish_metrics();
        state
    }

    pub async fn mark_local_healthy(&self) {
        self.local_ready.store(true, Ordering::Release);
        self.local_degraded.store(false, Ordering::Release);
        self.clear_error_if_recovered().await;
        self.publish_metrics();
    }

    pub async fn mark_local_degraded(&self, error: impl Into<String>) {
        self.local_degraded.store(true, Ordering::Release);
        self.record_error(error).await;
    }

    pub fn mark_subscriber_starting(&self) {
        self.subscriber_ready.store(false, Ordering::Release);
        self.publish_metrics();
    }

    /// The SUBSCRIBE ready hook also performs a durable generation read, so this transition can
    /// safely recover both the subscriber and reconciliation components.
    pub async fn mark_subscriber_ready_after_recovery(&self) {
        self.subscriber_ready.store(true, Ordering::Release);
        self.subscriber_degraded.store(false, Ordering::Release);
        self.reconciliation_healthy.store(true, Ordering::Release);
        self.reconciliation_degraded.store(false, Ordering::Release);
        self.clear_error_if_recovered().await;
        self.publish_metrics();
    }

    /// Successful message handling proves subscriber activity only. It must not mask an
    /// independent durable reconciliation failure.
    pub async fn mark_subscriber_activity_healthy(&self) {
        self.subscriber_ready.store(true, Ordering::Release);
        self.subscriber_degraded.store(false, Ordering::Release);
        self.clear_error_if_recovered().await;
        self.publish_metrics();
    }

    pub async fn mark_reconciliation_healthy(&self) {
        self.reconciliation_healthy.store(true, Ordering::Release);
        self.reconciliation_degraded.store(false, Ordering::Release);
        self.clear_error_if_recovered().await;
        self.publish_metrics();
    }

    pub async fn mark_subscriber_degraded(&self, error: impl Into<String>) {
        self.subscriber_ready.store(false, Ordering::Release);
        self.subscriber_degraded.store(true, Ordering::Release);
        self.record_error(error).await;
    }

    pub async fn mark_reconciliation_degraded(&self, error: impl Into<String>) {
        self.reconciliation_healthy.store(false, Ordering::Release);
        self.reconciliation_degraded.store(true, Ordering::Release);
        self.record_error(error).await;
    }

    async fn record_error(&self, error: impl Into<String>) {
        *self.last_error.write().await = Some(bounded_listener_error(error.into()));
        self.publish_metrics();
    }

    async fn clear_error_if_recovered(&self) {
        if !self.any_component_degraded() {
            *self.last_error.write().await = None;
        }
    }

    fn any_component_degraded(&self) -> bool {
        self.local_degraded.load(Ordering::Acquire)
            || self.subscriber_degraded.load(Ordering::Acquire)
            || self.reconciliation_degraded.load(Ordering::Acquire)
    }

    fn components(&self) -> TenantGenerationListenerMetrics {
        let local_ready = self.local_ready.load(Ordering::Acquire);
        let subscriber_ready = self.subscriber_ready.load(Ordering::Acquire);
        let reconciliation_healthy = self.reconciliation_healthy.load(Ordering::Acquire);
        let ready = if self.redis_required {
            subscriber_ready && reconciliation_healthy
        } else {
            local_ready
        };
        let status = if self.any_component_degraded() {
            TenantCacheGenerationListenerStatus::Degraded
        } else if ready {
            TenantCacheGenerationListenerStatus::Healthy
        } else {
            TenantCacheGenerationListenerStatus::Starting
        };

        TenantGenerationListenerMetrics {
            status: status.metric_value(),
            local_ready,
            subscriber_ready,
            reconciliation_healthy,
        }
    }

    fn publish_metrics(&self) {
        record_tenant_generation_listener_metrics(self.components());
    }

    pub async fn snapshot(&self) -> TenantCacheGenerationListenerSnapshot {
        let metrics = self.components();
        TenantCacheGenerationListenerSnapshot {
            status: TenantCacheGenerationListenerStatus::from_metric_value(metrics.status),
            last_error: self.last_error.read().await.clone(),
            redis_required: self.redis_required,
            local_ready: metrics.local_ready,
            subscriber_ready: metrics.subscriber_ready,
            reconciliation_healthy: metrics.reconciliation_healthy,
        }
    }
}

fn bounded_listener_error(error: String) -> String {
    if error.len() <= MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES {
        return error;
    }

    let mut boundary = MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES;
    while !error.is_char_boundary(boundary) {
        boundary -= 1;
    }
    format!("{}…", &error[..boundary])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn redis_health_requires_subscriber_and_reconciliation() {
        let state = TenantCacheGenerationListenerState::new(true);
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Starting
        );

        state.mark_reconciliation_healthy().await;
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Starting
        );

        state.mark_subscriber_ready_after_recovery().await;
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Healthy
        );

        state.mark_subscriber_degraded("subscriber closed").await;
        state.mark_reconciliation_healthy().await;
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Degraded
        );
    }

    #[tokio::test]
    async fn subscriber_activity_does_not_hide_reconciliation_failure() {
        let state = TenantCacheGenerationListenerState::new(true);
        state.mark_subscriber_ready_after_recovery().await;
        state
            .mark_reconciliation_degraded("generation store unavailable")
            .await;
        state.mark_subscriber_activity_healthy().await;

        let snapshot = state.snapshot().await;
        assert_eq!(snapshot.status, TenantCacheGenerationListenerStatus::Degraded);
        assert!(!snapshot.reconciliation_healthy);
        assert_eq!(
            snapshot.last_error.as_deref(),
            Some("generation store unavailable")
        );
    }

    #[tokio::test]
    async fn reconciliation_success_does_not_hide_subscriber_failure() {
        let state = TenantCacheGenerationListenerState::new(true);
        state.mark_subscriber_ready_after_recovery().await;
        state.mark_subscriber_degraded("subscriber closed").await;
        state.mark_reconciliation_healthy().await;

        let snapshot = state.snapshot().await;
        assert_eq!(snapshot.status, TenantCacheGenerationListenerStatus::Degraded);
        assert!(!snapshot.subscriber_ready);
        assert_eq!(snapshot.last_error.as_deref(), Some("subscriber closed"));
    }

    #[tokio::test]
    async fn local_only_listener_becomes_healthy_after_recovery() {
        let state = TenantCacheGenerationListenerState::new(false);
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Starting
        );
        state.mark_local_healthy().await;
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Healthy
        );
    }

    #[tokio::test]
    async fn degraded_snapshot_error_is_bounded() {
        let state = TenantCacheGenerationListenerState::new(true);
        state.mark_subscriber_degraded("é".repeat(1_024)).await;
        let degraded = state.snapshot().await;
        assert_eq!(degraded.status, TenantCacheGenerationListenerStatus::Degraded);
        assert!(degraded
            .last_error
            .as_deref()
            .is_some_and(|error| error.len() <= MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES + 3));
    }

    #[tokio::test]
    async fn independent_runtime_states_do_not_overwrite_each_other() {
        let local = TenantCacheGenerationListenerState::new(false);
        let redis = TenantCacheGenerationListenerState::new(true);

        local.mark_local_healthy().await;
        redis.mark_subscriber_degraded("redis unavailable").await;

        assert_eq!(
            local.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Healthy
        );
        assert_eq!(
            redis.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Degraded
        );
    }

    #[test]
    fn status_metric_values_are_stable() {
        assert_eq!(TenantCacheGenerationListenerStatus::Disabled.metric_value(), 0);
        assert_eq!(TenantCacheGenerationListenerStatus::Starting.metric_value(), 1);
        assert_eq!(TenantCacheGenerationListenerStatus::Healthy.metric_value(), 2);
        assert_eq!(TenantCacheGenerationListenerStatus::Degraded.metric_value(), 3);
    }
}
