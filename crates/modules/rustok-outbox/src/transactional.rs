use crate::transport::{ContractEventWriteOnceError, OutboxTransport};
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

    /// Publishes a validated registered root event through an owner transaction
    /// without requiring a separately configured transport handle.
    ///
    /// This is intended for domain helpers that receive only the active
    /// transaction. It preserves both `DomainEvent::validate()` and registered
    /// envelope/schema validation before inserting into the canonical outbox.
    pub async fn publish_root_in_tx<C>(
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        Self::publish_root_in_tx_with_envelope_id(txn, tenant_id, actor_id, event)
            .await
            .map(|_| ())
    }

    /// Publishes a validated registered root event and returns the exact durable
    /// envelope identity written by the same owner transaction.
    pub async fn publish_root_in_tx_with_envelope_id<C>(
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: DomainEvent,
    ) -> Result<Uuid>
    where
        C: ConnectionTrait,
    {
        validate_event(&event)?;
        let envelope = EventEnvelope::new(tenant_id, actor_id, event);
        let envelope_id = envelope.id;
        OutboxTransport::write_envelope_in_tx(txn, envelope).await?;
        Ok(envelope_id)
    }

    /// Publishes a sealed typed contract through an owner transaction that
    /// does not carry a separately composed transport handle.
    ///
    /// The typed envelope retains the exact predecessor envelope identity in
    /// `causation_id`. The canonical outbox row is written in the supplied
    /// transaction and its own envelope identity is returned for diagnostics.
    pub async fn publish_contract_direct_in_tx_with_causation_and_envelope_id<C, E>(
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Uuid,
        event: E,
    ) -> Result<Uuid>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        let envelope = build_contract_envelope(tenant_id, actor_id, Some(causation_id), event)?;
        let envelope_id = envelope.id();
        OutboxTransport::write_contract_envelope_in_tx(txn, envelope).await?;
        Ok(envelope_id)
    }

    /// Writes one sealed typed contract exactly once using an existing owner
    /// identity as the canonical envelope UUID.
    ///
    /// The caller owns the transaction. Concurrent exact replays return the
    /// same envelope UUID; reuse of that UUID for different envelope scope or
    /// typed payload returns `Conflict`. Delivery remains owned by
    /// `OutboxRelay` after commit.
    pub async fn publish_contract_once_direct_in_tx_with_envelope_id<C, E>(
        txn: &C,
        envelope_id: Uuid,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        event: E,
    ) -> std::result::Result<Uuid, ContractEventWriteOnceError>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        let event_type = event.event_type();
        let envelope =
            ContractEventEnvelope::new_with_envelope_id(envelope_id, tenant_id, actor_id, event)
                .map_err(|error| {
                    tracing::error!(
                        envelope_id = %envelope_id,
                        event_type,
                        error = %error,
                        "Canonical contract write-once envelope construction failed"
                    );
                    ContractEventWriteOnceError::Unavailable
                })?;

        OutboxTransport::write_contract_envelope_once_in_tx(txn, envelope).await
    }

    /// Writes one caused sealed contract exactly once using an existing owner
    /// identity as the canonical envelope UUID.
    ///
    /// Exact replay requires the same non-nil causal predecessor in addition to
    /// envelope scope and typed payload. Reusing the envelope UUID with another
    /// causation UUID is a conflict. Delivery still starts only after the owner
    /// transaction commits and remains owned by `OutboxRelay`.
    pub async fn publish_contract_once_direct_in_tx_with_envelope_id_and_causation<C, E>(
        txn: &C,
        envelope_id: Uuid,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Uuid,
        event: E,
    ) -> std::result::Result<Uuid, ContractEventWriteOnceError>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        let event_type = event.event_type();
        let envelope = ContractEventEnvelope::new_with_envelope_id_and_causation(
            envelope_id,
            tenant_id,
            actor_id,
            causation_id,
            event,
        )
        .map_err(|error| {
            tracing::error!(
                envelope_id = %envelope_id,
                causation_id = %causation_id,
                event_type,
                error = %error,
                "Canonical caused contract write-once envelope construction failed"
            );
            ContractEventWriteOnceError::Unavailable
        })?;

        OutboxTransport::write_contract_envelope_once_in_tx(txn, envelope).await
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
        let envelope = build_contract_envelope(tenant_id, actor_id, None, event)?;
        self.write_contract_envelope_in_tx(txn, envelope).await
    }

    /// Publishes a sealed typed contract caused by one exact predecessor
    /// envelope and discards only the new typed envelope identity.
    pub async fn publish_contract_in_tx_with_causation<C, E>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Uuid,
        event: E,
    ) -> Result<()>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        self.publish_contract_in_tx_with_causation_and_envelope_id(
            txn,
            tenant_id,
            actor_id,
            causation_id,
            event,
        )
        .await
        .map(|_| ())
    }

    /// Publishes a sealed typed contract caused by one exact predecessor
    /// envelope and returns the typed envelope identity written by the same
    /// transaction.
    pub async fn publish_contract_in_tx_with_causation_and_envelope_id<C, E>(
        &self,
        txn: &C,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        causation_id: Uuid,
        event: E,
    ) -> Result<Uuid>
    where
        C: ConnectionTrait,
        E: EventContract,
    {
        let envelope = build_contract_envelope(tenant_id, actor_id, Some(causation_id), event)?;
        self.write_contract_envelope_in_tx(txn, envelope).await
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

    async fn write_contract_envelope_in_tx<C>(
        &self,
        txn: &C,
        envelope: ContractEventEnvelope,
    ) -> Result<Uuid>
    where
        C: ConnectionTrait,
    {
        let envelope_id = envelope.id();
        let outbox = self
            .transport
            .as_any()
            .downcast_ref::<OutboxTransport>()
            .ok_or_else(|| transactional_transport_required(&*self.transport))?;
        outbox.write_contract_to_outbox(txn, envelope).await?;
        Ok(envelope_id)
    }
}

fn build_contract_envelope<E>(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    causation_id: Option<Uuid>,
    event: E,
) -> Result<ContractEventEnvelope>
where
    E: EventContract,
{
    let event_type = event.event_type();
    let envelope = match causation_id {
        Some(causation_id) => {
            ContractEventEnvelope::new_caused_by(tenant_id, actor_id, causation_id, event)
        }
        None => ContractEventEnvelope::new(tenant_id, actor_id, event),
    };
    envelope.map_err(|error| {
        tracing::error!(event_type, error = %error, "Event contract encoding failed");
        rustok_core::Error::Validation(format!("Event contract encoding failed: {error}"))
    })
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
