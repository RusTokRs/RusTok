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

const contractPath = "crates/rustok-forum/contracts/forum-mention-runtime-proof.json";
const contract = JSON.parse(read(contractPath) || "{}");
const testSource = read(contract.test_file ?? "");
const postgresSupport = read("crates/rustok-forum/tests/support/postgres.rs");
const mentionRelation = read("crates/rustok-forum/src/services/mention_relation.rs");
const quoteService = read("crates/rustok-forum/src/services/quote_command.rs");
const inlineResolver = read("crates/rustok-forum/src/services/relation_quote_input.rs");
const record = read("crates/rustok-forum/docs/forum-12-postgres-runtime-proof.md");

if (contract.schema_version !== 1) {
  failures.push("runtime proof contract must use schema_version=1");
}
if (contract.test_environment?.database !== "PostgreSQL") {
  failures.push("runtime proof must target PostgreSQL");
}
if (contract.test_environment?.url_env !== "RUSTOK_FORUM_TEST_DATABASE_URL") {
  failures.push("runtime proof must use the canonical Forum PostgreSQL test URL");
}
if (contract.test_environment?.notifications_composed !== false) {
  failures.push("notifications-off proof must not compose Notifications");
}
if (contract.execution?.status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted runtime evidence");
}

for (const marker of [
  "d1_replacement_wins_before_stale_d2_preserve_on_postgres",
  "ForumQuoteCommandService::new",
  "wait_until_lock_wait",
  "pg_stat_activity",
  "wait_event_type = 'Lock'",
  "FORUM_RELATION_REVISION_CONFLICT",
  "Stale D2 body must roll back",
  "expected only D1 to append one revision",
  "D1 explicit clear was replaced by the stale preserved quote set",
]) {
  requireText(testSource, marker, `PostgreSQL D1/D2 scenario is missing ${marker}`);
}

for (const marker of [
  "soft_deleted_reply_rejects_d1_and_d2_without_mutating_relation_history",
  "FORUM_REPLY_DELETED",
  "soft deletion must preserve the immutable quoted revision history",
]) {
  requireText(testSource, marker, `PostgreSQL soft-delete scenario is missing ${marker}`);
}

for (const marker of [
  "mention_owner_event_commits_with_notifications_not_composed",
  "@moderators please review",
  "forum.mention.audience_added",
  "forum_audience_mentions",
  "forum_domain_events",
  "SysEvents::find_by_id(event_id)",
]) {
  requireText(testSource, marker, `notifications-off scenario is missing ${marker}`);
}

for (const marker of [
  "RUSTOK_FORUM_TEST_DATABASE_URL",
  "OutboxModule.migrations()",
  "TaxonomyModule.migrations()",
  "ForumModule.migrations()",
  "pub async fn peer",
]) {
  requireText(postgresSupport, marker, `PostgreSQL support is missing ${marker}`);
}
reject(
  postgresSupport,
  /NotificationsModule|rustok_notifications/,
  "Forum PostgreSQL fixture must not compose Notifications",
);
reject(
  testSource,
  /NotificationsModule|rustok_notifications|NotificationService/,
  "runtime proof must exercise the notifications-off profile without a synchronous Notifications dependency",
);

for (const [source, marker, label] of [
  [mentionRelation, "lock_source_in_tx", "mention persistence root lock"],
  [quoteService, "persist_in_tx(&txn, prepared)", "D1 owner persistence"],
  [inlineResolver, "lock_source_and_assert_latest_in_tx", "D2 expected-revision CAS"],
]) {
  requireText(source, marker, `${label} is missing ${marker}`);
}

for (const marker of [
  "status: source_ready",
  "mention_quote_runtime_postgres",
  "Tests, Cargo, verifiers and CI were not run",
  "NOTIFY-03/07",
]) {
  requireText(record, marker, `runtime proof record is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum mention runtime proof verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum mention PostgreSQL runtime proof contract is source-ready.");
