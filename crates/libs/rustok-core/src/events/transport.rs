use async_trait::async_trait;
use std::any::Any;
use uuid::Uuid;

use crate::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventEnvelope};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReliabilityLevel {
    InMemory,
    Outbox,
    Streaming,
}

#[async_trait]
pub trait EventTransport: Send + Sync {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()>;

    /// Publishes an envelope from a sealed typed event family.
    ///
    /// The default adapter preserves compatibility for root `DomainEvent`
    /// envelopes. Streaming transports that support bounded event families must
    /// override this method and serialize the contract envelope directly.
    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> Result<()> {
        let event_type = envelope.event_type().to_string();
        let root = envelope.into_root_envelope().map_err(|error| {
            Error::Validation(format!(
                "event transport does not support contract event `{event_type}`: {error}"
            ))
        })?;
        self.publish(root).await
    }

    async fn publish_batch(&self, events: Vec<EventEnvelope>) -> Result<()> {
        for envelope in events {
            self.publish(envelope).await?;
        }
        Ok(())
    }

    async fn acknowledge(&self, _event_id: Uuid) -> Result<()> {
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel;

    fn as_any(&self) -> &dyn Any;
}
