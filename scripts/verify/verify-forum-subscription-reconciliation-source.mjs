#!/usr/bin/env node

import fs from "node:fs";

import {
  RECONCILIATION_EXECUTOR_PATH,
  assertReconciliationReport,
} from "./lib/forum-reconciliation-executor.mjs";

function read(path) {
  return fs.readFileSync(path, "utf8");
}

function requireText(text, marker, message) {
  if (!text.includes(marker)) throw new Error(message);
}

function requireAbsent(text, marker, message) {
  if (text.includes(marker)) throw new Error(message);
}

const servicePath = "crates/modules/rustok-forum/src/services/subscription/reconciliation.rs";
const subscriptionModulePath = "crates/modules/rustok-forum/src/services/subscription.rs";
const graphqlPath = "crates/modules/rustok-forum/src/graphql/subscription_reconciliation_query.rs";
const graphqlModulePath = "crates/modules/rustok-forum/src/graphql/mod.rs";

const service = read(servicePath);

// The operator fence, the entry-point metrics and the page bound are the executor's; this report has
// to reach them through its own `EXECUTOR` with its published metric labels.
assertReconciliationReport({
  executorSource: read(RECONCILIATION_EXECUTOR_PATH),
  serviceSource: service,
  name: "subscription",
});
const subscriptionModule = read(subscriptionModulePath);
const graphql = read(graphqlPath);
const graphqlModule = read(graphqlModulePath);

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

console.log("Forum FORUM-33D subscription reconciliation source: ok");
