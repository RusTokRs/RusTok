#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contractPath = "crates/rustok-forum/contracts/forum-read-state-runtime-proof.json";
const contract = JSON.parse(read(contractPath) || "{}");
const testSource = read(contract.test_file ?? "");
const postgresSupport = read("crates/rustok-forum/tests/support/postgres.rs");
const readModel = read("crates/rustok-forum/src/services/read_model.rs");
const readTracking = read("crates/rustok-forum/src/services/read_tracking.rs");
const migration = read(
  "crates/rustok-forum/src/migrations/m20260724_000001_add_forum_topic_read_states.rs",
);
const record = read(contract.record ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("read-state runtime proof contract must use schema_version=1");
}
if (contract.test_environment?.database !== "PostgreSQL") {
  failures.push("read-state runtime proof must target PostgreSQL");
}
if (contract.test_environment?.url_env !== "RUSTOK_FORUM_TEST_DATABASE_URL") {
  failures.push("read-state runtime proof must use the canonical Forum PostgreSQL URL");
}
if (contract.execution?.status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted PostgreSQL evidence");
}
if (contract.execution?.maintainer_execution_required !== true) {
  failures.push("maintainer PostgreSQL execution must remain explicit");
}
if (contract.scenarios?.query_plan?.latency_threshold_claimed !== false) {
  failures.push("source-ready query-plan evidence must not claim an unmeasured latency threshold");
}

for (const [field, expected] of [
  ["topics", 128],
  ["approved_replies_total", 8192],
  ["topic_revisions_total", 512],
  ["bounded_projection_topics", 100],
]) {
  if (contract.fixture?.[field] !== expected) {
    failures.push(`runtime proof fixture ${field} must equal ${expected}`);
  }
}

for (const marker of [
  "concurrent_devices_converge_to_component_wise_maximum_on_postgres",
  "context.peer().await?",
  "tokio::join!",
  "last_read_position: REPLIES_PER_TOPIC",
  "last_read_revision: latest_revision",
  "PostgreSQL read-state trigger accepted a direct regression",
]) {
  requireText(testSource, marker, `concurrent-device proof is missing ${marker}`);
}

for (const marker of [
  "bounded_unread_aggregate_matches_large_fixture_and_plan_contract_on_postgres",
  "const TOPIC_COUNT: usize = 128",
  "const REPLIES_PER_TOPIC: i64 = 64",
  "const PROJECTION_TOPIC_COUNT: usize = 100",
  "ForumReadModelService::new",
  "summarize_topic_ids",
  "reply-unread fixture mismatch",
  "revision-unread fixture mismatch",
  "unseen fixture mismatch",
]) {
  requireText(testSource, marker, `aggregate correctness proof is missing ${marker}`);
}

for (const marker of [
  "EXPLAIN ({options})",
  "ANALYZE, BUFFERS, COSTS OFF, FORMAT JSON",
  "SET enable_seqscan = off",
  "RESET enable_seqscan",
  "per-row SubPlan",
  "natural unread aggregate plan is missing relation",
  "forum_topic_read_states_pkey",
  "idx_forum_topic_revisions_tenant_topic_created",
]) {
  requireText(testSource, marker, `query-plan proof is missing ${marker}`);
}
reject(
  testSource,
  /execution_time_ms|<\s*\d+(?:\.\d+)?\s*,?\s*".*ms|latency.*threshold/i,
  "source-ready Forum read-state proof must not publish a latency threshold",
);
reject(testSource, /#\s*\[ignore/, "Forum PostgreSQL proof must use the canonical skip-on-missing-URL fixture");

for (const marker of [
  "RUSTOK_FORUM_TEST_DATABASE_URL",
  "OutboxModule.migrations()",
  "TaxonomyModule.migrations()",
  "ForumModule.migrations()",
  "pub async fn peer",
  "CREATE SCHEMA",
]) {
  requireText(postgresSupport, marker, `PostgreSQL support is missing ${marker}`);
}

for (const marker of [
  "LEFT JOIN forum_topic_read_states state",
  "LEFT JOIN forum_replies unread_reply",
  "unread_reply.status = 'approved'",
  "unread_reply.position > COALESCE(state.last_read_position, 0)",
  "unread_reply.updated_at > state.updated_at",
  "LEFT JOIN forum_topic_revisions unread_revision",
  "unread_revision.id > COALESCE(state.last_read_revision, 0)",
  "COUNT(DISTINCT unread_reply.id)",
  "COUNT(DISTINCT unread_revision.id)",
]) {
  requireText(readModel, marker, `owner unread aggregate is missing ${marker}`);
  requireText(testSource, marker, `proof SQL mirror is missing ${marker}`);
}

for (const marker of [
  ".do_nothing()",
  "LastReadPosition.lt(high_water.last_read_position)",
  "LastReadRevision.lt(high_water.last_read_revision)",
  "upsert_topic_read_high_water_in_tx",
]) {
  requireText(readTracking, marker, `read-state owner source is missing ${marker}`);
}
for (const marker of [
  "forum_topic_read_states_pkey",
  "forum_prevent_topic_read_state_regression",
  "idx_forum_topic_read_states_tenant_user_updated",
]) {
  requireText(migration, marker, `read-state migration is missing ${marker}`);
}

for (const marker of [
  "status: source_ready",
  "topic_read_state_postgres",
  "8,192 approved replies",
  "EXPLAIN (ANALYZE, BUFFERS, COSTS OFF, FORMAT JSON)",
  "Tests, Cargo, verifiers and CI were not run",
  "FORUM-20",
]) {
  requireText(record, marker, `runtime proof record is missing ${marker}`);
}
for (const marker of [
  "FORUM-16F",
  "topic_read_state_postgres",
  "source-ready PostgreSQL",
]) {
  requireText(plan, marker, `canonical FORUM-16 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum read-state runtime proof verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum read-state PostgreSQL runtime proof contract is source-ready.");
