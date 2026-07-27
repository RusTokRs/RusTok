#!/usr/bin/env node
// Social Graph durable command receipt and idempotency-identity guardrails.

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
  cargo: "crates/rustok-social-graph/Cargo.toml",
  entity: "crates/rustok-social-graph/src/entities/command_receipt.rs",
  entities: "crates/rustok-social-graph/src/entities/mod.rs",
  error: "crates/rustok-social-graph/src/error.rs",
  lib: "crates/rustok-social-graph/src/lib.rs",
  migration: "crates/rustok-social-graph/src/migrations/m20260726_000003_create_command_receipts.rs",
  migrations: "crates/rustok-social-graph/src/migrations/mod.rs",
  receipts: "crates/rustok-social-graph/src/receipts.rs",
  service: "crates/rustok-social-graph/src/service.rs",
  ports: "crates/rustok-social-graph/src/ports.rs",
  observability: "crates/rustok-social-graph/src/observability.rs",
  test: "crates/rustok-social-graph/tests/command_receipts_sqlite.rs",
  plan: "crates/rustok-social-graph/docs/implementation-plan.md",
};

for (const value of Object.values(paths)) assertExists(value);

const cargo = readRepo(paths.cargo);
const entity = readRepo(paths.entity);
const entities = readRepo(paths.entities);
const error = readRepo(paths.error);
const lib = readRepo(paths.lib);
const migration = readRepo(paths.migration);
const migrations = readRepo(paths.migrations);
const receipts = readRepo(paths.receipts);
const service = readRepo(paths.service);
const ports = readRepo(paths.ports);
const observability = readRepo(paths.observability);
const test = readRepo(paths.test);
const plan = readRepo(paths.plan);

assertContains(cargo, "serde_json.workspace = true", `${paths.cargo}: receipt serialization dependency missing`);
assertContains(entities, "pub mod command_receipt;", `${paths.entities}: receipt entity not wired`);
assertContains(entity, 'table_name = "social_graph_command_receipts"', `${paths.entity}: receipt table ownership missing`);
for (const field of ["tenant_id", "idempotency_key", "schema_version", "request_json", "status", "response_json", "completed_at"]) {
  assertContains(entity, `pub ${field}`, `${paths.entity}: receipt field missing: ${field}`);
}

assertContains(migration, "ux_social_graph_command_receipt_identity", `${paths.migration}: unique receipt identity missing`);
assertContains(migration, "(tenant_id, idempotency_key)", `${paths.migration}: tenant/key uniqueness missing`);
assertContains(migration, "schema_version INTEGER NOT NULL DEFAULT 1", `${paths.migration}: receipt schema version missing`);
assertContains(migration, "schema_version = 1", `${paths.migration}: receipt schema version constraint missing`);
assertContains(migration, "status IN ('processing', 'completed')", `${paths.migration}: receipt status constraint missing`);
assertContains(migration, "status = 'completed' AND response_json IS NOT NULL", `${paths.migration}: completed receipt integrity missing`);
assertContains(migration, "idx_social_graph_command_receipt_cleanup", `${paths.migration}: bounded cleanup index missing`);
assertContains(migration, "(tenant_id, status, completed_at, id)", `${paths.migration}: cleanup index ordering missing`);
assertContains(migrations, "m20260726_000003_create_command_receipts", `${paths.migrations}: receipt migration not registered`);
assertContains(lib, "mod receipts;", `${paths.lib}: receipt implementation must remain owner-private`);

for (const marker of [
  "COMMAND_RECEIPT_SCHEMA_VERSION: i32 = 1",
  "MAX_IDEMPOTENCY_KEY_BYTES: usize = 191",
  "OnConflict::columns",
  "command_receipt::Column::TenantId",
  "command_receipt::Column::IdempotencyKey",
  ".do_nothing()",
  "receipt.schema_version != COMMAND_RECEIPT_SCHEMA_VERSION",
  "receipt.request_json != expected_json",
  "SocialGraphError::IdempotencyConflict",
  "receipt.status != STATUS_COMPLETED",
  "serde_json::from_value(response)",
  "transaction.commit().await?",
  "transaction.rollback().await?",
]) {
  assertContains(receipts, marker, `${paths.receipts}: receipt guardrail missing: ${marker}`);
}

for (const marker of [
  "set_relation_state_with_receipt",
  "SocialGraphCommandReceiptRequest",
  "receipts::admit",
  "SocialGraphCommandReceiptAdmission::Replay",
  "receipts::replay",
  "&receipt.transaction",
  "receipts::complete",
  "receipts::rollback",
]) {
  assertContains(service, marker, `${paths.service}: transactional receipt integration missing: ${marker}`);
}
assertContains(service, "pub(crate) async fn set_relation_state_with_receipt", `${paths.service}: receipt-aware write path must stay crate-private`);
assertNotContains(service, "pub async fn set_relation_state(", `${paths.service}: public raw relation mutation bypasses receipt admission`);

assertContains(ports, ".set_relation_state_with_receipt(", `${paths.ports}: command port bypasses receipts`);
for (const code of [
  "social_graph.idempotency_key_invalid",
  "social_graph.idempotency_conflict",
  "social_graph.command_receipt_corrupt",
]) {
  assertContains(ports, `"${code}"`, `${paths.ports}: stable receipt error code missing: ${code}`);
}
for (const variant of ["IdempotencyKeyInvalid", "IdempotencyConflict", "CommandReceiptCorrupt"]) {
  assertContains(error, variant, `${paths.error}: receipt error variant missing: ${variant}`);
}

assertNotContains(observability, "idempotency_key =", `${paths.observability}: raw idempotency keys must not enter telemetry`);
assertNotContains(observability, "request_json =", `${paths.observability}: command payload must not enter telemetry`);

for (const marker of [
  "receipt_replays_original_result_and_rejects_payload_reuse",
  '"follow-command-1"',
  "assert_eq!(replay.revision, 1)",
  "assert_eq!(current.revision, 2)",
  '"social_graph.idempotency_conflict"',
  "assert_eq!(receipt_count, 2)",
]) {
  assertContains(test, marker, `${paths.test}: receipt scenario missing: ${marker}`);
}

for (const marker of [
  "## Receipt retention and rollout contract",
  "Automatic cleanup remains disabled",
  "completed receipts only",
  "never delete `processing` receipts",
  "retain the receipt table and rows",
]) {
  assertContains(plan, marker, `${paths.plan}: operational receipt guidance missing: ${marker}`);
}

if (failures.length > 0) {
  console.error("Social Graph command receipt verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Social Graph command receipt verification passed");
