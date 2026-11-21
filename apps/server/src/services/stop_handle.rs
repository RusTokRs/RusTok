//! Graceful-shutdown handle primitive for server background workers and runtimes.

use crate::services::server_runtime_context::ServerRuntimeContext;

/// A shutdown signal fences admission of new durable worker work.
///
/// Finish operations already claimed. Recheck immediately after awaited
/// preparation (broker receive, tenant policy, retry transition) and before
/// the next effectful owner call.
pub(crate) fn may_admit_next_worker_operation(
    stop_rx: &tokio::sync::watch::Receiver<bool>,
) -> bool {
    !*stop_rx.borrow()
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
    if stop.is_stopping() {
        return None;
    }
    let mut stopped = stop.subscribe();
    tokio::select! {
        biased;
        _ = stopped.changed() => None,
        value = preparation => (!stop.is_stopping()).then_some(value),
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

    /// Safely retrieves or initializes the shared `StopHandle` from the context without panic.
    pub fn ensure(ctx: &ServerRuntimeContext) -> Self {
        if let Some(existing) = ctx.shared_get::<Self>() {
            existing
        } else {
            let (handle, _rx) = Self::new();
            if ctx.shared_insert_if_absent(handle.clone()) {
                handle
            } else {
                ctx.shared_get::<Self>().unwrap_or(handle)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{StopHandle, await_worker_startup_or_stop, may_admit_next_worker_operation};

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
    async fn completed_bootstrap_is_not_reported_after_stop() {
        let (stop, _receiver) = StopHandle::new();
        assert_eq!(await_worker_startup_or_stop(&stop, async { 7 }).await, Some(7));
        stop.stop().await;
        assert_eq!(
            await_worker_startup_or_stop(&stop, async { 8 }).await,
            None
        );
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
