#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  solutionContract:
    "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  docs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  entity: "crates/rustok-forum/src/entities/forum_topic_merge_operation.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  receiptMigration:
    "crates/rustok-forum/src/migrations/m20260801_000010_add_forum_topic_merge_operations.rs",
  solutionMigration:
    "crates/rustok-forum/src/migrations/m20260803_000016_add_forum_topic_merge_solution_policy.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  service: "crates/rustok-forum/src/services/topic_merge.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  test: "crates/rustok-forum/tests/topic_merge_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-owner.mjs",
};

function read(path) {
  return readFileSync(path, "utf8");
}

function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

const contract = JSON.parse(read(paths.contract));
const solutionContract = JSON.parse(read(paths.solutionContract));
const docs = read(paths.docs);
const entity = read(paths.entity);
const entitiesMod = read(paths.entitiesMod);
const error = read(paths.error);
const lib = read(paths.lib);
const receiptMigration = read(paths.receiptMigration);
const solutionMigration = read(paths.solutionMigration);
const migrationsMod = read(paths.migrationsMod);
const service = read(paths.service);
const servicesMod = read(paths.servicesMod);
const test = read(paths.test);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_merge_owner_v1");
assert.equal(contract.task, "FORUM-21B");
assert.equal(contract.latest_policy_slice, "FORUM-21H");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_same_category_source_into_retained_target");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.input, "MergeForumTopicInput");
assert.equal(contract.result, "ForumTopicMergeResult");
assert.deepEqual(contract.migrations, [
  "m20260801_000010_add_forum_topic_merge_operations",
  "m20260803_000016_add_forum_topic_merge_solution_policy",
]);
assert.equal(contract.receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.equal(contract.bounds.reason_max_characters, 500);
assert.equal(contract.bounds.source_reply_rows_max, 500);
assert.equal(contract.bounds.same_category_only, true);
assert.equal(contract.bounds.accepted_solutions_per_topic_max, 1);
assert.equal(contract.semantic_event.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.event_id_equals_operation_id, true);
assert.equal(contract.solution_policy.solution_count_delta_during_merge, 0);
assert.equal(
  contract.solution_policy.source_and_target,
  "fail_before_mutation_with_FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
);
assert.equal(solutionContract.task, "FORUM-21H");
assert.equal(solutionContract.extends, "FORUM-21B");

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
    "TopicMergeSolutionConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_OPERATION_CONFLICT"',
    '"FORUM_TOPIC_MERGE_SOLUTION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260801_000010_add_forum_topic_merge_operations;",
    "Box::new(m20260801_000010_add_forum_topic_merge_operations::Migration)",
    "mod m20260803_000016_add_forum_topic_merge_solution_policy;",
    "Box::new(m20260803_000016_add_forum_topic_merge_solution_policy::Migration)",
  ],
  "migration registration",
);
includesAll(
  receiptMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_operations",
    "PRIMARY KEY (tenant_id, operation_id)",
    "source_topic_id <> target_topic_id",
    "moved_reply_count BETWEEN 0 AND 500",
    "event_id = operation_id",
    "forum topic merge operations are append-only",
  ],
  "receipt migration",
);
includesAll(
  solutionMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_solution_locks",
    "forum_lock_topic_solution_mutation",
    "forum_validate_topic_solution_target",
    "hashtextextended(",
    "31",
    "forum solution requires an active topic and approved reply",
  ],
  "solution policy migration",
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
    "forum_topic_merge_operation::Entity::find_by_id",
    "TopicMergeOperationConflict(input.operation_id)",
    "lock_merge_counter_scopes_in_tx(",
    "lock_topics_in_tx(&txn, tenant_id, input.source_topic_id, target_topic_id).await?;",
    "lock_topic_solution_scopes_in_tx(",
    "TopicMergeSolutionConflict(input.operation_id)",
    "delete_source_solution_in_tx",
    "move_replies_in_tx(",
    "insert_transferred_solution_in_tx",
    "source_reply_count > MAX_FORUM_TOPIC_MERGE_REPLIES",
    "moved_published_reply_count != source.reply_count",
    "target_published_reply_count != target.reply_count",
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
assert.ok(!service.includes("does not yet support a source accepted solution"));
assert.ok(!service.includes("UserStatsService"));
const receiptLookup = service.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const preliminaryRead = service.indexOf("let preliminary_source =");
const counterLocks = service.indexOf("lock_merge_counter_scopes_in_tx(");
const topicLocks = service.indexOf("lock_topics_in_tx(&txn");
const solutionLocks = service.indexOf("lock_topic_solution_scopes_in_tx(");
const solutionConflict = service.indexOf("TopicMergeSolutionConflict(input.operation_id)");
const solutionDelete = service.indexOf("delete_source_solution_in_tx(&txn");
const replyMove = service.indexOf("move_replies_in_tx(", solutionDelete);
const solutionInsert = service.indexOf("insert_transferred_solution_in_tx(&txn");
assert.ok(receiptLookup < preliminaryRead);
assert.ok(preliminaryRead < counterLocks);
assert.ok(counterLocks < topicLocks);
assert.ok(topicLocks < solutionLocks);
assert.ok(solutionLocks < solutionConflict);
assert.ok(solutionConflict < solutionDelete);
assert.ok(solutionDelete < replyMove);
assert.ok(replyMove < solutionInsert);
assert.equal((service.match(/publish_forum_topic_projection_in_tx\(/g) ?? []).length, 2);
assert.equal((service.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length, 1);
for (const forbidden of [
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
    "topic_merge_transfers_source_only_solution_and_preserves_target_only_solution",
    "topic_merge_rejects_cross_category_and_competing_solutions_without_partial_state",
    "topic_solution_database_guard_requires_active_topic_and_approved_reply",
    "moved_reply_count, 2",
    '"archived", true, 0',
    "source_root_reply_id",
    "source_child_reply_id",
    "TopicMergeOperationConflict",
    "TopicMergeSolutionConflict",
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
    paths.contract,
    paths.solutionContract,
    "source-only accepted solution",
    "target-only accepted solution",
    "two accepted solutions",
    "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
    "FORUM-21A through FORUM-21H",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21B handoff",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21B/H topic merge owner source is ready and canonical FORUM-21 remains planned.",
);
