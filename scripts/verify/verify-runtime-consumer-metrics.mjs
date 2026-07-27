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
  "rustok_runtime_consumer_source_offset",
]) {
  requireText("runtime consumer telemetry", files.telemetry, metric);
}

for (const labelSet of [
  '&["consumer"]',
  '&["consumer", "outcome"]',
  '&["consumer", "stage"]',
  '&["consumer", "stage", "error_code"]',
  '&["consumer", "result"]',
  '&["consumer", "reason"]',
  '&["consumer", "state"]',
]) {
  requireText("runtime consumer telemetry labels", files.telemetry, labelSet);
}

for (const forbiddenLabel of [
  '"tenant_id"',
  '"event_id"',
  '"partition"',
  '"payload"',
  '"ack_token"',
  '"error_message"',
]) {
  forbidText("runtime consumer telemetry labels", files.telemetry, forbiddenLabel);
}

for (const marker of [
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
  requireText("Social Graph Index worker metrics", files.worker, marker);
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
  "Runtime consumer metrics verification passed: shared registry export, bounded labels, throughput/outcome/retry/failure/DLQ/duration/lifecycle/timestamp/offset metrics, staged DLQ publication, and acknowledgement-only recovery are locked.",
);
