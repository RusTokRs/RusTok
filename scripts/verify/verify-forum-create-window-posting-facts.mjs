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

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-create-window-posting-facts.json";
const contract = JSON.parse(read(contractPath) || "{}");
const provider = read(contract.provider_file);
const serviceRegistry = read(contract.service_registry);
const crateRoot = read(contract.crate_root);
const host = read(contract.host_composition);
const migration = read(contract.migration_file);
const migrationRegistry = read(contract.migration_registry);
const sqliteProof = read(contract.sqlite_runtime_proof);
const postgresProof = read(contract.postgres_runtime_proof);
const note = read(contract.owner_note);
const upstream = read(contract.upstream_contract);
const plan = read(contract.canonical_plan);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-26J" ||
  contract.upstream_task !== "FORUM-26I"
) {
  failures.push("FORUM-26J create-window contract identity is invalid");
}

for (const marker of [
  "pub struct ForumTopicCreatesWindowFactPort",
  "pub struct ForumReplyCreatesWindowFactPort",
  "ForumPostingPolicyFactKind::TopicCreatesWindow",
  "ForumPostingPolicyFactKind::ReplyCreatesWindow",
  '"forum_topics"',
  '"forum_replies"',
  "created_at >= $3",
  "created_at <= $4",
  "created_at >= ?3",
  "created_at <= ?4",
  "ForumPostingWindowCount",
  "u32::try_from(value)",
]) {
  requireText(provider, marker, `create-window provider is missing ${marker}`);
}
rejectText(
  provider,
  "AND deleted_at IS NULL",
  "create-window owner queries must not let soft deletion reset the create budget",
);
rejectText(
  provider,
  "AND status = 'approved'",
  "reply-create windows must not be narrowed to current approved replies",
);

for (const marker of [
  "mod posting_policy_create_window_facts;",
  "ForumReplyCreatesWindowFactPort",
  "ForumTopicCreatesWindowFactPort",
]) {
  requireText(serviceRegistry, marker, `service registry is missing ${marker}`);
  requireText(crateRoot, marker.replace("mod posting_policy_create_window_facts;", "ForumTopicCreatesWindowFactPort"),
    `crate root is missing create-window public export ${marker}`);
}

for (const marker of [
  "ForumTopicCreatesWindowFactPort::shared(db.clone())",
  "ForumReplyCreatesWindowFactPort::shared(db.clone())",
  "ForumApprovedPostsFactPort::shared(db.clone())",
  "ForumTopicReadPostingFactPort::shared(db)",
]) {
  requireText(host, marker, `host posting-fact composition is missing ${marker}`);
}

for (const fact of Object.values(contract.facts ?? {})) {
  if (!fact?.fact_kind || !fact?.action || !fact?.table || !fact?.index) {
    failures.push("FORUM-26J contract contains an incomplete fact definition");
    continue;
  }
  requireText(provider, fact.fact_kind, `provider is missing fact ${fact.fact_kind}`);
  requireText(provider, fact.table, `provider is missing table ${fact.table}`);
  requireText(migration, fact.index, `migration is missing index ${fact.index}`);
  requireText(sqliteProof, fact.index, `SQLite proof is missing index ${fact.index}`);
  requireText(postgresProof, fact.index, `PostgreSQL proof is missing index ${fact.index}`);
}

for (const marker of [
  "mod m20260728_000006_add_forum_create_window_indexes;",
  "Box::new(m20260728_000006_add_forum_create_window_indexes::Migration)",
]) {
  requireText(migrationRegistry, marker, `migration registry is missing ${marker}`);
}

for (const marker of [
  "tenant_id, author_id, created_at DESC",
  "author_id IS NOT NULL",
  "DROP INDEX IF EXISTS idx_forum_topics_tenant_author_created_at",
  "DROP INDEX IF EXISTS idx_forum_replies_tenant_author_created_at",
]) {
  requireText(migration, marker, `create-window migration is missing ${marker}`);
}

for (const marker of [
  "EXPLAIN QUERY PLAN",
  "sqlite_master",
  "create_window_queries_use_author_time_indexes_on_sqlite",
]) {
  requireText(sqliteProof, marker, `SQLite create-window proof is missing ${marker}`);
}
for (const marker of [
  "EXPLAIN (COSTS OFF, FORMAT JSON)",
  "SET enable_seqscan = off",
  "RESET enable_seqscan",
  "pg_indexes",
  "create_window_queries_use_author_time_indexes_on_postgres",
]) {
  requireText(postgresProof, marker, `PostgreSQL create-window proof is missing ${marker}`);
}

for (const marker of [
  "persisted owner create activity",
  "soft deletion",
  "moderation state",
  "not a concurrency-safe distributed reservation",
  "edit-window fact",
  "bump-age fact",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `FORUM-26J owner note is missing ${marker}`);
}

for (const marker of [
  '"downstream_create_window_task": "FORUM-26J"',
  '"downstream_create_window_contract": "crates/rustok-forum/contracts/forum-create-window-posting-facts.json"',
]) {
  requireText(upstream, marker, `FORUM-26I contract is missing downstream marker ${marker}`);
}

for (const marker of [
  "FORUM-26A` through `FORUM-26J",
  "topic/reply create-window facts",
  "create_window_facts_index_sqlite",
  "create_window_facts_index_postgres",
]) {
  requireText(plan, marker, `canonical Forum plan is missing FORUM-26J marker ${marker}`);
}

for (const [key, expected] of [
  ["forum_owner_rows_are_authority", true],
  ["exact_tenant_user_scope", true],
  ["exact_requested_window", true],
  ["soft_deleted_rows_still_counted", true],
  ["reply_moderation_status_does_not_reset_budget", true],
  ["postgresql_sqlite_parity", true],
  ["author_time_indexes_added", true],
  ["topic_create_window_provider_added", true],
  ["reply_create_window_provider_added", true],
  ["posting_owner_enforcement_added", false],
  ["distributed_reservation_added", false],
  ["concurrency_safe_rate_limit_claimed", false],
  ["edit_window_provider_added", false],
  ["bump_age_provider_added", false],
  ["transport_changed", false],
]) {
  if (contract.composition?.[key] !== expected) {
    failures.push(`FORUM-26J composition.${key} must be ${expected}`);
  }
}

if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("FORUM-26J must not claim maintainer runtime execution");
}

if (failures.length > 0) {
  console.error("Forum create-window posting-fact verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum create-window posting facts are source-ready.");
