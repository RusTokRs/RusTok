#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  rootCargo: readFileSync("Cargo.toml", "utf8"),
  cargo: readFileSync("crates/rustok-iggy/Cargo.toml", "utf8"),
  lib: readFileSync("crates/rustok-iggy/src/lib.rs", "utf8"),
  position: readFileSync("crates/rustok-iggy/src/position.rs", "utf8"),
  telemetry: readFileSync(
    "crates/rustok-telemetry/src/runtime_consumer_metrics.rs",
    "utf8",
  ),
  services: readFileSync("apps/server/src/services/mod.rs", "utf8"),
  bootstrap: readFileSync(
    "apps/server/src/services/server_bootstrap.rs",
    "utf8",
  ),
  observer: readFileSync(
    "apps/server/src/services/social_graph_index_position_observer.rs",
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

requireText("workspace SDK pin", files.rootCargo, 'iggy = "0.10.0"');
for (const marker of [
  'iggy = { workspace = true, optional = true }',
  'iggy = ["dep:iggy", "rustok-iggy-connector/iggy"]',
]) {
  requireText("rustok-iggy Cargo contract", files.cargo, marker);
}
for (const marker of [
  '#[cfg(feature = "iggy")]',
  'pub mod position;',
  'IggyConsumerPositionObserver',
  'ConsumerPositionSnapshot',
]) {
  requireText("rustok-iggy public API", files.lib, marker);
}

for (const marker of [
  'ConsumerKind::ConsumerGroup',
  'IggyClient::from_connection_string',
  '.get_topic(&self.stream_id, &self.topic_id)',
  '.get_consumer_offset(',
  'Some(partition.id)',
  'position.current_offset.max(partition.current_offset)',
  'acknowledged_offset: offset.map(|position| position.stored_offset)',
  'self.high_watermark.checked_sub(offset)',
  'if self.messages_count == 0',
  'self.partitions.iter().all(|position| position.lag().is_some())',
  'total.checked_add(position.lag()?)',
  'captured_at_unix_seconds',
]) {
  requireText("Iggy consumer-position observer", files.position, marker);
}
for (const forbidden of [
  'EventEnvelope',
  'ContractEventEnvelope',
  'event_timestamp',
  'processing_duration',
  'publish(',
  'store_consumer_offset',
  'delete_consumer_offset',
]) {
  forbidText("read-only Iggy consumer-position observer", files.position, forbidden);
}

for (const metric of [
  'rustok_runtime_consumer_position_snapshot_timestamp_seconds',
  'rustok_runtime_consumer_position_partition_count',
  'rustok_runtime_consumer_position_complete',
  'rustok_runtime_consumer_lag',
]) {
  requireText("runtime consumer position metrics", files.telemetry, metric);
}
for (const marker of [
  'pub fn record_position_snapshot(',
  'let complete = total_lag.is_some() && max_lag.is_some();',
  '.with_label_values(&[consumer, "total"])',
  '.with_label_values(&[consumer, "max"])',
  '.set(metric_value(total_lag.unwrap_or(0)))',
  '.set(metric_value(max_lag.unwrap_or(0)))',
]) {
  requireText("runtime consumer complete-lag gate", files.telemetry, marker);
}
for (const forbiddenLabel of [
  '"partition"',
  '"offset"',
  '"tenant_id"',
  '"event_id"',
  '"payload"',
]) {
  forbidText("runtime consumer position labels", files.telemetry, forbiddenLabel);
}

requireText(
  "server service export",
  files.services,
  "pub mod social_graph_index_position_observer;",
);
requireText(
  "server bootstrap",
  files.bootstrap,
  "start_social_graph_index_position_observer_if_enabled",
);
for (const marker of [
  'RUSTOK_SOCIAL_GRAPH_INDEX_POSITION_POLL_MS',
  'social_graph_index_consumer_enabled()',
  'ctx.shared_get::<Arc<IggyTransport>>()',
  'transport.config().clone()',
  'IggyConsumerPositionObserver::connect(',
  'connected.snapshot().await',
  'runtime_consumer_metrics::record_position_snapshot(',
  'STAGE_POSITION_SNAPSHOT',
  'projection remains active',
  'reconnecting observer without stopping projection',
  'StopHandle',
]) {
  requireText("server position observer", files.observer, marker);
}
for (const forbidden of [
  'IggyTransport::new',
  'shutdown()',
  'shutdown_transport(',
  'RuntimeGuardrailStatus::Critical',
]) {
  forbidText("server position observer", files.observer, forbidden);
}

if (failures.length > 0) {
  console.error("Iggy consumer-position verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Iggy consumer-position verification passed: one workspace SDK pin, read-only every-partition committed/high-watermark observation, fail-closed lag calculation, completeness-gated bounded metrics, shared-transport configuration, independent retry, and no projection/readiness coupling are locked.",
);
