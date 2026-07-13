use std::sync::atomic::{AtomicU8, Ordering};
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
    fn as_u8(self) -> u8 {
        match self {
            Self::Disabled => 0,
            Self::Starting => 1,
            Self::Healthy => 2,
            Self::Degraded => 3,
        }
    }

    fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Starting,
            2 => Self::Healthy,
            3 => Self::Degraded,
            _ => Self::Disabled,
        }
    }

    pub fn metric_value(self) -> i64 {
        i64::from(self.as_u8())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantCacheGenerationListenerSnapshot {
    pub status: TenantCacheGenerationListenerStatus,
    pub last_error: Option<String>,
}

#[derive(Debug)]
struct TenantCacheGenerationListenerState {
    status: AtomicU8,
    last_error: RwLock<Option<String>>,
}

impl TenantCacheGenerationListenerState {
    fn new() -> Self {
        Self {
            status: AtomicU8::new(TenantCacheGenerationListenerStatus::Disabled.as_u8()),
            last_error: RwLock::new(Some(
                "tenant cache generation listener is not initialized".to_string(),
            )),
        }
    }

    async fn set(
        &self,
        status: TenantCacheGenerationListenerStatus,
        last_error: Option<String>,
    ) {
        self.status.store(status.as_u8(), Ordering::Release);
        *self.last_error.write().await = last_error.map(bounded_listener_error);
    }

    async fn snapshot(&self) -> TenantCacheGenerationListenerSnapshot {
        TenantCacheGenerationListenerSnapshot {
            status: TenantCacheGenerationListenerStatus::from_u8(
                self.status.load(Ordering::Acquire),
            ),
            last_error: self.last_error.read().await.clone(),
        }
    }
}

fn state() -> &'static Arc<TenantCacheGenerationListenerState> {
    static STATE: OnceLock<Arc<TenantCacheGenerationListenerState>> = OnceLock::new();
    STATE.get_or_init(|| Arc::new(TenantCacheGenerationListenerState::new()))
}

pub async fn mark_tenant_cache_generation_listener_starting() {
    state()
        .set(TenantCacheGenerationListenerStatus::Starting, None)
        .await;
}

pub async fn mark_tenant_cache_generation_listener_healthy() {
    state()
        .set(TenantCacheGenerationListenerStatus::Healthy, None)
        .await;
}

pub async fn mark_tenant_cache_generation_listener_degraded(error: impl Into<String>) {
    state()
        .set(
            TenantCacheGenerationListenerStatus::Degraded,
            Some(error.into()),
        )
        .await;
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
    async fn degraded_snapshot_is_bounded_and_recovers_to_healthy() {
        mark_tenant_cache_generation_listener_degraded("é".repeat(1_024)).await;
        let degraded = tenant_cache_generation_listener_snapshot().await;
        assert_eq!(degraded.status, TenantCacheGenerationListenerStatus::Degraded);
        assert!(degraded
            .last_error
            .as_deref()
            .is_some_and(|error| error.len() <= MAX_TENANT_GENERATION_LISTENER_ERROR_BYTES + 3));

        mark_tenant_cache_generation_listener_healthy().await;
        let healthy = tenant_cache_generation_listener_snapshot().await;
        assert_eq!(healthy.status, TenantCacheGenerationListenerStatus::Healthy);
        assert!(healthy.last_error.is_none());
    }

    #[test]
    fn status_metric_values_are_stable() {
        assert_eq!(TenantCacheGenerationListenerStatus::Disabled.metric_value(), 0);
        assert_eq!(TenantCacheGenerationListenerStatus::Starting.metric_value(), 1);
        assert_eq!(TenantCacheGenerationListenerStatus::Healthy.metric_value(), 2);
        assert_eq!(TenantCacheGenerationListenerStatus::Degraded.metric_value(), 3);
    }
}
