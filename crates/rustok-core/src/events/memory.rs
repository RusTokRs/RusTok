use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::broadcast;

use super::{EventBus, EventBusStats, EventEnvelope, EventTransport, ReliabilityLevel};

#[derive(Debug, Clone)]
pub struct MemoryTransport {
    bus: EventBus,
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self {
            bus: EventBus::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            bus: EventBus::with_capacity(capacity),
        }
    }

    /// Clone the delivery bus used by this transport.
    ///
    /// Consumers that must observe events after they pass through the configured transport use
    /// this bus instead of subscribing to an unrelated process-local publisher bus.
    pub fn event_bus(&self) -> EventBus {
        self.bus.clone()
    }

    pub fn stats(&self) -> Arc<EventBusStats> {
        self.bus.stats()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EventEnvelope> {
        self.bus.subscribe()
    }
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventTransport for MemoryTransport {
    async fn publish(&self, envelope: EventEnvelope) -> crate::Result<()> {
        self.bus.publish_envelope(envelope)
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
