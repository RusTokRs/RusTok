#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-vote-reconciliation.json",
  docs: "crates/rustok-forum/docs/forum-21f-topic-merge-vote-reconciliation.md",
  entity: "crates/rustok-forum/src/entities/forum_topic_merge_vote_reconciliation.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000014_add_forum_topic_merge_vote_reconciliations.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_merge_vote_reconciliation.rs",
  lock: "crates/rustok-forum/src/services/topic_vote_lock.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  vote: "crates/rustok-forum/src/services/vote.rs",
  searchProjection: "crates/rustok-forum/src/search_projection.rs",
  test: "crates/rustok-forum/tests/topic_merge_vote_reconciliation_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-vote-reconciliation.mjs",
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
const docs = read(paths.docs);
const entity = read(paths.entity);
const entitiesMod = read(paths.entitiesMod);
const error = read(paths.error);
const lib = read(paths.lib);
const migration = read(paths.migration);
const migrationsMod = read(paths.migrationsMod);
const owner = read(paths.owner);
const lock = read(paths.lock);
const servicesMod = read(paths.servicesMod);
const vote = read(paths.vote);
const searchProjection = read(paths.searchProjection);
const test = read(paths.test);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_merge_vote_reconciliation_v1");
assert.equal(contract.task, "FORUM-21F");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.depends_on, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_post_merge_topic_vote_union");
assert.equal(contract.owner_service, "ForumTopicMergeVoteReconciliationService");
assert.equal(contract.input, "ReconcileForumTopicMergeVotesInput");
assert.equal(contract.result, "ForumTopicMergeVoteReconciliationResult");
assert.equal(
  contract.migration,
  "m20260803_000014_add_forum_topic_merge_vote_reconciliations",
);
assert.equal(contract.receipt_table, "forum_topic_merge_vote_reconciliations");
assert.equal(contract.source_merge_receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  source_vote_rows_max: 10000,
  one_merge_receipt_per_reconciliation: true,
  one_reconciliation_per_merge_receipt: true,
});
assert.deepEqual(contract.target_authority_policy, {
  source_only: "move_row_to_target_preserving_user_value_created_at_and_updated_at",
  source_and_target_equal: "delete_source_and_preserve_target_row",
  source_and_target_conflict: "delete_source_and_preserve_target_value_and_timestamps",
  target_only: "unchanged",
  source_after_reconciliation: "zero_rows",
  reply_votes: "unchanged_because_reply_ids_are_preserved_by_topic_merge",
});
assert.deepEqual(contract.search_projection, {
  invalidations_emitted: false,
  reason:
    "current Forum Search projection does not contain topic vote score or current-user vote state",
});
assert.equal(contract.semantic_event.event_type, "forum.topic.merge_votes_reconciled");
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.aggregate_is_retained_target, true);
assert.equal(contract.semantic_event.event_id_equals_operation_id, true);
assert.equal(contract.semantic_event.shared_rustok_events_contract_changed, false);
assert.equal(contract.transactional_invariants.length, 16);
assert.equal(contract.ordinary_write_hardening.length, 4);
assert.equal(contract.database_guards.length, 8);
assert.equal(contract.test, paths.test);
assert.equal(contract.verifier, paths.verifier);
assert.equal(contract.documentation, paths.docs);
assert.deepEqual(contract.maintainer_commands, [
  `node ${paths.verifier}`,
  "cargo test -p rustok-forum --test topic_merge_vote_reconciliation_sqlite -- --nocapture",
]);

includesAll(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_vote_reconciliations")]',
    "pub merge_operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub source_vote_count: i32",
    "pub moved_source_only_count: i32",
    "pub deduplicated_equal_count: i32",
    "pub target_authority_conflict_count: i32",
    "pub event_id: Uuid",
  ],
  "vote reconciliation entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_vote_reconciliation;",
    "ForumTopicMergeVoteReconciliationEntity",
  ],
  "entity exports",
);
includesAll(
  error,
  [
    "TopicMergeVoteReconciliationConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_VOTE_RECONCILIATION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260803_000014_add_forum_topic_merge_vote_reconciliations;",
    "m20260803_000014_add_forum_topic_merge_vote_reconciliations::Migration,",
  ],
  "migration registry",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_vote_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_vote_reconciliation_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_vote_reconciliations",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "UNIQUE (tenant_id, merge_operation_id)",
    "source_vote_count BETWEEN 0 AND 10000",
    "source_vote_count = moved_source_only_count",
    "+ target_authority_conflict_count",
    "forum_lock_topic_vote_mutation",
    "forum_00_topic_vote_scope",
    "forum-topic-vote:%s:%s",
    "forum_reject_archived_topic_vote_write",
    "forum_10_topic_votes_active_write",
    "forum_topic_votes_active_insert",
    "forum_topic_votes_active_update",
    "forum topic merge vote reconciliations are append-only",
    "forum_topic_merge_vote_reconciliation_update",
    "forum_topic_merge_vote_reconciliation_delete",
  ],
  "vote reconciliation migration",
);

includesAll(
  lock,
  [
    "lock_active_topic_vote_write_in_tx",
    "lock_topic_rows_for_votes_in_tx",
    "lock_topic_vote_scopes_in_tx",
    "FOR SHARE",
    "forum-topic-vote:",
    "forum_topic_vote_locks",
    "TopicStatus::Archived",
    "cannot be changed after topic archival",
  ],
  "vote lock owner",
);
includesAll(
  vote,
  [
    "lock_active_topic_vote_write_in_tx",
    "lock_topic_vote_scopes_in_tx",
    "pub async fn set_topic_vote(",
    "pub async fn clear_topic_vote(",
    "self.upsert_topic_vote_in_tx",
    "forum_topic_vote::Entity::delete_many()",
    "pub async fn set_reply_vote(",
    "pub async fn clear_reply_vote(",
  ],
  "canonical vote writes",
);
assert.equal(
  vote.match(/lock_active_topic_vote_write_in_tx\(/g)?.length,
  2,
  "set and clear must both lock the active topic",
);
assert.equal(
  vote.match(/lock_topic_vote_scopes_in_tx\(/g)?.length,
  2,
  "set and clear must both lock the topic vote scope",
);
const setStart = vote.indexOf("pub async fn set_topic_vote(");
const setActiveLock = vote.indexOf("lock_active_topic_vote_write_in_tx", setStart);
const setScopeLock = vote.indexOf("lock_topic_vote_scopes_in_tx", setStart);
const setMutation = vote.indexOf("self.upsert_topic_vote_in_tx", setStart);
assert.ok(setStart >= 0 && setStart < setActiveLock);
assert.ok(setActiveLock < setScopeLock && setScopeLock < setMutation);
const clearStart = vote.indexOf("pub async fn clear_topic_vote(");
const clearActiveLock = vote.indexOf("lock_active_topic_vote_write_in_tx", clearStart);
const clearScopeLock = vote.indexOf("lock_topic_vote_scopes_in_tx", clearStart);
const clearMutation = vote.indexOf("forum_topic_vote::Entity::delete_many()", clearStart);
assert.ok(clearStart >= 0 && clearStart < clearActiveLock);
assert.ok(clearActiveLock < clearScopeLock && clearScopeLock < clearMutation);
assert.ok(!vote.includes("async fn find_topic("));

includesAll(
  owner,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_VOTES: u64 = 10_000;",
    "pub const MAX_FORUM_TOPIC_MERGE_VOTE_REASON_LEN: usize = 500;",
    "pub struct ReconcileForumTopicMergeVotesInput",
    "pub struct ForumTopicMergeVoteReconciliationResult",
    "pub struct ForumTopicMergeVoteReconciliationService",
    "pub async fn reconcile_merge_votes(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_vote_reconciliation::Entity::find_by_id",
    "TopicMergeVoteReconciliationConflict",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_merge_event_in_tx",
    "lock_topic_rows_for_votes_in_tx",
    "lock_topic_vote_scopes_in_tx",
    "source.status != TopicStatus::Archived || !source.is_locked",
    "target.status == TopicStatus::Archived",
    "MAX_FORUM_TOPIC_MERGE_VOTES + 1",
    "order_by_asc(forum_topic_vote::Column::UserId)",
    "move_source_row_in_tx",
    "delete_source_row_in_tx",
    "ensure_source_votes_empty_in_tx",
    '"forum.topic.merge_votes_reconciled"',
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_vote_reconciliation::ActiveModel",
    "txn.commit().await?;",
  ],
  "vote reconciliation owner",
);
assert.ok(
  owner.indexOf("forum_topic_merge_vote_reconciliation::Entity::find_by_id") <
    owner.indexOf("forum_topic_merge_operation::Entity::find_by_id"),
  "exact replay must precede current merge lookup",
);
assert.ok(
  owner.indexOf("lock_topic_rows_for_votes_in_tx") <
    owner.lastIndexOf("lock_topic_vote_scopes_in_tx"),
  "first execution must lock topic rows before vote scopes",
);
const sourceEmptiness = owner.indexOf("ensure_source_votes_empty_in_tx");
const semanticEvent = owner.indexOf("forum_domain_event::ActiveModel");
const receipt = owner.indexOf("forum_topic_merge_vote_reconciliation::ActiveModel");
assert.ok(sourceEmptiness >= 0 && sourceEmptiness < semanticEvent);
assert.ok(semanticEvent < receipt);
assert.ok(!owner.includes("publish_forum_topic_projection_in_tx"));
assert.ok(!owner.includes("forum_reply_vote"));
for (const forbidden of [
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
  "value: Set(source.value + target.value)",
]) {
  assert.ok(!owner.includes(forbidden), `owner contains forbidden marker: ${forbidden}`);
}

includesAll(
  servicesMod,
  [
    "mod topic_merge_vote_reconciliation;",
    "mod topic_vote_lock;",
    "ForumTopicMergeVoteReconciliationResult",
    "ForumTopicMergeVoteReconciliationService",
    "ReconcileForumTopicMergeVotesInput",
    "MAX_FORUM_TOPIC_MERGE_VOTE_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_VOTES",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeVoteReconciliationResult",
    "ForumTopicMergeVoteReconciliationService",
    "ReconcileForumTopicMergeVotesInput",
    "MAX_FORUM_TOPIC_MERGE_VOTE_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_VOTES",
  ],
  "crate exports",
);

assert.ok(!searchProjection.includes("vote_score"));
assert.ok(!searchProjection.includes("current_user_vote"));
includesAll(
  test,
  [
    "merge_vote_reconciliation_is_atomic_idempotent_and_target_authoritative",
    "merge_vote_reconciliation_requires_a_real_merge_receipt",
    "ForumTopicMergeService",
    "ForumTopicMergeVoteReconciliationService",
    "source_vote_count, 3",
    "moved_source_only_count, 1",
    "deduplicated_equal_count, 1",
    "target_authority_conflict_count, 1",
    "assert_eq!(moved_after.value, moved_before.value)",
    "assert_eq!(moved_after.created_at, moved_before.created_at)",
    "assert_eq!(moved_after.updated_at, moved_before.updated_at)",
    "TopicMergeVoteReconciliationConflict",
    "UPDATE forum_topic_merge_vote_reconciliations",
    "DELETE FROM forum_topic_merge_vote_reconciliations",
    '"forum.topic.merge_votes_reconciled"',
    "summary.current_user_vote, Some(-1)",
  ],
  "SQLite regression",
);
assert.ok(!test.includes("projection_event_count"));
includesAll(
  docs,
  [
    "# FORUM-21F topic merge vote reconciliation",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "Target-authority policy",
    "same `user_id`",
    "at most 10,000 existing source rows",
    "emits no Search invalidation",
    "FORUM_TOPIC_MERGE_VOTE_RECONCILIATION_CONFLICT",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21F handoff",
);
assert.ok(
  plan.includes(
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
  ),
);
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21F topic merge vote reconciliation source is ready and canonical FORUM-21 remains planned.",
);
