#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";

const args = new Set(process.argv.slice(2));
const staticOnly = args.has("--static-only");

if (args.has("--help")) {
  console.log(`Usage:
  node scripts/verify/verify-forum-subscription-levels.mjs [--static-only]

Checks FORUM-11 subscription levels, policies, auto-subscribe and event contracts.`);
  process.exit(0);
}
for (const arg of args) {
  if (!["--static-only", "--help"].includes(arg)) fail(`unknown argument: ${arg}`);
}

const paths = {
  model: "crates/modules/rustok-forum/src/subscription.rs",
  dto: "crates/modules/rustok-forum/src/dto/subscription.rs",
  service: "crates/modules/rustok-forum/src/services/subscription.rs",
  categoryService: "crates/modules/rustok-forum/src/services/subscription/category.rs",
  topicService: "crates/modules/rustok-forum/src/services/subscription/topic.rs",
  policyService: "crates/modules/rustok-forum/src/services/subscription/policy.rs",
  serviceHelpers: "crates/modules/rustok-forum/src/services/subscription/helpers.rs",
  categoryEntity: "crates/modules/rustok-forum/src/entities/forum_category_subscription.rs",
  topicEntity: "crates/modules/rustok-forum/src/entities/forum_topic_subscription.rs",
  policyEntity: "crates/modules/rustok-forum/src/entities/forum_subscription_policy.rs",
  migration:
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels.rs",
  migrationParts: [
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/schema.rs",
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/events.rs",
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/automation.rs",
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/schema.rs",
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/validation.rs",
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/events.rs",
    "crates/modules/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/automation.rs",
  ],
  migrationRegistry: "crates/modules/rustok-forum/src/migrations/mod.rs",
  controllers: "crates/modules/rustok-forum/src/controllers/subscriptions.rs",
  router: "crates/modules/rustok-forum/src/controllers/mod.rs",
  openapi: "crates/modules/rustok-forum/src/openapi.rs",
  graphqlMutation: "crates/modules/rustok-forum/src/graphql/mutation.rs",
  restTopics: "crates/modules/rustok-forum/src/controllers/topics.rs",
  restCategories: "crates/modules/rustok-forum/src/controllers/categories.rs",
  sqliteTest: "crates/modules/rustok-forum/tests/subscription_levels_sqlite.rs",
  contractTest: "crates/modules/rustok-forum/tests/subscription_levels_contract.rs",
};

function fail(message) {
  console.error("forum subscription-level verification failed:");
  console.error(`- ${message}`);
  process.exit(1);
}

function text(path) {
  if (!existsSync(path)) fail(`${path}: required file is missing`);
  return readFileSync(path, "utf8");
}

function requireTokens(path, source, tokens) {
  for (const token of tokens) {
    if (!source.includes(token)) fail(`${path}: missing token ${token}`);
  }
}

function verifyStatic() {
  const model = text(paths.model);
  const dto = text(paths.dto);
  const service = text(paths.service);
  const categoryService = text(paths.categoryService);
  const topicService = text(paths.topicService);
  const policyService = text(paths.policyService);
  const serviceHelpers = text(paths.serviceHelpers);
  const serviceContract = [service, categoryService, topicService, policyService, serviceHelpers].join("\n");
  const categoryEntity = text(paths.categoryEntity);
  const topicEntity = text(paths.topicEntity);
  const policyEntity = text(paths.policyEntity);
  const migration = [text(paths.migration), ...paths.migrationParts.map(text)].join("\n");
  const migrationRegistry = text(paths.migrationRegistry);
  const controllers = text(paths.controllers);
  const router = text(paths.router);
  const openapi = text(paths.openapi);
  const graphqlMutation = text(paths.graphqlMutation);
  const restTopics = text(paths.restTopics);
  const restCategories = text(paths.restCategories);
  text(paths.sqliteTest);
  text(paths.contractTest);

  requireTokens(paths.model, model, [
    "ForumSubscriptionLevel",
    "Watching",
    "Tracking",
    "Normal",
    "Muted",
    "ForumDigestMode",
    "is_explicitly_subscribed",
    "Muted => ForumSubscriptionPreferences",
  ]);
  requireTokens(paths.dto, dto, [
    "UpdateForumSubscriptionInput",
    "expected_revision",
    "ForumSubscriptionResponse",
    "UpdateForumSubscriptionPolicyInput",
    "ForumSubscriptionPolicyResponse",
  ]);
  for (const [path, source] of [
    [paths.categoryEntity, categoryEntity],
    [paths.topicEntity, topicEntity],
  ]) {
    requireTokens(path, source, [
      "pub level: ForumSubscriptionLevel",
      "pub notify_mentions: bool",
      "pub notify_replies: bool",
      "pub notify_new_topics: bool",
      "pub digest_mode: ForumDigestMode",
      "pub last_notified_at",
      "pub revision: i64",
      "pub updated_at",
    ]);
  }
  requireTokens(paths.policyEntity, policyEntity, [
    "forum_subscription_policies",
    "auto_subscribe_topic_authors",
    "topic_author_level",
    "auto_subscribe_reply_participants",
    "reply_participant_level",
    "revision",
  ]);
  requireTokens("forum subscription service modules", serviceContract, [
    "UpdateForumSubscriptionInput::watching()",
    "ForumSubscriptionLevel::Normal",
    "ForumSubscriptionLevel::Muted",
    "validate_expected_revision",
    "update_category_subscription",
    "update_topic_subscription",
    "update_policy",
    "is_explicitly_subscribed",
  ]);
  requireTokens(paths.migration, migration, [
    "forum.subscription.changed",
    "forum_subscription_policies",
    "forum_auto_subscribe_topic_author",
    "forum_auto_subscribe_reply_participant",
    "ON CONFLICT (tenant_id, topic_id, user_id) DO UPDATE SET",
    "ON CONFLICT(tenant_id,topic_id,user_id) DO UPDATE SET",
    "AFTER INSERT OR UPDATE OR DELETE ON forum_topic_subscriptions",
    "AFTER INSERT OR UPDATE OR DELETE ON forum_category_subscriptions",
  ]);
  requireTokens(paths.migrationRegistry, migrationRegistry, [
    "m20260713_000013_add_forum_subscription_levels",
  ]);
  requireTokens(paths.controllers, controllers, [
    "get_category_subscription_settings",
    "update_category_subscription_settings",
    "get_topic_subscription_settings",
    "update_topic_subscription_settings",
    "get_subscription_policy",
    "update_subscription_policy",
  ]);
  requireTokens(paths.router, router, [
    "pub mod subscriptions;",
    "subscriptions::get_category_subscription_settings",
    "subscriptions::get_topic_subscription_settings",
    '"/api/forum/subscription-policy"',
  ]);
  requireTokens(paths.openapi, openapi, [
    "UpdateForumSubscriptionInput",
    "ForumSubscriptionResponse",
    "UpdateForumSubscriptionPolicyInput",
    "ForumSubscriptionPolicyResponse",
  ]);
  requireTokens(paths.graphqlMutation, graphqlMutation, [
    "category.is_subscribed = true;",
    "category.is_subscribed = false;",
    "response.is_subscribed = true;",
    "async fn subscription_topic_response(",
  ]);
  const topicSet = graphqlMutation.slice(
    graphqlMutation.indexOf("async fn set_forum_topic_subscription("),
    graphqlMutation.indexOf("async fn clear_forum_topic_subscription("),
  );
  const read = topicSet.indexOf("subscription_topic_response(");
  const write = topicSet.indexOf(".set_topic_subscription(");
  if (read < 0 || write < 0 || read > write ||
      topicSet.indexOf("response.is_subscribed = true;") < write) {
    fail(`${paths.graphqlMutation}: topic set must pre-read and overlay the post-write flag`);
  }
  const blindClear = graphqlMutation.slice(
    graphqlMutation.indexOf("async fn clear_forum_topic_subscription("),
    graphqlMutation.indexOf("async fn create_forum_reply("),
  );
  if (!blindClear.includes(".clear_topic_subscription(") ||
      !blindClear.includes("resolve_tenant_scope(") ||
      !blindClear.includes("-> Result<bool>") ||
      !blindClear.includes("Ok(true)") ||
      graphqlMutation.includes("async fn clear_forum_topic_subscription_by_id(") ||
      blindClear.includes("subscription_topic_response(") ||
      blindClear.includes("get_authenticated_owner_visible_with_audience_context(")) {
    fail(`${paths.graphqlMutation}: canonical topic clear must only acknowledge the caller's row`);
  }
  const restClear = restTopics.slice(
    restTopics.indexOf("pub async fn unsubscribe_topic("),
    restTopics.indexOf("fn ensure_forum_permission(", restTopics.indexOf("pub async fn unsubscribe_topic(")),
  );
  if (!restClear.includes(".clear_topic_subscription(") ||
      !restClear.includes("Ok(StatusCode::NO_CONTENT)") ||
      restClear.includes("get_authenticated_owner_visible_with_audience_context(")) {
    fail(`${paths.restTopics}: REST blind clear must return an acknowledgement, never a topic body`);
  }
  const categoryRest = restCategories.slice(
    restCategories.indexOf("pub async fn subscribe_category("),
    restCategories.indexOf("fn category_audience_read_service(", restCategories.indexOf("pub async fn subscribe_category(")),
  );
  requireTokens(paths.restCategories, categoryRest, [
    "category.is_subscribed = true;", "category.is_subscribed = false;",
  ]);
  for (const [operation, flag] of [["subscribe", "true"], ["unsubscribe", "false"]]) {
    const section = categoryRest.slice(
      categoryRest.indexOf(`pub async fn ${operation}_category(`),
      operation === "subscribe" ? categoryRest.indexOf("pub async fn unsubscribe_category(") : undefined,
    );
    const read = section.indexOf("get_authenticated_owner_visible_with_audience_context(");
    const write = section.indexOf(`.${operation === "subscribe" ? "set" : "clear"}_category_subscription(`);
    const overlay = section.indexOf(`category.is_subscribed = ${flag};`);
    if (read < 0 || write < 0 || overlay < 0 || read > write || overlay < write) {
      fail(`${paths.restCategories}: ${operation} must pre-read and overlay the post-write flag`);
    }
  }

  if (/DELETE FROM forum_(topic|category)_subscriptions[\s\S]{0,200}muted/i.test(serviceContract)) {
    fail(`${paths.service}: explicit mute must remain a persisted row`);
  }
  const eventOccurrences = migration.match(/forum\.subscription\.changed/g) ?? [];
  if (eventOccurrences.length < 8 || !migration.includes("'forum.subscription.changed',1")) {
    fail(`${paths.migration}: expected event type with schema v1 wiring for both backends`);
  }

  console.log(
    "forum subscription-level static verification passed (4 levels, revision, policy, auto-subscribe, schema v1 event)",
  );
}

function run(label, command, commandArgs) {
  console.log(`\n==> ${label}`);
  const executable =
    process.platform === "win32" && command === "cargo" ? "cargo.exe" : command;
  const result = spawnSync(executable, commandArgs, {
    cwd: process.cwd(),
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) fail(`${label}: ${result.error.message}`);
  if (result.status !== 0) fail(`${label}: exited with status ${result.status}`);
}

verifyStatic();
if (staticOnly) process.exit(0);
run("forum subscription-level tests", "cargo", [
  "test",
  "-p",
  "rustok-forum",
  "--test",
  "subscription_levels_sqlite",
  "--test",
  "subscription_levels_contract",
]);
console.log("\nforum subscription-level verification passed");
