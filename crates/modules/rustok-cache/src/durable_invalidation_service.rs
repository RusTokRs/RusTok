use crate::{
    CacheInvalidationOutcome, CacheInvalidationService, DurableCacheInvalidationError,
    DurableCacheInvalidationRecord, durable_invalidation_to_message,
};

impl CacheInvalidationService {
    /// Publish a validated durable invalidation through local subscribers and Redis pub/sub.
    ///
    /// Durable persistence remains the caller's responsibility and must happen in the mutation's
    /// transaction. This method is the fan-out step used by an outbox relay after commit.
    pub async fn publish_durable(
        &self,
        record: &DurableCacheInvalidationRecord,
    ) -> Result<CacheInvalidationOutcome, DurableCacheInvalidationError> {
        let message = durable_invalidation_to_message(record)?;
        Ok(self.publish(message).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[tokio::test]
    async fn durable_publish_reaches_local_subscribers_with_generation_payload() {
        let service = crate::CacheService::from_url(None).invalidations();
        let mut subscriber = service.subscribe_local_channel("tenant.invalidate");
        let record = DurableCacheInvalidationRecord::new(
            Uuid::from_u128(11),
            Some(Uuid::from_u128(42)),
            "tenant.invalidate",
            "tenant:42",
            7,
            1_000,
            "tenant.updated",
            None,
        )
        .unwrap();

        let outcome = service.publish_durable(&record).await.unwrap();
        assert_eq!(outcome.local_subscribers, 1);

        let message = subscriber.recv().await.unwrap();
        let decoded = crate::VersionedCacheInvalidation::from_message(&message).unwrap();
        assert_eq!(decoded.generation, 7);
        assert_eq!(decoded.key, "tenant:42");
    }
}
