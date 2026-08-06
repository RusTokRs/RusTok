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

function absolute(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  if (!existsSync(absolute(relativePath))) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolute(relativePath), "utf8");
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
    if (position < previous) {
      failures.push(`${label}: marker appears out of order ${marker}`);
    }
    previous = position;
  }
}

const migration = read(paths.migration);
const migrationsMod = read(paths.migrationsMod);
const topicOwner = read(paths.topicOwner);
const topicOwnerInline = read(paths.topicOwnerInline);
const snapshotOwner = read(paths.snapshotOwner);
const categoryVisibility = read(paths.categoryVisibility);
const topicAudienceLock = read(paths.topicAudienceLock);
const graphql = read(paths.graphql);
const native = read(paths.native);
const host = read(paths.host);
const contractText = read(paths.contract);
const test = read(paths.test);
const docs = read(paths.docs);

let contract = null;
try {
  contract = JSON.parse(contractText);
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
  "idx_forum_topic_route_tombstone_channel_lookup",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
]) {
  requireText(migration, marker, paths.migration);
}
requireText(
  migrationsMod,
  "m20260806_000025_add_forum_topic_route_tombstone_visibility",
  paths.migrationsMod,
);

requireOrder(
  topicOwner,
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
  topicOwnerInline,
  'include!("topic_route_tombstone_visibility.rs")',
  paths.topicOwnerInline,
);
requireText(
  topicOwnerInline,
  "topic_audience_lock",
  paths.topicOwnerInline,
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
  "Some(existing)",
  "None =>",
  "stored_channels != channel_slugs",
  "validate_sealed_channel_scope",
  "pub async fn can_disclose_public_gone(",
]) {
  requireText(snapshotOwner, marker, paths.snapshotOwner);
}
for (const marker of [
  "async_graphql",
  "axum::",
  "forum_category_policy::",
  "hashtextextended",
  "GqlForumStorefrontTopicRouteDisposition::Gone",
  "StorefrontForumTopicRouteDisposition::Gone",
  "StatusCode::GONE",
]) {
  forbidText(snapshotOwner, marker, paths.snapshotOwner);
}
requireText(
  topicAudienceLock,
  "pub(crate) async fn lock_topic_audience_scopes_in_tx(",
  paths.topicAudienceLock,
);
requireText(
  topicAudienceLock,
  '"SELECT pg_advisory_xact_lock(hashtextextended($1, 5))"',
  paths.topicAudienceLock,
);

requireText(
  categoryVisibility,
  "pub(crate) async fn is_category_public_to_anonymous",
  paths.categoryVisibility,
);
requireText(
  categoryVisibility,
  "super::category_audience::lock_category_tree_in_tx(&txn, tenant_id).await?",
  paths.categoryVisibility,
);

requireText(
  graphql,
  "ForumTopicRouteDisposition::Gone => return Ok(None)",
  paths.graphql,
);
forbidText(
  graphql,
  "GqlForumStorefrontTopicRouteDisposition::Gone",
  paths.graphql,
);
forbidText(
  native,
  "StorefrontForumTopicRouteDisposition::Gone",
  paths.native,
);
forbidText(host, "StatusCode::GONE", paths.host);

for (const marker of [
  "migration_adds_sealed_append_only_snapshot_storage",
  "delete_matches_canonical_policy_lock_order_before_snapshot",
  "snapshot_reuses_visibility_and_lock_owners_and_seals_exact_channel_scope",
  "this_slice_does_not_publish_gone_transport_or_http_policy",
]) {
  requireText(test, marker, paths.test);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "FORUM-24K",
  "route_channel_count",
  "SHA-256",
  "same order as the canonical topic audience owner",
  "does not change",
  "No tests, verifiers, formatting, Cargo commands, migrations, workflows",
]) {
  requireText(docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24J") {
    failures.push(`${paths.contract}: task must be FORUM-24J`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.owner !== "ForumTopicRouteTombstoneVisibilityService") {
    failures.push(`${paths.contract}: unexpected owner`);
  }
  if (contract.storage?.route_channel_count_sealed !== true) {
    failures.push(`${paths.contract}: channel count must be sealed`);
  }
  if (contract.storage?.route_channel_sha256_digest_sealed !== true) {
    failures.push(`${paths.contract}: channel digest must be sealed`);
  }
  if (contract.lock_owner_reuse?.custom_advisory_namespace_added !== false) {
    failures.push(`${paths.contract}: snapshot must reuse canonical lock owners`);
  }
  if (contract.replay?.existing_channels_appended !== false) {
    failures.push(`${paths.contract}: replay must never append channels`);
  }
  if (contract.compatibility?.public_gone_exposed !== false) {
    failures.push(`${paths.contract}: public GONE must remain hidden`);
  }
  if (contract.compatibility?.public_410_added !== false) {
    failures.push(`${paths.contract}: public 410 must remain out of scope`);
  }
}

if (failures.length > 0) {
  console.error("forum topic route tombstone visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum topic route tombstone visibility verification passed");
