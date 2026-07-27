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
  "acknowledge_consumed(consumed)",
  "move_to_dlq_and_acknowledge",
  "retry_delay",
  "settings.events.dlq.enabled",
  "retrying acknowledgement only",
  "broker offset uncommitted",
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
  "pub const fn stable_code(&self)",
  "pub fn is_retryable(&self)",
  "pub async fn receive_next(",
  "pub async fn project_consumed(",
  "pub async fn acknowledge_consumed(",
  "pub async fn move_to_dlq_and_acknowledge(",
  "consumed.raw_payload().to_vec()",
  "self.transport",
  ".move_to_dlq(entry)",
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
const durableAck = files.worker.indexOf("acknowledge_durable_result(");
if (durableApply < 0 || durableAck <= durableApply) {
  failures.push(
    "worker must enter acknowledgement-only handling only after projection succeeds",
  );
}

const dlqPublish = files.consumer.indexOf(".move_to_dlq(entry)");
const dlqAck = files.consumer.indexOf(
  "self.acknowledge_consumed(consumed).await",
);
if (dlqPublish < 0 || dlqAck <= dlqPublish) {
  failures.push("DLQ publication must complete before source acknowledgement");
}

const acknowledgeOnlyStart = files.worker.indexOf(
  "async fn acknowledge_durable_result(",
);
const acknowledgeOnlyBody =
  acknowledgeOnlyStart >= 0 ? files.worker.slice(acknowledgeOnlyStart) : "";
for (const forbidden of [
  "move_to_dlq_and_acknowledge",
  "project_consumed(consumed)",
]) {
  forbidText("acknowledgement-only path", acknowledgeOnlyBody, forbidden);
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
  "Social Graph Index worker lifecycle verification passed: default-off host composition, one shared EventRuntime Iggy connector, outbox_iggy gating, StopHandle shutdown, bounded retries, result-first acknowledgement-only recovery, exact-byte DLQ-before-ack, and owner-table isolation are locked.",
);
