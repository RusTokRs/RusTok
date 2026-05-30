use crate::transport::OutboxTransport;
use rustok_core::events::EventTransport;
use rustok_core::Result;
use rustok_events::{DomainEvent, EventEnvelope, ValidateEvent};
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
            tracing::warn!(
                "EventTransport doesn't support transactional writes. \
                 Event may be lost if transaction fails."
            );
            self.transport.publish(envelope).await?;
        }

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
