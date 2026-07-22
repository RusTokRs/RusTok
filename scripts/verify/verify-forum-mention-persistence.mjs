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

function requireOrder(source, first, second, message) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) failures.push(message);
}

function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const migrationPath =
  "crates/rustok-forum/src/migrations/m20260722_000004_add_forum_mention_quote_relations.rs";
const seedMigrationPath =
  "crates/rustok-forum/src/migrations/m20260722_000005_seed_forum_relation_revisions.rs";
const servicePath = "crates/rustok-forum/src/services/mention_relation.rs";
const testPath = "crates/rustok-forum/src/services/mention_relation_tests.rs";
const migrationRegistryPath = "crates/rustok-forum/src/migrations/mod.rs";
const serviceRegistryPath = "crates/rustok-forum/src/services/mod.rs";
const entityRegistryPath = "crates/rustok-forum/src/entities/mod.rs";
const errorPath = "crates/rustok-forum/src/error.rs";
const b2RecordPath = "crates/rustok-forum/docs/forum-12b2-owner-write-integration.md";
const cRecordPath = "crates/rustok-forum/docs/forum-12c-mention-events.md";
const crateApiPath = "crates/rustok-forum/CRATE_API.md";

const migration = read(migrationPath);
const seedMigration = read(seedMigrationPath);
const service = read(servicePath);
const tests = read(testPath);
const migrationRegistry = read(migrationRegistryPath);
const serviceRegistry = read(serviceRegistryPath);
const entityRegistry = read(entityRegistryPath);
const error = read(errorPath);
const b2Record = read(b2RecordPath);
const cRecord = read(cRecordPath);
const crateApi = read(crateApiPath);

for (const marker of [
  "forum_relation_revisions",
  "forum_user_mentions",
  "forum_audience_mentions",
  "forum_quotes",
  "projection_fingerprint VARCHAR(64)",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
  "forum_relation_revision_source_guard",
  "forum_relation_revision_immutable_guard",
  "forum_user_mentions_immutable_guard",
  "forum_audience_mentions_immutable_guard",
  "forum_quotes_immutable_guard",
  "forum_quotes_target_guard",
  "forum relation projections are immutable",
  "'legacy'",
]) {
  requireText(migration, marker, `${migrationPath}: missing schema marker ${marker}`);
}

for (const marker of [
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
  "forum_topic_translation_relation_revision_seed",
  "forum_reply_body_relation_revision_seed",
  "AFTER INSERT ON forum_topic_translations",
  "AFTER INSERT ON forum_reply_bodies",
  "forum_relation_revisions",
  "'legacy'",
]) {
  requireText(seedMigration, marker, `${seedMigrationPath}: missing rollout seed marker ${marker}`);
}
reject(
  seedMigration,
  /ProfilesReader|ProfileService|rustok_profiles|forum_user_mentions|forum_quotes/,
  `${seedMigrationPath}: rollout seeding must not infer mentions or read Profiles`,
);

for (const marker of [
  "ProfilesReader",
  "DatabaseTransaction",
  "pub(crate) async fn prepare",
  "pub(crate) async fn persist_in_tx",
  "lock_source_in_tx",
  "ensure_prepared_matches_source_in_tx",
  "latest_revision_in_tx",
  "load_snapshot_in_tx",
  "validate_quote_targets_in_tx",
  "Sha256",
  "projection_fingerprint",
  "added_user_ids",
  "replayed: true",
  "ForumError::quote_target_unavailable()",
  "ForumMentionEvent",
  "TransactionalEventBus",
  "publish_contract_in_tx_with_envelope_id",
  "forum_domain_event::ActiveModel",
]) {
  requireText(service, marker, `${servicePath}: missing owner marker ${marker}`);
}
requireOrder(
  service,
  "validate_quote_targets_in_tx(txn, prepared.tenant_id, &prepared.quotes).await?;",
  "let revision = forum_relation_revision::ActiveModel",
  `${servicePath}: quote targets must be validated before the first relation write`,
);
requireOrder(
  service,
  "let result = MentionRelationSyncResult",
  "publish_added_target_events_in_tx",
  `${servicePath}: semantic events must use the exact persisted added-target diff`,
);

reject(
  service,
  /rustok_notifications|notification_source|NotificationService/,
  `${servicePath}: Forum must publish owner events instead of calling Notifications`,
);
reject(
  service,
  /rustok_profiles::entities|profile::Entity/,
  `${servicePath}: mention persistence must resolve identity through ProfilesReader`,
);
reject(
  service,
  /pub\s+struct\s+MentionRelationService/,
  `${servicePath}: transaction-scoped persistence service must remain crate-private`,
);

for (const marker of [
  "relation_revision_replay_diff_quotes_and_guards_are_atomic",
  "identical replay should persist idempotently",
  "cross-tenant quote revision must fail closed",
  "quote validation must run before the first relation write",
  "new source rows must receive one legacy relation identity before FORUM-12B2",
  "persisted mention rows must be immutable",
]) {
  requireText(tests, marker, `${testPath}: missing persistence coverage ${marker}`);
}

for (const marker of [
  "m20260722_000004_add_forum_mention_quote_relations",
  "m20260722_000005_seed_forum_relation_revisions",
  "m20260722_000006_add_forum_mention_events",
]) {
  requireText(migrationRegistry, marker, `${migrationRegistryPath}: migration is not registered: ${marker}`);
}
requireOrder(
  migrationRegistry,
  "Box::new(m20260722_000004_add_forum_mention_quote_relations::Migration)",
  "Box::new(m20260722_000005_seed_forum_relation_revisions::Migration)",
  `${migrationRegistryPath}: rollout seed migration must follow relation schema creation`,
);
requireOrder(
  migrationRegistry,
  "Box::new(m20260722_000005_seed_forum_relation_revisions::Migration)",
  "Box::new(m20260722_000006_add_forum_mention_events::Migration)",
  `${migrationRegistryPath}: mention event journal migration must follow active relation rollout`,
);
requireText(
  serviceRegistry,
  "mod mention_relation;",
  `${serviceRegistryPath}: owner persistence service is not registered`,
);
for (const marker of [
  "forum_relation_revision",
  "forum_user_mention",
  "forum_audience_mention",
  "forum_quote",
]) {
  requireText(entityRegistry, marker, `${entityRegistryPath}: missing entity ${marker}`);
}
for (const marker of [
  '"FORUM_QUOTE_TARGET_UNAVAILABLE"',
  '"FORUM_RELATION_REVISION_UNAVAILABLE"',
]) {
  requireText(error, marker, `${errorPath}: safe relation failure code is missing: ${marker}`);
}
for (const marker of ["FORUM-12B2", "persist_in_tx", "same transaction"]) {
  requireText(b2Record, marker, `${b2RecordPath}: active owner integration record is missing ${marker}`);
}
for (const marker of [
  "FORUM-12C",
  "forum.mention.user_added",
  "ForumRelationReadService",
  "same event UUID",
]) {
  requireText(cRecord, marker, `${cRecordPath}: event/read rollout record is missing ${marker}`);
}
for (const marker of [
  "forum_relation_revisions",
  "MentionRelationService",
  "FORUM_QUOTE_TARGET_UNAVAILABLE",
  "source INSERT seed triggers",
  "forum.mention.user_added",
  "ForumRelationReadService",
]) {
  requireText(crateApi, marker, `${crateApiPath}: missing contract marker ${marker}`);
}

if (failures.length > 0) {
  console.error("forum mention persistence verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention persistence verification passed");
