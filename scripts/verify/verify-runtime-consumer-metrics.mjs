#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  telemetryLib: readFileSync("crates/rustok-telemetry/src/lib.rs", "utf8"),
  telemetry: readFileSync(
    "crates/rustok-telemetry/src/runtime_consumer_metrics.rs",
    "utf8",
  ),
  metricsEndpoint: readFileSync("apps/server/src/controllers/metrics.rs", "utf8"),
  worker: readFileSync(
    "apps/server/src/services/social_graph_index_worker.rs",
    "utf8",
  ),
  positionObserver: readFileSync(
    "apps/server/src/services/social_graph_index_position_observer.rs",
    "utf8",
  ),
  position: readFileSync("crates/rustok-iggy/src/position.rs", "utf8"),
  consumer: readFileSync(
    "crates/rustok-social-graph/src/index_consumer.rs",
    "utf8",
  ),
};

const failures = [];

function requireText(name, source, text) {
  if (!source.includes(text)) {
    failures.push(`${name} is missing required marker: ${text}`);
  }
}

function forbidText(name, source, text) {
  if (source.includes(text)) {
    failures.push(`${name} contains forbidden marker: ${text}`);
  }
}

requireText(
  "telemetry export",
  files.telemetryLib,
  "pub mod runtime_consumer_metrics;",
);
requireText(
  "telemetry registration",
  files.telemetry,
  "crate::register_runtime_collector",
);
requireText(
  "metrics endpoint",
  files.metricsEndpoint,
  "let mut payload = handle.render();",
);

for (const metric of [
  "rustok_runtime_consumer_received_total",
  "rustok_runtime_consumer_deliveries_total",
  "rustok_runtime_consumer_retries_total",
  "rustok_runtime_consumer_failures_total",
  "rustok_runtime_consumer_dlq_total",
  "rustok_runtime_consumer_processing_duration_seconds",
  "rustok_runtime_consumer_worker_starts_total",
  "rustok_runtime_consumer_worker_terminations_total",
  "rustok_runtime_consumer_in_flight",
  "rustok_runtime_consumer_in_flight_started_timestamp_seconds",
  "rustok_runtime_consumer_last_success_timestamp_seconds",
  "rustok_runtime_consumer_position_snapshot_timestamp_seconds",
  "rustok_runtime_consumer_position_partition_count",
  "rustok_runtime_consumer_position_complete",
  "rustok_runtime_consumer_lag",
]) {
  requireText("runtime consumer telemetry", files.telemetry, metric);
}

for (const forbiddenMetric of [
  "rustok_runtime_consumer_source_offset",
  "rustok_runtime_consumer_source_partition",
]) {
  forbidText("runtime consumer telemetry", files.telemetry, forbiddenMetric);
}

for (const labelSet of [
  '&["consumer"]',
  '&["consumer", "outcome"]',
  '&["consumer", "stage"]',
  '&["consumer", "stage", "error_code"]',
  '&["consumer", "result"]',
  '&["consumer", "reason"]',
  '&["consumer", "aggregation"]',
]) {
  requireText("runtime consumer telemetry labels", files.telemetry, labelSet);
}

for (const forbiddenLabel of [
  '"tenant_id"',
  '"event_id"',
  '"partition"',
  '"offset"',
  '"state"',
  '"payload"',
  '"ack_token"',
  '"error_message"',
]) {
  forbidText("runtime consumer telemetry labels", files.telemetry, forbiddenLabel);
}

for (const marker of [
  "pub fn record_worker_start(consumer: &str)",
  "pub fn record_worker_termination(consumer: &str, reason: &str)",
  "pub fn record_position_snapshot(",
  "let complete = total_lag.is_some() && max_lag.is_some();",
  '.with_label_values(&[consumer, "total"])',
  '.with_label_values(&[consumer, "max"])',
  '.set(metric_value(total_lag.unwrap_or(0)))',
  '.set(metric_value(max_lag.unwrap_or(0)))',
  "runtime_consumer_metrics::ensure_registered()",
  "runtime_consumer_metrics::record_worker_start",
  "runtime_consumer_metrics::record_worker_termination",
  "runtime_consumer_metrics::begin_delivery",
  "runtime_consumer_metrics::record_retry",
  "runtime_consumer_metrics::record_failure",
  "runtime_consumer_metrics::record_dlq",
  "runtime_consumer_metrics::complete_delivery",
  'const METRICS_CONSUMER: &str = "social_graph_index"',
  'const STAGE_STARTUP: &str = "startup"',
  'const STAGE_RECEIVE: &str = "receive"',
  'const STAGE_PROJECTION: &str = "projection"',
  'const STAGE_DLQ_PUBLISH: &str = "dlq_publish"',
  'const STAGE_ACKNOWLEDGEMENT: &str = "acknowledgement"',
  '"applied"',
  '"duplicate"',
  '"stale_ignored"',
  '"ignored_unrelated"',
  '"dead_lettered"',
]) {
  const source = marker.startsWith("runtime_consumer_metrics::") || marker.startsWith("const ") || marker.startsWith('"')
    ? files.worker
    : files.telemetry;
  requireText("runtime consumer metrics contract", source, marker);
}

for (const marker of [
  "IggyConsumerPositionObserver::connect(",
  "connected.snapshot().await",
  "runtime_consumer_metrics::record_position_snapshot(",
  "snapshot.total_lag()",
  "snapshot.max_lag()",
  "projection remains active",
]) {
  requireText("position observer metrics integration", files.positionObserver, marker);
}
for (const marker of [
  "self.high_watermark.checked_sub(offset)",
  "if self.messages_count == 0",
  "self.partitions.iter().all(|position| position.lag().is_some())",
  "total.checked_add(position.lag()?)",
  ".get_topic(&self.stream_id, &self.topic_id)",
  ".get_consumer_offset(",
]) {
  requireText("broker-backed position contract", files.position, marker);
}

const inFlightClearMarker = `.in_flight
        .with_label_values(&[consumer])
        .set(0);`;
const inFlightTimestampClearMarker = `.in_flight_started_timestamp_seconds
        .with_label_values(&[consumer])
        .set(0);`;
if (files.telemetry.split(inFlightClearMarker).length - 1 < 2) {
  failures.push("in-flight gauge must be initialized and cleared on worker termination");
}
if (files.telemetry.split(inFlightTimestampClearMarker).length - 1 < 2) {
  failures.push(
    "in-flight start timestamp must be initialized and cleared on worker termination",
  );
}

for (const marker of [
  "pub async fn publish_consumed_to_dlq(",
  "consumed.raw_payload().to_vec()",
  ".move_to_dlq(entry)",
  "self.publish_consumed_to_dlq(consumed, stable_error_code, retry_count)",
  "self.acknowledge_consumed(consumed).await",
]) {
  requireText("Social Graph staged DLQ API", files.consumer, marker);
}

const dlqPublish = files.worker.indexOf(".publish_consumed_to_dlq(");
const dlqAckOnly = files.worker.indexOf(
  "return acknowledge_terminal_result(",
  dlqPublish,
);
if (dlqPublish < 0 || dlqAckOnly <= dlqPublish) {
  failures.push(
    "worker must publish to DLQ before entering source acknowledgement-only recovery",
  );
}

const acknowledgeOnlyStart = files.worker.indexOf(
  "async fn acknowledge_terminal_result(",
);
const acknowledgeOnlyBody =
  acknowledgeOnlyStart >= 0 ? files.worker.slice(acknowledgeOnlyStart) : "";
for (const forbidden of [
  "publish_consumed_to_dlq",
  "move_to_dlq_and_acknowledge",
  "project_consumed(consumed)",
]) {
  forbidText("terminal acknowledgement-only path", acknowledgeOnlyBody, forbidden);
}

if (failures.length > 0) {
  console.error("Runtime consumer metrics verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Runtime consumer metrics verification passed: shared registry export, bounded labels, throughput/outcome/retry/failure/DLQ/duration/lifecycle timestamps, broker-backed complete total/max lag, incomplete-snapshot clearing, clean in-flight lifecycle, staged DLQ publication, and acknowledgement-only recovery are locked.",
);
