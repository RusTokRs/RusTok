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

const servicePath = "crates/rustok-forum/src/services/mention_reconciliation.rs";
const servicesModPath = "crates/rustok-forum/src/services/mod.rs";
const graphqlPath = "crates/rustok-forum/src/graphql/mention_reconciliation_query.rs";
const graphqlModPath = "crates/rustok-forum/src/graphql/mod.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const packetPath = "docs/modules/forum-33-mention-reconciliation-actualization-2026-08-08.md";

const service = read(servicePath);
const servicesMod = read(servicesModPath);
const graphql = read(graphqlPath);
const graphqlMod = read(graphqlModPath);
const plan = read(planPath);
const packet = read(packetPath);

for (const marker of [
  "pub struct ForumMentionReconciliationService",
  "pub struct ForumMentionReconciliationReport",
  "pub enum ForumMentionDriftKind",
  "SourceUnavailable",
  "ChildSourceMismatch",
  "TargetLimitExceeded",
  "LocaleInvalid",
  "ProjectionFingerprintInvalid",
  "security: &SecurityContext",
  "enforce_operations_scope(security)",
  "enforce_scope(security, Resource::ForumCategories, Action::Manage)?",
  "enforce_scope(security, Resource::ForumTopics, Action::Manage)",
  "relation_after: Option<i64>",
  "revision_id > ?2",
  "revision_id > $2",
  "effective_limit.saturating_add(1)",
  "forum_relation_revisions",
  "forum_user_mentions",
  "forum_audience_mentions",
  "forum_topic_translations",
  "forum_reply_bodies",
  "FORUM_MAX_MENTION_TARGETS_PER_REVISION",
  "normalize_locale_tag(&source_locale)",
  'value == "legacy"',
  "value.len() == 64",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "DatabaseBackend::Sqlite => self.db.begin().await?",
  "transaction.commit().await?",
  "transaction.rollback().await",
  '"mention_reconciliation_report"',
  '"mention_reconciliation"',
]) {
  requireText(service, marker, `Forum mention reconciliation service missing ${marker}`);
}

for (const forbidden of [
  "UPDATE forum_",
  "DELETE FROM forum_",
  "INSERT INTO forum_",
  "ActiveModel",
  " OFFSET ",
  "ProfilesReader",
  "ProfileService",
  "rustok_notifications",
  "NotificationService",
]) {
  requireAbsent(
    service,
    forbidden,
    `read-only mention reconciliation service must not contain ${forbidden}`,
  );
}

requireText(
  servicesMod,
  "pub mod mention_reconciliation;",
  "Forum services composition must expose mention reconciliation owner",
);

for (const marker of [
  "pub struct ForumMentionReconciliationQuery",
  "forum_mention_reconciliation_report",
  "ForumMentionReconciliationService::new(db)",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "Permission::FORUM_CATEGORIES_MANAGE",
  "Permission::FORUM_TOPICS_MANAGE",
  "categories_manage && topics_manage",
  "auth.tenant_id != tenant.id",
  "SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)",
  "relation_after: Option<String>",
  "normalize_relation_cursor",
  "parse::<i64>()",
  "pub relation_cursor: Option<String>",
  "Whole-tenant clean requires exhausting the cursor chain",
]) {
  requireText(graphql, marker, `Forum mention reconciliation GraphQL missing ${marker}`);
}

for (const forbidden of [
  "tenant_id: Option<Uuid>",
  "Mutation",
  "UPDATE forum_",
  "ProfilesReader",
  "NotificationService",
]) {
  requireAbsent(graphql, forbidden, `mention report must not expose ${forbidden}`);
}

for (const marker of [
  "mod mention_reconciliation_query;",
  "GqlForumMentionDrift",
  "GqlForumMentionReconciliationReport",
  "mention_reconciliation_query::ForumMentionReconciliationQuery",
]) {
  requireText(graphqlMod, marker, `Forum GraphQL composition missing ${marker}`);
}

for (const marker of [
  "counter, accepted-solution, persisted-subscription and mention reconciliation",
  "attachments/permitted shared-owner reconciliation",
  "forumMentionReconciliationReport",
  "relationAfter: String",
]) {
  requireText(plan, marker, `canonical Forum plan missing ${marker}`);
}

for (const marker of [
  "Status: `source-ready / maintainer-execution-open / repair-open`",
  "FORUM-33D",
  "next source reconciliation cursor to mentions",
  "MentionRelationService",
  "does not re-resolve handles through Profiles",
  "does not query Notifications-owned",
  "source_unavailable",
  "child_source_mismatch",
  "target_limit_exceeded",
  "locale_invalid",
  "projection_fingerprint_invalid",
  "revision_id > relationAfter",
  "relationAfter: String",
  "forum_categories:manage",
  "forum_topics:manage",
  "REPEATABLE READ READ ONLY",
  "adds no relation repair",
  "next FORUM-33 source reconciliation cursor is **attachments**",
]) {
  requireText(packet, marker, `FORUM-33E actualization missing ${marker}`);
}

console.log("Forum FORUM-33E mention reconciliation source: ok");
