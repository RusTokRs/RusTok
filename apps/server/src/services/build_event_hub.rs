use std::sync::Arc;

use loco_rs::app::AppContext;
use tokio::sync::broadcast;

use crate::services::build_service::{BuildEvent, BuildEventPublisher};

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

    pub fn publish(&self, event: BuildEvent) {
        let _ = self.sender.send(event);
    }
}

pub fn build_event_hub_from_context(ctx: &AppContext) -> Arc<BuildEventHub> {
    if let Some(shared) = ctx.shared_store.get::<SharedBuildEventHub>() {
        return shared.0.clone();
    }

    let hub = Arc::new(BuildEventHub::new(128));
    ctx.shared_store.insert(SharedBuildEventHub(hub.clone()));
    hub
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
