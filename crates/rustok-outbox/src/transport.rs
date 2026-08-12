use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, Set,
    sea_query::OnConflict,
};
use std::any::Any;
use uuid::Uuid;

use rustok_core::events::{EventTransport, ReliabilityLevel};
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventContractEnvelopeError, EventEnvelope};

use crate::entity;
use crate::entity::SysEventStatus;
use crate::ports::TransactionalEventWriter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContractEventWriteOnceError {
    /// The requested envelope identity already belongs to different canonical facts.
    Conflict,
    /// The canonical row could not be written or read safely.
    Unavailable,
}

#[derive(Clone, Debug)]
pub struct OutboxTransport {
    db: DatabaseConnection,
}

impl OutboxTransport {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Writes one validated root event through an owner-supplied transaction.
    ///
    /// This static boundary is used by the validated transactional bus. Keeping
    /// it crate-private avoids adding a second public raw-envelope entry point.
    /// New external domain code must publish through `TransactionalEventBus`;
    /// the existing instance compatibility API remains unchanged.
    pub(crate) async fn write_envelope_in_tx<C>(txn: &C, envelope: EventEnvelope) -> Result<()>
    where
        C: ConnectionTrait,
    {
        entity::Entity::insert(Self::model_from_envelope(envelope)?)
            .exec_without_returning(txn)
            .await?;
        Ok(())
    }

    /// Writes one validated sealed contract event through an owner transaction.
    pub(crate) async fn write_contract_envelope_in_tx<C>(
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

    /// Writes one sealed contract exactly once under its caller-supplied envelope ID.
    ///
    /// The primary-key insert is concurrency-safe. An existing row is accepted
    /// only when its validated envelope scope and typed payload exactly match
    /// the requested publication. Timestamp and trace metadata are deliberately
    /// ignored because they are generated when the first writer wins.
    pub(crate) async fn write_contract_envelope_once_in_tx<C>(
        txn: &C,
        envelope: ContractEventEnvelope,
    ) -> std::result::Result<Uuid, ContractEventWriteOnceError>
    where
        C: ConnectionTrait,
    {
        let envelope_id = envelope.id();
        let event_type = envelope.event_type().to_string();
        let active_model = Self::model_from_contract_envelope(envelope.clone())
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;

        entity::Entity::insert(active_model)
            .on_conflict(
                OnConflict::column(entity::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(txn)
            .await
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;

        let stored = entity::Entity::find_by_id(envelope_id)
            .one(txn)
            .await
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?
            .ok_or_else(|| {
                write_once_unavailable(
                    envelope_id,
                    event_type.as_str(),
                    "canonical row missing after write-once insert",
                )
            })?;

        if stored.event_type.as_str() != envelope.event_type()
            || stored.schema_version
                != i16::try_from(envelope.schema_version()).map_err(|error| {
                    write_once_unavailable(envelope_id, event_type.as_str(), error)
                })?
        {
            return Err(ContractEventWriteOnceError::Conflict);
        }

        let stored_envelope: ContractEventEnvelope = serde_json::from_value(stored.payload.clone())
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;
        stored_envelope
            .validate_registered_schema()
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;

        if stored_envelope.id() != stored.id
            || stored_envelope.event_type() != stored.event_type.as_str()
            || i16::try_from(stored_envelope.schema_version()).ok() != Some(stored.schema_version)
        {
            return Err(write_once_unavailable(
                envelope_id,
                event_type.as_str(),
                "canonical row metadata does not match its decoded envelope",
            ));
        }

        if same_contract_publication(&stored_envelope, &envelope)
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?
        {
            Ok(envelope_id)
        } else {
            Err(ContractEventWriteOnceError::Conflict)
        }
    }

    pub async fn write_to_outbox<C>(&self, txn: &C, envelope: EventEnvelope) -> Result<()>
    where
        C: ConnectionTrait,
    {
        Self::write_envelope_in_tx(txn, envelope).await
    }

    pub async fn write_contract_to_outbox<C>(
        &self,
        txn: &C,
        envelope: ContractEventEnvelope,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        Self::write_contract_envelope_in_tx(txn, envelope).await
    }

    fn model_from_envelope(envelope: EventEnvelope) -> Result<entity::ActiveModel> {
        envelope
            .validate_registered_schema()
            .map_err(|error| Error::Validation(error.to_string()))?;

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

fn same_contract_publication(
    stored: &ContractEventEnvelope,
    expected: &ContractEventEnvelope,
) -> std::result::Result<bool, EventContractEnvelopeError> {
    if stored.id() != expected.id()
        || stored.correlation_id() != expected.correlation_id()
        || stored.causation_id() != expected.causation_id()
        || stored.tenant_id() != expected.tenant_id()
        || stored.actor_id() != expected.actor_id()
        || stored.event_type() != expected.event_type()
        || stored.schema_version() != expected.schema_version()
    {
        return Ok(false);
    }

    Ok(stored.payload()? == expected.payload()?)
}

fn write_once_unavailable(
    envelope_id: Uuid,
    event_type: &str,
    error: impl std::fmt::Display,
) -> ContractEventWriteOnceError {
    tracing::error!(
        envelope_id = %envelope_id,
        event_type,
        error = %error,
        "Canonical contract write-once operation is unavailable"
    );
    ContractEventWriteOnceError::Unavailable
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

    use super::{OutboxTransport, same_contract_publication};

    fn envelope() -> EventEnvelope {
        EventEnvelope::new(
            Uuid::new_v4(),
            None,
            DomainEvent::UserLoggedIn {
                user_id: Uuid::new_v4(),
            },
        )
    }

    fn contract_event(terms_version: i32) -> MarketplaceListingEvent {
        MarketplaceListingEvent::MarketplaceListingCreated {
            listing_id: Uuid::new_v4(),
            seller_id: Uuid::new_v4(),
            master_product_id: Uuid::new_v4(),
            master_variant_id: Uuid::new_v4(),
            market_slug: "us-market".to_string(),
            channel_slug: "web-store".to_string(),
            terms_version,
        }
    }

    #[test]
    fn rejects_an_unregistered_event_type() {
        let mut envelope = envelope();
        envelope.event_type = "wrong.event".to_string();

        let error = OutboxTransport::model_from_envelope(envelope)
            .expect_err("unregistered event type must be rejected");
        assert!(error.to_string().contains("is not registered"));
    }

    #[test]
    fn rejects_schema_version_mismatch() {
        let mut envelope = envelope();
        envelope.schema_version = envelope.schema_version.saturating_add(1);

        let error = OutboxTransport::model_from_envelope(envelope)
            .expect_err("schema version mismatch must be rejected");
        assert!(error.to_string().contains("schema version mismatch"));
    }

    #[test]
    fn accepts_sealed_marketplace_listing_contract_envelope() {
        let envelope =
            ContractEventEnvelope::new(Uuid::new_v4(), Some(Uuid::new_v4()), contract_event(1))
                .expect("valid marketplace listing contract envelope");

        let model = OutboxTransport::model_from_contract_envelope(envelope)
            .expect("contract envelope should map to outbox row");
        assert_eq!(model.event_type.unwrap(), "marketplace.listing.created");
        assert_eq!(model.schema_version.unwrap(), 1);
    }

    #[test]
    fn write_once_comparison_ignores_generated_timestamp_and_trace_only() {
        let envelope_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let event = contract_event(1);
        let first = ContractEventEnvelope::new_with_envelope_id(
            envelope_id,
            tenant_id,
            Some(actor_id),
            event.clone(),
        )
        .expect("first contract envelope");
        let replay = ContractEventEnvelope::new_with_envelope_id(
            envelope_id,
            tenant_id,
            Some(actor_id),
            event,
        )
        .expect("replay contract envelope");

        assert!(same_contract_publication(&first, &replay).unwrap());
    }

    #[test]
    fn write_once_comparison_rejects_scope_or_payload_reuse() {
        let envelope_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let event = contract_event(1);
        let expected = ContractEventEnvelope::new_with_envelope_id(
            envelope_id,
            tenant_id,
            Some(actor_id),
            event.clone(),
        )
        .expect("expected contract envelope");
        let different_actor = ContractEventEnvelope::new_with_envelope_id(
            envelope_id,
            tenant_id,
            Some(Uuid::new_v4()),
            event,
        )
        .expect("different actor envelope");
        let different_payload = ContractEventEnvelope::new_with_envelope_id(
            envelope_id,
            tenant_id,
            Some(actor_id),
            contract_event(2),
        )
        .expect("different payload envelope");

        assert!(!same_contract_publication(&expected, &different_actor).unwrap());
        assert!(!same_contract_publication(&expected, &different_payload).unwrap());
    }
}
