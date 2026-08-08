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
const servicesModPath = "crates/rustok-forum/src/services/mod.rs";
const graphqlPath = "crates/rustok-forum/src/graphql/reconciliation_query.rs";
const graphqlModPath = "crates/rustok-forum/src/graphql/mod.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const packetPath = "docs/modules/forum-33-counter-reconciliation-actualization-2026-08-08.md";

const service = read(servicePath);
const servicesMod = read(servicesModPath);
const graphql = read(graphqlPath);
const graphqlMod = read(graphqlModPath);
const lib = read(libPath);
const plan = read(planPath);
const packet = read(packetPath);

for (const marker of [
  "pub const DEFAULT_FORUM_COUNTER_RECONCILIATION_LIMIT: u64 = 100",
  "pub const MAX_FORUM_COUNTER_RECONCILIATION_LIMIT: u64 = 500",
  "pub struct ForumCounterReconciliationService",
  "pub struct ForumCounterReconciliationReport",
  "security: &SecurityContext",
  "enforce_operations_scope(security)",
  "enforce_scope(security, Resource::ForumCategories, Action::Manage)?",
  "enforce_scope(security, Resource::ForumTopics, Action::Manage)",
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
  "begin_with_config(",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "DatabaseBackend::Sqlite => self.db.begin().await?",
  "report_in_transaction(&transaction",
  "transaction.commit().await?",
  "transaction.rollback().await",
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
]) {
  requireAbsent(service, forbidden, `read-only reconciliation service must not contain ${forbidden}`);
}

for (const marker of [
  "mod counter_reconciliation;",
  "pub use counter_reconciliation::{",
  "ForumCounterReconciliationService",
  "MAX_FORUM_COUNTER_RECONCILIATION_LIMIT",
]) {
  requireText(servicesMod, marker, `Forum services composition missing ${marker}`);
}

for (const marker of [
  "pub struct ForumReconciliationQuery",
  "forum_counter_reconciliation_report",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "Permission::FORUM_CATEGORIES_MANAGE",
  "Permission::FORUM_TOPICS_MANAGE",
  "categories_manage && topics_manage",
  "auth.tenant_id != tenant.id",
  "SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)",
  "ForumCounterReconciliationService::new(db.clone())",
  ".report(tenant.id, &security, requested_limit)",
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
  "pub mod services;",
  "ForumCounterReconciliationService",
  "MAX_FORUM_COUNTER_RECONCILIATION_LIMIT",
]) {
  requireText(lib, marker, `Forum owner export missing ${marker}`);
}
requireAbsent(
  lib,
  '#[path = "services/counter_reconciliation.rs"]',
  "Forum reconciliation must use the canonical services module boundary",
);

for (const marker of [
  "| `FORUM-33` | `in_progress` | Bounded snapshot-consistent owner counter reconciliation report",
  "## `FORUM-33` — analytics, observability and reconciliation",
  "**Status:** `in_progress`",
  "forumCounterReconciliationReport(limit: Int)",
  "REPEATABLE READ READ ONLY",
  "node scripts/verify/verify-forum-counter-reconciliation-source.mjs",
  "For FORUM-33, retain SQLite and PostgreSQL execution evidence",
]) {
  requireText(plan, marker, `canonical Forum plan missing ${marker}`);
}

for (const marker of [
  "Status: `in-progress / bounded-owner-report-source-ready / repair-and-runtime-evidence-open`",
  "forumCounterReconciliationReport(limit: Int)",
  "forum_categories:manage",
  "forum_topics:manage",
  "Authorization is deliberately enforced twice",
  "services::rbac::enforce_scope",
  "exactly two tenant-scoped aggregate queries inside one database snapshot",
  "REPEATABLE READ READ ONLY",
  "does **not** add a repair mutation",
  "idempotent job/receipt state",
]) {
  requireText(packet, marker, `FORUM-33 actualization missing ${marker}`);
}

console.log("Forum FORUM-33 counter reconciliation source: ok");
