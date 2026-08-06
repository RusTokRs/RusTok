#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const cargoPath = "crates/rustok-search/Cargo.toml";
const testPath = "crates/rustok-search/tests/forum_canonical_route_reindex_postgres.rs";
const forumProjectionPath = "crates/rustok-forum/src/search_projection.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const projectorPath = "crates/rustok-search/src/forum_projector.rs";
const enginePath = "crates/rustok-search/src/engine.rs";
const contractPath =
  "crates/rustok-forum/contracts/forum-search-canonical-route-reindex-harness.json";
const notePath =
  "crates/rustok-forum/docs/forum-24r-search-canonical-route-reindex-harness.md";
const upstreamPath =
  "crates/rustok-forum/contracts/forum-search-canonical-route-cutover.json";

const cargo = read(cargoPath);
const test = read(testPath);
const forumProjection = read(forumProjectionPath);
const ingestion = read(ingestionPath);
const projector = read(projectorPath);
const engine = read(enginePath);
const note = read(notePath);
let contract = null;
let upstream = null;
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}
try {
  upstream = JSON.parse(read(upstreamPath));
} catch (error) {
  failures.push(`${upstreamPath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "flex.workspace = true",
  "rustok-forum.workspace = true",
  "rustok-outbox.workspace = true",
  "rustok-taxonomy.workspace = true",
]) {
  requireMarker(cargo, marker, cargoPath);
}

for (const marker of [
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  'SET search_path TO "{schema}", public',
  "max_connections(5)",
  "OutboxModule.migrations()",
  "TaxonomyModule.migrations()",
  "create_field_definition_cache_generation_table",
  "ForumModule.migrations()",
  "SearchModule.migrations()",
  "CategoryService::new",
  "TopicService::new",
  "ReplyService::new",
  "ForumSearchProjectionSourceFactory.build",
  "SearchIngestionHandler::with_forum_source",
  "ContractEventEnvelope::new",
  'target_type: "forum".into()',
  "forum_reindex_atomically_replaces_legacy_routes_with_owner_canonical_routes",
  "insert_legacy(",
  "stale_orphan_id",
  "other_id",
  'format!("/modules/forum?category={category_id}")',
  'format!("/modules/forum?topic={topic_id}")',
  '"/en/forum/c/platform"',
  'format!("/en/forum/t/{topic_short_id}/canonical-search")',
  "canonical_search_result_url(&result)",
  "count_legacy_routes",
  "assert_inbox_completed",
  'assert_eq!(status, "completed")',
]) {
  requireMarker(test, marker, testPath);
}
for (const forbidden of [
  "ForumSearchProjector::new",
  "ForumCategoryRouteService::new",
  "ForumTopicRouteService::new",
  "CREATE TEMP TABLE forum_search_projection_stage",
  "DELETE FROM search_documents WHERE tenant_id",
]) {
  rejectMarker(test, forbidden, testPath);
}

for (const marker of [
  "ForumCategoryRouteService",
  "ForumTopicRouteService",
  '"route": route',
  'format!("{topic_route}?reply={reply_id}")',
]) {
  requireMarker(forumProjection, marker, forumProjectionPath);
}
for (const marker of [
  "ForumProjectionInbox",
  "ForumProjectionScope::for_event",
  "inbox.enqueue",
  "reconcile_forum_inbox",
  '("forum", _) | ("forum_topic", Some(_))',
]) {
  requireMarker(ingestion, marker, ingestionPath);
}
for (const marker of [
  "CREATE TEMP TABLE forum_search_projection_stage",
  "ON COMMIT DROP",
  "delete_forum_scope(&tx, tenant_id)",
  "FROM forum_search_projection_stage",
  "tx.commit()",
]) {
  requireMarker(projector, marker, projectorPath);
}
for (const marker of [
  "canonical_forum_projected_result_url",
  'value.payload.get("route")',
  "canonical_forum_category_route",
  "canonical_forum_topic_route",
]) {
  requireMarker(engine, marker, enginePath);
}

if (contract) {
  if (contract.schema_version !== 1) {
    failures.push(`${contractPath}: schema_version must be 1`);
  }
  if (
    contract.module !== "forum" ||
    contract.surface !== "search_canonical_route_reindex" ||
    contract.task !== "FORUM-24R" ||
    contract.upstream_task !== "FORUM-24Q"
  ) {
    failures.push(`${contractPath}: identity drift`);
  }
  if (
    contract.status !== "executable_no_run" ||
    contract.compile_policy !== "not_run_by_request"
  ) {
    failures.push(`${contractPath}: execution status drift`);
  }
  if (contract.test_target !== testPath) {
    failures.push(`${contractPath}: test target drift`);
  }
  const production = contract.production_contract ?? {};
  for (const [key, expected] of Object.entries({
    forum_projection: forumProjectionPath,
    search_ingestion: ingestionPath,
    search_projector: projectorPath,
    search_url_projection: enginePath,
    source_guardrail:
      "scripts/verify/verify-forum-search-canonical-route-reindex-harness.mjs",
  })) {
    if (production[key] !== expected) {
      failures.push(`${contractPath}: ${key} path drift`);
    }
  }
  const cases = new Set((contract.cases ?? []).map((entry) => entry.name));
  for (const required of [
    "real_owner_migrations",
    "real_forum_writes",
    "durable_forum_reindex",
    "atomic_scope_replacement",
    "localized_owner_routes",
    "search_projection_acceptance",
    "legacy_route_removal",
    "stale_orphan_removal",
    "tenant_isolation",
  ]) {
    if (!cases.has(required)) failures.push(`${contractPath}: missing case ${required}`);
  }
  for (const key of [
    "runtime_code_changed",
    "search_storage_schema_changed",
    "forum_storage_schema_changed",
    "event_schema_changed",
    "route_owner_changed",
    "visibility_policy_changed",
    "graphql_or_native_dto_changed",
    "migration_added",
    "compatibility_fallback_added",
  ]) {
    if (contract.preserved_boundaries?.[key] !== false) {
      failures.push(`${contractPath}: preserved boundary ${key} must be false`);
    }
  }
}

if (upstream) {
  if (
    upstream.reindex?.evidence_contract !== contractPath ||
    upstream.reindex?.harness_task !== "FORUM-24R"
  ) {
    failures.push(`${upstreamPath}: FORUM-24R handoff drift`);
  }
}

for (const marker of [
  "FORUM-24R",
  "executable PostgreSQL",
  "RUSTOK_SEARCH_TEST_DATABASE_URL",
  "durable Forum inbox",
  "staged replacement",
  "No tests",
]) {
  requireMarker(note, marker, notePath);
}

if (failures.length > 0) {
  console.error("Forum canonical-route Search reindex harness verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum canonical-route Search reindex harness verified");
