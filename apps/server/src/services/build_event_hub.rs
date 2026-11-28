use std::sync::Arc;

use tokio::sync::{broadcast, OwnedSemaphorePermit, Semaphore};

use crate::services::{
    build_control::ServerBuildControl, server_runtime_context::ServerRuntimeContext,
};
use rustok_build::{BuildEvent, BuildEventPublisher, SharedBuildControl};

const MAX_BUILD_PROGRESS_STREAMS_PER_PROCESS: usize = 64;

#[derive(Clone)]
pub struct SharedBuildEventHub(pub Arc<BuildEventHub>);

/// Process-local wake-up fan-out for subscribers of the durable build-state
/// cursor. The database-backed BuildControl port is the source of truth.
pub struct BuildEventHub {
    sender: broadcast::Sender<()>,
    control: SharedBuildControl,
    stream_slots: Arc<Semaphore>,
}

impl BuildEventHub {
    pub fn new(control: SharedBuildControl, capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            control,
            stream_slots: Arc::new(Semaphore::new(
                MAX_BUILD_PROGRESS_STREAMS_PER_PROCESS,
            )),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<()> {
        self.sender.subscribe()
    }

    pub fn control(&self) -> SharedBuildControl {
        self.control.clone()
    }

    /// Bound per-process database polling from GraphQL and WebSocket streams.
    pub fn try_acquire_stream(&self) -> Option<OwnedSemaphorePermit> {
        self.stream_slots.clone().try_acquire_owned().ok()
    }

    #[cfg(test)]
    pub(crate) fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    /// Notify local subscribers that durable build state may have changed.
    /// Losing this hint is harmless: every subscriber also polls BuildControl.
    pub fn publish(&self, _event: BuildEvent) {
        if let Err(error) = self.sender.send(()) {
            tracing::trace!(%error, "Build progress wake-up had no local subscribers");
        }
    }
}

pub fn build_event_hub_from_context(ctx: &ServerRuntimeContext) -> Arc<BuildEventHub> {
    let control =
        ctx.shared_get_or_insert_with(|| ServerBuildControl::shared(ctx.clone()));
    ctx.shared_get_or_insert_with(|| {
        SharedBuildEventHub(Arc::new(BuildEventHub::new(control, 128)))
    })
    .0
}

pub struct BuildEventHubPublisher {
    hub: Arc<BuildEventHub>,
}

impl BuildEventHubPublisher {
    pub fn new(hub: Arc<BuildEventHub>) -> Self {
        Self { hub }
    }
}

#[async_trait::async_trait]
impl BuildEventPublisher for BuildEventHubPublisher {
    async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
        self.hub.publish(event);
        Ok(())
    }
}

pub struct CompositeBuildEventPublisher {
    publishers: Vec<Arc<dyn BuildEventPublisher>>,
}

impl CompositeBuildEventPublisher {
    pub fn new(publishers: Vec<Arc<dyn BuildEventPublisher>>) -> Self {
        Self { publishers }
    }
}

#[async_trait::async_trait]
impl BuildEventPublisher for CompositeBuildEventPublisher {
    async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
        for publisher in &self.publishers {
            publisher.publish(event.clone()).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::settings::RustokSettings;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_initialization_returns_the_same_hub() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let barrier = Arc::new(tokio::sync::Barrier::new(16));
        let mut tasks = Vec::new();
        for _ in 0..16 {
            let ctx = ctx.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                build_event_hub_from_context(&ctx)
            }));
        }
        let mut hubs = Vec::new();
        for task in tasks {
            hubs.push(task.await.expect("initialization task"));
        }
        assert!(hubs.iter().all(|hub| Arc::ptr_eq(hub, &hubs[0])));
    }

    #[tokio::test]
    async fn stream_slots_are_bounded_and_released_with_the_connection() {
        let db = sea_orm::Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        let ctx = ServerRuntimeContext::new(db, RustokSettings::default());
        let hub = build_event_hub_from_context(&ctx);
        let permits = (0..MAX_BUILD_PROGRESS_STREAMS_PER_PROCESS)
            .map(|_| hub.try_acquire_stream().expect("available stream slot"))
            .collect::<Vec<_>>();
        assert!(hub.try_acquire_stream().is_none());
        drop(permits);
        assert!(hub.try_acquire_stream().is_some());
    }
}
