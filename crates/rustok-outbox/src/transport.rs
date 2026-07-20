use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set,
};
use std::any::Any;

use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventEnvelope};

use crate::entity;
use crate::entity::SysEventStatus;
use crate::ports::TransactionalEventWriter;

#[derive(Clone, Debug)]
pub struct OutboxTransport {
    db: DatabaseConnection,
}

impl OutboxTransport {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn write_to_outbox<C>(&self, txn: &C, envelope: EventEnvelope) -> Result<()>
    where
        C: ConnectionTrait,
    {
        entity::Entity::insert(Self::model_from_envelope(envelope)?)
            .exec_without_returning(txn)
            .await?;
        Ok(())
    }

    pub async fn write_contract_to_outbox<C>(
        &self,
        txn: &C,
        envelope: ContractEventEnvelope,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        entity::Entity::insert(Self::model_from_contract_envelope(envelope)?)
            .exec_without_returning(txn)
            .await?;
        Ok(())
    }

    fn model_from_envelope(envelope: EventEnvelope) -> Result<entity::ActiveModel> {
        let expected_event_type = envelope.event.event_type();
        if envelope.event_type != expected_event_type {
            return Err(Error::Validation(format!(
                "outbox envelope event_type mismatch: envelope=`{}`, event=`{expected_event_type}`",
                envelope.event_type
            )));
        }

        let expected_schema_version = envelope.event.schema_version();
        if envelope.schema_version != expected_schema_version {
            return Err(Error::Validation(format!(
                "outbox envelope schema_version mismatch: envelope={}, event={expected_schema_version}",
                envelope.schema_version
            )));
        }

        let payload = serde_json::to_value(&envelope)?;
        Self::model(
            envelope.id,
            envelope.event_type,
            envelope.schema_version,
            payload,
        )
    }

    fn model_from_contract_envelope(
        envelope: ContractEventEnvelope,
    ) -> Result<entity::ActiveModel> {
        envelope
            .validate_registered_schema()
            .map_err(|error| Error::Validation(error.to_string()))?;
        let id = envelope.id();
        let event_type = envelope.event_type().to_string();
        let schema_version = envelope.schema_version();
        let payload = serde_json::to_value(&envelope)?;
        Self::model(id, event_type, schema_version, payload)
    }

    fn model(
        id: uuid::Uuid,
        event_type: String,
        schema_version: u16,
        payload: serde_json::Value,
    ) -> Result<entity::ActiveModel> {
        let schema_version = i16::try_from(schema_version).map_err(|_| {
            Error::Validation(format!(
                "outbox schema_version {schema_version} exceeds database SMALLINT range"
            ))
        })?;

        Ok(entity::ActiveModel {
            id: Set(id),
            event_type: Set(event_type),
            schema_version: Set(schema_version),
            payload: Set(payload),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(Utc::now()),
            dispatched_at: Set(None),
        })
    }
}

#[async_trait]
impl TransactionalEventWriter for OutboxTransport {
    async fn write_event(
        &self,
        transaction: &DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> Result<()> {
        self.write_to_outbox(transaction, envelope).await
    }
}

#[async_trait]
impl EventTransport for OutboxTransport {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        entity::Entity::insert(Self::model_from_envelope(envelope)?)
            .exec_without_returning(&self.db)
            .await?;
        Ok(())
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> Result<()> {
        let mut model: entity::ActiveModel = entity::Entity::find_by_id(event_id)
            .one(&self.db)
            .await?
            .ok_or_else(|| Error::NotFound(format!("sys_event {event_id}")))?
            .into();
        model.status = Set(SysEventStatus::Dispatched);
        model.dispatched_at = Set(Some(Utc::now()));
        model.claimed_by = Set(None);
        model.claimed_at = Set(None);
        model.last_error = Set(None);
        model.next_attempt_at = Set(None);
        model.update(&self.db).await?;
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use rustok_events::{
        ContractEventEnvelope, DomainEvent, EventEnvelope, MarketplaceListingEvent,
    };
    use uuid::Uuid;

    use super::OutboxTransport;

    fn envelope() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            None,
            DomainEvent::UserLoggedIn {
                user_id: Uuid::new_v4(),
            },
        )
    }

    #[test]
    fn rejects_event_type_mismatch() {
        let mut envelope = envelope();
        envelope.event_type = "wrong.event".to_string();

        let error = OutboxTransport::model_from_envelope(envelope)
            .expect_err("event type mismatch must be rejected");
        assert!(error.to_string().contains("event_type mismatch"));
    }

    #[test]
    fn rejects_schema_version_mismatch() {
        let mut envelope = envelope();
        envelope.schema_version = envelope.schema_version.saturating_add(1);

        let error = OutboxTransport::model_from_envelope(envelope)
            .expect_err("schema version mismatch must be rejected");
        assert!(error.to_string().contains("schema_version mismatch"));
    }

    #[test]
    fn accepts_sealed_marketplace_listing_contract_envelope() {
        let envelope = ContractEventEnvelope::new(
            Uuid::new_v4(),
            Some(Uuid::new_v4()),
            MarketplaceListingEvent::MarketplaceListingCreated {
                listing_id: Uuid::new_v4(),
                seller_id: Uuid::new_v4(),
                master_product_id: Uuid::new_v4(),
                master_variant_id: Uuid::new_v4(),
                market_slug: "us-market".to_string(),
                channel_slug: "web-store".to_string(),
                terms_version: 1,
            },
        )
        .expect("valid marketplace listing contract envelope");

        let model = OutboxTransport::model_from_contract_envelope(envelope)
            .expect("contract envelope should map to outbox row");
        assert_eq!(model.event_type.unwrap(), "marketplace.listing.created");
        assert_eq!(model.schema_version.unwrap(), 1);
    }
}
