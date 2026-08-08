#!/usr/bin/env node

import fs from "node:fs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) throw new Error(message);
}

const servicePath = "crates/rustok-forum/src/services/counter_reconciliation.rs";
const graphqlPath = "crates/rustok-forum/src/graphql/reconciliation_query.rs";
const graphqlModPath = "crates/rustok-forum/src/graphql/mod.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const packetPath = "docs/modules/forum-33-counter-reconciliation-actualization-2026-08-08.md";

const service = read(servicePath);
const graphql = read(graphqlPath);
const graphqlMod = read(graphqlModPath);
const lib = read(libPath);
const packet = read(packetPath);

for (const marker of [
  "pub const DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT: u64 = 100",
  "pub const MAX_FORUM_COUNTER_RECONCILIATION_LIMIT: u64 = 500",
  "pub struct ForumCounterReconciliationService",
  "pub struct ForumCounterReconciliationReport",
  "ForumCounterDriftKind::TopicReplyCount",
  "ForumCounterDriftKind::CategoryTopicCount",
  "ForumCounterDriftKind::CategoryReplyCount",
  "r.status = 'approved'",
  "WHERE t.tenant_id = ?1",
  "WHERE t.tenant_id = $1",
  "WHERE c.tenant_id = ?1",
  "WHERE c.tenant_id = $1",
  "COUNT(DISTINCT t.id)",
  "effective_limit.saturating_add(1)",
  "has_more_topics",
  "has_more_categories",
  "record_module_entrypoint_call(",
  '"counter_reconciliation_report"',
  "record_span_duration(",
]) {
  requireText(service, marker, `Forum counter reconciliation service missing ${marker}`);
}

for (const forbidden of [
  "UPDATE forum_topics",
  "UPDATE forum_categories",
  "DELETE FROM forum_",
  "INSERT INTO forum_",
  "ActiveModel",
  "TransactionTrait",
]) {
  requireAbsent(service, forbidden, `read-only reconciliation service must not contain ${forbidden}`);
}

for (const marker of [
  "pub struct ForumReconciliationQuery",
  "forum_counter_reconciliation_report",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "Permission::FORUM_CATEGORIES_MANAGE",
  "Permission::FORUM_TOPICS_MANAGE",
  "categories_manage && topics_manage",
  "auth.tenant_id != tenant.id",
  "ForumCounterReconciliationService::new(db.clone())",
]) {
  requireText(graphql, marker, `Forum reconciliation GraphQL boundary missing ${marker}`);
}
for (const forbidden of ["tenant_id: Option<Uuid>", "Mutation", "UPDATE forum_"]) {
  requireAbsent(graphql, forbidden, `operator report must not expose ${forbidden}`);
}

for (const marker of [
  "mod reconciliation_query;",
  "reconciliation_query::ForumReconciliationQuery",
]) {
  requireText(graphqlMod, marker, `Forum GraphQL composition missing ${marker}`);
}
for (const marker of [
  '#[path = "services/counter_reconciliation.rs"]',
  "ForumCounterReconciliationService",
  "MAX_FORUM_COUNTER_RECONCILIATION_LIMIT",
]) {
  requireText(lib, marker, `Forum owner export missing ${marker}`);
}

for (const marker of [
  "Status: `in-progress / bounded-owner-report-source-ready / repair-and-runtime-evidence-open`",
  "forumCounterReconciliationReport(limit: Int)",
  "forum_categories:manage",
  "forum_topics:manage",
  "exactly two tenant-scoped aggregate queries",
  "does **not** add a repair mutation",
  "idempotent job/receipt state",
]) {
  requireText(packet, marker, `FORUM-33 actualization missing ${marker}`);
}

console.log("Forum FORUM-33 counter reconciliation source: ok");
