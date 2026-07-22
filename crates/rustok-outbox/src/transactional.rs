use crate::transport::OutboxTransport;
use rustok_core::Result;
use rustok_core::events::EventTransport;
use rustok_events::{
    ContractEventEnvelope, DomainEvent, EventContract, EventEnvelope, ValidateEvent,
};
use sea_orm::ConnectionTrait;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct TransactionalEventBus {
    transport: Arc<dyn EventTransport>,
}

impl TransactionalEventBus {
    pub fn new(transport: Arc<dyn EventTransport>) -> Self {
        Self { transport }
    }

    pub async fn publish_in_tx<C>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.publish_in_tx_with_envelope_id(txn, tenant_id, actor_id, event)
            .await
            .map(|_| ())
    }

    pub async fn publish_in_tx_with_envelope_id<C>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<Uuid>
    where
        C: ConnectionTrait,
    {
        let envelope = self.build_envelope(tenant_id, actor_id, event)?;
        let envelope_id = envelope.id;

        if let Some(outbox) = self.transport.as_any().downcast_ref::<OutboxTransport>() {
            outbox.write_to_outbox(txn, envelope).await?;
        } else {
            #[cfg(feature = "test-transport-fallback")]
            {
                self.transport.publish(envelope).await?;
            }
            #[cfg(not(feature = "test-transport-fallback"))]
            {
                return Err(transactional_transport_required(&*self.transport));
            }
        }

        Ok(envelope_id)
    }

    /// Publishes a sealed typed event contract through the same owner transaction.
    ///
    /// Unlike the legacy root `DomainEvent` API, this supports module event families
    /// without reopening a platform-wide enum. External crates cannot implement
    /// `EventContract`, so arbitrary event names remain impossible.
    pub async fn publish_contract_in_tx<C, E>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: E,
    ) -> Result<()>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        self.publish_contract_in_tx_with_envelope_id(txn, tenant_id, actor_id, event)
            .await
            .map(|_| ())
    }

    pub async fn publish_contract_in_tx_with_envelope_id<C, E>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: E,
    ) -> Result<Uuid>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        let event_type = event.event_type();
        let envelope = ContractEventEnvelope::new(tenant_id, actor_id, event).map_err(|error| {
            tracing::error!(event_type, error = %error, "Event contract encoding failed");
            rustok_core::Error::Validation(format!("Event contract encoding failed: {error}"))
        })?;
        let envelope_id = envelope.id();
        let outbox = self
            .transport
            .as_any()
            .downcast_ref::<OutboxTransport>()
            .ok_or_else(|| transactional_transport_required(&*self.transport))?;
        outbox.write_contract_to_outbox(txn, envelope).await?;
        Ok(envelope_id)
    }

    pub async fn publish(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<()> {
        self.publish_with_envelope_id(tenant_id, actor_id, event)
            .await
            .map(|_| ())
    }

    pub async fn publish_with_envelope_id(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<Uuid> {
        let envelope = self.build_envelope(tenant_id, actor_id, event)?;
        let envelope_id = envelope.id;
        self.transport.publish(envelope).await?;
        Ok(envelope_id)
    }

    fn build_envelope(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<EventEnvelope> {
        validate_event(&event)?;
        Ok(EventEnvelope::new(tenant_id, actor_id, event))
    }
}

fn transactional_transport_required(transport: &dyn EventTransport) -> rustok_core::Error {
    rustok_core::Error::Validation(format!(
        "transactional event publishing requires OutboxTransport; configured transport reliability is {:?}",
        transport.reliability_level()
    ))
}

fn validate_event(event: &DomainEvent) -> Result<()> {
    event.validate().map_err(|e| {
        tracing::error!(
            event_type = event.event_type(),
            error = %e,
            "Event validation failed"
        );
        rustok_core::Error::Validation(format!("Event validation failed: {}", e))
    })
}
