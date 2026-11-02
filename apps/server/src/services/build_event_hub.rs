use std::sync::Arc;

use tokio::sync::broadcast;

use crate::services::server_runtime_context::ServerRuntimeContext;
use rustok_build::{BuildEvent, BuildEventPublisher};

#[derive(Clone)]
pub struct SharedBuildEventHub(pub Arc<BuildEventHub>);

pub struct BuildEventHub {
    sender: broadcast::Sender<BuildEvent>,
}

impl BuildEventHub {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<BuildEvent> {
        self.sender.subscribe()
    }

    #[cfg(test)]
    pub(crate) fn receiver_count(&self) -> usize {
        self.sender.receiver_count()
    }

    pub fn publish(&self, event: BuildEvent) {
        let _ = self.sender.send(event);
    }
}

pub fn build_event_hub_from_context(ctx: &ServerRuntimeContext) -> Arc<BuildEventHub> {
    ctx.shared_get_or_insert_with(|| SharedBuildEventHub(Arc::new(BuildEventHub::new(128))))
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
}
