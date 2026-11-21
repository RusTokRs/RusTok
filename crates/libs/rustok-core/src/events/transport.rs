use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use std::any::Any;
use uuid::Uuid;

use crate::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventEnvelope};

/// Appends an event using the caller's transaction. The writer leaves commit and
/// rollback to the caller and returns an error when it cannot persist the event.
#[async_trait]
pub trait TransactionalEventWriter: Send + Sync {
    async fn write_event(
        &self,
        transaction: &DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> Result<()>;
}

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

    /// Whether this transport can append envelopes inside a caller-owned
    /// transaction (the outbox transport can; streaming and in-memory
    /// transports cannot). Callers check this before handing over an
    /// envelope so "not supported" never consumes the value, and wrappers
    /// that delegate to a transactional transport must forward it together
    /// with the two write methods below. This replaces downcasting to a
    /// concrete transport type at transactional call sites.
    fn supports_transactional_writes(&self) -> bool {
        false
    }

    /// Appends a root envelope inside the caller's transaction. Only
    /// invoked when [`Self::supports_transactional_writes`] is `true`.
    async fn write_root_in_transaction(
        &self,
        _transaction: &DatabaseTransaction,
        _envelope: EventEnvelope,
    ) -> Result<()> {
        Err(Error::Validation(
            "event transport does not support transactional writes".to_string(),
        ))
    }

    /// Contract-envelope counterpart of [`Self::write_root_in_transaction`].
    async fn write_contract_in_transaction(
        &self,
        _transaction: &DatabaseTransaction,
        _envelope: ContractEventEnvelope,
    ) -> Result<()> {
        Err(Error::Validation(
            "event transport does not support transactional writes".to_string(),
        ))
    }

    fn reliability_level(&self) -> ReliabilityLevel;

    fn as_any(&self) -> &dyn Any;
}
