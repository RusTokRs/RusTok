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
  receipt: readFileSync(
    "crates/rustok-social-graph/src/index_dlq_receipt.rs",
    "utf8",
  ),
  poisonReceipt: readFileSync(
    "crates/rustok-iggy-connector/src/consumer_poison_receipt.rs",
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
  "receive_delivery()",
  "PersistentContractDelivery::Event",
  "PersistentContractDelivery::DecodeFailure",
  "project_consumed(consumed)",
  "publish_consumed_to_dlq",
  "publish_dead_lettered_result",
  "acknowledge_consumed(consumed)",
  "acknowledge_terminal_result",
  "ConsumerPoisonReceiptStore::new(ctx.db_clone())",
  "ConsumerPoisonIdentity::new(",
  "poison_receipts.find(&identity).await",
  "!config.dlq_enabled && !continuing_durable_receipt",
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await",
  "mark_raw_poison_published",
  "acknowledge_raw_poison_result",
  "acknowledge_decode_failure(failure)",
  "retry_delay",
  "settings.events.dlq.enabled",
  "config.dlq_enabled || continuing_durable_receipt",
  "retrying acknowledgement only",
  "durable DLQ receipt remains published",
  "durable neutral receipt remains published",
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
  "redelivery may republish until a durable DLQ identity exists",
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
  "rustok_runtime_consumer_position_snapshot_timestamp_seconds",
  "rustok_runtime_consumer_position_partition_count",
  "rustok_runtime_consumer_position_complete",
  "rustok_runtime_consumer_lag",
]) {
  requireText("runtime consumer telemetry", files.telemetry, marker);
}
for (const forbiddenMetric of [
  "rustok_runtime_consumer_source_offset",
  "rustok_runtime_consumer_source_partition",
]) {
  forbidText("runtime consumer telemetry", files.telemetry, forbiddenMetric);
}

for (const marker of [
  "pub const fn stable_code(&self)",
  "pub fn is_retryable(&self)",
  "pub async fn receive_delivery(",
  "pub async fn receive_next(",
  "pub async fn project_consumed(",
  "pub async fn acknowledge_consumed(",
  "pub async fn acknowledge_decode_failure(",
  "pub async fn publish_consumed_to_dlq(",
  "pub async fn move_to_dlq_and_acknowledge(",
  "self.consumed_dlq_receipt(consumed).await?",
  "SocialGraphIndexDlqReceiptState::Published",
  "consumed.raw_payload().to_vec()",
  "self.transport",
  ".move_to_dlq(entry)",
  ".mark_published(&identity, self.dlq_publisher_id)",
  "self.publish_consumed_to_dlq(consumed, stable_error_code, retry_count)",
  "self.acknowledge_consumed(consumed).await",
  "DeadLettered",
]) {
  requireText("Social Graph consumer", files.consumer, marker);
}

for (const marker of [
  "pub enum SocialGraphIndexDlqReceiptState",
  "pub async fn reserve_and_claim(",
  "pub async fn mark_published(",
  "pub async fn mark_acknowledged(",
  "SocialGraphIndexDlqPublishClaim::AlreadyPublished",
]) {
  requireText("Social Graph DLQ receipt", files.receipt, marker);
}

for (const marker of [
  "pub struct ConsumerPoisonReceiptStore",
  "pub async fn find(",
  "pub async fn reserve_and_claim(",
  "pub async fn mark_published(",
  "pub async fn mark_acknowledged(",
  "ConsumerPoisonPublishClaim::AlreadyPublished",
  "ConsumerPoisonPublishClaim::AlreadyAcknowledged",
]) {
  requireText("neutral poison receipt", files.poisonReceipt, marker);
}

for (const marker of [
  "pub raw_payload: Vec<u8>",
  "let raw_payload = message.payload;",
  "pub enum PersistentContractDelivery",
  "PersistentContractDelivery::DecodeFailure",
  "pub fn raw_payload(&self) -> &[u8]",
  "pub async fn acknowledge_decode_failure",
]) {
  requireText("Iggy contract cursor", files.contractCursor, marker);
}

const durableApply = files.worker.indexOf(
  "consumer.project_consumed(consumed).await",
);
const durableAck = files.worker.indexOf("acknowledge_terminal_result(");
if (durableApply < 0 || durableAck <= durableApply) {
  failures.push(
    "worker must enter decoded terminal acknowledgement-only handling only after projection or durable receipt recognition succeeds",
  );
}

const workerDlqPublish = files.worker.indexOf(".publish_consumed_to_dlq(");
const workerDlqAck = files.worker.indexOf(
  "return acknowledge_terminal_result(",
  workerDlqPublish,
);
if (workerDlqPublish < 0 || workerDlqAck <= workerDlqPublish) {
  failures.push(
    "worker must publish or recognize a decoded-event durable DLQ receipt before source acknowledgement-only handling",
  );
}

const rawStart = files.worker.indexOf("async fn process_decode_failure(");
const rawEnd = files.worker.indexOf("async fn acknowledge_terminal_result(", rawStart);
const rawFlow =
  rawStart >= 0 && rawEnd > rawStart
    ? files.worker.slice(rawStart, rawEnd)
    : "";
const rawLookup = rawFlow.indexOf("poison_receipts.find(&identity).await");
const rawDisabledGate = rawFlow.indexOf(
  "!config.dlq_enabled && !continuing_durable_receipt",
);
const rawReserve = rawFlow.indexOf(".reserve_and_claim(");
const rawPublish = rawFlow.indexOf(
  "transport.move_to_dlq(failure.to_dlq_entry(1)).await",
);
const rawPublished = rawFlow.indexOf("mark_raw_poison_published(", rawPublish);
const rawAck = rawFlow.indexOf("acknowledge_raw_poison_result(", rawPublished);
if (
  rawLookup < 0 ||
  rawDisabledGate <= rawLookup ||
  rawReserve <= rawDisabledGate ||
  rawPublish <= rawReserve ||
  rawPublished <= rawPublish ||
  rawAck <= rawPublished
) {
  failures.push(
    "raw worker path must recognize durable recovery, reject only new disabled-DLQ work, reserve, publish exact bytes, persist published, and only then acknowledge",
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
    "convenience decoded DLQ operation must publish or recognize the durable receipt before source acknowledgement",
  );
}

const decodedAcknowledgeOnlyStart = files.worker.indexOf(
  "async fn acknowledge_terminal_result(",
);
const decodedAcknowledgeOnlyBody =
  decodedAcknowledgeOnlyStart >= 0
    ? files.worker.slice(decodedAcknowledgeOnlyStart)
    : "";
for (const forbidden of [
  "publish_consumed_to_dlq",
  "move_to_dlq_and_acknowledge",
  "project_consumed(consumed)",
]) {
  forbidText(
    "decoded terminal acknowledgement-only path",
    decodedAcknowledgeOnlyBody,
    forbidden,
  );
}

const rawAckStart = files.worker.indexOf("async fn acknowledge_raw_poison_result(");
const rawAckBody =
  rawAckStart >= 0 && decodedAcknowledgeOnlyStart > rawAckStart
    ? files.worker.slice(rawAckStart, decodedAcknowledgeOnlyStart)
    : "";
const rawSourceAck = rawAckBody.indexOf(
  "consumer.acknowledge_decode_failure(failure).await",
);
const rawReceiptAck = rawAckBody.indexOf(
  "poison_receipts.mark_acknowledged(identity).await",
);
if (rawSourceAck < 0 || rawReceiptAck <= rawSourceAck) {
  failures.push(
    "raw acknowledgement-only path must commit the source before best-effort receipt bookkeeping",
  );
}
for (const forbidden of [
  "move_to_dlq(",
  "reserve_and_claim",
  "mark_published",
  "project_consumed",
]) {
  forbidText("raw acknowledgement-only path", rawAckBody, forbidden);
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
for (const forbidden of [
  "tenant_id",
  "event_id",
  "ProfilePresentationService",
  "SocialGraphPrivacyReadPort",
]) {
  forbidText("raw poison worker flow", rawFlow, forbidden);
}

if (failures.length > 0) {
  console.error("Social Graph Index worker lifecycle verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index worker lifecycle verification passed: default-off host composition, one shared EventRuntime Iggy connector, outbox_iggy gating, StopHandle shutdown, enabled-worker readiness, bounded telemetry, decoded projection/DLQ/ack retries, typed raw delivery, neutral receipt recovery, exact-byte durable publish-before-ack, acknowledgement-only recovery, and owner-table/privacy isolation are locked.",
);
