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
const servicePath = "crates/rustok-forum/src/services/mention_relation.rs";
const testPath = "crates/rustok-forum/src/services/mention_relation_tests.rs";
const migrationRegistryPath = "crates/rustok-forum/src/migrations/mod.rs";
const serviceRegistryPath = "crates/rustok-forum/src/services/mod.rs";
const entityRegistryPath = "crates/rustok-forum/src/entities/mod.rs";
const errorPath = "crates/rustok-forum/src/error.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const crateApiPath = "crates/rustok-forum/CRATE_API.md";

const migration = read(migrationPath);
const service = read(servicePath);
const tests = read(testPath);
const migrationRegistry = read(migrationRegistryPath);
const serviceRegistry = read(serviceRegistryPath);
const entityRegistry = read(entityRegistryPath);
const error = read(errorPath);
const plan = read(planPath);
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
]) {
  requireText(service, marker, `${servicePath}: missing owner marker ${marker}`);
}
requireOrder(
  service,
  "validate_quote_targets_in_tx(txn, prepared.tenant_id, &prepared.quotes).await?;",
  "let revision = forum_relation_revision::ActiveModel",
  `${servicePath}: quote targets must be validated before the first relation write`,
);

reject(
  service,
  /TransactionalEventBus|DomainEvent|rustok_notifications|notification_source|publish_in_tx/,
  `${servicePath}: FORUM-12B1 must not publish events or call Notifications`,
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
  "persisted mention rows must be immutable",
]) {
  requireText(tests, marker, `${testPath}: missing persistence coverage ${marker}`);
}

requireText(
  migrationRegistry,
  "m20260722_000004_add_forum_mention_quote_relations",
  `${migrationRegistryPath}: migration is not registered`,
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
requireText(
  error,
  '"FORUM_QUOTE_TARGET_UNAVAILABLE"',
  `${errorPath}: safe quote target code is missing`,
);
requireText(plan, "Delivered in `FORUM-12B1`", `${planPath}: FORUM-12B1 is not recorded`);
requireText(
  plan,
  "FORUM-12B2",
  `${planPath}: active write-path integration must remain explicit`,
);
for (const marker of [
  "forum_relation_revisions",
  "MentionRelationService",
  "FORUM_QUOTE_TARGET_UNAVAILABLE",
]) {
  requireText(crateApi, marker, `${crateApiPath}: missing contract marker ${marker}`);
}

if (failures.length > 0) {
  console.error("forum mention persistence verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention persistence verification passed");
