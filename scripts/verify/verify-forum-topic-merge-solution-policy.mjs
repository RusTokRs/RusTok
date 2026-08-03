#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21h-topic-merge-solution-policy.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  error: "crates/rustok-forum/src/error.rs",
  merge: "crates/rustok-forum/src/services/topic_merge.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000016_add_forum_topic_merge_solution_policy.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  test: "crates/rustok-forum/tests/topic_merge_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-solution-policy.mjs",
};

const read = (path) => readFileSync(path, "utf8");
function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

const contract = JSON.parse(read(paths.contract));
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const cumulativeDocs = read(paths.cumulativeDocs);
const error = read(paths.error);
const merge = read(paths.merge);
const migration = read(paths.migration);
const migrationsMod = read(paths.migrationsMod);
const test = read(paths.test);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_merge_solution_policy_v1");
assert.equal(contract.task, "FORUM-21H");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.postgres_advisory_seed, 31);
assert.equal(contract.conflict.stable_code, "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT");
assert.deepEqual(
  contract.policy_matrix.map(({ source, target, outcome }) => [source, target, outcome]),
  [
    ["none", "none", "merge_without_solution_mutation"],
    ["none", "accepted_solution", "preserve_target_solution"],
    ["accepted_solution", "none", "transfer_source_solution_to_target"],
    ["accepted_solution", "accepted_solution", "fail_before_mutation"],
  ],
);
assert.equal(contract.atomicity.transfer_commits_with_existing_merge_transaction, true);
assert.equal(contract.atomicity.exact_merge_replay_adds_no_solution_mutation, true);
assert.equal(contract.compatibility.merge_input_changed, false);
assert.equal(contract.compatibility.merge_result_changed, false);
assert.equal(contract.compatibility.merge_receipt_changed, false);
assert.equal(contract.compatibility.forum_topic_merged_event_changed, false);
assert.equal(contract.compatibility.existing_projection_invalidation_targets_changed, false);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21H");
assert.equal(cumulativeContract.solution_policy.solution_count_delta_during_merge, 0);

includesAll(
  error,
  ["TopicMergeSolutionConflict(Uuid)", '"FORUM_TOPIC_MERGE_SOLUTION_CONFLICT"'],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260803_000016_add_forum_topic_merge_solution_policy;",
    "Box::new(m20260803_000016_add_forum_topic_merge_solution_policy::Migration)",
  ],
  "migration registry",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_solution_locks",
    "forum_lock_topic_solution_mutation",
    "forum_validate_topic_solution_target",
    "forum_00_topic_solution_scope",
    "forum_10_topic_solution_target",
    "FOR SHARE",
    "hashtextextended(",
    "31",
    "forum solution requires an active topic and approved reply",
    "CREATE TRIGGER IF NOT EXISTS forum_00_topic_solution_scope_insert",
    "CREATE TRIGGER IF NOT EXISTS forum_00_topic_solution_scope_update",
    "CREATE TRIGGER IF NOT EXISTS forum_00_topic_solution_scope_delete",
    "CREATE TRIGGER IF NOT EXISTS forum_10_topic_solution_target_insert",
    "CREATE TRIGGER IF NOT EXISTS forum_10_topic_solution_target_update",
    "topic.deleted_at IS NULL",
    "topic.status <> 'archived'",
    "reply.deleted_at IS NULL",
    "reply.status = 'approved'",
  ],
  "solution migration",
);
assert.ok((migration.match(/forum_00_topic_solution_scope/g) ?? []).length >= 4);
assert.ok((migration.match(/forum_10_topic_solution_target/g) ?? []).length >= 4);

includesAll(
  merge,
  [
    "struct ForumTopicMergeSolutionTransfer",
    "lock_topic_solution_scopes_in_tx(",
    "load_valid_solution_in_tx(&txn, tenant_id, source.id, \"source\")",
    "load_valid_solution_in_tx(&txn, tenant_id, target.id, \"target\")",
    "source_solution.is_some() && target_solution.is_some()",
    "TopicMergeSolutionConflict(input.operation_id)",
    "delete_source_solution_in_tx(&txn, tenant_id, source.id).await?;",
    "move_replies_in_tx(",
    "insert_transferred_solution_in_tx(&txn, tenant_id, target.id, &solution).await?;",
    "marked_by_user_id: Set(solution.marked_by_user_id)",
    "marked_at: Set(solution.marked_at)",
    "deleted_at IS NULL AND status = 'approved'",
    "load_valid_solution_in_tx(&txn, tenant_id, target.id, \"transferred target\")",
    "Forum transferred accepted solution metadata changed",
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "txn.commit().await?;",
  ],
  "merge owner",
);
assert.ok(!merge.includes("does not yet support a source accepted solution"));
assert.ok(!merge.includes("UserStatsService"));

const topicLocks = merge.indexOf("lock_topics_in_tx(&txn");
const solutionLocks = merge.indexOf("lock_topic_solution_scopes_in_tx(");
const sourceSolutionRead = merge.indexOf(
  "load_valid_solution_in_tx(&txn, tenant_id, source.id, \"source\")",
);
const conflict = merge.indexOf("TopicMergeSolutionConflict(input.operation_id)");
const sourceDelete = merge.indexOf(
  "delete_source_solution_in_tx(&txn, tenant_id, source.id).await?;",
);
const replyMove = merge.indexOf("move_replies_in_tx(", sourceDelete);
const solutionInsert = merge.indexOf(
  "insert_transferred_solution_in_tx(&txn, tenant_id, target.id, &solution).await?;",
);
const sourceArchive = merge.indexOf("source_active.status = Set(TopicStatus::Archived);");
const semanticEvent = merge.indexOf("forum_domain_event::ActiveModel");
const receipt = merge.indexOf("forum_topic_merge_operation::ActiveModel");
const invalidation = merge.indexOf("publish_forum_topic_projection_in_tx");
assert.ok(topicLocks < solutionLocks);
assert.ok(solutionLocks < sourceSolutionRead);
assert.ok(sourceSolutionRead < conflict);
assert.ok(conflict < sourceDelete);
assert.ok(sourceDelete < replyMove);
assert.ok(replyMove < solutionInsert);
assert.ok(solutionInsert < sourceArchive);
assert.ok(sourceArchive < semanticEvent);
assert.ok(semanticEvent < receipt);
assert.ok(receipt < invalidation);

includesAll(
  test,
  [
    "topic_merge_is_atomic_idempotent_and_append_only",
    "topic_merge_transfers_source_only_solution_and_preserves_target_only_solution",
    "topic_merge_rejects_cross_category_and_competing_solutions_without_partial_state",
    "topic_solution_database_guard_requires_active_topic_and_approved_reply",
    "ModerationService",
    "SolutionSnapshot",
    "source_solution_before",
    "target_solution_before",
    "user_solution_count",
    "TopicMergeSolutionConflict",
    '"FORUM_TOPIC_MERGE_SOLUTION_CONFLICT"',
    "merge_event_count",
    "baseline_projection_ids",
    "forum_user_stats",
    "pending_reply_id",
    "archived_topic_id",
  ],
  "SQLite regression",
);
assert.ok(!test.includes("source_solution_without_partial_state"));

includesAll(
  docs,
  [
    "# FORUM-21H topic merge accepted-solution policy",
    "`source_ready_maintainer_execution_pending`",
    "Outcome matrix",
    "Source-only transfer",
    "Statistics",
    "Competing solutions",
    "advisory lock seed `31`",
    "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21H docs",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21H extends",
    "source-only accepted solution",
    "target-only accepted solution",
    "two accepted solutions",
    "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
    "FORUM-21A through FORUM-21H",
  ],
  "cumulative merge docs",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("## `FORUM-21` — move, merge, split and fork topics"));
assert.ok(plan.includes("**Status:** `planned`"));
assert.ok(plan.includes("revalidate solutions and ACL"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21H topic merge accepted-solution policy source is ready; canonical FORUM-21 remains planned.",
);
