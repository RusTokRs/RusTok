#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-split-owner.json",
  docs: "crates/rustok-forum/docs/forum-21p-topic-split-owner.md",
  service: "crates/rustok-forum/src/services/topic_split.rs",
  serviceRegistry: "crates/rustok-forum/src/services/mod.rs",
  crateApi: "crates/rustok-forum/src/lib.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000020_add_forum_topic_split_operations.rs",
  migrationRegistry: "crates/rustok-forum/src/migrations/mod.rs",
  sqlite: "crates/rustok-forum/tests/topic_split_sqlite.rs",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const docs = read(paths.docs);
const service = read(paths.service);
const serviceRegistry = read(paths.serviceRegistry);
const crateApi = read(paths.crateApi);
const migration = read(paths.migration);
const migrationRegistry = read(paths.migrationRegistry);
const sqlite = read(paths.sqlite);

assert.equal(contract.contract, "forum_topic_split_owner_v1");
assert.equal(contract.task, "FORUM-21P");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner, "ForumTopicSplitService");
assert.equal(contract.command, "split_selected_replies");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.human_actor_required, true);
assert.equal(contract.command_shape.maximum_selected_replies, 500);
assert.equal(contract.scope.target_category, "source_category");
assert.equal(contract.scope.cross_category_split, false);
assert.equal(contract.scope.source_must_remain_nonempty, true);
assert.equal(contract.reply_identity.reply_ids_preserved, true);
assert.equal(contract.reply_identity.revisions_preserved_by_identity, true);
assert.equal(contract.reply_identity.mentions_preserved_by_identity, true);
assert.equal(contract.reply_identity.quotes_preserved_by_identity, true);
assert.equal(contract.parent_policy.selected_child_requires_selected_parent, true);
assert.equal(contract.parent_policy.selected_parent_requires_all_children_selected, true);
assert.equal(contract.parent_policy.cross_topic_parent_edges, false);
assert.equal(contract.target_access.target_cannot_broaden_source_access, true);
assert.equal(contract.solution_policy.selected_solution_moves_with_reply, true);
assert.equal(contract.solution_policy.unselected_solution_stays_with_source, true);
assert.equal(contract.counter_policy.category_topic_count_delta, 1);
assert.equal(contract.counter_policy.category_reply_count_delta, 0);
assert.equal(contract.idempotency.exact_replay_returns_original_receipt, true);
assert.equal(contract.audit.receipt_append_only, true);
assert.equal(contract.audit.reply_items_append_only, true);
assert.equal(contract.audit.semantic_event, "forum.topic.split");
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(contract.compatibility.rest_routes_changed, false);
assert.equal(contract.compatibility.admin_ui_changed, false);

includesAll(
  service,
  [
    "pub struct SplitForumTopicRepliesInput",
    "pub struct ForumTopicSplitResult",
    "pub struct ForumTopicSplitService",
    "pub async fn split_selected_replies(",
    "Resource::ForumTopics, Action::Manage",
    "MAX_FORUM_TOPIC_SPLIT_REPLIES: usize = 500",
    "fingerprint_command",
    "Sha256::digest",
    "load_split_operation_in_tx",
    "validate_replay_in_tx",
    "must leave at least one reply in the source topic",
    "cannot detach a selected reply from its parent",
    "cannot leave a child reply behind its selected parent",
    "move_selected_replies_in_tx",
    "target_position",
    "clone_topic_access_in_tx",
    "forum_topic_channel_access",
    "forum_topic_audience_policies",
    "forum_topic_reply_create_audience_policies",
    "validate_cloned_access_in_tx",
    "transfer_solution_in_tx",
    "validate_solution_after_split_in_tx",
    "increment_category_topic_count_in_tx",
    "UserStatsService::adjust_topic_count_in_tx",
    'const FORUM_TOPIC_SPLIT_EVENT_TYPE: &str = "forum.topic.split"',
    "insert_split_operation_in_tx",
    "insert_split_reply_audit_in_tx",
    "publish_forum_topic_projection_in_tx",
    "publish_forum_category_projection_in_tx",
  ],
  "topic split owner",
);
assert.ok(!service.includes("async_graphql"));
assert.ok(!service.includes("axum"));
assert.ok(!service.includes("GraphqlRequest"));
assert.ok(!service.includes("reqwest"));
assert.ok(!service.includes("rest_adapter"));

includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_split_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_split_operations",
    "CREATE TABLE IF NOT EXISTS forum_topic_split_reply_items",
    "command_fingerprint VARCHAR(64)",
    "moved_reply_count BETWEEN 1 AND 500",
    "target_resulting_published_reply_count = moved_published_reply_count",
    "UNIQUE (tenant_id, target_topic_id)",
    "UNIQUE (tenant_id, operation_id, target_position)",
    "forum topic split audit is append-only",
    "forum_topic_split_operation_update",
    "forum_topic_split_operation_delete",
    "forum_topic_split_reply_item_update",
    "forum_topic_split_reply_item_delete",
    "DatabaseBackend::Postgres",
    "DatabaseBackend::Sqlite",
  ],
  "topic split migration",
);

includesAll(
  serviceRegistry,
  [
    "mod topic_split;",
    "ForumTopicSplitResult",
    "ForumTopicSplitService",
    "SplitForumTopicRepliesInput",
  ],
  "service registry",
);
includesAll(
  crateApi,
  [
    "ForumTopicSplitResult",
    "ForumTopicSplitService",
    "MAX_FORUM_TOPIC_SPLIT_REPLIES",
    "SplitForumTopicRepliesInput",
  ],
  "crate API",
);
includesAll(
  migrationRegistry,
  [
    "mod m20260803_000020_add_forum_topic_split_operations;",
    "m20260803_000020_add_forum_topic_split_operations::Migration",
  ],
  "migration registry",
);

includesAll(
  sqlite,
  [
    "selected_reply_split_is_atomic_idempotent_and_append_only",
    "selected_reply_split_rejects_cross_boundary_parent_edges",
    "assert_eq!(first, replay)",
    "forum_topic_split_operations",
    "forum_topic_split_reply_items",
    "forum.topic.split",
    "forum_reply_bodies",
    "forum_topic_channel_access",
    "UPDATE forum_topic_split_operations",
    "DELETE FROM forum_topic_split_reply_items",
  ],
  "SQLite owner contract",
);

includesAll(
  docs,
  [
    "# FORUM-21P selected-reply topic split owner",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "parent-closed in both directions",
    "cannot broaden the source topic's local restrictions",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21P handoff",
);

console.log(
  "FORUM-21P selected-reply topic split owner source is ready; maintainer execution evidence and public transport/UI remain pending.",
);
