use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use lazy_static::lazy_static;
use prometheus::core::{Collector, Desc};
use prometheus::proto::MetricFamily;
use prometheus::{IntGaugeVec, Opts};

const STATES: [&str; 6] = [
    "total",
    "reserved",
    "publishing",
    "expired_publishing",
    "published",
    "acknowledged",
];

#[derive(Clone)]
struct ConsumerPoisonMetrics {
    receipts: IntGaugeVec,
    snapshot_available: IntGaugeVec,
    snapshot_timestamp_seconds: IntGaugeVec,
}

impl ConsumerPoisonMetrics {
    fn new() -> Self {
        Self {
            receipts: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_poison_receipts",
                    "Count-only neutral poison receipts by bounded durable state",
                ),
                &["consumer", "state"],
            )
            .expect("Failed to create consumer poison receipt gauge"),
            snapshot_available: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_poison_snapshot_available",
                    "Whether the latest count-only poison receipt snapshot is available",
                ),
                &["consumer"],
            )
            .expect("Failed to create consumer poison snapshot availability gauge"),
            snapshot_timestamp_seconds: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_poison_snapshot_timestamp_seconds",
                    "Unix timestamp of the latest available count-only poison receipt snapshot",
                ),
                &["consumer"],
            )
            .expect("Failed to create consumer poison snapshot timestamp gauge"),
        }
    }
}

impl Collector for ConsumerPoisonMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut descriptions = Vec::new();
        descriptions.extend(self.receipts.desc());
        descriptions.extend(self.snapshot_available.desc());
        descriptions.extend(self.snapshot_timestamp_seconds.desc());
        descriptions
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut families = Vec::new();
        families.extend(self.receipts.collect());
        families.extend(self.snapshot_available.collect());
        families.extend(self.snapshot_timestamp_seconds.collect());
        families
    }
}

lazy_static! {
    static ref CONSUMER_POISON_METRICS: ConsumerPoisonMetrics = ConsumerPoisonMetrics::new();
}

static CONSUMER_POISON_METRICS_REGISTERING: AtomicBool = AtomicBool::new(false);

/// Registers the count-only neutral poison-receipt collector after telemetry initialization.
///
/// Metric names and state labels are fixed. Callers must use a bounded consumer identifier and
/// must not derive labels from delivery identifiers, source coordinates, errors, or payloads.
pub fn ensure_registered() -> Result<(), prometheus::Error> {
    if CONSUMER_POISON_METRICS_REGISTERING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }

    match crate::register_runtime_collector(Box::new(CONSUMER_POISON_METRICS.clone())) {
        Ok(()) => Ok(()),
        Err(error) => {
            CONSUMER_POISON_METRICS_REGISTERING.store(false, Ordering::Release);
            Err(error)
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn record_snapshot(
    consumer: &str,
    total: u64,
    reserved: u64,
    publishing: u64,
    expired_publishing: u64,
    published: u64,
    acknowledged: u64,
) {
    for (state, value) in STATES.into_iter().zip([
        total,
        reserved,
        publishing,
        expired_publishing,
        published,
        acknowledged,
    ]) {
        CONSUMER_POISON_METRICS
            .receipts
            .with_label_values(&[consumer, state])
            .set(metric_value(value));
    }
    CONSUMER_POISON_METRICS
        .snapshot_available
        .with_label_values(&[consumer])
        .set(1);
    CONSUMER_POISON_METRICS
        .snapshot_timestamp_seconds
        .with_label_values(&[consumer])
        .set(unix_timestamp_seconds());
}

/// Clears count gauges when storage inspection is unavailable so stale values cannot look current.
pub fn record_unavailable(consumer: &str) {
    for state in STATES {
        CONSUMER_POISON_METRICS
            .receipts
            .with_label_values(&[consumer, state])
            .set(0);
    }
    CONSUMER_POISON_METRICS
        .snapshot_available
        .with_label_values(&[consumer])
        .set(0);
    CONSUMER_POISON_METRICS
        .snapshot_timestamp_seconds
        .with_label_values(&[consumer])
        .set(0);
}

fn unix_timestamp_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| metric_value(duration.as_secs()))
        .unwrap_or(0)
}

fn metric_value(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_labels_are_fixed_and_bounded() {
        assert_eq!(
            STATES,
            [
                "total",
                "reserved",
                "publishing",
                "expired_publishing",
                "published",
                "acknowledged",
            ]
        );
    }

    #[test]
    fn metric_values_saturate_instead_of_wrapping() {
        assert_eq!(metric_value(1), 1);
        assert_eq!(metric_value(u64::MAX), i64::MAX);
    }
}
