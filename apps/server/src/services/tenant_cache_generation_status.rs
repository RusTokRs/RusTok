use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

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

#[derive(Debug)]
struct TenantCacheGenerationListenerState {
    initialized: AtomicBool,
    redis_required: AtomicBool,
    local_ready: AtomicBool,
    subscriber_ready: AtomicBool,
    reconciliation_healthy: AtomicBool,
    degraded: AtomicBool,
    last_error: RwLock<Option<String>>,
}

impl TenantCacheGenerationListenerState {
    fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            redis_required: AtomicBool::new(false),
            local_ready: AtomicBool::new(false),
            subscriber_ready: AtomicBool::new(false),
            reconciliation_healthy: AtomicBool::new(false),
            degraded: AtomicBool::new(false),
            last_error: RwLock::new(Some(
                "tenant cache generation listener is not initialized".to_string(),
            )),
        }
    }

    async fn initialize(&self, redis_required: bool) {
        self.redis_required
            .store(redis_required, Ordering::Release);
        self.local_ready.store(false, Ordering::Release);
        self.subscriber_ready.store(false, Ordering::Release);
        self.reconciliation_healthy
            .store(!redis_required, Ordering::Release);
        self.degraded.store(false, Ordering::Release);
        self.initialized.store(true, Ordering::Release);
        *self.last_error.write().await = None;
    }

    async fn mark_local_healthy(&self) {
        self.local_ready.store(true, Ordering::Release);
        if !self.redis_required.load(Ordering::Acquire) {
            self.degraded.store(false, Ordering::Release);
            *self.last_error.write().await = None;
        }
    }

    async fn mark_subscriber_starting(&self) {
        self.subscriber_ready.store(false, Ordering::Release);
    }

    async fn mark_subscriber_healthy(&self) {
        self.subscriber_ready.store(true, Ordering::Release);
        self.reconciliation_healthy.store(true, Ordering::Release);
        self.degraded.store(false, Ordering::Release);
        *self.last_error.write().await = None;
    }

    async fn mark_reconciliation_healthy(&self) {
        self.reconciliation_healthy.store(true, Ordering::Release);
        if self.subscriber_ready.load(Ordering::Acquire) {
            self.degraded.store(false, Ordering::Release);
            *self.last_error.write().await = None;
        }
    }

    async fn mark_degraded(&self, error: impl Into<String>) {
        self.degraded.store(true, Ordering::Release);
        *self.last_error.write().await = Some(bounded_listener_error(error.into()));
    }

    async fn mark_subscriber_degraded(&self, error: impl Into<String>) {
        self.subscriber_ready.store(false, Ordering::Release);
        self.mark_degraded(error).await;
    }

    async fn mark_reconciliation_degraded(&self, error: impl Into<String>) {
        self.reconciliation_healthy.store(false, Ordering::Release);
        self.mark_degraded(error).await;
    }

    async fn snapshot(&self) -> TenantCacheGenerationListenerSnapshot {
        let initialized = self.initialized.load(Ordering::Acquire);
        let redis_required = self.redis_required.load(Ordering::Acquire);
        let local_ready = self.local_ready.load(Ordering::Acquire);
        let subscriber_ready = self.subscriber_ready.load(Ordering::Acquire);
        let reconciliation_healthy = self.reconciliation_healthy.load(Ordering::Acquire);
        let degraded = self.degraded.load(Ordering::Acquire);
        let status = if !initialized {
            TenantCacheGenerationListenerStatus::Disabled
        } else if degraded {
            TenantCacheGenerationListenerStatus::Degraded
        } else if if redis_required {
            subscriber_ready && reconciliation_healthy
        } else {
            local_ready
        } {
            TenantCacheGenerationListenerStatus::Healthy
        } else {
            TenantCacheGenerationListenerStatus::Starting
        };

        TenantCacheGenerationListenerSnapshot {
            status,
            last_error: self.last_error.read().await.clone(),
            redis_required,
            local_ready,
            subscriber_ready,
            reconciliation_healthy,
        }
    }
}

fn state() -> &'static Arc<TenantCacheGenerationListenerState> {
    static STATE: OnceLock<Arc<TenantCacheGenerationListenerState>> = OnceLock::new();
    STATE.get_or_init(|| Arc::new(TenantCacheGenerationListenerState::new()))
}

pub async fn initialize_tenant_cache_generation_listener(redis_required: bool) {
    state().initialize(redis_required).await;
}

pub async fn mark_tenant_cache_generation_local_healthy() {
    state().mark_local_healthy().await;
}

pub async fn mark_tenant_cache_generation_subscriber_starting() {
    state().mark_subscriber_starting().await;
}

pub async fn mark_tenant_cache_generation_subscriber_healthy() {
    state().mark_subscriber_healthy().await;
}

pub async fn mark_tenant_cache_generation_subscriber_degraded(error: impl Into<String>) {
    state().mark_subscriber_degraded(error).await;
}

pub async fn mark_tenant_cache_generation_reconciliation_healthy() {
    state().mark_reconciliation_healthy().await;
}

pub async fn mark_tenant_cache_generation_reconciliation_degraded(error: impl Into<String>) {
    state().mark_reconciliation_degraded(error).await;
}

pub async fn mark_tenant_cache_generation_listener_degraded(error: impl Into<String>) {
    state().mark_degraded(error).await;
}

pub async fn tenant_cache_generation_listener_snapshot(
) -> TenantCacheGenerationListenerSnapshot {
    state().snapshot().await
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
        let state = TenantCacheGenerationListenerState::new();
        state.initialize(true).await;
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Starting
        );

        state.mark_reconciliation_healthy().await;
        assert_eq!(
            state.snapshot().await.status,
            TenantCacheGenerationListenerStatus::Starting
        );

        state.mark_subscriber_healthy().await;
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
    async fn local_only_listener_becomes_healthy_after_recovery() {
        let state = TenantCacheGenerationListenerState::new();
        state.initialize(false).await;
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
        let state = TenantCacheGenerationListenerState::new();
        state.initialize(true).await;
        state.mark_degraded("é".repeat(1_024)).await;
        let degraded = state.snapshot().await;
        assert_eq!(degraded.status, TenantCacheGenerationListenerStatus::Degraded);
        assert!(degraded
            .last_error
            .as_deref()
            .is_some_and(|error| error.len() <= MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES + 3));
    }

    #[test]
    fn status_metric_values_are_stable() {
        assert_eq!(TenantCacheGenerationListenerStatus::Disabled.metric_value(), 0);
        assert_eq!(TenantCacheGenerationListenerStatus::Starting.metric_value(), 1);
        assert_eq!(TenantCacheGenerationListenerStatus::Healthy.metric_value(), 2);
        assert_eq!(TenantCacheGenerationListenerStatus::Degraded.metric_value(), 3);
    }
}
