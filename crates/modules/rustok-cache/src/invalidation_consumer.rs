use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::sync::Mutex as AsyncMutex;

use crate::{
    BoundedCacheInvalidationGapTracker, BoundedInvalidationTrackerError,
    CacheInvalidationObservation, DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS,
    DurableCacheInvalidationError, DurableCacheInvalidationRecord,
};

/// Consumer decision for one durable invalidation record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableInvalidationDecision {
    /// The record is the next contiguous generation and may be applied directly.
    Apply { generation: u64 },
    /// The record was already applied or is older than the acknowledged offset.
    Ignore { generation: u64 },
    /// The sequence is unverified or has a gap; rebuild/rotate the namespace before acknowledgement.
    RecoverThrough { generation: u64 },
}

/// Bounded, label-free snapshot of durable invalidation processing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DurableInvalidationConsumerStats {
    pub attempted: u64,
    pub applied: u64,
    pub ignored: u64,
    pub recovered: u64,
    pub failed: u64,
    pub saturated: u64,
    pub in_flight: u64,
}

/// Render durable invalidation metrics without channel, tenant, key or cause labels.
pub fn format_durable_invalidation_prometheus_metrics(
    stats: &DurableInvalidationConsumerStats,
) -> String {
    format!(
        "rustok_cache_durable_invalidation_attempted_total {attempted}\n\
         rustok_cache_durable_invalidation_applied_total {applied}\n\
         rustok_cache_durable_invalidation_ignored_total {ignored}\n\
         rustok_cache_durable_invalidation_recovered_total {recovered}\n\
         rustok_cache_durable_invalidation_failed_total {failed}\n\
         rustok_cache_durable_invalidation_saturated_total {saturated}\n\
         rustok_cache_durable_invalidation_in_flight {in_flight}\n",
        attempted = stats.attempted,
        applied = stats.applied,
        ignored = stats.ignored,
        recovered = stats.recovered,
        failed = stats.failed,
        saturated = stats.saturated,
        in_flight = stats.in_flight,
    )
}

#[derive(Default)]
struct DurableInvalidationConsumerMetrics {
    attempted: AtomicU64,
    applied: AtomicU64,
    ignored: AtomicU64,
    recovered: AtomicU64,
    failed: AtomicU64,
    saturated: AtomicU64,
    in_flight: AtomicU64,
}

type ProcessGateMap = HashMap<String, Arc<AsyncMutex<()>>>;

/// Bounded process-local consumer state for durable cache invalidations.
#[derive(Clone)]
pub struct DurableCacheInvalidationConsumer {
    tracker: BoundedCacheInvalidationGapTracker,
    process_gates: Arc<Mutex<ProcessGateMap>>,
    maximum_process_channels: usize,
    metrics: Arc<DurableInvalidationConsumerMetrics>,
}

impl Default for DurableCacheInvalidationConsumer {
    fn default() -> Self {
        Self {
            tracker: BoundedCacheInvalidationGapTracker::default(),
            process_gates: Arc::new(Mutex::new(HashMap::new())),
            maximum_process_channels: DEFAULT_MAX_TRACKED_INVALIDATION_CHANNELS,
            metrics: Arc::new(DurableInvalidationConsumerMetrics::default()),
        }
    }
}

impl DurableCacheInvalidationConsumer {
    pub fn new(maximum_channels: usize) -> Result<Self, BoundedInvalidationTrackerError> {
        Ok(Self {
            tracker: BoundedCacheInvalidationGapTracker::new(maximum_channels)?,
            process_gates: Arc::new(Mutex::new(HashMap::new())),
            maximum_process_channels: maximum_channels,
            metrics: Arc::new(DurableInvalidationConsumerMetrics::default()),
        })
    }

    pub fn seed(
        &self,
        channel: impl Into<String>,
        last_generation: u64,
    ) -> Result<Option<u64>, BoundedInvalidationTrackerError> {
        self.tracker.seed(channel, last_generation)
    }

    pub fn decide(
        &self,
        record: &DurableCacheInvalidationRecord,
    ) -> Result<DurableInvalidationDecision, DurableCacheInvalidationError> {
        let event = record.to_versioned_invalidation()?;
        Ok(match self.tracker.observe(&event) {
            CacheInvalidationObservation::InOrder { generation } => {
                DurableInvalidationDecision::Apply { generation }
            }
            CacheInvalidationObservation::Duplicate { generation } => {
                DurableInvalidationDecision::Ignore { generation }
            }
            CacheInvalidationObservation::Stale { received, .. } => {
                DurableInvalidationDecision::Ignore {
                    generation: received,
                }
            }
            CacheInvalidationObservation::UnverifiedFirst { generation }
            | CacheInvalidationObservation::Gap {
                received: generation,
                ..
            } => DurableInvalidationDecision::RecoverThrough { generation },
        })
    }

    /// Acknowledge only after the in-order invalidation handler completes successfully.
    pub fn acknowledge_applied(
        &self,
        channel: impl Into<String>,
        applied_generation: u64,
    ) -> Result<Option<u64>, BoundedInvalidationTrackerError> {
        self.tracker
            .acknowledge_applied(channel, applied_generation)
    }

    /// Acknowledge only after namespace clear/rebuild/generation rotation completes successfully.
    pub fn acknowledge_recovery(
        &self,
        channel: impl Into<String>,
        recovered_through_generation: u64,
    ) -> Result<Option<u64>, BoundedInvalidationTrackerError> {
        self.tracker
            .acknowledge_recovery(channel, recovered_through_generation)
    }

    pub fn last_generation(&self, channel: &str) -> Option<u64> {
        self.tracker.last_generation(channel)
    }

    pub fn tracked_channels(&self) -> usize {
        self.tracker.channel_count()
    }

    pub fn in_flight_process_channels(&self) -> usize {
        self.process_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    pub fn stats(&self) -> DurableInvalidationConsumerStats {
        DurableInvalidationConsumerStats {
            attempted: self.metrics.attempted.load(Ordering::Relaxed),
            applied: self.metrics.applied.load(Ordering::Relaxed),
            ignored: self.metrics.ignored.load(Ordering::Relaxed),
            recovered: self.metrics.recovered.load(Ordering::Relaxed),
            failed: self.metrics.failed.load(Ordering::Relaxed),
            saturated: self.metrics.saturated.load(Ordering::Relaxed),
            in_flight: self.metrics.in_flight.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn begin_process(&self) -> DurableInvalidationInFlightGuard {
        self.metrics.attempted.fetch_add(1, Ordering::Relaxed);
        self.metrics.in_flight.fetch_add(1, Ordering::Relaxed);
        DurableInvalidationInFlightGuard {
            metrics: Arc::clone(&self.metrics),
        }
    }

    pub(crate) fn record_applied(&self) {
        self.metrics.applied.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_ignored(&self) {
        self.metrics.ignored.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_recovered(&self) {
        self.metrics.recovered.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_failure(&self, saturated: bool) {
        self.metrics.failed.fetch_add(1, Ordering::Relaxed);
        if saturated {
            self.metrics.saturated.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub(crate) fn process_gate(
        &self,
        channel: &str,
    ) -> Result<DurableInvalidationProcessGateLease, DurableInvalidationProcessGateError> {
        let mut gates = self
            .process_gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let gate = if let Some(gate) = gates.get(channel) {
            Arc::clone(gate)
        } else {
            if gates.len() >= self.maximum_process_channels {
                return Err(DurableInvalidationProcessGateError::Saturated {
                    count: gates.len(),
                    maximum: self.maximum_process_channels,
                });
            }
            let gate = Arc::new(AsyncMutex::new(()));
            gates.insert(channel.to_string(), Arc::clone(&gate));
            gate
        };
        Ok(DurableInvalidationProcessGateLease {
            channel: channel.to_string(),
            gate,
            gates: Arc::clone(&self.process_gates),
        })
    }
}

pub(crate) struct DurableInvalidationInFlightGuard {
    metrics: Arc<DurableInvalidationConsumerMetrics>,
}

impl Drop for DurableInvalidationInFlightGuard {
    fn drop(&mut self) {
        self.metrics.in_flight.fetch_sub(1, Ordering::Relaxed);
    }
}

pub(crate) struct DurableInvalidationProcessGateLease {
    channel: String,
    pub(crate) gate: Arc<AsyncMutex<()>>,
    gates: Arc<Mutex<ProcessGateMap>>,
}

impl Drop for DurableInvalidationProcessGateLease {
    fn drop(&mut self) {
        let mut gates = self
            .gates
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if gates.get(&self.channel).is_some_and(|current| {
            Arc::ptr_eq(current, &self.gate) && Arc::strong_count(current) <= 2
        }) {
            gates.remove(&self.channel);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DurableInvalidationProcessGateError {
    Saturated { count: usize, maximum: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
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

    #[test]
    fn unseeded_first_record_requires_recovery_and_does_not_advance() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        assert_eq!(
            consumer.decide(&record(5)).unwrap(),
            DurableInvalidationDecision::RecoverThrough { generation: 5 }
        );
        assert_eq!(consumer.last_generation("tenant.invalidate"), None);
    }

    #[test]
    fn failed_apply_is_retried_until_explicit_acknowledgement() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 3).unwrap();

        assert_eq!(
            consumer.decide(&record(4)).unwrap(),
            DurableInvalidationDecision::Apply { generation: 4 }
        );
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(3));
        assert_eq!(
            consumer.decide(&record(4)).unwrap(),
            DurableInvalidationDecision::Apply { generation: 4 }
        );

        consumer
            .acknowledge_applied("tenant.invalidate", 4)
            .unwrap();
        assert_eq!(
            consumer.decide(&record(4)).unwrap(),
            DurableInvalidationDecision::Ignore { generation: 4 }
        );
    }

    #[test]
    fn gap_requires_recovery_acknowledgement() {
        let consumer = DurableCacheInvalidationConsumer::new(4).unwrap();
        consumer.seed("tenant.invalidate", 4).unwrap();

        assert_eq!(
            consumer.decide(&record(6)).unwrap(),
            DurableInvalidationDecision::RecoverThrough { generation: 6 }
        );
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(4));

        consumer
            .acknowledge_recovery("tenant.invalidate", 6)
            .unwrap();
        assert_eq!(consumer.last_generation("tenant.invalidate"), Some(6));
    }

    #[tokio::test]
    async fn process_gates_are_bounded_and_reused_by_channel() {
        let consumer = DurableCacheInvalidationConsumer::new(1).unwrap();
        let first = consumer.process_gate("tenant.invalidate").unwrap();
        let same = consumer.process_gate("tenant.invalidate").unwrap();
        assert!(Arc::ptr_eq(&first.gate, &same.gate));
        assert_eq!(consumer.in_flight_process_channels(), 1);
        assert!(matches!(
            consumer.process_gate("other.invalidate"),
            Err(DurableInvalidationProcessGateError::Saturated {
                count: 1,
                maximum: 1
            })
        ));
        drop(same);
        drop(first);
        assert_eq!(consumer.in_flight_process_channels(), 0);
    }

    #[test]
    fn in_flight_guard_is_cancellation_safe() {
        let consumer = DurableCacheInvalidationConsumer::new(1).unwrap();
        let guard = consumer.begin_process();
        assert_eq!(consumer.stats().attempted, 1);
        assert_eq!(consumer.stats().in_flight, 1);
        drop(guard);
        assert_eq!(consumer.stats().in_flight, 0);
    }

    #[test]
    fn prometheus_metrics_are_label_free() {
        let metrics =
            format_durable_invalidation_prometheus_metrics(&DurableInvalidationConsumerStats {
                attempted: 9,
                applied: 3,
                ignored: 2,
                recovered: 1,
                failed: 3,
                saturated: 1,
                in_flight: 2,
            });
        assert!(metrics.contains("rustok_cache_durable_invalidation_attempted_total 9"));
        assert!(metrics.contains("rustok_cache_durable_invalidation_recovered_total 1"));
        assert!(metrics.contains("rustok_cache_durable_invalidation_saturated_total 1"));
        assert!(metrics.contains("rustok_cache_durable_invalidation_in_flight 2"));
        assert!(!metrics.contains('{'));
    }
}
