use std::future::Future;

use crate::{
    BoundedInvalidationTrackerError, DurableCacheInvalidationConsumer,
    DurableCacheInvalidationError, DurableCacheInvalidationRecord, DurableInvalidationDecision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableInvalidationProcessOutcome {
    Applied { generation: u64 },
    Ignored { generation: u64 },
    Recovered { generation: u64 },
}

impl DurableCacheInvalidationConsumer {
    /// Execute one durable record and advance its offset only after the handler succeeds.
    ///
    /// Both callbacks receive an owned record so their futures do not borrow process-local tracker
    /// state. A failed callback leaves the acknowledged offset unchanged and the record remains
    /// eligible for retry.
    pub async fn process<A, AFut, R, RFut>(
        &self,
        record: DurableCacheInvalidationRecord,
        apply: A,
        recover: R,
    ) -> Result<DurableInvalidationProcessOutcome, DurableInvalidationProcessError>
    where
        A: FnOnce(DurableCacheInvalidationRecord) -> AFut,
        AFut: Future<Output = rustok_core::Result<()>>,
        R: FnOnce(DurableCacheInvalidationRecord) -> RFut,
        RFut: Future<Output = rustok_core::Result<()>>,
    {
        match self
            .decide(&record)
            .map_err(DurableInvalidationProcessError::Record)?
        {
            DurableInvalidationDecision::Apply { generation } => {
                apply(record.clone())
                    .await
                    .map_err(DurableInvalidationProcessError::Handler)?;
                self.acknowledge_applied(record.channel(), generation)
                    .map_err(DurableInvalidationProcessError::Tracker)?;
                Ok(DurableInvalidationProcessOutcome::Applied { generation })
            }
            DurableInvalidationDecision::Ignore { generation } => {
                Ok(DurableInvalidationProcessOutcome::Ignored { generation })
            }
            DurableInvalidationDecision::RecoverThrough { generation } => {
                recover(record.clone())
                    .await
                    .map_err(DurableInvalidationProcessError::Handler)?;
                self.acknowledge_recovery(record.channel(), generation)
                    .map_err(DurableInvalidationProcessError::Tracker)?;
                Ok(DurableInvalidationProcessOutcome::Recovered { generation })
            }
        }
    }
}

#[derive(Debug)]
pub enum DurableInvalidationProcessError {
    Record(DurableCacheInvalidationError),
    Tracker(BoundedInvalidationTrackerError),
    Handler(rustok_core::Error),
}

impl std::fmt::Display for DurableInvalidationProcessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Record(error) => write!(formatter, "invalid durable cache record: {error}"),
            Self::Tracker(error) => write!(formatter, "durable cache offset update failed: {error}"),
            Self::Handler(error) => write!(formatter, "durable cache handler failed: {error}"),
        }
    }
}

impl std::error::Error for DurableInvalidationProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            Self::Tracker(error) => Some(error),
            Self::Handler(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use uuid::Uuid;

    fn record(generation: u64) -> DurableCacheInvalidationRecord {
        DurableCacheInvalidationRecord::new(
            Uuid::from_u128(generation as u128 + 1),
            Some(Uuid::from_u128(42)),
            "tenant.invalidate",
            "tenant:42",
            generation,
            1_000 + generation,
            "tenant.updated",
            None,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn successful_apply_advances_offset_after_handler() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 3).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let apply_calls = Arc::clone(&calls);

        let outcome = consumer
            .process(
                record(4),
                move |_| async move {
                    apply_calls.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
                |_| async { Ok(()) },
            )
            .await
            .unwrap();

        assert_eq!(
            outcome,
            DurableInvalidationProcessOutcome::Applied { generation: 4 }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(4));
    }

    #[tokio::test]
    async fn failed_apply_keeps_offset_for_retry() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 3).unwrap();

        let error = consumer
            .process(
                record(4),
                |_| async {
                    Err(rustok_core::Error::Cache(
                        "simulated invalidation failure".to_string(),
                    ))
                },
                |_| async { Ok(()) },
            )
            .await
            .unwrap_err();

        assert!(matches!(error, DurableInvalidationProcessError::Handler(_)));
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(3));
        assert_eq!(
            consumer.decide(&record(4)).unwrap(),
            DurableInvalidationDecision::Apply { generation: 4 }
        );
    }

    #[tokio::test]
    async fn recovery_is_acknowledged_only_after_rebuild_succeeds() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 2).unwrap();

        let error = consumer
            .process(
                record(5),
                |_| async { Ok(()) },
                |_| async {
                    Err(rustok_core::Error::Cache(
                        "simulated recovery failure".to_string(),
                    ))
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, DurableInvalidationProcessError::Handler(_)));
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(2));

        let outcome = consumer
            .process(record(5), |_| async { Ok(()) }, |_| async { Ok(()) })
            .await
            .unwrap();
        assert_eq!(
            outcome,
            DurableInvalidationProcessOutcome::Recovered { generation: 5 }
        );
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(5));
    }
}
