#!/usr/bin/env node

import { readFileSync } from "node:fs";

const files = {
  cargo: readFileSync("crates/rustok-social-graph/Cargo.toml", "utf8"),
  lib: readFileSync("crates/rustok-social-graph/src/lib.rs", "utf8"),
  adapter: readFileSync("crates/rustok-social-graph/src/index.rs", "utf8"),
  consumer: readFileSync(
    "crates/rustok-social-graph/src/index_consumer.rs",
    "utf8",
  ),
  api: readFileSync("crates/rustok-social-graph/CRATE_API.md", "utf8"),
  socialPlan: readFileSync(
    "crates/rustok-social-graph/docs/implementation-plan.md",
    "utf8",
  ),
  profilesPlan: readFileSync(
    "crates/rustok-profiles/docs/implementation-plan.md",
    "utf8",
  ),
};
const consumerProduction = files.consumer.split("#[cfg(test)]")[0];

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
  "Cargo.toml",
  files.cargo,
  'rustok-iggy = { workspace = true, optional = true }',
);
requireText(
  "Cargo.toml",
  files.cargo,
  'index-consumer = ["index", "dep:rustok-iggy"]',
);
requireText("lib.rs", files.lib, '#[cfg(feature = "index-consumer")]');
requireText("lib.rs", files.lib, "pub mod index_consumer;");

for (const marker of [
  "PersistentContractConsumerGroup",
  "PersistentContractDelivery",
  "ConsumedContractDecodeFailure",
  "open_persistent_contract_consumer_group",
  "SOCIAL_GRAPH_INDEX_CONSUMER_GROUP",
  'SOCIAL_GRAPH_INDEX_TOPIC: &str = "domain"',
  "ContractEventPayload::SocialGraphRelation",
  "MutationDelivery::from_event",
  "SocialGraphIndexProjector",
  "SchemaRegistry::new()",
  "registry.register(schema.clone())?",
  "PostgresSchemaRegistrationStore::new(db.clone())",
  ".register(envelope.tenant_id(), &self.schema)",
  "PostgresMutationStore::new(db)",
  "self.store.apply(&self.registry, &delivery).await?",
  "pub async fn receive_delivery(",
  ".receive_delivery()",
  "pub async fn receive_next(",
  "PersistentContractDelivery::Event(consumed)",
  "PersistentContractDelivery::DecodeFailure(failure)",
  "pub async fn project_consumed(",
  "self.projector.apply_envelope(&consumed.envelope).await",
  "pub async fn acknowledge_consumed(",
  ".acknowledge(consumed)",
  "pub async fn acknowledge_decode_failure(",
  ".acknowledge_decode_failure(consumed)",
  "pub async fn process_next(",
  "IgnoredUnrelated",
]) {
  requireText("runtime consumer", consumerProduction, marker);
}
for (const marker of [
  "projector_persists_schema_before_result_first_mutation_apply",
  "MutationApplyOutcome::Duplicate",
  "is_deleted = TRUE AND source_version = 2",
]) {
  requireText("runtime consumer tests", files.consumer, marker);
}

const registrationPosition = consumerProduction.indexOf(
  ".register(envelope.tenant_id(), &self.schema)",
);
const applyPosition = consumerProduction.indexOf(
  "self.store.apply(&self.registry, &delivery).await?",
);
if (
  registrationPosition < 0 ||
  applyPosition < 0 ||
  applyPosition <= registrationPosition
) {
  failures.push(
    "projector must persist the tenant schema before durable mutation apply",
  );
}

const typedReceiveStart = consumerProduction.indexOf(
  "pub async fn receive_delivery(",
);
const compatibilityReceiveStart = consumerProduction.indexOf(
  "pub async fn receive_next(",
  typedReceiveStart,
);
if (
  typedReceiveStart < 0 ||
  compatibilityReceiveStart <= typedReceiveStart
) {
  failures.push(
    "typed receive must be the primary owner boundary and compatibility receive must delegate after it",
  );
}

const processStart = consumerProduction.indexOf("pub async fn process_next(");
const processBody = processStart >= 0 ? consumerProduction.slice(processStart) : "";
const projectPosition = processBody.indexOf(
  "let outcome = self.project_consumed(&consumed).await?;",
);
const acknowledgePosition = processBody.indexOf(
  "self.acknowledge_consumed(&consumed).await?;",
);
if (
  projectPosition < 0 ||
  acknowledgePosition < 0 ||
  acknowledgePosition <= projectPosition
) {
  failures.push(
    "direct decoded consumer path must durably project before broker acknowledgement",
  );
}

const rawAckStart = consumerProduction.indexOf(
  "pub async fn acknowledge_decode_failure(",
);
const rawAckEnd = consumerProduction.indexOf(
  "pub async fn consumed_dlq_receipt(",
  rawAckStart,
);
const rawAckBody =
  rawAckStart >= 0 && rawAckEnd > rawAckStart
    ? consumerProduction.slice(rawAckStart, rawAckEnd)
    : "";
requireText(
  "raw acknowledgement adapter",
  rawAckBody,
  ".acknowledge_decode_failure(consumed)",
);
for (const forbidden of [
  "project_consumed",
  "publish_consumed_to_dlq",
  "move_to_dlq",
  "tenant_id",
  "event_id",
]) {
  forbidText("raw acknowledgement adapter", rawAckBody, forbidden);
}

for (const forbidden of [
  "social_graph_relations",
  "command_receipt",
  "index_schemas",
  "ProfilePresentationService",
  "SocialGraphPrivacyReadPort",
  "SELECT ",
  "INSERT INTO",
  "DELETE FROM",
  "UPDATE social_graph",
]) {
  forbidText("runtime consumer production code", consumerProduction, forbidden);
}

for (const marker of [
  "result-first",
  "duplicate/stale",
  "Bounded Social Graph replay",
  "must not authorize",
]) {
  requireText("CRATE_API.md", files.api, marker);
}
requireText(
  "Social Graph plan",
  files.socialPlan,
  "Delivered persistent consumer, host lifecycle, and readiness",
);
requireText(
  "Social Graph plan",
  files.socialPlan,
  "Social Graph persistence remains authoritative for drift repair",
);
requireText(
  "Profiles plan",
  files.profilesPlan,
  "never authorizes from an event",
);
requireText(
  "Profiles plan",
  files.profilesPlan,
  "never authorize visibility",
);

if (failures.length > 0) {
  console.error("Social Graph Index runtime consumer verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index runtime consumer verification passed: optional runtime composition, typed event/decode-failure receive, sealed-family filtering, Index-owned tenant schema persistence, staged durable inbox apply, decoded result-first acknowledgement, raw acknowledgement isolation, unrelated-event handling, integration fixtures, and authoritative owner-port privacy are locked.",
);
