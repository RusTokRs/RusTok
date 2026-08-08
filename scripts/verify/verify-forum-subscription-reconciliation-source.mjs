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

const servicePath = "crates/rustok-forum/src/services/subscription/reconciliation.rs";
const subscriptionModulePath = "crates/rustok-forum/src/services/subscription.rs";
const graphqlPath = "crates/rustok-forum/src/graphql/subscription_reconciliation_query.rs";
const graphqlModulePath = "crates/rustok-forum/src/graphql/mod.rs";
const packetPath = "docs/modules/forum-33-subscription-reconciliation-actualization-2026-08-08.md";

const service = read(servicePath);
const subscriptionModule = read(subscriptionModulePath);
const graphql = read(graphqlPath);
const graphqlModule = read(graphqlModulePath);
const packet = read(packetPath);

for (const marker of [
  "pub struct ForumSubscriptionReconciliationService",
  "pub struct ForumSubscriptionReconciliationReport",
  "pub struct ForumSubscriptionCursor",
  "pub enum ForumSubscriptionDriftKind",
  "TargetMissing",
  "MergedTopicSourceSubscription",
  "MutedPreferencesInvalid",
  "RevisionInvalid",
  "security: &SecurityContext",
  "enforce_operations_scope(security)",
  "enforce_scope(security, Resource::ForumCategories, Action::Manage)?",
  "enforce_scope(security, Resource::ForumTopics, Action::Manage)",
  "topic_after_target: Option<Uuid>",
  "topic_after_user: Option<Uuid>",
  "category_after_target: Option<Uuid>",
  "category_after_user: Option<Uuid>",
  "subscription_cursor(",
  "cursor requires both target and user components",
  "effective_limit.saturating_add(1)",
  "TOPIC_SUBSCRIPTIONS_AFTER_SQLITE",
  "TOPIC_SUBSCRIPTIONS_AFTER_POSTGRES",
  "CATEGORY_SUBSCRIPTIONS_AFTER_SQLITE",
  "CATEGORY_SUBSCRIPTIONS_AFTER_POSTGRES",
  "s.topic_id > ?2 OR (s.topic_id = ?2 AND s.user_id > ?3)",
  "s.topic_id > $2 OR (s.topic_id = $2 AND s.user_id > $3)",
  "s.category_id > ?2 OR (s.category_id = ?2 AND s.user_id > ?3)",
  "s.category_id > $2 OR (s.category_id = $2 AND s.user_id > $3)",
  "FROM forum_topic_merge_operations merge_operation",
  "merge_operation.source_topic_id = s.topic_id",
  "s.level <> 'muted'",
  "s.digest_mode = 'disabled'",
  "revision <= 0",
  "begin_with_config(",
  "IsolationLevel::RepeatableRead",
  "AccessMode::ReadOnly",
  "DatabaseBackend::Sqlite => self.db.begin().await?",
  "transaction.commit().await?",
  "transaction.rollback().await",
  '"subscription_reconciliation_report"',
  '"subscription_reconciliation"',
]) {
  requireText(service, marker, `Forum subscription reconciliation service missing ${marker}`);
}

for (const forbidden of [
  "UPDATE forum_",
  "DELETE FROM forum_",
  "INSERT INTO forum_",
  "ActiveModel",
  " OFFSET ",
]) {
  requireAbsent(
    service,
    forbidden,
    `read-only subscription reconciliation service must not contain ${forbidden}`,
  );
}

requireText(
  subscriptionModule,
  "pub mod reconciliation;",
  "Forum subscription module must expose the reconciliation owner",
);

for (const marker of [
  "pub struct ForumSubscriptionReconciliationQuery",
  "forum_subscription_reconciliation_report",
  "ForumSubscriptionReconciliationService::new(db)",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "Permission::FORUM_CATEGORIES_MANAGE",
  "Permission::FORUM_TOPICS_MANAGE",
  "categories_manage && topics_manage",
  "auth.tenant_id != tenant.id",
  "SecurityContext::from_permission_snapshot(Some(auth.user_id), &auth.permissions)",
  "topic_after: Option<Uuid>",
  "topic_user_after: Option<Uuid>",
  "category_after: Option<Uuid>",
  "category_user_after: Option<Uuid>",
  "pub topic_cursor: Option<GqlForumSubscriptionCursor>",
  "pub category_cursor: Option<GqlForumSubscriptionCursor>",
  "Whole-tenant clean requires exhausting both composite cursor chains",
]) {
  requireText(graphql, marker, `Forum subscription reconciliation GraphQL missing ${marker}`);
}

for (const forbidden of ["tenant_id: Option<Uuid>", "Mutation", "UPDATE forum_"]) {
  requireAbsent(graphql, forbidden, `subscription report must not expose ${forbidden}`);
}

for (const marker of [
  "mod subscription_reconciliation_query;",
  "GqlForumSubscriptionCursor",
  "GqlForumSubscriptionDrift",
  "GqlForumSubscriptionReconciliationReport",
  "subscription_reconciliation_query::ForumSubscriptionReconciliationQuery",
]) {
  requireText(graphqlModule, marker, `Forum GraphQL composition missing ${marker}`);
}

for (const marker of [
  "Status: `source-ready / maintainer-execution-open / repair-open`",
  "FORUM-33C",
  "The next explicit source cursor recorded by FORUM-33C was subscriptions",
  "services::subscription::reconciliation",
  "does **not** infer",
  "(topic_id, user_id) > (topicAfter, topicUserAfter)",
  "(category_id, user_id) > (categoryAfter, categoryUserAfter)",
  "forumSubscriptionReconciliationReport(",
  "target_missing",
  "merged_topic_source_subscription",
  "muted_preferences_invalid",
  "revision_invalid",
  "reversible ordinary `archived` topic",
  "forum_categories:manage",
  "forum_topics:manage",
  "REPEATABLE READ READ ONLY",
  "adds no repair mutation",
  "next source reconciliation cursor is mentions",
]) {
  requireText(packet, marker, `FORUM-33D actualization missing ${marker}`);
}

console.log("Forum FORUM-33D subscription reconciliation source: ok");
