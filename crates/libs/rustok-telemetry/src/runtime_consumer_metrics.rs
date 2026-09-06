use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lazy_static::lazy_static;
use prometheus::core::{Collector, Desc};
use prometheus::proto::MetricFamily;
use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts};

#[derive(Clone)]
struct RuntimeConsumerMetrics {
    received_total: IntCounterVec,
    deliveries_total: IntCounterVec,
    retries_total: IntCounterVec,
    failures_total: IntCounterVec,
    dlq_total: IntCounterVec,
    processing_duration_seconds: HistogramVec,
    worker_starts_total: IntCounterVec,
    worker_terminations_total: IntCounterVec,
    in_flight: IntGaugeVec,
    in_flight_started_timestamp_seconds: IntGaugeVec,
    last_success_timestamp_seconds: IntGaugeVec,
    position_snapshot_timestamp_seconds: IntGaugeVec,
    position_partition_count: IntGaugeVec,
    position_complete: IntGaugeVec,
    lag: IntGaugeVec,
}

impl RuntimeConsumerMetrics {
    fn new() -> Self {
        Self {
            received_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_received_total",
                    "Total broker deliveries received by durable runtime consumers",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer receive counter"),
            deliveries_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_deliveries_total",
                    "Total durable runtime-consumer deliveries by terminal outcome",
                ),
                &["consumer", "outcome"],
            )
            .expect("Failed to create runtime consumer delivery counter"),
            retries_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_retries_total",
                    "Total durable runtime-consumer retries by stage",
                ),
                &["consumer", "stage"],
            )
            .expect("Failed to create runtime consumer retry counter"),
            failures_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_failures_total",
                    "Total durable runtime-consumer failures by bounded stage and error code",
                ),
                &["consumer", "stage", "error_code"],
            )
            .expect("Failed to create runtime consumer failure counter"),
            dlq_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_dlq_total",
                    "Total durable runtime-consumer DLQ publication outcomes",
                ),
                &["consumer", "result"],
            )
            .expect("Failed to create runtime consumer DLQ counter"),
            processing_duration_seconds: HistogramVec::new(
                HistogramOpts::new(
                    "rustok_runtime_consumer_processing_duration_seconds",
                    "Receive-to-terminal-ack duration for durable runtime-consumer deliveries",
                )
                .buckets(vec![
                    0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0,
                    10.0, 30.0, 60.0,
                ]),
                &["consumer", "outcome"],
            )
            .expect("Failed to create runtime consumer processing histogram"),
            worker_starts_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_worker_starts_total",
                    "Total durable runtime-consumer worker starts",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer start counter"),
            worker_terminations_total: IntCounterVec::new(
                Opts::new(
                    "rustok_runtime_consumer_worker_terminations_total",
                    "Total durable runtime-consumer worker terminations by bounded reason",
                ),
                &["consumer", "reason"],
            )
            .expect("Failed to create runtime consumer termination counter"),
            in_flight: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_in_flight",
                    "Whether a durable runtime consumer currently owns an unacknowledged delivery",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer in-flight gauge"),
            in_flight_started_timestamp_seconds: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_in_flight_started_timestamp_seconds",
                    "Unix timestamp when the current unacknowledged delivery was received, or zero",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer in-flight timestamp gauge"),
            last_success_timestamp_seconds: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_last_success_timestamp_seconds",
                    "Unix timestamp of the last terminally acknowledged delivery",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer success timestamp gauge"),
            position_snapshot_timestamp_seconds: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_position_snapshot_timestamp_seconds",
                    "Unix timestamp of the last broker-backed consumer-position snapshot",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer position timestamp gauge"),
            position_partition_count: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_position_partition_count",
                    "Topic partitions included in the last consumer-position snapshot",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer partition-count gauge"),
            position_complete: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_position_complete",
                    "Whether every topic partition has a coherent committed offset and high-watermark",
                ),
                &["consumer"],
            )
            .expect("Failed to create runtime consumer position-completeness gauge"),
            lag: IntGaugeVec::new(
                Opts::new(
                    "rustok_runtime_consumer_lag",
                    "Exact broker offset lag from a complete partition-qualified snapshot",
                ),
                &["consumer", "aggregation"],
            )
            .expect("Failed to create runtime consumer lag gauge"),
        }
    }
}

impl Collector for RuntimeConsumerMetrics {
    fn desc(&self) -> Vec<&Desc> {
        let mut descriptions = Vec::new();
        descriptions.extend(self.received_total.desc());
        descriptions.extend(self.deliveries_total.desc());
        descriptions.extend(self.retries_total.desc());
        descriptions.extend(self.failures_total.desc());
        descriptions.extend(self.dlq_total.desc());
        descriptions.extend(self.processing_duration_seconds.desc());
        descriptions.extend(self.worker_starts_total.desc());
        descriptions.extend(self.worker_terminations_total.desc());
        descriptions.extend(self.in_flight.desc());
        descriptions.extend(self.in_flight_started_timestamp_seconds.desc());
        descriptions.extend(self.last_success_timestamp_seconds.desc());
        descriptions.extend(self.position_snapshot_timestamp_seconds.desc());
        descriptions.extend(self.position_partition_count.desc());
        descriptions.extend(self.position_complete.desc());
        descriptions.extend(self.lag.desc());
        descriptions
    }

    fn collect(&self) -> Vec<MetricFamily> {
        let mut families = Vec::new();
        families.extend(self.received_total.collect());
        families.extend(self.deliveries_total.collect());
        families.extend(self.retries_total.collect());
        families.extend(self.failures_total.collect());
        families.extend(self.dlq_total.collect());
        families.extend(self.processing_duration_seconds.collect());
        families.extend(self.worker_starts_total.collect());
        families.extend(self.worker_terminations_total.collect());
        families.extend(self.in_flight.collect());
        families.extend(self.in_flight_started_timestamp_seconds.collect());
        families.extend(self.last_success_timestamp_seconds.collect());
        families.extend(self.position_snapshot_timestamp_seconds.collect());
        families.extend(self.position_partition_count.collect());
        families.extend(self.position_complete.collect());
        families.extend(self.lag.collect());
        families
    }
}

lazy_static! {
    static ref RUNTIME_CONSUMER_METRICS: RuntimeConsumerMetrics = RuntimeConsumerMetrics::new();
}

static RUNTIME_CONSUMER_METRICS_REGISTERING: AtomicBool = AtomicBool::new(false);

/// Registers the bounded shared runtime-consumer collector after telemetry initialization.
///
/// The operation is idempotent for one process. When telemetry is disabled or has not been
/// initialized yet, the caller receives the registry error and may continue without metrics.
pub fn ensure_registered() -> Result<(), prometheus::Error> {
    if RUNTIME_CONSUMER_METRICS_REGISTERING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }

    match crate::register_runtime_collector(Box::new(RUNTIME_CONSUMER_METRICS.clone())) {
        Ok(()) => Ok(()),
        Err(error) => {
            RUNTIME_CONSUMER_METRICS_REGISTERING.store(false, Ordering::Release);
            Err(error)
        }
    }
}

pub fn record_worker_start(consumer: &str) {
    RUNTIME_CONSUMER_METRICS
        .worker_starts_total
        .with_label_values(&[consumer])
        .inc();
    RUNTIME_CONSUMER_METRICS
        .in_flight
        .with_label_values(&[consumer])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .in_flight_started_timestamp_seconds
        .with_label_values(&[consumer])
        .set(0);
    let _ = RUNTIME_CONSUMER_METRICS
        .last_success_timestamp_seconds
        .with_label_values(&[consumer]);
    RUNTIME_CONSUMER_METRICS
        .position_snapshot_timestamp_seconds
        .with_label_values(&[consumer])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .position_partition_count
        .with_label_values(&[consumer])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .position_complete
        .with_label_values(&[consumer])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .lag
        .with_label_values(&[consumer, "total"])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .lag
        .with_label_values(&[consumer, "max"])
        .set(0);
}

pub fn record_worker_termination(consumer: &str, reason: &str) {
    RUNTIME_CONSUMER_METRICS
        .worker_terminations_total
        .with_label_values(&[consumer, reason])
        .inc();
    RUNTIME_CONSUMER_METRICS
        .in_flight
        .with_label_values(&[consumer])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .in_flight_started_timestamp_seconds
        .with_label_values(&[consumer])
        .set(0);
}

pub fn begin_delivery(consumer: &str, _source_offset: Option<u64>) {
    RUNTIME_CONSUMER_METRICS
        .received_total
        .with_label_values(&[consumer])
        .inc();
    RUNTIME_CONSUMER_METRICS
        .in_flight
        .with_label_values(&[consumer])
        .set(1);
    RUNTIME_CONSUMER_METRICS
        .in_flight_started_timestamp_seconds
        .with_label_values(&[consumer])
        .set(unix_timestamp_seconds());
}

pub fn record_retry(consumer: &str, stage: &str) {
    RUNTIME_CONSUMER_METRICS
        .retries_total
        .with_label_values(&[consumer, stage])
        .inc();
}

pub fn record_failure(consumer: &str, stage: &str, error_code: &str) {
    RUNTIME_CONSUMER_METRICS
        .failures_total
        .with_label_values(&[consumer, stage, error_code])
        .inc();
}

pub fn record_dlq(consumer: &str, result: &str) {
    RUNTIME_CONSUMER_METRICS
        .dlq_total
        .with_label_values(&[consumer, result])
        .inc();
}

pub fn complete_delivery(
    consumer: &str,
    outcome: &str,
    processing_duration: Duration,
    _acknowledged_offset: Option<u64>,
) {
    RUNTIME_CONSUMER_METRICS
        .deliveries_total
        .with_label_values(&[consumer, outcome])
        .inc();
    RUNTIME_CONSUMER_METRICS
        .processing_duration_seconds
        .with_label_values(&[consumer, outcome])
        .observe(processing_duration.as_secs_f64());
    RUNTIME_CONSUMER_METRICS
        .last_success_timestamp_seconds
        .with_label_values(&[consumer])
        .set(unix_timestamp_seconds());
    RUNTIME_CONSUMER_METRICS
        .in_flight
        .with_label_values(&[consumer])
        .set(0);
    RUNTIME_CONSUMER_METRICS
        .in_flight_started_timestamp_seconds
        .with_label_values(&[consumer])
        .set(0);
}

/// Records a complete or explicitly incomplete broker-backed position snapshot.
///
/// Lag values are accepted only as a pair. An incomplete snapshot clears both lag gauges and
/// sets `position_complete` to zero so old values cannot be mistaken for current group lag.
pub fn record_position_snapshot(
    consumer: &str,
    captured_at_unix_seconds: u64,
    partition_count: usize,
    total_lag: Option<u64>,
    max_lag: Option<u64>,
) {
    RUNTIME_CONSUMER_METRICS
        .position_snapshot_timestamp_seconds
        .with_label_values(&[consumer])
        .set(metric_value(captured_at_unix_seconds));
    RUNTIME_CONSUMER_METRICS
        .position_partition_count
        .with_label_values(&[consumer])
        .set(metric_value(
            u64::try_from(partition_count).unwrap_or(u64::MAX),
        ));

    let complete = total_lag.is_some() && max_lag.is_some();
    RUNTIME_CONSUMER_METRICS
        .position_complete
        .with_label_values(&[consumer])
        .set(i64::from(complete));
    RUNTIME_CONSUMER_METRICS
        .lag
        .with_label_values(&[consumer, "total"])
        .set(metric_value(total_lag.unwrap_or(0)));
    RUNTIME_CONSUMER_METRICS
        .lag
        .with_label_values(&[consumer, "max"])
        .set(metric_value(max_lag.unwrap_or(0)));
}

fn unix_timestamp_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

fn metric_value(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_values_saturate_instead_of_wrapping() {
        assert_eq!(metric_value(1), 1);
        assert_eq!(metric_value(u64::MAX), i64::MAX);
    }
}
