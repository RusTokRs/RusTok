//! Graceful-shutdown handle primitive for server background workers and runtimes.

use crate::services::server_runtime_context::ServerRuntimeContext;

/// Stored in `ServerRuntimeContext`; `on_shutdown` calls `stop()` to abort workers.
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
