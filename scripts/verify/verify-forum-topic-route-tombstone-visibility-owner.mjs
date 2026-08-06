#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  migration:
    "crates/rustok-forum/src/migrations/m20260806_000025_add_forum_topic_route_tombstone_visibility.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  topicOwner: "crates/rustok-forum/src/services/topic_owner.rs",
  topicOwnerInline: "crates/rustok-forum/src/services/topic_owner_inline.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  snapshotOwner:
    "crates/rustok-forum/src/services/topic_route_tombstone_visibility.rs",
  categoryVisibility: "crates/rustok-forum/src/services/category_visibility.rs",
  topicAudienceLock: "crates/rustok-forum/src/services/topic_audience_lock.rs",
  graphql: "crates/rustok-forum/src/graphql/topic_route_query.rs",
  native:
    "crates/rustok-forum/storefront/src/transport/native_server_adapter_topic_route.rs",
  host: "apps/storefront/src/forum_topic_route.rs",
  contract:
    "crates/rustok-forum/contracts/forum-topic-route-tombstone-visibility-owner.json",
  test: "crates/rustok-forum/tests/topic_route_tombstone_visibility_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24j-topic-route-tombstone-visibility.md",
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function requireOrder(content, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const position = content.indexOf(marker);
    if (position < 0) {
      failures.push(`${label}: missing ordered marker ${marker}`);
      continue;
    }
    if (position < previous) failures.push(`${label}: marker appears out of order ${marker}`);
    previous = position;
  }
}

const source = Object.fromEntries(
  Object.entries(paths).map(([key, value]) => [key, read(value)]),
);

let contract = null;
try {
  contract = JSON.parse(source.contract);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}

for (const marker of [
  "forum_topic_route_tombstone_visibility",
  "forum_topic_route_tombstone_channels",
  "route_channel_count BIGINT NOT NULL",
  "route_channel_digest VARCHAR(64) NOT NULL",
  "route_channel_digest ~ '^[0-9a-f]{64}$'",
  "route_channel_digest NOT GLOB '*[^0-9a-f]*'",
  "forum topic route tombstone visibility is append-only",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
]) {
  requireText(source.migration, marker, paths.migration);
}
requireText(
  source.migrationsMod,
  "m20260806_000025_add_forum_topic_route_tombstone_visibility",
  paths.migrationsMod,
);

requireOrder(
  source.topicOwner,
  [
    "ForumTopicRouteTombstoneVisibilityService::lock_category_scope_in_tx(",
    "claim_topic_delete_in_tx(&txn, tenant_id, topic_id).await?",
    "ForumTopicRouteTombstoneVisibilityService::lock_topic_audience_scope_in_tx(",
    "ForumTopicRouteTombstoneVisibilityService::record_locked_delete_snapshot_in_tx(",
    "ForumTopicRouteService::record_delete_tombstones_in_tx(",
    "mark_topic_thread_deleted_in_tx(&txn, tenant_id, topic_id).await?",
  ],
  paths.topicOwner,
);
requireText(
  source.topicOwnerInline,
  'include!("topic_route_tombstone_visibility.rs")',
  paths.topicOwnerInline,
);
requireText(
  source.servicesMod,
  "ForumTopicRouteTombstoneVisibilityService",
  paths.servicesMod,
);

for (const marker of [
  "lock_category_tree_in_tx(txn, tenant_id).await",
  "lock_topic_audience_scopes_in_tx(txn, tenant_id, &[topic_id]).await",
  "is_category_public_to_anonymous(txn, tenant_id, topic.category_id)",
  "load_policy_for_topic(txn, tenant_id, topic)",
  "ForumAudienceEvaluator::decide(",
  "SecurityContext::public_read()",
  "topic.status == TopicStatus::Open",
  "route_channel_count",
  "route_channel_digest",
  "stored_channels != channel_slugs",
  "validate_sealed_channel_scope",
  "pub async fn can_disclose_public_gone(",
]) {
  requireText(source.snapshotOwner, marker, paths.snapshotOwner);
}
for (const marker of [
  "async_graphql",
  "axum::",
  "forum_category_policy::",
  "GqlForumStorefrontTopicRouteDecisionDisposition",
  "StorefrontForumTopicRouteDisposition",
  "StatusCode::GONE",
]) {
  forbidText(source.snapshotOwner, marker, paths.snapshotOwner);
}
for (const marker of [
  "StoredForumTopicRouteTombstoneVisibility",
  "route_channel_digest",
  "load_snapshot_channel_slugs",
]) {
  forbidText(source.servicesMod, marker, paths.servicesMod);
}

requireText(
  source.topicAudienceLock,
  "pub(crate) async fn lock_topic_audience_scopes_in_tx(",
  paths.topicAudienceLock,
);
requireText(
  source.categoryVisibility,
  "pub(crate) async fn is_category_public_to_anonymous",
  paths.categoryVisibility,
);

for (const content of [source.graphql, source.native]) {
  requireText(
    content,
    "ForumTopicRouteTombstoneVisibilityService",
    "FORUM-24K transport consumer",
  );
  requireText(content, ".can_disclose_public_gone(", "FORUM-24K transport consumer");
  for (const forbidden of [
    "forum_topic_route_tombstone_visibility",
    "forum_topic_route_tombstone_channels",
    "SELECT ",
  ]) {
    forbidText(content, forbidden, "FORUM-24K transport consumer");
  }
}
requireText(source.host, "StatusCode::GONE", paths.host);
forbidText(
  source.host,
  "ForumTopicRouteTombstoneVisibilityService",
  paths.host,
);
forbidText(source.host, "can_disclose_public_gone", paths.host);

for (const marker of [
  "migration_adds_sealed_append_only_snapshot_storage",
  "delete_matches_canonical_policy_lock_order_before_snapshot",
  "snapshot_reuses_visibility_and_lock_owners_and_seals_exact_channel_scope",
  "this_slice_does_not_publish_gone_transport_or_http_policy",
]) {
  requireText(source.test, marker, paths.test);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "FORUM-24K",
  "route_channel_count",
  "SHA-256",
  "same order as the canonical topic audience owner",
  "No tests, verifiers, formatting, Cargo commands, migrations, workflows",
]) {
  requireText(source.docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24J") failures.push(`${paths.contract}: task must be FORUM-24J`);
  if (contract.owner !== "ForumTopicRouteTombstoneVisibilityService") {
    failures.push(`${paths.contract}: unexpected owner`);
  }
  if (contract.storage?.route_channel_count_sealed !== true) {
    failures.push(`${paths.contract}: channel count must be sealed`);
  }
  if (contract.storage?.route_channel_sha256_digest_sealed !== true) {
    failures.push(`${paths.contract}: channel digest must be sealed`);
  }
  if (contract.replay?.existing_channels_appended !== false) {
    failures.push(`${paths.contract}: replay must never append channels`);
  }
}

if (failures.length > 0) {
  console.error("forum topic route tombstone visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum topic route tombstone visibility verification passed");
