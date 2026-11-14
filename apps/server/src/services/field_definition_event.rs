use rustok_core::field_schema::FlexError;
use rustok_events::EventEnvelope;
use sea_orm::DatabaseTransaction;

/// Persist a field-definition event in the same transaction as its owner write.
pub(crate) async fn persist(
    transaction: &DatabaseTransaction,
    event: EventEnvelope,
) -> Result<(), FlexError> {
    let event_type = event.event_type.clone();
    rustok_outbox::TransactionalEventBus::publish_root_envelope_in_tx(transaction, event)
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::error!(%error, event_type, "Failed to persist field-definition event");
            FlexError::Database(format!("failed to persist field-definition event: {error}"))
        })
}
