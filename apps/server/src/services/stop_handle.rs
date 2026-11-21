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
    use super::{StopHandle, may_admit_next_worker_operation};

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
