#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  serverCargo: readFileSync("apps/server/Cargo.toml", "utf8"),
  services: readFileSync("apps/server/src/services/mod.rs", "utf8"),
  bootstrap: readFileSync(
    "apps/server/src/services/server_bootstrap.rs",
    "utf8",
  ),
  eventFactory: readFileSync(
    "apps/server/src/services/event_transport_factory.rs",
    "utf8",
  ),
  worker: readFileSync(
    "apps/server/src/services/social_graph_index_worker.rs",
    "utf8",
  ),
  guardrails: readFileSync(
    "apps/server/src/services/runtime_guardrails.rs",
    "utf8",
  ),
  health: readFileSync("apps/server/src/controllers/health.rs", "utf8"),
  metrics: readFileSync("apps/server/src/controllers/metrics.rs", "utf8"),
  telemetry: readFileSync(
    "crates/rustok-telemetry/src/runtime_consumer_metrics.rs",
    "utf8",
  ),
  consumer: readFileSync(
    "crates/rustok-social-graph/src/index_consumer.rs",
    "utf8",
  ),
  contractCursor: readFileSync(
    "crates/rustok-iggy/src/contract_consumer.rs",
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
  "server Cargo feature",
  files.serverCargo,
  'mod-social_graph = ["rustok-social-graph/graphql", "rustok-social-graph/index-consumer"]',
);
requireText(
  "services module",
  files.services,
  "pub mod social_graph_index_worker;",
);
requireText(
  "server bootstrap",
  files.bootstrap,
  "start_social_graph_index_worker_if_enabled",
);
requireText("server bootstrap", files.bootstrap, ".await?;");

for (const marker of [
  "let iggy_transport = Arc::new(",
  "ctx.shared_insert(Arc::clone(&iggy_transport));",
  "let transport: Arc<dyn EventTransport> = iggy_transport;",
  "Creating another bundled transport would start a second native broker",
]) {
  requireText("event runtime", files.eventFactory, marker);
}

for (const marker of [
  "RUSTOK_SOCIAL_GRAPH_INDEX_CONSUMER_ENABLED",
  "Err(env::VarError::NotPresent) => Ok(false)",
  "EventDeliveryProfile::OutboxIggy",
  "requires rustok.events.delivery_profile=outbox_iggy",
  "ctx.shared_get::<Arc<IggyTransport>>()",
  "outbox_iggy runtime did not publish its configured Iggy transport",
  "SocialGraphIndexWorkerHandle",
  "pub fn is_ready(&self) -> bool",
  "StopHandle",
  "receive_next()",
  "project_consumed(consumed)",
  "publish_consumed_to_dlq",
  "acknowledge_consumed(consumed)",
  "acknowledge_terminal_result",
  "retry_delay",
  "settings.events.dlq.enabled",
  "retrying acknowledgement only",
  "broker offset uncommitted",
  "runtime_consumer_metrics::ensure_registered()",
  "runtime_consumer_metrics::begin_delivery",
  "runtime_consumer_metrics::complete_delivery",
]) {
  requireText("server worker", files.worker, marker);
}

for (const forbidden of [
  "IggyTransport::new",
  "IggyConnectorSettingsService::resolved_config",
  "shutdown_transport(",
]) {
  forbidText("server worker", files.worker, forbidden);
}

for (const marker of [
  "SocialGraphIndexWorkerHandle",
  "social_graph_index_consumer_enabled",
  "observe_social_graph_index_worker",
  "Ok(false) => {}",
  "Ok(true) => observe_worker(",
  "Social Graph Index durable consumer",
  "handle.is_ready()",
  "RuntimeGuardrailStatus::Critical",
  "Social Graph Index consumer enablement is invalid",
]) {
  requireText("runtime guardrails", files.guardrails, marker);
}
requireText(
  "readiness endpoint",
  files.health,
  "checks.push(check_runtime_guardrails(&ctx).await);",
);
requireText(
  "aggregate guardrail metrics",
  files.metrics,
  "payload.push_str(&render_runtime_guardrail_metrics(&ctx).await);",
);

for (const marker of [
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
  requireText("runtime consumer telemetry", files.telemetry, marker);
}

for (const marker of [
  "pub const fn stable_code(&self)",
  "pub fn is_retryable(&self)",
  "pub async fn receive_next(",
  "pub async fn project_consumed(",
  "pub async fn acknowledge_consumed(",
  "pub async fn publish_consumed_to_dlq(",
  "pub async fn move_to_dlq_and_acknowledge(",
  "consumed.raw_payload().to_vec()",
  "self.transport",
  ".move_to_dlq(entry)",
  "self.publish_consumed_to_dlq(consumed, stable_error_code, retry_count)",
  "self.acknowledge_consumed(consumed).await",
  "DeadLettered",
]) {
  requireText("Social Graph consumer", files.consumer, marker);
}

for (const marker of [
  "pub raw_payload: Vec<u8>",
  "let raw_payload = message.payload;",
  "pub fn raw_payload(&self) -> &[u8]",
]) {
  requireText("Iggy contract cursor", files.contractCursor, marker);
}

const durableApply = files.worker.indexOf(
  "consumer.project_consumed(consumed).await",
);
const durableAck = files.worker.indexOf("acknowledge_terminal_result(");
if (durableApply < 0 || durableAck <= durableApply) {
  failures.push(
    "worker must enter terminal acknowledgement-only handling only after projection succeeds",
  );
}

const workerDlqPublish = files.worker.indexOf(".publish_consumed_to_dlq(");
const workerDlqAck = files.worker.indexOf(
  "return acknowledge_terminal_result(",
  workerDlqPublish,
);
if (workerDlqPublish < 0 || workerDlqAck <= workerDlqPublish) {
  failures.push(
    "worker must publish poison delivery to DLQ before entering source acknowledgement-only handling",
  );
}

const consumerDlqPublish = files.consumer.indexOf(
  "self.publish_consumed_to_dlq(consumed, stable_error_code, retry_count)",
);
const consumerDlqAck = files.consumer.indexOf(
  "self.acknowledge_consumed(consumed).await",
  consumerDlqPublish,
);
if (consumerDlqPublish < 0 || consumerDlqAck <= consumerDlqPublish) {
  failures.push(
    "convenience DLQ operation must publish before source acknowledgement",
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

const productionConsumer = files.consumer.split("#[cfg(test)]")[0];
const productionWorker = files.worker.split("#[cfg(test)]")[0];
for (const [name, source] of [
  ["Social Graph consumer", productionConsumer],
  ["server worker", productionWorker],
]) {
  for (const forbidden of [
    "index_schemas",
    "index_entities",
    "social_graph_relations",
    "SELECT ",
    "INSERT INTO",
    "UPDATE ",
    "DELETE FROM",
  ]) {
    forbidText(name, source, forbidden);
  }
}

if (failures.length > 0) {
  console.error("Social Graph Index worker lifecycle verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index worker lifecycle verification passed: default-off host composition, one shared EventRuntime Iggy connector, outbox_iggy gating, StopHandle shutdown, enabled-worker readiness, shared Prometheus consumer metrics, bounded retries, result-first acknowledgement-only recovery, staged exact-byte DLQ-before-ack, and owner-table isolation are locked.",
);
