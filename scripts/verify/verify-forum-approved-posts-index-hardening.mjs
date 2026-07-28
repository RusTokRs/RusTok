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
  const absolute = path.join(repoRoot, relativePath ?? "");
  if (!relativePath || !existsSync(absolute)) {
    failures.push(`${relativePath || "<missing path>"}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-approved-posts-index-hardening.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration_file);
const migrationRegistry = read(contract.migration_registry);
const provider = read(contract.provider_file);
const sqliteProof = read(contract.sqlite_runtime_proof);
const postgresProof = read(contract.postgres_runtime_proof);
const postgresBootstrap = read(contract.postgres_test_bootstrap);
const note = read(contract.owner_note);
const upstream = read(contract.upstream_contract);
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26I" ||
  contract.upstream_task !== "FORUM-26H"
) {
  failures.push("approved-post index hardening contract identity is invalid");
}

for (const index of [contract.indexes?.topics, contract.indexes?.replies]) {
  if (!index?.name || !Array.isArray(index.columns) || !Array.isArray(index.predicate)) {
    failures.push("approved-post index contract is missing a bounded index definition");
    continue;
  }
  requireText(migration, index.name, `migration is missing index ${index.name}`);
  for (const marker of [...index.columns, ...index.predicate]) {
    requireText(
      migration,
      marker,
      `migration index ${index.name} is missing contract marker ${marker}`,
    );
  }
  requireText(sqliteProof, index.name, `SQLite proof is missing index ${index.name}`);
  requireText(postgresProof, index.name, `PostgreSQL proof is missing index ${index.name}`);
}

for (const marker of [
  "mod m20260728_000005_add_forum_approved_posts_indexes;",
  "Box::new(m20260728_000005_add_forum_approved_posts_indexes::Migration)",
]) {
  requireText(migrationRegistry, marker, `migration registry is missing ${marker}`);
}

for (const marker of [
  "topic.tenant_id = $1",
  "topic.author_id = $2",
  "topic.deleted_at IS NULL",
  "reply.tenant_id = $1",
  "reply.author_id = $2",
  "reply.status = 'approved'",
  "reply.deleted_at IS NULL",
  "topic.id = reply.topic_id",
]) {
  requireText(provider, marker, `ApprovedPosts provider drifted from index proof marker ${marker}`);
}

for (const marker of [
  "EXPLAIN QUERY PLAN",
  "sqlite_master",
  "approved_posts_aggregate_uses_partial_author_indexes_on_sqlite",
]) {
  requireText(sqliteProof, marker, `SQLite proof is missing ${marker}`);
}
for (const marker of [
  "EXPLAIN (COSTS OFF, FORMAT JSON)",
  "SET enable_seqscan = off",
  "RESET enable_seqscan",
  "pg_indexes",
  "approved_posts_aggregate_uses_partial_author_indexes_on_postgres",
]) {
  requireText(postgresProof, marker, `PostgreSQL proof is missing ${marker}`);
}

for (const marker of [
  "CREATE TABLE users",
  "id UUID NOT NULL PRIMARY KEY",
  "tenant_id UUID NOT NULL",
  "Forum trust-state migrations reference the platform-owned users table",
]) {
  requireText(postgresBootstrap, marker, `PostgreSQL Forum bootstrap repair is missing ${marker}`);
}

for (const marker of [
  "FORUM-26I",
  "partial author indexes",
  "owner query and its semantics remain unchanged",
  "posting-policy evaluation or precedence change",
  "posting owner enforcement",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26I owner note is missing ${marker}`);
}

for (const marker of [
  '"downstream_index_hardening_task": "FORUM-26I"',
  '"downstream_index_hardening_contract": "crates/rustok-forum/contracts/forum-approved-posts-index-hardening.json"',
]) {
  requireText(upstream, marker, `FORUM-26H contract is missing downstream marker ${marker}`);
}

for (const marker of [
  "| `FORUM-26` | `in_progress` |",
  "### Delivered in `FORUM-26A` through `FORUM-26I`",
  "approved-post author indexes",
  "approved_posts_index_sqlite",
  "approved_posts_index_postgres",
]) {
  requireText(plan, marker, `canonical Forum plan is missing FORUM-26I marker ${marker}`);
}

for (const [key, expected] of [
  ["postgresql_sqlite_parity", true],
  ["exact_provider_query_preserved", true],
  ["partial_indexes_only", true],
  ["sqlite_explain_source_proof_added", true],
  ["postgres_explain_source_proof_added", true],
  ["backfill_required", false],
  ["provider_output_changed", false],
  ["posting_owner_enforcement_added", false],
  ["rate_limit_execution_added", false],
  ["transport_changed", false],
]) {
  if (contract.composition?.[key] !== expected) {
    failures.push(`FORUM-26I composition.${key} must be ${expected}`);
  }
}

if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26I must not claim maintainer runtime execution");
}

if (failures.length > 0) {
  console.error("Forum approved-post index hardening verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum approved-post index hardening contract is source-ready.");
