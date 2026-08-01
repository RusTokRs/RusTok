#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const contractPath = "crates/rustok-forum/contracts/forum-topic-merge-owner.json";
const docsPath = "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md";
const entityPath = "crates/rustok-forum/src/entities/forum_topic_merge_operation.rs";
const entitiesModPath = "crates/rustok-forum/src/entities/mod.rs";
const errorPath = "crates/rustok-forum/src/error.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const migrationPath =
  "crates/rustok-forum/src/migrations/m20260801_000010_add_forum_topic_merge_operations.rs";
const migrationsModPath = "crates/rustok-forum/src/migrations/mod.rs";
const servicePath = "crates/rustok-forum/src/services/topic_merge.rs";
const servicesModPath = "crates/rustok-forum/src/services/mod.rs";
const testPath = "crates/rustok-forum/tests/topic_merge_sqlite.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const verifierPath = "scripts/verify/verify-forum-topic-merge-owner.mjs";

function read(path) {
  return readFileSync(path, "utf8");
}

function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

const contract = JSON.parse(read(contractPath));
const docs = read(docsPath);
const entity = read(entityPath);
const entitiesMod = read(entitiesModPath);
const error = read(errorPath);
const lib = read(libPath);
const migration = read(migrationPath);
const migrationsMod = read(migrationsModPath);
const service = read(servicePath);
const servicesMod = read(servicesModPath);
const test = read(testPath);
const plan = read(planPath);
const verifier = read(verifierPath);

assert.equal(contract.contract, "forum_topic_merge_owner_v1");
assert.equal(contract.task, "FORUM-21B");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_same_category_source_into_retained_target");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.input, "MergeForumTopicInput");
assert.equal(contract.result, "ForumTopicMergeResult");
assert.equal(contract.migration, "m20260801_000010_add_forum_topic_merge_operations");
assert.equal(contract.receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  source_reply_rows_max: 500,
  one_source_topic_per_operation: true,
  one_target_topic_per_operation: true,
  same_category_only: true,
});
assert.equal(contract.semantic_event.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.aggregate_is_retained_target, true);
assert.equal(contract.semantic_event.event_id_equals_operation_id, true);
assert.equal(contract.semantic_event.shared_rustok_events_contract_changed, false);
assert.equal(contract.transactional_invariants.length, 16);
assert.ok(
  contract.transactional_invariants.some((value) =>
    value.includes("category lifecycle") &&
    value.includes("counter scopes are acquired before topic row locks"),
  ),
);
assert.ok(
  contract.transactional_invariants.some((value) =>
    value.includes("category retained topic_count") && value.includes("remain unchanged"),
  ),
);
assert.equal(contract.database_guards.length, 6);
assert.equal(contract.retained_identity.category_topic_count, "unchanged_until_source_soft_delete");
assert.equal(contract.test, testPath);
assert.equal(contract.verifier, verifierPath);
assert.equal(contract.documentation, docsPath);
assert.deepEqual(contract.maintainer_commands, [
  `node ${verifierPath}`,
  "cargo test -p rustok-forum --test topic_merge_sqlite -- --nocapture",
]);

includesAll(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_operations")]',
    "pub tenant_id: Uuid",
    "pub operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub category_id: Uuid",
    "pub moved_reply_count: i32",
    "pub moved_published_reply_count: i32",
    "pub resulting_published_reply_count: i32",
    "pub position_offset: i64",
    "pub event_id: Uuid",
  ],
  "topic merge entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_operation;",
    "pub use forum_topic_merge_operation::Entity as ForumTopicMergeOperationEntity;",
  ],
  "entity exports",
);
includesAll(
  error,
  [
    "TopicMergeOperationConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_OPERATION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260801_000010_add_forum_topic_merge_operations;",
    "Box::new(m20260801_000010_add_forum_topic_merge_operations::Migration)",
  ],
  "migration registration",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_operations",
    "PRIMARY KEY (tenant_id, operation_id)",
    "FOREIGN KEY (tenant_id, source_topic_id)",
    "FOREIGN KEY (tenant_id, target_topic_id)",
    "FOREIGN KEY (tenant_id, category_id)",
    "FOREIGN KEY (tenant_id, actor_id)",
    "source_topic_id <> target_topic_id",
    "moved_reply_count BETWEEN 0 AND 500",
    "event_id = operation_id",
    "forum topic merge operations are append-only",
    "forum_topic_merge_operation_update",
    "forum_topic_merge_operation_delete",
  ],
  "topic merge migration",
);

includesAll(
  service,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500;",
    "pub const MAX_FORUM_TOPIC_MERGE_REPLIES: u64 = 500;",
    'const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";',
    "pub struct MergeForumTopicInput",
    "pub struct ForumTopicMergeResult",
    "pub struct ForumTopicMergeService",
    "pub async fn merge_topic(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_topic_merge_tenant_in_tx(&txn, tenant_id).await?;",
    '"SELECT pg_advisory_xact_lock(hashtextextended($1, 0))"',
    "forum_topic_merge_operation::Entity::find_by_id",
    "TopicMergeOperationConflict(input.operation_id)",
    "let preliminary_source =",
    "let preliminary_target =",
    "lock_merge_counter_scopes_in_tx(",
    'format!("forum:category:{tenant_id}:{category_id}")',
    'format!("forum:topic:{tenant_id}:{}", topic_ids[0])',
    'format!("forum:topic:{tenant_id}:{}", topic_ids[1])',
    '"SELECT forum_counter_lock($1)"',
    "lock_topics_in_tx(&txn, tenant_id, input.source_topic_id, target_topic_id).await?;",
    "Forum topic merge category changed concurrently",
    "source.category_id != target.category_id",
    "source accepted solution",
    "source_reply_count > MAX_FORUM_TOPIC_MERGE_REPLIES",
    "moved_published_reply_count != source.reply_count",
    "target_published_reply_count != target.reply_count",
    "position_offset",
    ".checked_add(source_max_position)",
    "move_replies_in_tx(",
    "source_active.status = Set(TopicStatus::Archived);",
    "source_active.is_locked = Set(true);",
    "source_active.reply_count = Set(0);",
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "publish_forum_category_projection_in_tx",
    "validate_existing_semantic_event_in_tx",
    "txn.commit().await?;",
  ],
  "topic merge service",
);
const receiptLookup = service.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const preliminaryRead = service.indexOf("let preliminary_source =");
const counterLocks = service.indexOf("lock_merge_counter_scopes_in_tx(");
const topicLocks = service.indexOf("lock_topics_in_tx(&txn");
const replyCount = service.indexOf("let source_reply_count =");
assert.ok(receiptLookup < preliminaryRead);
assert.ok(preliminaryRead < counterLocks);
assert.ok(counterLocks < topicLocks);
assert.ok(topicLocks < replyCount);
assert.equal((service.match(/publish_forum_topic_projection_in_tx\(/g) ?? []).length, 2);
assert.equal((service.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length, 1);
for (const forbidden of [
  "delete_many()",
  "forum_topic::Entity::delete",
  "forum_reply::Entity::delete",
  "category_active.topic_count",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
  "sourceCommitOverride",
  "bestEffort",
]) {
  assert.ok(!service.includes(forbidden), `service contains forbidden marker: ${forbidden}`);
}

includesAll(
  servicesMod,
  [
    "mod topic_merge;",
    "ForumTopicMergeResult, ForumTopicMergeService",
    "MergeForumTopicInput",
    "MAX_FORUM_TOPIC_MERGE_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_REPLIES",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeResult, ForumTopicMergeService",
    "MergeForumTopicInput",
    "MAX_FORUM_TOPIC_MERGE_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_REPLIES",
  ],
  "crate exports",
);
includesAll(
  test,
  [
    "topic_merge_is_atomic_idempotent_and_append_only",
    "topic_merge_rejects_cross_category_and_source_solution_without_partial_state",
    "ForumTopicMergeService",
    "MergeForumTopicInput",
    "moved_reply_count, 2",
    '"archived", true, 0',
    "category_id, 2, 3",
    "source_root_reply_id",
    "source_child_reply_id",
    "TopicMergeOperationConflict",
    "UPDATE forum_topic_merge_operations",
    "DELETE FROM forum_topic_merge_operations",
    '"forum.topic.merged"',
  ],
  "SQLite regression",
);
includesAll(
  docs,
  [
    "# FORUM-21B idempotent topic merge owner",
    "`source_ready_maintainer_execution_pending`",
    contractPath,
    "same active category",
    "source may contain at most 500 reply rows",
    "category-tree lifecycle lock",
    "category counter scope followed by source and target",
    "retained non-deleted-row counter",
    "source topic becomes archived and locked",
    "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
    "topic subscriptions",
    "source accepted solution",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21B handoff",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21B topic merge owner source is ready and canonical FORUM-21 remains planned.",
);
