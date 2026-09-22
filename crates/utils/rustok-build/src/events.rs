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

/// Explicit durable scope for events emitted by the platform build service.
///
/// A platform composition build changes the one global `platform_state`
/// projection, so it must not be attributed to the tenant through which an
/// operator happened to authenticate. Tenant-owned build workflows retain
/// their tenant identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildEventScope {
    Platform,
    Tenant(Uuid),
}

impl BuildEventScope {
    fn envelope_tenant_id(self) -> Uuid {
        match self {
            Self::Platform => Uuid::nil(),
            Self::Tenant(tenant_id) => tenant_id,
        }
    }
}

/// Immutable evidence supplied by the owner command that requested a build.
/// The EventBus publisher copies it into the durable event envelope instead of
/// generating a second identity after the composition transaction commits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildEventPublicationContext {
    pub actor_id: Uuid,
    pub correlation_id: Uuid,
    pub trace_id: String,
}

impl BuildEventPublicationContext {
    fn validate(&self) -> anyhow::Result<()> {
        if self.actor_id.is_nil() || self.correlation_id.is_nil() {
            return Err(anyhow::anyhow!(
                "build event publication requires non-nil actor and correlation identities"
            ));
        }
        if self.trace_id.trim().is_empty() || self.trace_id.len() > 512 {
            return Err(anyhow::anyhow!(
                "build event publication requires a non-empty trace identity up to 512 bytes"
            ));
        }
        Ok(())
    }
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
    scope: BuildEventScope,
    context: BuildEventPublicationContext,
}

impl EventBusBuildEventPublisher {
    pub fn new(
        event_bus: rustok_core::EventBus,
        scope: BuildEventScope,
        context: BuildEventPublicationContext,
    ) -> Self {
        Self {
            event_bus,
            scope,
            context,
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
        self.context.validate()?;
        let mut envelope = rustok_events::EventEnvelope::new(
            self.scope.envelope_tenant_id(),
            Some(self.context.actor_id),
            domain_event,
        );
        envelope.correlation_id = self.context.correlation_id;
        envelope.trace_id = Some(self.context.trace_id.clone());
        self.event_bus
            .publish_envelope(envelope)
            .map_err(|error| anyhow::anyhow!("failed to publish build event: {error}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn platform_build_events_use_the_platform_envelope_scope() {
        let event_bus = rustok_core::EventBus::new();
        let mut events = event_bus.subscribe();
        let context = BuildEventPublicationContext {
            actor_id: Uuid::new_v4(),
            correlation_id: Uuid::new_v4(),
            trace_id: "test:platform-build".to_string(),
        };
        let publisher =
            EventBusBuildEventPublisher::new(event_bus, BuildEventScope::Platform, context.clone());

        publisher
            .publish(BuildEvent::BuildRequested {
                build_id: Uuid::new_v4(),
                requested_by: "operator".to_string(),
            })
            .await
            .expect("publish platform build event");

        let envelope = events.recv().await.expect("receive platform build event");
        assert!(envelope.tenant_id.is_nil());
        assert_eq!(envelope.actor_id, Some(context.actor_id));
        assert_eq!(envelope.correlation_id, context.correlation_id);
        assert_eq!(
            envelope.trace_id.as_deref(),
            Some(context.trace_id.as_str())
        );
        assert!(matches!(
            envelope.event,
            rustok_events::DomainEvent::BuildRequested { .. }
        ));
    }
}
