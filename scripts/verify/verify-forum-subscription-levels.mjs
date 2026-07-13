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
  model: "crates/rustok-forum/src/subscription.rs",
  dto: "crates/rustok-forum/src/dto/subscription.rs",
  service: "crates/rustok-forum/src/services/subscription.rs",
  categoryService: "crates/rustok-forum/src/services/subscription/category.rs",
  topicService: "crates/rustok-forum/src/services/subscription/topic.rs",
  policyService: "crates/rustok-forum/src/services/subscription/policy.rs",
  serviceHelpers: "crates/rustok-forum/src/services/subscription/helpers.rs",
  categoryEntity: "crates/rustok-forum/src/entities/forum_category_subscription.rs",
  topicEntity: "crates/rustok-forum/src/entities/forum_topic_subscription.rs",
  policyEntity: "crates/rustok-forum/src/entities/forum_subscription_policy.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels.rs",
  migrationParts: [
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/schema.rs",
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/events.rs",
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/automation.rs",
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/schema.rs",
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/validation.rs",
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/events.rs",
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/automation.rs",
  ],
  migrationRegistry: "crates/rustok-forum/src/migrations/mod.rs",
  controllers: "crates/rustok-forum/src/controllers/subscriptions.rs",
  router: "crates/rustok-forum/src/controllers/mod.rs",
  openapi: "crates/rustok-forum/src/openapi.rs",
  sqliteTest: "crates/rustok-forum/tests/subscription_levels_sqlite.rs",
  contractTest: "crates/rustok-forum/tests/subscription_levels_contract.rs",
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
    "forum.subscription.changed.v1",
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

  if (/DELETE FROM forum_(topic|category)_subscriptions[\s\S]{0,200}muted/i.test(serviceContract)) {
    fail(`${paths.service}: explicit mute must remain a persisted row`);
  }
  const eventOccurrences = migration.match(/forum\.subscription\.changed\.v1/g) ?? [];
  if (eventOccurrences.length < 8) {
    fail(`${paths.migration}: expected versioned event wiring for both backends`);
  }

  console.log(
    "forum subscription-level static verification passed (4 levels, revision, policy, auto-subscribe, v1 event)",
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
