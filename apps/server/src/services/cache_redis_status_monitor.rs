use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use rustok_cache::{CacheService, RedisCacheStatus};
use tokio::task::JoinHandle;

use crate::services::server_runtime_context::ServerRuntimeContext;

const CACHE_REDIS_STATUS_INTERVAL: Duration = Duration::from_secs(10);
const CACHE_REDIS_STATUS_RESTART_DELAY: Duration = Duration::from_secs(1);
const CACHE_REDIS_STATUS_MONITOR_CHANNEL: &str = "cache.redis.status";

struct CacheRedisStatusMonitorRuntime {
    task: Option<JoinHandle<()>>,
}

impl CacheRedisStatusMonitorRuntime {
    fn is_running(&self) -> bool {
        self.task.as_ref().is_none_or(|task| !task.is_finished())
    }

    fn abort(&self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

impl Drop for CacheRedisStatusMonitorRuntime {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

#[derive(Clone)]
pub struct CacheRedisStatusMonitorHandle(Arc<CacheRedisStatusMonitorRuntime>);

impl CacheRedisStatusMonitorHandle {
    fn new(task: Option<JoinHandle<()>>) -> Self {
        Self(Arc::new(CacheRedisStatusMonitorRuntime { task }))
    }

    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    fn abort(&self) {
        self.0.abort();
    }

    #[cfg(test)]
    fn abort_for_test(&self) {
        self.abort();
    }
}

#[derive(Clone, Default)]
struct CacheRedisStatusMonitorStartLock(Arc<tokio::sync::Mutex<()>>);

pub async fn start_cache_redis_status_monitor(ctx: &ServerRuntimeContext, cache: CacheService) {
    let _ = ctx.shared_insert_if_absent(CacheRedisStatusMonitorStartLock::default());
    let Some(start_lock) = ctx.shared_get::<CacheRedisStatusMonitorStartLock>() else {
        tracing::error!("Cache Redis status monitor start lock is unavailable");
        return;
    };
    let _start_guard = start_lock.0.lock().await;

    if let Some(existing) = ctx.shared_get::<CacheRedisStatusMonitorHandle>() {
        if existing.is_running() {
            return;
        }
        tracing::warn!("Cache Redis status monitor stopped; replacing runtime");
        existing.abort();
    }

    let initial = cache.redis_status().await;
    log_transition(None, &initial);

    let task = if cache.redis_configuration_present() {
        let first_previous = Arc::new(Mutex::new(Some(initial)));
        Some(tokio::spawn(supervise_cache_redis_status_worker(
            move || {
                let cache = cache.clone();
                let previous = first_previous
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .take();
                async move {
                    let previous = match previous {
                        Some(previous) => previous,
                        None => {
                            let current = cache.redis_status().await;
                            log_transition(None, &current);
                            current
                        }
                    };
                    run_cache_redis_status_monitor(cache, previous).await;
                }
            },
            CACHE_REDIS_STATUS_RESTART_DELAY,
        )))
    } else {
        None
    };

    ctx.shared_insert(CacheRedisStatusMonitorHandle::new(task));
}

async fn run_cache_redis_status_monitor(cache: CacheService, mut previous: RedisCacheStatus) {
    let mut interval = tokio::time::interval(CACHE_REDIS_STATUS_INTERVAL);
    // The startup or restart probe already populated the collector.
    interval.tick().await;
    loop {
        interval.tick().await;
        let current = cache.redis_status().await;
        log_transition(Some(&previous), &current);
        previous = current;
    }
}

async fn supervise_cache_redis_status_worker<F, Fut>(mut worker_factory: F, restart_delay: Duration)
where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    loop {
        let worker = match std::panic::catch_unwind(AssertUnwindSafe(&mut worker_factory)) {
            Ok(worker) => worker,
            Err(_) => {
                record_monitor_restart("factory_panicked");
                tokio::time::sleep(restart_delay).await;
                continue;
            }
        };
        let outcome = AssertUnwindSafe(worker).catch_unwind().await;
        if outcome.is_err() {
            record_monitor_restart("worker_panicked");
        } else {
            record_monitor_restart("worker_exited");
        }
        tokio::time::sleep(restart_delay).await;
    }
}

fn record_monitor_restart(reason: &'static str) {
    tracing::error!(reason, "Cache Redis status monitor stopped; restarting");
    rustok_telemetry::metrics::record_event_error(CACHE_REDIS_STATUS_MONITOR_CHANNEL, reason);
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
    use std::sync::atomic::{AtomicUsize, Ordering};

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

    #[tokio::test]
    async fn monitor_handle_reports_terminal_tasks() {
        let handle = CacheRedisStatusMonitorHandle::new(Some(tokio::spawn(async {
            std::future::pending::<()>().await;
        })));
        assert!(handle.is_running());
        handle.abort_for_test();
        tokio::task::yield_now().await;
        assert!(!handle.is_running());
    }

    #[tokio::test]
    async fn monitor_supervisor_restarts_after_worker_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = Arc::clone(&attempts);
        let supervisor = tokio::spawn(supervise_cache_redis_status_worker(
            move || {
                let attempt = worker_attempts.fetch_add(1, Ordering::SeqCst);
                async move {
                    if attempt == 0 {
                        panic!("Redis status monitor regression fixture");
                    }
                    std::future::pending::<()>().await;
                }
            },
            Duration::from_millis(1),
        ));

        tokio::time::timeout(Duration::from_secs(1), async {
            while attempts.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("Redis status monitor supervisor should restart the worker");
        supervisor.abort();
    }

    #[tokio::test]
    async fn monitor_supervisor_restarts_after_factory_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = Arc::clone(&attempts);
        let supervisor = tokio::spawn(supervise_cache_redis_status_worker(
            move || {
                let attempt = worker_attempts.fetch_add(1, Ordering::SeqCst);
                assert!(attempt > 0, "Redis status factory regression fixture");
                async { std::future::pending::<()>().await }
            },
            Duration::from_millis(1),
        ));

        tokio::time::timeout(Duration::from_secs(1), async {
            while attempts.load(Ordering::SeqCst) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("Redis status monitor supervisor should restart factory creation");
        supervisor.abort();
    }
}
