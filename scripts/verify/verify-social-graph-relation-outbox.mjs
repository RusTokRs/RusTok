#!/usr/bin/env node
// Social Graph sealed relation-event and transactional-outbox guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: expected file`);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const paths = {
  modules: "modules.toml",
  moduleManifest: "crates/rustok-social-graph/rustok-module.toml",
  api: "crates/rustok-social-graph/CRATE_API.md",
  event: "crates/rustok-events/src/social_graph.rs",
  contract: "crates/rustok-events/src/contract.rs",
  eventsLib: "crates/rustok-events/src/lib.rs",
  digestGenerator: "crates/rustok-events/examples/event_contract_digests.rs",
  eventTest: "crates/rustok-events/tests/social_graph_contracts.rs",
  cargo: "crates/rustok-social-graph/Cargo.toml",
  mapper: "crates/rustok-social-graph/src/external_events.rs",
  service: "crates/rustok-social-graph/src/service.rs",
  receipts: "crates/rustok-social-graph/src/receipts.rs",
  ports: "crates/rustok-social-graph/src/ports.rs",
  error: "crates/rustok-social-graph/src/error.rs",
  graphql: "crates/rustok-social-graph/src/graphql.rs",
  storefrontCargo: "crates/rustok-profiles/storefront/Cargo.toml",
  storefrontNative: "crates/rustok-profiles/storefront/src/transport/native_server_adapter.rs",
  test: "crates/rustok-social-graph/tests/relation_outbox_sqlite.rs",
};

for (const value of Object.values(paths)) assertExists(value);

const modules = readRepo(paths.modules);
const moduleManifest = readRepo(paths.moduleManifest);
const api = readRepo(paths.api);
const event = readRepo(paths.event);
const contract = readRepo(paths.contract);
const eventsLib = readRepo(paths.eventsLib);
const digestGenerator = readRepo(paths.digestGenerator);
const eventTest = readRepo(paths.eventTest);
const cargo = readRepo(paths.cargo);
const mapper = readRepo(paths.mapper);
const service = readRepo(paths.service);
const receipts = readRepo(paths.receipts);
const ports = readRepo(paths.ports);
const error = readRepo(paths.error);
const graphql = readRepo(paths.graphql);
const storefrontCargo = readRepo(paths.storefrontCargo);
const storefrontNative = readRepo(paths.storefrontNative);
const test = readRepo(paths.test);

assertContains(
  modules,
  'social_graph = { crate = "rustok-social-graph", source = "path", path = "crates/rustok-social-graph", depends_on = ["outbox"] }',
  `${paths.modules}: Social Graph must declare its required Outbox dependency`,
);
assertContains(moduleManifest, "[dependencies]", `${paths.moduleManifest}: dependencies section missing`);
assertContains(
  moduleManifest,
  'outbox = { version_req = ">=0.1.0" }',
  `${paths.moduleManifest}: Outbox dependency missing`,
);
for (const marker of [
  "SocialGraphService::new(DatabaseConnection) creates a read-only owner service",
  "SocialGraphService::with_event_bus(DatabaseConnection, TransactionalEventBus)",
  "social_graph.relation.state_changed",
  "social_graph.event_publication_unavailable",
]) {
  assertContains(api, marker, `${paths.api}: public write contract missing: ${marker}`);
}

for (const marker of [
  'event_type: "social_graph.relation.state_changed"',
  /name:\s*"active",[\s\S]{0,120}data_type:\s*"bool"/,
  "pub enum SocialGraphRelationEvent",
  "RelationStateChanged",
  "impl sealed::Sealed for SocialGraphRelationEvent",
  "impl EventContract for SocialGraphRelationEvent",
  "impl ValidateEvent for SocialGraphRelationEvent",
  'matches!(relation_kind.as_str(), "block" | "mute" | "follow")',
  'validate_range("revision", *revision, 1, i64::MAX)',
]) {
  assertContains(event, marker, `${paths.event}: event guardrail missing: ${marker}`);
}
assertNotContains(event, 'data_type: "boolean"', `${paths.event}: unsupported schema primitive boolean`);

for (const marker of [
  "SocialGraphRelation(SocialGraphRelationEvent)",
  "Self::SocialGraphRelation(event) => event.event_type()",
  "Self::SocialGraphRelation(event) => event.schema_version()",
  "Self::SocialGraphRelation(event) => event.validate()",
]) {
  assertContains(contract, marker, `${paths.contract}: sealed payload registration missing: ${marker}`);
}
for (const marker of [
  "mod social_graph;",
  "SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS",
  "social_graph_relation_event_schema",
  ".chain(SOCIAL_GRAPH_RELATION_EVENT_SCHEMAS.iter())",
]) {
  assertContains(eventsLib, marker, `${paths.eventsLib}: event registry missing: ${marker}`);
}
for (const marker of [
  "event_contract_digests()",
  'flag == "--write"',
  'contracts/event-contract-digests.json',
]) {
  assertContains(digestGenerator, marker, `${paths.digestGenerator}: digest generator missing: ${marker}`);
}

for (const marker of [
  "social_graph_family_has_one_registered_versioned_contract",
  "social_graph_contract_is_typed_validated_and_enveloped",
  "social_graph_contract_rejects_invalid_identity_kind_and_revision",
  "social_graph_external_payload_excludes_command_and_request_metadata",
]) {
  assertContains(eventTest, marker, `${paths.eventTest}: contract proof missing: ${marker}`);
}

for (const dependency of ["rustok-events.workspace = true", "rustok-outbox.workspace = true"]) {
  assertContains(cargo, dependency, `${paths.cargo}: dependency missing: ${dependency}`);
}
for (const marker of [
  "event_for_relation",
  "SocialGraphRelationEvent::RelationStateChanged",
  "relation.relation_kind.as_str()",
]) {
  assertContains(mapper, marker, `${paths.mapper}: owner event mapper missing: ${marker}`);
}
for (const forbidden of ["idempotency_key", "expected_revision", "request_json", "response_json"]) {
  assertNotContains(mapper.split("#[cfg(test)]")[0], forbidden, `${paths.mapper}: event mapper leaked ${forbidden}`);
}

for (const marker of [
  "event_bus: Option<TransactionalEventBus>",
  "pub fn with_event_bus",
  "ok_or(SocialGraphError::EventPublicationUnavailable)",
  "state_changed: bool",
  "state_changed: false",
  "state_changed: true",
  "created_at: Set(now.clone())",
]) {
  assertContains(service, marker, `${paths.service}: write composition missing: ${marker}`);
}

for (const marker of [
  "publish_contract_in_tx(&transaction, tenant_id, actor_id, event)",
  "SocialGraphError::EventPublicationUnavailable",
  "transaction.rollback().await?",
  "transaction.commit().await?",
]) {
  assertContains(receipts, marker, `${paths.receipts}: transactional event guardrail missing: ${marker}`);
}
const publishIndex = receipts.indexOf("publish_contract_in_tx");
const completeIndex = receipts.indexOf("command_receipt::Entity::update_many()", publishIndex);
const commitIndex = receipts.indexOf("transaction.commit().await?", completeIndex);
if (!(publishIndex >= 0 && completeIndex > publishIndex && commitIndex > completeIndex)) {
  fail(`${paths.receipts}: expected event publish -> receipt completion -> commit order`);
}

for (const marker of [
  '"social_graph.event_publication_unavailable"',
  "SocialGraphError::EventPublicationUnavailable",
]) {
  assertContains(ports + error, marker, `stable event failure contract missing: ${marker}`);
}

assertContains(graphql, "ctx.data::<TransactionalEventBus>()", `${paths.graphql}: GraphQL writes must require transactional bus`);
assertContains(graphql, "SocialGraphService::with_event_bus", `${paths.graphql}: GraphQL write service is not composed`);
assertContains(storefrontCargo, '"dep:rustok-outbox"', `${paths.storefrontCargo}: SSR outbox dependency missing`);
assertContains(storefrontNative, "shared_get::<TransactionalEventBus>()", `${paths.storefrontNative}: native writes must require host bus`);
assertContains(storefrontNative, "SocialGraphService::with_event_bus", `${paths.storefrontNative}: native write service is not composed`);

for (const marker of [
  "relation_changes_publish_once_while_noop_and_replay_do_not",
  "missing_transactional_outbox_rolls_back_relation_and_receipt",
  'assert_eq!(event_count(&db).await, 1, "no-op must not emit an event")',
  'assert_eq!(event_count(&db).await, 2, "replay must not emit an event")',
  '"social_graph.event_publication_unavailable"',
  'table_count(&db, "social_graph_relations")',
  'table_count(&db, "social_graph_command_receipts")',
]) {
  assertContains(test, marker, `${paths.test}: transactional outbox scenario missing: ${marker}`);
}

if (failures.length > 0) {
  console.error("Social Graph relation outbox verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Social Graph relation outbox verification passed");
