//! Graceful-shutdown handle primitive for server background workers and runtimes.

use crate::services::server_runtime_context::ServerRuntimeContext;

/// A shutdown signal fences admission of new durable worker work.
///
/// Finish operations already claimed. Recheck immediately after awaited
/// preparation (broker receive, tenant policy, retry transition) and before
/// the next effectful owner call. A closed watch channel also fences new work.
pub(crate) fn may_admit_next_worker_operation(
    stop_rx: &tokio::sync::watch::Receiver<bool>,
) -> bool {
    stop_rx.has_changed().is_ok() && !*stop_rx.borrow()
}

/// Wait between durable attempts, prioritizing shutdown over a ready timer.
/// Recheck the current signal after waking: a timer becoming ready does not
/// grant admission if the shutdown sender has closed or requested stop.
pub(crate) async fn wait_for_worker_retry_or_stop(
    delay: std::time::Duration,
    stop_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    if !may_admit_next_worker_operation(stop_rx) {
        return true;
    }
    tokio::select! {
        biased;
        _ = stop_rx.changed() => {}
        _ = tokio::time::sleep(delay) => {}
    }
    !may_admit_next_worker_operation(stop_rx)
}

/// Check shutdown before invoking the next durable operation, then await the
/// admitted operation to completion. A graceful stop must not drop the owner
/// future between a non-transactional side effect and its acknowledgement.
///
/// This is a process-local admission check, not a distributed lease or a
/// guarantee against process death or the host's bounded forced abort.
pub(crate) async fn run_admitted_worker_operation<Operation, Future>(
    stop_rx: &tokio::sync::watch::Receiver<bool>,
    operation: Operation,
) -> Option<Future::Output>
where
    Operation: FnOnce() -> Future,
    Future: std::future::Future,
{
    if !may_admit_next_worker_operation(stop_rx) {
        return None;
    }
    Some(operation().await)
}

/// Await a cancellable, pre-publication worker bootstrap step. Startup often
/// holds its registration mutex while resolving cache state or joining old
/// workers; if that I/O stalls, shutdown's bounded handle snapshot must not
/// wait for the underlying database or Redis call to complete. On stop, drop
/// the *unpublished* preparation future and release its startup mutex.
///
/// Never use this helper around an already claimed durable operation or
/// externally visible effect that requires completion/compensation on cancel.
pub(crate) async fn await_worker_startup_or_stop<F>(
    stop: &StopHandle,
    preparation: F,
) -> Option<F::Output>
where
    F: std::future::Future,
{
    // Subscribe before inspecting the current value: a stop sent between a
    // separate check and subscription would otherwise be considered already
    // observed by watch::Receiver::changed(), leaving pending setup stuck.
    let mut stopped = stop.subscribe();
    if *stopped.borrow() {
        return None;
    }
    tokio::select! {
        biased;
        _ = stopped.changed() => None,
        value = preparation => (!stop.is_stopping()).then_some(value),
    }
}

/// Run a cancellation-safe background listener until host shutdown.
///
/// Use only for workers whose current operation is safe to cancel (for
/// example, cache invalidation listeners and read-only reconciliation). Durable
/// claims and externally visible operations must finish or compensate instead.
pub(crate) async fn run_until_stopped<F>(mut stop_rx: tokio::sync::watch::Receiver<bool>, worker: F)
where
    F: std::future::Future<Output = ()>,
{
    if !may_admit_next_worker_operation(&stop_rx) {
        return;
    }
    tokio::select! {
        biased;
        _ = stop_rx.changed() => {}
        _ = worker => {}
    }
}

/// Stored in `ServerRuntimeContext`; shutdown signals each worker so it can
/// complete its current durable operation and stop before the next one.
#[derive(Clone)]
pub struct StopHandle {
    stop_tx: tokio::sync::watch::Sender<bool>,
}

impl StopHandle {
    pub fn new() -> (Self, tokio::sync::watch::Receiver<bool>) {
        let (tx, rx) = tokio::sync::watch::channel(false);
        (Self { stop_tx: tx }, rx)
    }

    /// Subscribes a new watch `Receiver` to the host shutdown signal.
    ///
    /// The returned receiver immediately sees the current value and will be
    /// notified when [`StopHandle::stop`] is called. Clone it once per background worker
    /// so each worker gets its own independent view of the channel.
    pub fn subscribe(&self) -> tokio::sync::watch::Receiver<bool> {
        self.stop_tx.subscribe()
    }

    pub async fn stop(&self) {
        let was_stopping = self.stop_tx.send_replace(true);
        if !was_stopping {
            // Yield so spawned tasks have a chance to notice the signal.
            tokio::task::yield_now().await;
        }
    }

    pub fn is_stopping(&self) -> bool {
        *self.stop_tx.borrow()
    }

    /// Atomically retrieves or initializes the runtime-owned shutdown signal.
    pub fn ensure(ctx: &ServerRuntimeContext) -> Self {
        ctx.shared_get_or_insert_with(|| Self::new().0)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StopHandle, await_worker_startup_or_stop, may_admit_next_worker_operation,
        run_admitted_worker_operation, run_until_stopped, wait_for_worker_retry_or_stop,
    };

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_stop_handle_resolution_shares_one_terminal_signal() {
        let ctx = crate::services::server_runtime_context::ServerRuntimeContext::new(
            sea_orm::DatabaseConnection::default(),
            crate::common::settings::RustokSettings::default(),
        );
        let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(16));
        let mut tasks = Vec::new();
        for _ in 0..16 {
            let ctx = ctx.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                StopHandle::ensure(&ctx)
            }));
        }
        let mut handles = Vec::new();
        for task in tasks {
            handles.push(task.await.expect("concurrent stop handle"));
        }
        handles[0].stop().await;
        assert!(handles.iter().all(StopHandle::is_stopping));
        assert!(StopHandle::ensure(&ctx).is_stopping());
    }

    #[tokio::test]
    async fn stop_interrupts_a_stalled_unpublished_bootstrap() {
        let (stop, _receiver) = StopHandle::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel();

        struct CancellationMarker(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for CancellationMarker {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let stop_for_start = stop.clone();
        let startup = tokio::spawn(async move {
            let marker = CancellationMarker(Some(cancelled_tx));
            await_worker_startup_or_stop(&stop_for_start, async move {
                let _marker = marker;
                started_tx.send(()).expect("preparation started");
                std::future::pending::<u64>().await
            })
            .await
        });
        started_rx.await.expect("preparation polled");
        stop.stop().await;
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), startup)
                .await
                .expect("shutdown interrupts pending preparation")
                .expect("startup task joins"),
            None
        );
        cancelled_rx.await.expect("unpublished preparation dropped");
    }

    #[tokio::test]
    async fn stop_releases_a_starter_waiting_on_a_busy_registration_lock() {
        let gate = std::sync::Arc::new(tokio::sync::Mutex::new(()));
        let held = gate.lock().await;
        let (stop, _receiver) = StopHandle::new();
        let (waiting_tx, waiting_rx) = tokio::sync::oneshot::channel();
        let stop_for_start = stop.clone();
        let next_gate = std::sync::Arc::clone(&gate);
        let starter = tokio::spawn(async move {
            await_worker_startup_or_stop(&stop_for_start, async {
                waiting_tx.send(()).expect("startup is waiting");
                next_gate.lock().await
            })
            .await
            .is_none()
        });

        waiting_rx
            .await
            .expect("startup reached the busy registration mutex");
        stop.stop().await;

        assert!(
            tokio::time::timeout(std::time::Duration::from_secs(1), starter)
                .await
                .expect("stop releases a waiting initializer")
                .expect("initializer task joins"),
            "startup must not acquire a registration slot after host stop"
        );
        // The original initializer still owns the gate: success must not
        // depend on shutdown waiting for unrelated in-flight setup.
        drop(held);
    }

    #[tokio::test]
    async fn completed_bootstrap_is_not_reported_after_stop() {
        let (stop, _receiver) = StopHandle::new();
        assert_eq!(
            await_worker_startup_or_stop(&stop, async { 7 }).await,
            Some(7)
        );
        stop.stop().await;
        assert_eq!(await_worker_startup_or_stop(&stop, async { 8 }).await, None);
    }

    #[tokio::test]
    async fn run_until_stopped_cancels_a_running_listener() {
        let (stop, receiver) = StopHandle::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (cancelled_tx, cancelled_rx) = tokio::sync::oneshot::channel();

        struct CancellationMarker(Option<tokio::sync::oneshot::Sender<()>>);
        impl Drop for CancellationMarker {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }

        let worker = tokio::spawn(run_until_stopped(receiver, async move {
            let _marker = CancellationMarker(Some(cancelled_tx));
            started_tx.send(()).expect("listener started");
            std::future::pending::<()>().await;
        }));
        started_rx.await.expect("listener future was polled");
        stop.stop().await;
        tokio::time::timeout(std::time::Duration::from_secs(1), worker)
            .await
            .expect("listener exits promptly on stop")
            .expect("listener wrapper joins");
        cancelled_rx.await.expect("listener future was cancelled");
    }

    #[tokio::test]
    async fn stopped_host_never_invokes_next_durable_operation() {
        let (stop, receiver) = StopHandle::new();
        stop.stop().await;
        let invoked = std::cell::Cell::new(false);
        let result = run_admitted_worker_operation(&receiver, || {
            invoked.set(true);
            async { 42_u32 }
        })
        .await;
        assert_eq!(result, None);
        assert!(
            !invoked.get(),
            "the effectful operation must not be admitted"
        );
    }

    #[tokio::test]
    async fn retry_delay_does_not_admit_work_on_closed_or_stopped_watch() {
        use std::time::Duration;

        let (stop, mut receiver) = StopHandle::new();
        assert!(!wait_for_worker_retry_or_stop(Duration::ZERO, &mut receiver).await);

        stop.stop().await;
        assert!(wait_for_worker_retry_or_stop(Duration::ZERO, &mut receiver).await);
        // An already-observed stop is terminal too; changed() alone cannot
        // detect it until a new value arrives or the sender disappears.
        receiver.borrow_and_update();
        assert!(wait_for_worker_retry_or_stop(Duration::ZERO, &mut receiver).await);

        let (stop, mut receiver) = StopHandle::new();
        drop(stop);
        assert!(wait_for_worker_retry_or_stop(Duration::ZERO, &mut receiver).await);
    }

    #[tokio::test]
    async fn closed_stop_channel_never_admits_next_durable_operation() {
        let (stop, receiver) = StopHandle::new();
        assert!(may_admit_next_worker_operation(&receiver));
        drop(stop);
        assert!(!may_admit_next_worker_operation(&receiver));
        let invoked = std::cell::Cell::new(false);
        let result = run_admitted_worker_operation(&receiver, || {
            invoked.set(true);
            async { 42_u32 }
        })
        .await;
        assert_eq!(result, None);
        assert!(!invoked.get());
    }

    #[tokio::test]
    async fn closing_stop_channel_does_not_cancel_admitted_work() {
        let (stop, receiver) = StopHandle::new();
        let result = run_admitted_worker_operation(&receiver, || async move {
            drop(stop);
            tokio::task::yield_now().await;
            42_u32
        })
        .await;
        assert_eq!(result, Some(42));
        assert!(!may_admit_next_worker_operation(&receiver));
    }

    #[tokio::test]
    async fn stop_during_admitted_durable_operation_waits_for_its_acknowledgement() {
        let (stop, receiver) = StopHandle::new();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (finish_tx, finish_rx) = tokio::sync::oneshot::channel();
        let operation = tokio::spawn(async move {
            run_admitted_worker_operation(&receiver, || async move {
                started_tx.send(()).expect("durable operation started");
                finish_rx.await.expect("durable operation completes");
                42_u32
            })
            .await
        });

        started_rx.await.expect("operation entered");
        stop.stop().await;
        assert!(
            !operation.is_finished(),
            "graceful stop must not cancel an already-admitted durable operation"
        );
        finish_tx.send(()).expect("allow acknowledgement");
        assert_eq!(operation.await.expect("worker joins"), Some(42));
    }

    #[tokio::test]
    async fn stopped_while_receiving_does_not_admit_delivered_work() {
        let (stop, receiver) = StopHandle::new();
        assert!(may_admit_next_worker_operation(&receiver));

        let (receiving_tx, receiving_rx) = tokio::sync::oneshot::channel();
        let (deliver_tx, deliver_rx) = tokio::sync::oneshot::channel::<u32>();
        let consumer = tokio::spawn(async move {
            receiving_tx.send(()).expect("receive began");
            let item = deliver_rx.await.expect("receive completed");
            may_admit_next_worker_operation(&receiver).then_some(item)
        });
        receiving_rx.await.expect("consumer awaiting delivery");

        stop.stop().await;
        deliver_tx.send(42).expect("deliver after shutdown");
        assert_eq!(
            consumer.await.expect("consumer joins"),
            None,
            "a delivered but unacknowledged item stays for recovery"
        );
    }
}
