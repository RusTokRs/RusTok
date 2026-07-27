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
  "open_persistent_contract_consumer_group",
  "SOCIAL_GRAPH_INDEX_CONSUMER_GROUP",
  'SOCIAL_GRAPH_INDEX_TOPIC: &str = "domain"',
  "ContractEventPayload::SocialGraphRelation",
  "MutationDelivery::from_event",
  "SchemaRegistry::new()",
  "registry.register(schema.clone())?",
  "PostgresSchemaRegistrationStore::new(db.clone())",
  ".register(envelope.tenant_id(), &self.schema)",
  "PostgresMutationStore::new(db)",
  "self.store.apply(&self.registry, &delivery).await?",
  "self.group",
  ".acknowledge(&consumed)",
  "pub async fn process_next(",
  "&mut self",
  "IgnoredUnrelated",
]) {
  requireText("runtime consumer", files.consumer, marker);
}

const registrationPosition = files.consumer.indexOf(
  ".register(envelope.tenant_id(), &self.schema)",
);
const applyPosition = files.consumer.indexOf(
  "self.store.apply(&self.registry, &delivery).await?",
);
const acknowledgePosition = files.consumer.indexOf(".acknowledge(&consumed)");
if (
  registrationPosition < 0 ||
  applyPosition < 0 ||
  acknowledgePosition < 0 ||
  applyPosition <= registrationPosition ||
  acknowledgePosition <= applyPosition
) {
  failures.push(
    "runtime consumer must persist the tenant schema, durably apply or terminally recognize the Index delivery, and only then acknowledge the broker message",
  );
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
  forbidText("runtime consumer", files.consumer, forbidden);
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
  "Delivered durable Index apply/ack consumer",
);
requireText(
  "Social Graph plan",
  files.socialPlan,
  "host lifecycle composition",
);
requireText(
  "Profiles plan",
  files.profilesPlan,
  "projection-based authorization",
);

if (failures.length > 0) {
  console.error("Social Graph Index runtime consumer verification failed:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log(
  "Social Graph Index runtime consumer verification passed: optional runtime composition, sealed-family filtering, Index-owned tenant schema persistence, durable inbox apply, result-first acknowledgement, unrelated-event handling, and authoritative owner-port privacy are locked.",
);
