#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  telemetryLib: readFileSync("crates/rustok-telemetry/src/lib.rs", "utf8"),
  metrics: readFileSync(
    "crates/rustok-telemetry/src/consumer_poison_metrics.rs",
    "utf8",
  ),
  services: readFileSync("apps/server/src/services/mod.rs", "utf8"),
  bootstrap: readFileSync(
    "apps/server/src/services/server_bootstrap.rs",
    "utf8",
  ),
  observer: readFileSync(
    "apps/server/src/services/social_graph_index_poison_observer.rs",
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
  "telemetry lib",
  files.telemetryLib,
  "pub mod consumer_poison_metrics;",
);

for (const marker of [
  '"rustok_runtime_consumer_poison_receipts"',
  '"rustok_runtime_consumer_poison_snapshot_available"',
  '"rustok_runtime_consumer_poison_snapshot_timestamp_seconds"',
  '&["consumer", "state"]',
  '"total"',
  '"reserved"',
  '"publishing"',
  '"expired_publishing"',
  '"published"',
  '"acknowledged"',
  "pub fn record_snapshot(",
  "pub fn record_unavailable(",
  ".set(0);",
  "metric_value(u64::MAX)",
]) {
  requireText("poison metrics", files.metrics, marker);
}

for (const marker of [
  '#[cfg(feature = "mod-social_graph")]\npub mod social_graph_index_poison_observer;',
  "start_social_graph_index_poison_observer_if_enabled",
]) {
  requireText("server composition", `${files.services}\n${files.bootstrap}`, marker);
}

for (const marker of [
  "ConsumerPoisonReceiptInspector::new(ctx.db_clone())",
  ".summarize(SOCIAL_GRAPH_INDEX_CONSUMER_GROUP)",
  "consumer_poison_metrics::record_snapshot(",
  "consumer_poison_metrics::record_unavailable(METRICS_CONSUMER)",
  "error.stable_code()",
  "projection remains active",
  "RUSTOK_SOCIAL_GRAPH_INDEX_POISON_POLL_MS",
  "MAX_POLL_MS: u64 = 300_000",
]) {
  requireText("poison observer", files.observer, marker);
}

const observerProduction = files.observer.split("#[cfg(test)]")[0];
for (const forbidden of [
  ".reserve_and_claim(",
  ".release_claim(",
  ".mark_published(",
  ".mark_acknowledged(",
  ".acknowledge_decode_failure(",
  ".move_to_dlq(",
  "DELETE FROM",
  "UPDATE iggy_consumer_poison_receipts",
  "INSERT INTO iggy_consumer_poison_receipts",
  "delivery_id =",
  "source_offset =",
  "publisher_id =",
  "stable_error_code =",
]) {
  forbidText("read-only poison observer production code", observerProduction, forbidden);
}

for (const forbidden of [
  '"delivery_id"',
  '"source_stream"',
  '"source_topic"',
  '"source_partition"',
  '"source_offset"',
  '"payload"',
  '"error_code"',
  '"publisher_id"',
  '"tenant_id"',
  '"event_id"',
]) {
  forbidText("poison metric labels", files.metrics, forbidden);
}

if (failures.length > 0) {
  console.error("Social Graph Index poison observer verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index poison observer verification passed: count-only bounded states, stale-value clearing, fixed consumer scope, read-only inspection, and delivery/privacy isolation are locked.",
);
