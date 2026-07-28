#!/usr/bin/env node
// Social Graph completed-receipt maintenance boundary guardrails.

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
  lib: "crates/rustok-social-graph/src/lib.rs",
  ports: "crates/rustok-social-graph/src/ports.rs",
  maintenance: "crates/rustok-social-graph/src/maintenance.rs",
  receipts: "crates/rustok-social-graph/src/receipts.rs",
  test: "crates/rustok-social-graph/tests/receipt_cleanup_sqlite.rs",
};

for (const value of Object.values(paths)) assertExists(value);

const lib = readRepo(paths.lib);
const ports = readRepo(paths.ports);
const maintenance = readRepo(paths.maintenance);
const receipts = readRepo(paths.receipts);
const test = readRepo(paths.test);

for (const marker of [
  "pub mod maintenance;",
  "SocialGraphReceiptMaintenanceService",
  "SocialGraphReceiptCleanupCommand",
  "SocialGraphReceiptCleanupResult",
  "SocialGraphReceiptMaintenancePort",
]) {
  assertContains(lib, marker, `${paths.lib}: cleanup export missing: ${marker}`);
}

for (const marker of [
  "MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH: u32 = 1_000",
  "completed_before_unix_seconds: i64",
  "pub limit: u32",
  "pub dry_run: bool",
  "pub matched_receipts: u64",
  "pub deleted_receipts: u64",
  "pub oldest_retained_completed_at_unix_seconds: Option<i64>",
  "trait SocialGraphReceiptMaintenancePort",
]) {
  assertContains(ports, marker, `${paths.ports}: cleanup contract missing: ${marker}`);
}

for (const marker of [
  "PortCallPolicy::write()",
  "PortActorKind::User",
  '"social_graph.receipt_cleanup_forbidden"',
  '"social_graph.receipt_cleanup_limit_invalid"',
  '"social_graph.receipt_cleanup_cutoff_invalid"',
  '"social_graph.receipt_cleanup_cutoff_future"',
  "DateTime::<Utc>::from_timestamp",
  "completed_before >= Utc::now()",
  "command.limit > MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH",
  "receipts::cleanup_completed(",
  'RECEIPT_CLEANUP_OPERATION: &str = "social_graph.receipt_cleanup"',
  'target: "rustok_social_graph::operations"',
  "log_cleanup_failure(",
  "matched_receipts",
  "deleted_receipts",
  "oldest_retained_completed_at_unix_seconds = ?oldest_retained_completed_at_unix_seconds",
  "duration_ms",
]) {
  assertContains(maintenance, marker, `${paths.maintenance}: cleanup guardrail missing: ${marker}`);
}
assertNotContains(maintenance, "idempotency_key =", `${paths.maintenance}: raw idempotency key must not enter telemetry`);
assertNotContains(maintenance, "request_json =", `${paths.maintenance}: receipt payload must not enter telemetry`);
assertNotContains(maintenance, "response_json =", `${paths.maintenance}: receipt response must not enter telemetry`);

for (const marker of [
  "pub(crate) async fn cleanup_completed(",
  "command_receipt::Column::TenantId.eq(tenant_id)",
  "command_receipt::Column::SchemaVersion.eq(COMMAND_RECEIPT_SCHEMA_VERSION)",
  "command_receipt::Column::Status.eq(STATUS_COMPLETED)",
  "command_receipt::Column::CompletedAt.is_not_null()",
  "command_receipt::Column::CompletedAt.lt(completed_before.clone())",
  ".order_by_asc(command_receipt::Column::CompletedAt)",
  ".order_by_asc(command_receipt::Column::Id)",
  ".limit(limit)",
  "for receipt in &candidates",
  "validate_cleanup_candidate(receipt)?",
  "serde_json::from_value::<SocialGraphCommandReceiptRequest>",
  "serde_json::from_value::<relation::Model>",
  "if dry_run || candidates.is_empty()",
  "command_receipt::Entity::delete_many()",
  "command_receipt::Column::Id.is_in(candidate_ids)",
  "oldest_completed_timestamp(db, tenant_id).await?",
  "receipt.completed_at.map(|value| value.timestamp())",
]) {
  assertContains(receipts, marker, `${paths.receipts}: bounded cleanup core missing: ${marker}`);
}

for (const marker of [
  "cleanup_is_bounded_tenant_scoped_and_completed_only",
  "cleanup_rejects_user_actor_invalid_limits_and_future_cutoffs",
  "cleanup_stops_before_deleting_when_one_candidate_is_corrupt",
  "cleanup_command(CLEANUP_CUTOFF, 1, true)",
  "cleanup_command(CLEANUP_CUTOFF, 1, false)",
  "Some(AGED_COMPLETION)",
  "oldest_retained_completed_at_unix_seconds, None",
  'receipt_count(&db, tenant_id, "processing")',
  'receipt_count(&db, other_tenant_id, "completed")',
  '"social_graph.receipt_cleanup_forbidden"',
  '"social_graph.receipt_cleanup_limit_invalid"',
  '"social_graph.receipt_cleanup_cutoff_future"',
  '"social_graph.command_receipt_corrupt"',
  'assert_eq!(receipt_count(&db, tenant_id, "completed").await, 2)',
]) {
  assertContains(test, marker, `${paths.test}: cleanup scenario missing: ${marker}`);
}

if (failures.length > 0) {
  console.error("Social Graph receipt cleanup verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Social Graph receipt cleanup verification passed");
