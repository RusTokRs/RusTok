#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  resolutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21h-topic-merge-solution-policy.md",
  resolutionDocs: "crates/rustok-forum/docs/forum-21l-topic-merge-solution-resolution.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  error: "crates/rustok-forum/src/error.rs",
  merge: "crates/rustok-forum/src/services/topic_merge.rs",
  stats: "crates/rustok-forum/src/services/user_stats.rs",
  moderationOwner: "crates/rustok-forum/src/services/moderation_owner.rs",
  solutionLock: "crates/rustok-forum/src/services/topic_solution_lock.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000016_add_forum_topic_merge_solution_policy.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  ordinaryTest: "crates/rustok-forum/tests/topic_merge_sqlite.rs",
  resolutionTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
  graphqlTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const resolutionContract = JSON.parse(read(paths.resolutionContract));
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const resolutionDocs = read(paths.resolutionDocs);
const cumulativeDocs = read(paths.cumulativeDocs);
const error = read(paths.error);
const merge = read(paths.merge);
const stats = read(paths.stats);
const moderationOwner = read(paths.moderationOwner);
const solutionLock = read(paths.solutionLock);
const migration = read(paths.migration);
const migrationsMod = read(paths.migrationsMod);
const ordinaryTest = read(paths.ordinaryTest);
const resolutionTest = read(paths.resolutionTest);
const graphqlTest = read(paths.graphqlTest);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_merge_solution_policy_v1");
assert.equal(contract.task, "FORUM-21H");
assert.equal(contract.latest_resolution_slice, "FORUM-21L");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.ordinary_operation, "merge_topic");
assert.equal(contract.explicit_resolution_operation, "merge_topic_resolving_solution");
assert.equal(contract.postgres_advisory_seed, 31);
assert.equal(contract.conflict.stable_code, "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT");
assert.equal(contract.conflict.ordinary_merge_remains_fail_closed, true);
assert.equal(contract.policy_matrix.length, 6);
assert.equal(contract.atomicity.invalid_explicit_selection_precedes_all_mutation, true);
assert.equal(
  contract.atomicity.winner_selection_marker_changes_and_loser_statistic_commit_with_existing_merge_transaction,
  true,
);
assert.equal(contract.serialization.negative_solution_statistic_transition_is_atomic_and_exact, true);
assert.equal(contract.database_guards.negative_solution_statistic_requires_existing_positive_count, true);
assert.equal(contract.compatibility.ordinary_merge_input_changed, false);
assert.equal(contract.compatibility.merge_result_changed, false);
assert.equal(contract.compatibility.merge_receipt_changed, false);
assert.equal(contract.compatibility.ordinary_forum_topic_merged_schema_changed, false);
assert.equal(contract.compatibility.resolved_event_schema_version_added, 2);
assert.equal(contract.compatibility.migration_added, false);
assert.equal(resolutionContract.task, "FORUM-21L");
assert.equal(resolutionContract.selection.requires_exactly_two_valid_competing_solutions, true);
assert.equal(resolutionContract.statistics.loser_solution_count_delta, -1);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21L");
assert.equal(cumulativeContract.solution_policy.loser_solution_count_delta, -1);

includesAll(
  error,
  [
    "TopicMergeSolutionConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_SOLUTION_CONFLICT"',
    '"FORUM_TOPIC_MERGE_OPERATION_CONFLICT"',
    '"FORUM_VALIDATION_FAILED"',
  ],
  "Forum errors",
);
includesAll(
  migrationsMod,
  [
    "m20260803_000016_add_forum_topic_merge_solution_policy",
    "Box::new(m20260803_000016_add_forum_topic_merge_solution_policy::Migration)",
  ],
  "migration registration",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_solution_locks",
    "forum_lock_topic_solution_mutation",
    "forum_validate_topic_solution_target",
    "hashtextextended(",
    "31",
    "forum solution requires an active topic and approved reply",
  ],
  "solution migration",
);
includesAll(
  solutionLock,
  [
    "pub(crate) async fn lock_topic_solution_scopes_in_tx",
    "topic_ids.sort()",
    "topic_ids.dedup()",
    "hashtextextended($1, 31)",
    "forum_topic_solution_locks",
  ],
  "shared solution lock helper",
);
includesAll(
  moderationOwner,
  [
    "lock_topic_solution_scopes_in_tx",
    "UserStatsService::adjust_solution_count_in_tx",
  ],
  "ordinary solution owner",
);

includesAll(
  merge,
  [
    "pub async fn merge_topic(",
    "pub async fn merge_topic_resolving_solution(",
    "async fn merge_topic_internal(",
    "lock_topic_solution_scopes_in_tx(&txn, tenant_id, &[source.id, target.id])",
    "load_valid_solution_in_tx",
    "plan_solution_merge(",
    "TopicMergeSolutionConflict(operation_id)",
    "selected == source.reply_id",
    "selected == target.reply_id",
    "delete_solution_in_tx",
    "UserStatsService::adjust_solution_count_in_tx",
    "insert_transferred_solution_in_tx",
    "transferred.marked_by_user_id != solution.marked_by_user_id",
    "transferred.marked_at != solution.marked_at",
    "solution_resolution",
    "FORUM_TOPIC_MERGED_SOLUTION_RESOLUTION_SCHEMA_VERSION",
    "validate_solution_resolution_audit",
  ],
  "merge solution policy",
);
assert.equal((merge.match(/self\.db\.begin\(\)\.await\?/g) ?? []).length, 1);
const solutionLocks = merge.indexOf("lock_topic_solution_scopes_in_tx(");
const plan = merge.indexOf("let solution_plan = plan_solution_merge");
const sourceDelete = merge.indexOf("delete_solution_in_tx(&txn, tenant_id, source.id");
const targetDelete = merge.indexOf("delete_solution_in_tx(&txn, tenant_id, target.id");
const statDelta = merge.indexOf("UserStatsService::adjust_solution_count_in_tx");
const replyMove = merge.indexOf("move_replies_in_tx(", sourceDelete);
const transferInsert = merge.indexOf("insert_transferred_solution_in_tx", replyMove);
assert.ok(solutionLocks >= 0 && solutionLocks < plan);
assert.ok(plan < sourceDelete && sourceDelete < statDelta && statDelta < replyMove);
assert.ok(targetDelete > sourceDelete && targetDelete < statDelta);
assert.ok(replyMove < transferInsert);
for (const forbidden of [
  "newest_solution",
  "highest_score",
  "prefer_target",
  "prefer_source",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!merge.includes(forbidden), `merge policy contains forbidden marker: ${forbidden}`);
}

includesAll(
  stats,
  [
    "if delta == -1",
    "decrement_solution_count_exact_in_tx",
    "solution_count = solution_count - 1",
    "solution_count > 0",
    "rows_affected() != 1",
  ],
  "exact solution statistics",
);

includesAll(
  ordinaryTest,
  [
    "topic_merge_transfers_source_only_solution_and_preserves_target_only_solution",
    "topic_merge_rejects_cross_category_and_competing_solutions_without_partial_state",
    "TopicMergeSolutionConflict",
  ],
  "ordinary policy regression",
);
includesAll(
  resolutionTest,
  [
    "manager_can_select_source_solution_and_replay_exact_audit",
    "manager_can_select_target_solution_and_invalid_selection_is_atomic",
    "ordinary merge must keep competing solutions fail-closed",
    "Select the source answer after moderator review",
    "Retain the target answer after moderator review",
    "assert_resolution_event",
    "schema_version",
    "TopicMergeOperationConflict",
  ],
  "resolution regression",
);
includesAll(
  graphqlTest,
  [
    '"mergeForumTopicResolvingSolution"',
    '"selectedSolutionReplyId"',
    "ordinary_and_resolved_commands_share_one_private_transaction_owner",
  ],
  "resolution GraphQL contract",
);

includesAll(
  docs,
  [
    "# FORUM-21H topic merge accepted-solution policy",
    "FORUM-21L",
    "## Explicit competing-solution resolution",
    "## Fail-closed statistics",
    "schema version 2",
    "No command above was run by the implementation agent",
  ],
  "solution policy handoff",
);
includesAll(
  resolutionDocs,
  [
    "# FORUM-21L competing accepted-solution resolution",
    "merge_topic_resolving_solution",
    "mergeForumTopicResolvingSolution",
    "one exact decrement",
  ],
  "resolution handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21L",
    "mergeForumTopicResolvingSolution",
    "Negative solution-count transitions",
  ],
  "cumulative handoff",
);

assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));

console.log(
  "FORUM-21H/L topic merge solution policy source is ready; canonical FORUM-21 remains planned.",
);
