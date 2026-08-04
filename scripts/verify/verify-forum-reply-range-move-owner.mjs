#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-reply-range-move-owner.json",
  docs: "crates/rustok-forum/docs/forum-21s-reply-range-move-owner.md",
  readme: "crates/rustok-forum/docs/README.md",
  owner: "crates/rustok-forum/src/services/topic_reply_range_move.rs",
  serviceRegistry: "crates/rustok-forum/src/services/mod.rs",
  crateApi: "crates/rustok-forum/src/lib.rs",
  error: "crates/rustok-forum/src/error.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260804_000022_add_forum_reply_range_move_operations.rs",
  positionMigration:
    "crates/rustok-forum/src/migrations/m20260804_000023_advance_forum_reply_range_move_positions.rs",
  migrationRegistry: "crates/rustok-forum/src/migrations/mod.rs",
  sqlite: "crates/rustok-forum/tests/reply_range_move_sqlite.rs",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const docs = read(paths.docs);
const readme = read(paths.readme);
const owner = read(paths.owner);
const serviceRegistry = read(paths.serviceRegistry);
const crateApi = read(paths.crateApi);
const error = read(paths.error);
const migration = read(paths.migration);
const positionMigration = read(paths.positionMigration);
const migrationRegistry = read(paths.migrationRegistry);
const sqlite = read(paths.sqlite);

assert.equal(contract.contract, "forum_reply_range_move_owner_v1");
assert.equal(contract.task, "FORUM-21S");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner, "ForumReplyRangeMoveService");
assert.equal(contract.command, "move_reply_range");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.human_actor_required, true);
assert.equal(contract.command_shape.maximum_range_replies, 500);
assert.equal(
  contract.selection.policy,
  "all_current_replies_between_occupied_inclusive_endpoints",
);
assert.equal(contract.selection.source_must_retain_reply, true);
assert.equal(contract.identity_and_positions.reply_ids_preserved, true);
assert.equal(
  contract.identity_and_positions.target_policy,
  "append_after_current_target_max",
);
assert.equal(
  contract.identity_and_positions.target_next_reply_position,
  "monotonic_at_least_target_end_plus_one",
);
assert.equal(
  contract.identity_and_positions.source_next_reply_position,
  "monotonic_not_reduced",
);
assert.equal(contract.identity_and_positions.source_positions_compacted, false);
assert.equal(contract.parent_policy.incoming_external_parent, "detach_to_null");
assert.equal(contract.parent_policy.internal_parent, "preserve_unchanged_reply_id");
assert.equal(contract.parent_policy.outgoing_child_left_in_source, "reject");
assert.equal(
  contract.access_policy.source_target_effective_visibility,
  "exact_match_required",
);
assert.equal(contract.access_policy.same_category_required, false);
assert.equal(
  contract.solution_policy.selected_source_solution_when_target_unsolved,
  "follows_reply_via_fk_cascade",
);
assert.equal(
  contract.solution_policy.stable_conflict_code,
  "FORUM_TOPIC_REPLY_RANGE_MOVE_SOLUTION_CONFLICT",
);
assert.equal(contract.counter_policy.same_category_reply_count_delta, 0);
assert.equal(contract.counter_policy.topic_count_delta, 0);
assert.equal(contract.counter_policy.user_reply_count_delta, 0);
assert.equal(
  contract.idempotency.stable_conflict_code,
  "FORUM_TOPIC_REPLY_RANGE_MOVE_OPERATION_CONFLICT",
);
assert.deepEqual(contract.migrations, [
  "m20260804_000022_add_forum_reply_range_move_operations",
  "m20260804_000023_advance_forum_reply_range_move_positions",
]);
assert.equal(contract.audit.receipt_append_only, true);
assert.equal(contract.audit.reply_items_append_only, true);
assert.equal(contract.audit.semantic_event, "forum.topic.reply_range_moved");
assert.equal(contract.atomicity.partial_move_prevented, true);
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(contract.compatibility.rest_routes_changed, false);
assert.equal(contract.compatibility.admin_ui_changed, false);

includesAll(
  owner,
  [
    "pub struct MoveForumReplyRangeInput",
    "pub struct ForumReplyRangeMoveResult",
    "pub struct ForumReplyRangeMoveService",
    "pub async fn move_reply_range(",
    "Resource::ForumTopics, Action::Manage",
    "MAX_FORUM_REPLY_RANGE_MOVE_REPLIES: usize = 500",
    "Position.gte(start_position)",
    "Position.lte(end_position)",
    "endpoints must identify occupied source positions",
    "must leave at least one reply in the source topic",
    "cannot leave a child behind its moved parent",
    "parent-before-child source positions",
    "target_parent_reply_id = reply",
    ".filter(|parent_id| selected_ids.contains(parent_id))",
    "maximum_reply_position_in_tx",
    "target_start_position.checked_add(offset)",
    "TopicReplyRangeMoveSolutionConflict",
    "validate_equal_access_in_tx",
    "requires exactly equal effective visibility policy",
    "requires exactly equal effective reply-create policy",
    "requires exactly equal topic channel access",
    "approved_reply_count_in_tx",
    "reconcile_category_counters_in_tx",
    "forum.topic.reply_range_moved",
    "insert_operation_in_tx",
    "insert_reply_audit_in_tx",
    "TopicReplyRangeMoveOperationConflict",
  ],
  "reply range move owner",
);

for (const forbidden of [
  "async_graphql",
  "axum",
  "GraphqlRequest",
  "reqwest",
  "rest_adapter",
  "forum_reply_body::ActiveModel",
  "forum_reply_revision::ActiveModel",
  "forum_relation_revision::ActiveModel",
  "forum_user_mention::ActiveModel",
  "forum_quote::ActiveModel",
  "forum_reply_vote::ActiveModel",
]) {
  assert.ok(!owner.includes(forbidden), `owner copied or transported forbidden state: ${forbidden}`);
}

includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_reply_range_move_locks",
    "CREATE TABLE IF NOT EXISTS forum_reply_range_move_operations",
    "CREATE TABLE IF NOT EXISTS forum_reply_range_move_items",
    "moved_reply_count BETWEEN 1 AND 500",
    "target_end_position = target_start_position + moved_reply_count - 1",
    "forum reply range move audit is append-only",
    "forum_reply_range_move_operation_update",
    "forum_reply_range_move_operation_delete",
    "forum_reply_range_move_item_update",
    "forum_reply_range_move_item_delete",
    "DatabaseBackend::Postgres",
    "DatabaseBackend::Sqlite",
  ],
  "reply range move audit migration",
);

includesAll(
  positionMigration,
  [
    "forum_advance_moved_reply_position_watermark",
    "forum_reply_range_move_advance_target_position",
    "AFTER UPDATE OF topic_id, position",
    "GREATEST(next_reply_position, NEW.position + 1)",
    "MAX(next_reply_position, NEW.position + 1)",
    "DatabaseBackend::Postgres",
    "DatabaseBackend::Sqlite",
  ],
  "reply range move position watermark migration",
);

includesAll(
  error,
  [
    "TopicReplyRangeMoveOperationConflict(Uuid)",
    "TopicReplyRangeMoveSolutionConflict(Uuid)",
    '"FORUM_TOPIC_REPLY_RANGE_MOVE_OPERATION_CONFLICT"',
    '"FORUM_TOPIC_REPLY_RANGE_MOVE_SOLUTION_CONFLICT"',
  ],
  "Forum error contract",
);
includesAll(
  serviceRegistry,
  [
    "mod topic_reply_range_move;",
    "ForumReplyRangeMoveResult",
    "ForumReplyRangeMoveService",
    "MoveForumReplyRangeInput",
    "MAX_FORUM_REPLY_RANGE_MOVE_REPLIES",
  ],
  "service registry",
);
includesAll(
  crateApi,
  [
    "ForumReplyRangeMoveResult",
    "ForumReplyRangeMoveService",
    "MoveForumReplyRangeInput",
    "MAX_FORUM_REPLY_RANGE_MOVE_REPLIES",
  ],
  "crate API",
);
includesAll(
  migrationRegistry,
  [
    "mod m20260804_000022_add_forum_reply_range_move_operations;",
    "m20260804_000022_add_forum_reply_range_move_operations::Migration",
    "mod m20260804_000023_advance_forum_reply_range_move_positions;",
    "m20260804_000023_advance_forum_reply_range_move_positions::Migration",
  ],
  "migration registry",
);

includesAll(
  sqlite,
  [
    "reply_range_move_is_atomic_idempotent_and_preserves_identity",
    "reply_range_move_rejects_outgoing_child_boundary_atomically",
    "reply_range_move_rejects_competing_solutions_atomically",
    "assert_eq!(first, replay)",
    "forum_reply_range_move_operations",
    "forum_reply_range_move_items",
    "forum_reply_bodies",
    "forum_reply_revisions",
    "forum_reply_votes",
    "forum_relation_revisions",
    "forum.topic.reply_range_moved",
    "TopicReplyRangeMoveOperationConflict",
    "TopicReplyRangeMoveSolutionConflict",
    "UPDATE forum_reply_range_move_operations",
    "DELETE FROM forum_reply_range_move_items",
  ],
  "SQLite owner contract",
);

includesAll(
  docs,
  [
    "# FORUM-21S bounded reply-range move owner",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "a selected reply whose parent remains outside the range is detached",
    "an unselected child may not remain in source",
    "next_reply_position",
    "target_end_position + 1",
    "FORUM_TOPIC_REPLY_RANGE_MOVE_SOLUTION_CONFLICT",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21S handoff",
);
includesAll(
  readme,
  [
    "FORUM-21S adds the transport-neutral bounded reply-range move owner",
    "./forum-21s-reply-range-move-owner.md",
  ],
  "Forum docs index",
);

console.log(
  "FORUM-21S reply-range move owner source is ready; maintainer execution evidence and public transport/UI remain pending.",
);
