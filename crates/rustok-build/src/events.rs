//! Build lifecycle events and their host-implemented publisher contract.

use async_trait::async_trait;
use tracing::warn;
use uuid::Uuid;

use crate::BuildStage;

#[derive(Debug, Clone)]
pub enum BuildEvent {
    BuildRequested {
        build_id: Uuid,
        requested_by: String,
    },
    BuildStarted {
        build_id: Uuid,
        stage: BuildStage,
        progress: i32,
    },
    BuildProgress {
        build_id: Uuid,
        stage: BuildStage,
        progress: i32,
    },
    BuildCompleted {
        build_id: Uuid,
        release_id: Option<String>,
    },
    BuildCancelled {
        build_id: Uuid,
        stage: BuildStage,
        progress: i32,
    },
    BuildFailed {
        build_id: Uuid,
        stage: BuildStage,
        progress: i32,
        error: String,
    },
}

#[async_trait]
pub trait BuildEventPublisher: Send + Sync {
    async fn publish(&self, event: BuildEvent) -> anyhow::Result<()>;
}

#[derive(Default)]
pub struct NoopBuildEventPublisher;

#[async_trait]
impl BuildEventPublisher for NoopBuildEventPublisher {
    async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
        warn!(
            ?event,
            "Build event publisher is not configured, skipping event"
        );
        Ok(())
    }
}

pub struct EventBusBuildEventPublisher {
    event_bus: rustok_core::EventBus,
    tenant_id: Uuid,
}

impl EventBusBuildEventPublisher {
    pub fn new(event_bus: rustok_core::EventBus, tenant_id: Uuid) -> Self {
        Self {
            event_bus,
            tenant_id,
        }
    }
}

#[async_trait]
impl BuildEventPublisher for EventBusBuildEventPublisher {
    async fn publish(&self, event: BuildEvent) -> anyhow::Result<()> {
        let domain_event = match event {
            BuildEvent::BuildRequested {
                build_id,
                requested_by,
            } => rustok_events::DomainEvent::BuildRequested {
                build_id,
                requested_by,
            },
            unsupported => {
                warn!(
                    ?unsupported,
                    "Build event is not mapped to DomainEvent yet, skipping"
                );
                return Ok(());
            }
        };
        self.event_bus
            .publish(self.tenant_id, None, domain_event)
            .map_err(|error| anyhow::anyhow!("failed to publish build event: {error}"))
    }
}
