use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter, Set, TransactionTrait, TryIntoModel, sea_query::{Expr, OnConflict},
};
use std::any::Any;
use uuid::Uuid;

use rustok_core::events::{EventTransport, ReliabilityLevel, TransactionalEventWriter};
use rustok_core::{Error, Result};
use rustok_events::{ContractEventEnvelope, EventContractEnvelopeError, EventEnvelope};

use crate::entity;
use crate::entity::SysEventStatus;

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
        let active = Self::model_from_envelope(envelope, crate::database_now(txn).await?)?;
        insert_exact_event(txn, active).await
    }

    /// Writes one validated sealed contract event through an owner transaction.
    pub(crate) async fn write_contract_envelope_in_tx<C>(
        txn: &C,
        envelope: ContractEventEnvelope,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let active = Self::model_from_contract_envelope(envelope, crate::database_now(txn).await?)?;
        insert_exact_event(txn, active).await
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
        if envelope_id.is_nil() {
            return Err(write_once_unavailable(
                envelope_id,
                envelope.event_type(),
                "write-once envelope id must not be nil",
            ));
        }
        let event_type = envelope.event_type().to_string();
        let now = crate::database_now(txn).await
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;
        let active_model = Self::model_from_contract_envelope(envelope.clone(), now)
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;

        let expected = active_model.clone().try_into_model()
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;
        let affected = entity::Entity::insert(active_model)
            .on_conflict(
                OnConflict::column(entity::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(txn)
            .await
            .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;
        if affected == 1 {
            verify_inserted_event(txn, &expected).await
                .map_err(|error| write_once_unavailable(envelope_id, event_type.as_str(), error))?;
            return Ok(envelope_id);
        }
        if affected != 0 {
            return Err(write_once_unavailable(envelope_id, event_type.as_str(),
                format!("unexpected affected row count: {affected}")));
        }

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

        if !stored.matches_envelope_identity(stored_envelope.id(), stored_envelope.tenant_id())
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

    fn model_from_envelope(envelope: EventEnvelope, now: DateTime<Utc>) -> Result<entity::ActiveModel> {
        envelope
            .validate_registered_schema()
            .map_err(|error| Error::Validation(error.to_string()))?;

        let payload = serde_json::to_value(&envelope)?;
        Self::model(
            envelope.id,
            Some(envelope.tenant_id),
            envelope.event_type,
            envelope.schema_version,
            payload,
            now,
        )
    }

    fn model_from_contract_envelope(
        envelope: ContractEventEnvelope,
        now: DateTime<Utc>,
    ) -> Result<entity::ActiveModel> {
        envelope
            .validate_registered_schema()
            .map_err(|error| Error::Validation(error.to_string()))?;
        let id = envelope.id();
        let tenant_id = envelope.tenant_id();
        let event_type = envelope.event_type().to_string();
        let schema_version = envelope.schema_version();
        let payload = serde_json::to_value(&envelope)?;
        Self::model(id, Some(tenant_id), event_type, schema_version, payload, now)
    }

    fn model(
        id: uuid::Uuid,
        tenant_id: Option<uuid::Uuid>,
        event_type: String,
        schema_version: u16,
        payload: serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<entity::ActiveModel> {
        if id.is_nil() {
            return Err(Error::Validation(
                "outbox event id must not be nil".to_string(),
            ));
        }
        // A nil tenant UUID denotes platform scope, same as no tenant.
        let tenant_id = tenant_id.filter(|tenant_id| !tenant_id.is_nil());
        if event_type.trim().is_empty() || event_type.len() > 255 {
            return Err(Error::Validation(
                "outbox event_type must contain 1 to 255 bytes".to_string(),
            ));
        }
        let schema_version = i16::try_from(schema_version).map_err(|_| {
            Error::Validation(format!(
                "outbox schema_version {schema_version} exceeds database SMALLINT range"
            ))
        })?;

        Ok(entity::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            event_type: Set(event_type),
            schema_version: Set(schema_version),
            payload: Set(payload),
            status: Set(SysEventStatus::Pending),
            retry_count: Set(0),
            next_attempt_at: Set(None),
            last_error: Set(None),
            claimed_by: Set(None),
            claimed_at: Set(None),
            created_at: Set(now),
            dispatched_at: Set(None),
        })
    }
}

// Unconditional publication is not write-once replay. A suppressed/zero-row insert
// cannot acknowledge an owner mutation whose required event was never persisted.
fn require_inserted_event(affected: u64) -> Result<()> {
    if affected != 1 {
        return Err(sea_orm::DbErr::RecordNotInserted.into());
    }
    Ok(())
}

// The caller owns the transaction and must roll it back on any write error.
async fn insert_exact_event(txn: &impl ConnectionTrait, active: entity::ActiveModel) -> Result<()> {
    let expected = active.clone().try_into_model()?;
    let affected = entity::Entity::insert(active).exec_without_returning(txn).await?;
    require_inserted_event(affected)?;
    verify_inserted_event(txn, &expected).await
}

async fn verify_inserted_event(txn: &impl ConnectionTrait, expected: &entity::Model) -> Result<()> {
    let stored = entity::Entity::find_by_id(expected.id).one(txn).await?;
    if stored.as_ref() != Some(expected) {
        return Err(Error::External(format!(
            "outbox event {} insert readback mismatch", expected.id
        )));
    }
    Ok(())
}

fn same_root_publication(stored: &EventEnvelope, expected: &EventEnvelope) -> bool {
    stored.id == expected.id
        && stored.correlation_id == expected.correlation_id
        && stored.causation_id == expected.causation_id
        && stored.tenant_id == expected.tenant_id
        && stored.actor_id == expected.actor_id
        && stored.event_type == expected.event_type
        && stored.schema_version == expected.schema_version
        && stored.event == expected.event
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
        let envelope_id = envelope.id;
        let event_type = envelope.event_type.clone();
        let schema_version = i16::try_from(envelope.schema_version).map_err(|_| {
            Error::Validation(format!(
                "outbox schema_version {} exceeds database SMALLINT range",
                envelope.schema_version
            ))
        })?;
        let payload_envelope = envelope.clone();
        let active_model = Self::model_from_envelope(envelope, crate::database_now(&self.db).await?)?;
        let expected = active_model.clone().try_into_model()?;
        // Keep insertion and readback in one transaction; otherwise an
        // altered insert can commit before its integrity is checked.
        let txn = self.db.begin().await?;

        let affected = entity::Entity::insert(active_model)
            .on_conflict(
                OnConflict::column(entity::Column::Id)
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(&txn)
            .await?;
        if affected == 1 {
            verify_inserted_event(&txn, &expected).await?;
            txn.commit().await?;
            return Ok(());
        }
        if affected != 0 {
            return Err(Error::Internal(format!(
                "outbox write for event {envelope_id} affected {affected} rows"
            )));
        }

        let stored = entity::Entity::find_by_id(envelope_id)
            .one(&txn)
            .await?
            .ok_or_else(|| Error::NotFound(format!("sys_event {envelope_id}")))?;
        // Idempotent replay compares canonical facts only. Generated
        // metadata (`timestamp`, `trace_id`, `retry_count`) legitimately
        // differs when a producer rebuilds the same logical event, exactly
        // as in the contract write-once comparison.
        let stored_envelope: EventEnvelope = serde_json::from_value(stored.payload.clone())
            .map_err(|_| {
                Error::Validation(format!(
                    "outbox event id {envelope_id} already belongs to different canonical facts"
                ))
            })?;
        if !stored.matches_envelope_identity(stored_envelope.id, stored_envelope.tenant_id)
            || stored.event_type != event_type
            || stored.schema_version != schema_version
            || !same_root_publication(&stored_envelope, &payload_envelope)
        {
            return Err(Error::Validation(format!(
                "outbox event id {envelope_id} already belongs to different canonical facts"
            )));
        }
        txn.commit().await?;
        Ok(())
    }

    async fn acknowledge(&self, event_id: uuid::Uuid) -> Result<()> {
        if event_id.is_nil() {
            return Err(Error::Validation(
                "outbox event_id must not be nil".to_string(),
            ));
        }
        // Keep the ACK transition and exact readback in one transaction.
        // Relay claims and unresolved attempts are never stolen by this path.
        let txn = self.db.begin().await?;
        let now = crate::database_now(&txn).await?;
        let result = entity::Entity::update_many()
            .filter(entity::Column::Id.eq(event_id))
            .filter(entity::Column::Status.eq(SysEventStatus::Pending))
            .filter(entity::Column::ClaimedBy.is_null())
            .filter(entity::Column::ClaimedAt.is_null())
            .filter(Expr::cust("NOT EXISTS (SELECT 1 FROM outbox_dispatch_attempts WHERE event_id = sys_events.id AND state = 'open')"))
            .set(entity::ActiveModel {
                status: Set(SysEventStatus::Dispatched),
                dispatched_at: Set(Some(now)),
                claimed_by: Set(None),
                claimed_at: Set(None),
                last_error: Set(None),
                next_attempt_at: Set(None),
                ..Default::default()
            })
            .exec(&txn)
            .await?;
        if result.rows_affected == 1 {
            let stored = entity::Entity::find_by_id(event_id).one(&txn).await?
                .ok_or_else(|| Error::NotFound(format!("sys_event {event_id}")))?;
            if stored.status != SysEventStatus::Dispatched
                || stored.dispatched_at != Some(now)
                || stored.claimed_by.is_some()
                || stored.claimed_at.is_some()
                || stored.last_error.is_some()
                || stored.next_attempt_at.is_some()
            {
                return Err(Error::External(format!("outbox event {event_id} ACK readback mismatch")));
            }
            txn.commit().await?;
            return Ok(());
        }
        if result.rows_affected != 0 {
            return Err(Error::External(format!("outbox event {event_id} ACK affected an unexpected row count")));
        }
        // Status alone is not proof of completion: an unresolved attempt
        // must also block the idempotent ACK path. Read both in one snapshot.
        if entity::Entity::find_by_id(event_id)
            .filter(entity::Column::Status.eq(SysEventStatus::Dispatched))
            .filter(entity::Column::ClaimedBy.is_null())
            .filter(entity::Column::ClaimedAt.is_null())
            .filter(Expr::cust("NOT EXISTS (SELECT 1 FROM outbox_dispatch_attempts WHERE event_id = sys_events.id AND state = 'open')"))
            .one(&txn).await?.is_some()
        {
            txn.commit().await?;
            return Ok(());
        }
        entity::Entity::find_by_id(event_id)
            .one(&txn)
            .await?
            .ok_or_else(|| Error::NotFound(format!("sys_event {event_id}")))?;
        Err(Error::Validation(format!(
            "outbox event {event_id} cannot be acknowledged from its current state"
        )))
    }

    fn supports_transactional_writes(&self) -> bool {
        true
    }

    async fn write_root_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        envelope: EventEnvelope,
    ) -> Result<()> {
        self.write_to_outbox(transaction, envelope).await
    }

    async fn write_contract_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        envelope: ContractEventEnvelope,
    ) -> Result<()> {
        self.write_contract_to_outbox(transaction, envelope).await
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

    use super::{
        OutboxTransport, require_inserted_event, same_contract_publication, same_root_publication,
    };

    #[test]
    fn unconditional_publication_rejects_zero_or_multiple_inserted_rows() {
        assert!(require_inserted_event(0).is_err());
        assert!(require_inserted_event(2).is_err());
    }

    #[test]
    fn unconditional_publication_accepts_exactly_one_inserted_row() {
        require_inserted_event(1).expect("one durable canonical event");
    }

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
    fn root_replay_comparison_ignores_generated_metadata() {
        let first = envelope();
        let mut replay = first.clone();
        replay.timestamp = chrono::Utc::now();
        replay.trace_id = Some("another-trace".to_string());
        replay.retry_count = 3;
        assert!(same_root_publication(&first, &replay));

        let mut different_facts = first.clone();
        different_facts.event = DomainEvent::UserLoggedIn {
            user_id: Uuid::new_v4(),
        };
        assert!(!same_root_publication(&first, &different_facts));
    }

    #[test]
    fn rejects_an_unregistered_event_type() {
        let mut envelope = envelope();
        envelope.event_type = "wrong.event".to_string();

        let error = OutboxTransport::model_from_envelope(envelope, chrono::Utc::now())
            .expect_err("unregistered event type must be rejected");
        assert!(error.to_string().contains("is not registered"));
    }

    #[test]
    fn rejects_schema_version_mismatch() {
        let mut envelope = envelope();
        envelope.schema_version = envelope.schema_version.saturating_add(1);

        let error = OutboxTransport::model_from_envelope(envelope, chrono::Utc::now())
            .expect_err("schema version mismatch must be rejected");
        assert!(error.to_string().contains("schema version mismatch"));
    }

    #[test]
    fn accepts_sealed_marketplace_listing_contract_envelope() {
        let envelope =
            ContractEventEnvelope::new(Uuid::new_v4(), Some(Uuid::new_v4()), contract_event(1))
                .expect("valid marketplace listing contract envelope");

        let model = OutboxTransport::model_from_contract_envelope(envelope, chrono::Utc::now())
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
