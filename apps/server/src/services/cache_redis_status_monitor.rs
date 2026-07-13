use std::time::Duration;

use rustok_cache::{CacheService, RedisCacheStatus};

use crate::services::server_runtime_context::ServerRuntimeContext;

const CACHE_REDIS_STATUS_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct CacheRedisStatusMonitorHandle;

pub async fn start_cache_redis_status_monitor(
    ctx: &ServerRuntimeContext,
    cache: CacheService,
) {
    if ctx
        .shared_get::<CacheRedisStatusMonitorHandle>()
        .is_some()
    {
        return;
    }

    let initial = cache.redis_status().await;
    log_transition(None, &initial);

    if cache.redis_configuration_present() {
        tokio::spawn(async move {
            let mut previous = initial;
            let mut interval = tokio::time::interval(CACHE_REDIS_STATUS_INTERVAL);
            // The initial probe above already populated the collector.
            interval.tick().await;
            loop {
                interval.tick().await;
                let current = cache.redis_status().await;
                log_transition(Some(&previous), &current);
                previous = current;
            }
        });
    }

    ctx.shared_insert(CacheRedisStatusMonitorHandle);
}

fn log_transition(previous: Option<&RedisCacheStatus>, current: &RedisCacheStatus) {
    let state = state_tuple(current);
    if previous.is_some_and(|previous| state_tuple(previous) == state) {
        return;
    }

    if current.is_degraded() {
        tracing::warn!(
            redis_url_present = current.url_present,
            redis_client_initialized = current.client_initialized,
            redis_connectivity_healthy = current.connectivity_healthy,
            "Cache Redis lifecycle entered degraded state"
        );
    } else {
        tracing::info!(
            redis_url_present = current.url_present,
            redis_client_initialized = current.client_initialized,
            redis_connectivity_healthy = current.connectivity_healthy,
            "Cache Redis lifecycle state changed"
        );
    }
}

fn state_tuple(status: &RedisCacheStatus) -> (bool, bool, bool) {
    (
        status.url_present,
        status.client_initialized,
        status.connectivity_healthy,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_transition_ignores_error_text_and_credentials() {
        let first = RedisCacheStatus {
            url_present: true,
            client_initialized: true,
            connectivity_healthy: false,
            last_error: Some("secret-a".to_string()),
        };
        let second = RedisCacheStatus {
            last_error: Some("secret-b".to_string()),
            ..first.clone()
        };
        assert_eq!(state_tuple(&first), state_tuple(&second));
    }

    #[test]
    fn healthy_memory_only_state_is_not_degraded() {
        let status = RedisCacheStatus::default();
        assert!(!status.is_degraded());
        assert_eq!(state_tuple(&status), (false, false, false));
    }
}
