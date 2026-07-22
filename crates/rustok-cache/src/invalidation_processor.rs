use std::future::Future;

use crate::invalidation_consumer::DurableInvalidationProcessGateError;
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
    /// Work is serialized per channel. Concurrent delivery of the same generation waits for the
    /// first handler and is then classified as `Ignored`, while the number of distinct active
    /// channels remains bounded. Both callbacks receive an owned record so their futures do not
    /// borrow process-local tracker state. A failed or cancelled callback leaves the acknowledged
    /// offset unchanged and the record remains eligible for retry.
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
        let _in_flight = self.begin_process();
        let mut completion = DurableInvalidationProcessCompletionGuard::new(self);
        let result = async {
            let lease = self
                .process_gate(record.channel())
                .map_err(|error| match error {
                    DurableInvalidationProcessGateError::Saturated { count, maximum } => {
                        DurableInvalidationProcessError::Saturated { count, maximum }
                    }
                })?;
            let _guard = lease.gate.lock().await;

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
        .await;

        match &result {
            Ok(DurableInvalidationProcessOutcome::Applied { .. }) => self.record_applied(),
            Ok(DurableInvalidationProcessOutcome::Ignored { .. }) => self.record_ignored(),
            Ok(DurableInvalidationProcessOutcome::Recovered { .. }) => self.record_recovered(),
            Err(DurableInvalidationProcessError::Saturated { .. }) => self.record_failure(true),
            Err(_) => self.record_failure(false),
        }
        completion.complete();

        result
    }
}

struct DurableInvalidationProcessCompletionGuard<'a> {
    consumer: &'a DurableCacheInvalidationConsumer,
    completed: bool,
}

impl<'a> DurableInvalidationProcessCompletionGuard<'a> {
    fn new(consumer: &'a DurableCacheInvalidationConsumer) -> Self {
        Self {
            consumer,
            completed: false,
        }
    }

    fn complete(&mut self) {
        self.completed = true;
    }
}

impl Drop for DurableInvalidationProcessCompletionGuard<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.consumer.record_failure(false);
        }
    }
}

#[derive(Debug)]
pub enum DurableInvalidationProcessError {
    Record(DurableCacheInvalidationError),
    Tracker(BoundedInvalidationTrackerError),
    Saturated { count: usize, maximum: usize },
    Handler(rustok_core::Error),
}

impl std::fmt::Display for DurableInvalidationProcessError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Record(error) => write!(formatter, "invalid durable cache record: {error}"),
            Self::Tracker(error) => {
                write!(formatter, "durable cache offset update failed: {error}")
            }
            Self::Saturated { count, maximum } => write!(
                formatter,
                "durable invalidation processor saturated at {maximum} channels; current count {count}"
            ),
            Self::Handler(error) => write!(formatter, "durable cache handler failed: {error}"),
        }
    }
}

impl std::error::Error for DurableInvalidationProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            Self::Tracker(error) => Some(error),
            Self::Saturated { .. } => None,
            Self::Handler(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    fn record(generation: u64) -> DurableCacheInvalidationRecord {
        record_for("tenant.invalidate", generation)
    }

    fn record_for(channel: &str, generation: u64) -> DurableCacheInvalidationRecord {
        DurableCacheInvalidationRecord::new(
            Uuid::from_u128(generation as u128 + 1),
            Some(Uuid::from_u128(42)),
            channel,
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
        assert_eq!(consumer.stats().attempted, 1);
        assert_eq!(consumer.stats().applied, 1);
        assert_eq!(consumer.stats().in_flight, 0);
    }

    #[tokio::test]
    async fn concurrent_duplicate_delivery_runs_apply_once() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 3).unwrap();
        let calls = Arc::new(AtomicUsize::new(0));

        let first_consumer = consumer.clone();
        let first_calls = Arc::clone(&calls);
        let first = tokio::spawn(async move {
            first_consumer
                .process(
                    record(4),
                    move |_| async move {
                        first_calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                        Ok(())
                    },
                    |_| async { Ok(()) },
                )
                .await
                .unwrap()
        });

        let second_consumer = consumer.clone();
        let second_calls = Arc::clone(&calls);
        let second = tokio::spawn(async move {
            second_consumer
                .process(
                    record(4),
                    move |_| async move {
                        second_calls.fetch_add(1, Ordering::SeqCst);
                        Ok(())
                    },
                    |_| async { Ok(()) },
                )
                .await
                .unwrap()
        });

        let (first, second) = tokio::join!(first, second);
        let outcomes = [first.unwrap(), second.unwrap()];
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(outcomes.contains(&DurableInvalidationProcessOutcome::Applied { generation: 4 }));
        assert!(outcomes.contains(&DurableInvalidationProcessOutcome::Ignored { generation: 4 }));
        let stats = consumer.stats();
        assert_eq!(stats.attempted, 2);
        assert_eq!(stats.applied, 1);
        assert_eq!(stats.ignored, 1);
        assert_eq!(stats.failed, 0);
        assert_eq!(stats.in_flight, 0);
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
        let stats = consumer.stats();
        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.applied, 0);
        assert_eq!(stats.in_flight, 0);
    }

    #[tokio::test]
    async fn cancelled_apply_counts_as_failure_and_keeps_offset_for_retry() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 3).unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let task_consumer = consumer.clone();
        let task_entered = Arc::clone(&entered);

        let task = tokio::spawn(async move {
            task_consumer
                .process(
                    record(4),
                    move |_| async move {
                        task_entered.notify_one();
                        std::future::pending::<rustok_core::Result<()>>().await
                    },
                    |_| async { Ok(()) },
                )
                .await
        });

        entered.notified().await;
        assert_eq!(consumer.stats().attempted, 1);
        assert_eq!(consumer.stats().in_flight, 1);
        task.abort();
        let _ = task.await;
        tokio::task::yield_now().await;

        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(3));
        assert_eq!(
            consumer.decide(&record(4)).unwrap(),
            DurableInvalidationDecision::Apply { generation: 4 }
        );
        let stats = consumer.stats();
        assert_eq!(stats.attempted, 1);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.applied, 0);
        assert_eq!(stats.in_flight, 0);
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
        let stats = consumer.stats();
        assert_eq!(stats.attempted, 2);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.recovered, 1);
        assert_eq!(stats.in_flight, 0);
    }

    #[tokio::test]
    async fn distinct_channel_work_fails_fast_when_processor_is_saturated() {
        let consumer = DurableCacheInvalidationConsumer::new(1).unwrap();
        consumer.seed("tenant.invalidate", 3).unwrap();
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());

        let first_consumer = consumer.clone();
        let first_entered = Arc::clone(&entered);
        let first_release = Arc::clone(&release);
        let first = tokio::spawn(async move {
            first_consumer
                .process(
                    record(4),
                    move |_| async move {
                        first_entered.notify_one();
                        first_release.notified().await;
                        Ok(())
                    },
                    |_| async { Ok(()) },
                )
                .await
        });

        entered.notified().await;
        let error = consumer
            .process(
                record_for("other.invalidate", 1),
                |_| async { Ok(()) },
                |_| async { Ok(()) },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            DurableInvalidationProcessError::Saturated {
                count: 1,
                maximum: 1
            }
        ));
        let saturated_stats = consumer.stats();
        assert_eq!(saturated_stats.attempted, 2);
        assert_eq!(saturated_stats.failed, 1);
        assert_eq!(saturated_stats.saturated, 1);
        assert_eq!(saturated_stats.in_flight, 1);

        release.notify_one();
        first.await.unwrap().unwrap();
        assert_eq!(consumer.in_flight_process_channels(), 0);
        assert_eq!(consumer.stats().in_flight, 0);
    }
}
