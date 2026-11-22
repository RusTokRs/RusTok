use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::FutureExt;
use rustok_cache::{CacheService, RedisCacheStatus};
use tokio::task::JoinHandle;

use crate::services::server_runtime_context::ServerRuntimeContext;
use crate::services::stop_handle::{StopHandle, await_worker_startup_or_stop};

const CACHE_REDIS_STATUS_INTERVAL: Duration = Duration::from_secs(10);
const CACHE_REDIS_STATUS_RESTART_DELAY: Duration = Duration::from_secs(1);
const CACHE_REDIS_STATUS_MONITOR_CHANNEL: &str = "cache.redis.status";

struct CacheRedisStatusMonitorRuntime {
    task: std::sync::Mutex<Option<JoinHandle<()>>>,
    required: bool,
}

impl CacheRedisStatusMonitorRuntime {
    fn is_running(&self) -> bool {
        if !self.required {
            return true;
        }
        self.task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or(false, |task| !task.is_finished())
    }

    fn abort(&self) {
        if let Some(task) = self
            .task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            task.abort();
        }
    }

    fn take_task(&self) -> Option<JoinHandle<()>> {
        self.task
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

impl Drop for CacheRedisStatusMonitorRuntime {
    fn drop(&mut self) {
        if let Some(task) = self
            .task
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            task.abort();
        }
    }
}

#[derive(Clone)]
pub struct CacheRedisStatusMonitorHandle(Arc<CacheRedisStatusMonitorRuntime>);

impl CacheRedisStatusMonitorHandle {
    fn new(task: Option<JoinHandle<()>>) -> Self {
        let required = task.is_some();
        Self(Arc::new(CacheRedisStatusMonitorRuntime {
            task: std::sync::Mutex::new(task),
            required,
        }))
    }

    pub fn is_running(&self) -> bool {
        self.0.is_running()
    }

    pub(crate) fn take_task(&self) -> Option<JoinHandle<()>> {
        self.0.take_task()
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
    ctx.shared_insert_if_absent(CacheRedisStatusMonitorStartLock::default());
    let Some(start_lock) = ctx.shared_get::<CacheRedisStatusMonitorStartLock>() else {
        tracing::error!("Cache Redis status monitor start lock is unavailable");
        return;
    };
    let stop = StopHandle::ensure(ctx);
    let Some(_start_guard) = await_worker_startup_or_stop(&stop, start_lock.0.lock()).await
    else {
        return;
    };

    if let Some(existing) = ctx.shared_get::<CacheRedisStatusMonitorHandle>() {
        if existing.is_running() {
            return;
        }
        tracing::warn!("Cache Redis status monitor stopped; replacing runtime");
        existing.abort();
        if let Some(task) = existing.take_task() {
            let Some(result) = await_worker_startup_or_stop(&stop, task).await else {
                return;
            };
            if let Err(error) = result {
                if !error.is_cancelled() {
                    tracing::warn!(
                        ?error,
                        "Old Cache Redis status monitor failed while being replaced"
                    );
                }
            }
        }
    }

    let Some(initial) = await_worker_startup_or_stop(&stop, cache.redis_status()).await
    else {
        return;
    };
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
            stop.subscribe(),
        )))
    } else {
        None
    };

    ctx.shared_insert(CacheRedisStatusMonitorHandle::new(task));
}

pub(crate) async fn take_task_for_shutdown(ctx: &ServerRuntimeContext) -> Option<JoinHandle<()>> {
    let start_lock = ctx.shared_get_or_insert_with(CacheRedisStatusMonitorStartLock::default);
    let _start_guard = match tokio::time::timeout(Duration::from_secs(5), start_lock.0.lock()).await
    {
        Ok(guard) => guard,
        Err(_) => {
            tracing::warn!(
                "Timed out waiting for Cache Redis status monitor startup during shutdown"
            );
            return None;
        }
    };
    ctx.shared_take::<CacheRedisStatusMonitorHandle>()
        .and_then(|worker| worker.take_task())
}

async fn run_cache_redis_status_monitor(cache: CacheService, mut previous: RedisCacheStatus) {
    let mut interval = tokio::time::interval(CACHE_REDIS_STATUS_INTERVAL);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // The startup or restart probe already populated the collector.
    interval.tick().await;
    loop {
        interval.tick().await;
        let current = cache.redis_status().await;
        log_transition(Some(&previous), &current);
        previous = current;
    }
}

async fn supervise_cache_redis_status_worker<F, Fut>(
    mut worker_factory: F,
    restart_delay: Duration,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) where
    F: FnMut() -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    loop {
        if *stop_rx.borrow() {
            return;
        }

        let worker = match std::panic::catch_unwind(AssertUnwindSafe(&mut worker_factory)) {
            Ok(worker) => worker,
            Err(_) => {
                if *stop_rx.borrow() {
                    return;
                }
                record_monitor_restart("factory_panicked");
                tokio::select! {
                    _ = tokio::time::sleep(restart_delay) => {}
                    changed = stop_rx.changed() => {
                        if changed.is_err() || *stop_rx.borrow() {
                            return;
                        }
                    }
                }
                continue;
            }
        };

        let outcome = tokio::select! {
            biased;
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    return;
                }
                continue;
            }
            outcome = AssertUnwindSafe(worker).catch_unwind() => outcome,
        };
        if *stop_rx.borrow() {
            return;
        }
        if outcome.is_err() {
            record_monitor_restart("worker_panicked");
        } else {
            record_monitor_restart("worker_exited");
        }

        tokio::select! {
            _ = tokio::time::sleep(restart_delay) => {}
            changed = stop_rx.changed() => {
                if changed.is_err() || *stop_rx.borrow() {
                    return;
                }
            }
        }
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
            generation_recovery_pending = current.generation_recovery_pending,
            generation_recovery_failed = current.recovery_error.is_some(),
            "Cache Redis lifecycle entered degraded state"
        );
    } else {
        tracing::info!(
            redis_url_present = current.url_present,
            redis_client_initialized = current.client_initialized,
            redis_connectivity_healthy = current.connectivity_healthy,
            generation_recovery_pending = current.generation_recovery_pending,
            generation_recovery_failed = current.recovery_error.is_some(),
            "Cache Redis lifecycle state changed"
        );
    }
}

/// Lifecycle identity of a status sample.
///
/// Error *text* is deliberately excluded (it can carry credentials and would retrigger the log on
/// every probe), but presence of a recovery error and the pending-generation backlog are part of
/// the identity: a monitor that ignores them would never announce that generation recovery is
/// still unfinished after Redis answered again.
fn state_tuple(status: &RedisCacheStatus) -> (bool, bool, bool, usize, bool) {
    (
        status.url_present,
        status.client_initialized,
        status.connectivity_healthy,
        status.generation_recovery_pending,
        status.recovery_error.is_some(),
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
            generation_recovery_pending: 0,
            recovery_error: None,
        };
        let second = RedisCacheStatus {
            last_error: Some("secret-b".to_string()),
            ..first.clone()
        };
        assert_eq!(state_tuple(&first), state_tuple(&second));
    }

    #[test]
    fn state_transition_reports_generation_recovery_backlog() {
        let reached = RedisCacheStatus {
            url_present: true,
            client_initialized: true,
            connectivity_healthy: true,
            last_error: None,
            generation_recovery_pending: 0,
            recovery_error: None,
        };
        assert!(reached.is_healthy());
        assert!(!reached.is_degraded());

        let recovering = RedisCacheStatus {
            generation_recovery_pending: 3,
            ..reached.clone()
        };
        assert!(recovering.is_connectivity_healthy());
        assert!(!recovering.is_healthy());
        assert!(recovering.is_degraded());
        assert_ne!(state_tuple(&reached), state_tuple(&recovering));

        let failed = RedisCacheStatus {
            recovery_error: Some("recovery rejected".to_string()),
            ..recovering.clone()
        };
        assert_ne!(state_tuple(&recovering), state_tuple(&failed));
        assert_ne!(state_tuple(&failed), state_tuple(&reached));
    }

    #[test]
    fn healthy_memory_only_state_is_not_degraded() {
        let status = RedisCacheStatus::default();
        assert!(!status.is_degraded());
        assert_eq!(state_tuple(&status), (false, false, false, 0, false));
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
        let (_stop, stop_rx) = StopHandle::new();
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
            stop_rx,
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
    async fn monitor_supervisor_stops_promptly_without_restarting() {
        let (stop, stop_rx) = StopHandle::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = Arc::clone(&attempts);
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let mut started_tx = Some(started_tx);
        let supervisor = tokio::spawn(supervise_cache_redis_status_worker(
            move || {
                worker_attempts.fetch_add(1, Ordering::SeqCst);
                if let Some(started) = started_tx.take() {
                    let _ = started.send(());
                }
                async { std::future::pending::<()>().await }
            },
            Duration::from_secs(30),
            stop_rx,
        ));

        started_rx.await.expect("monitor worker starts");
        stop.stop().await;
        tokio::time::timeout(Duration::from_secs(1), supervisor)
            .await
            .expect("monitor supervisor exits on host stop")
            .expect("monitor supervisor joins");
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn monitor_supervisor_restarts_after_factory_panic() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let worker_attempts = Arc::clone(&attempts);
        let (_stop, stop_rx) = StopHandle::new();
        let supervisor = tokio::spawn(supervise_cache_redis_status_worker(
            move || {
                let attempt = worker_attempts.fetch_add(1, Ordering::SeqCst);
                assert!(attempt > 0, "Redis status factory regression fixture");
                async { std::future::pending::<()>().await }
            },
            Duration::from_millis(1),
            stop_rx,
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
