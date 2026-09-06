use uuid::Uuid;

use crate::{
    CacheInvalidationMessage, DurableCacheInvalidationError, DurableCacheInvalidationRecord,
    VersionedCacheInvalidation,
};

/// Convert an outbox-backed durable record to the compact Redis/local invalidation payload.
///
/// Source event, tenant, cause and trace metadata remain in the durable record. The fan-out
/// message carries only channel, key, generation and emission time, which are sufficient for
/// gap detection and cache recovery.
pub fn durable_invalidation_to_message(
    record: &DurableCacheInvalidationRecord,
) -> Result<CacheInvalidationMessage, DurableCacheInvalidationError> {
    record
        .to_versioned_invalidation()?
        .to_message()
        .map_err(DurableCacheInvalidationError::Invalidation)
}

/// Rebuild a durable record from a validated transport message and outbox metadata.
#[allow(clippy::too_many_arguments)]
pub fn durable_invalidation_from_message(
    source_event_id: Uuid,
    tenant_id: Option<Uuid>,
    cause: impl Into<String>,
    trace_id: Option<String>,
    message: &CacheInvalidationMessage,
) -> Result<DurableCacheInvalidationRecord, DurableCacheInvalidationError> {
    let event = VersionedCacheInvalidation::from_message(message)
        .map_err(DurableCacheInvalidationError::Invalidation)?;
    DurableCacheInvalidationRecord::new(
        source_event_id,
        tenant_id,
        event.channel,
        event.key,
        event.generation,
        event.emitted_at_unix_ms,
        cause,
        trace_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durable_record_round_trips_through_compact_transport_message() {
        let record = DurableCacheInvalidationRecord::new(
            Uuid::from_u128(17),
            Some(Uuid::from_u128(23)),
            "tenant.invalidate",
            "tenant:42",
            12,
            5_000,
            "tenant.updated",
            Some("trace-abc".to_string()),
        )
        .unwrap();

        let message = durable_invalidation_to_message(&record).unwrap();
        assert!(!message.key.contains("tenant.updated"));
        assert!(!message.key.contains("trace-abc"));

        let rebuilt = durable_invalidation_from_message(
            record.source_event_id(),
            record.tenant_id(),
            record.cause(),
            record.trace_id().map(ToOwned::to_owned),
            &message,
        )
        .unwrap();

        assert_eq!(rebuilt, record);
        assert_eq!(rebuilt.idempotency_key(), record.idempotency_key());
    }

    #[test]
    fn malformed_transport_payload_is_rejected_before_record_creation() {
        let message = CacheInvalidationMessage::new("tenant.invalidate", "v999:1:2:00");
        let error = durable_invalidation_from_message(
            Uuid::from_u128(1),
            None,
            "tenant.updated",
            None,
            &message,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            DurableCacheInvalidationError::Invalidation(_)
        ));
    }
}
