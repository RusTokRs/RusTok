#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const contractPath =
  "crates/rustok-forum/contracts/forum-topic-merge-read-state-reconciliation.json";
const docsPath =
  "crates/rustok-forum/docs/forum-21d-topic-merge-read-state-reconciliation.md";
const entityPath =
  "crates/rustok-forum/src/entities/forum_topic_merge_read_state_reconciliation.rs";
const entitiesModPath = "crates/rustok-forum/src/entities/mod.rs";
const errorPath = "crates/rustok-forum/src/error.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const lockPath = "crates/rustok-forum/src/services/topic_read_state_lock.rs";
const migrationPath =
  "crates/rustok-forum/src/migrations/m20260801_000012_add_forum_topic_merge_read_state_reconciliations.rs";
const migrationsModPath = "crates/rustok-forum/src/migrations/mod.rs";
const readTrackingPath = "crates/rustok-forum/src/services/read_tracking.rs";
const readTrackingAudiencePath =
  "crates/rustok-forum/src/services/read_tracking_audience.rs";
const servicePath =
  "crates/rustok-forum/src/services/topic_merge_read_state_reconciliation.rs";
const servicesModPath = "crates/rustok-forum/src/services/mod.rs";
const testPath =
  "crates/rustok-forum/tests/topic_merge_read_state_reconciliation_sqlite.rs";
const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const verifierPath =
  "scripts/verify/verify-forum-topic-merge-read-state-reconciliation.mjs";

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
const lock = read(lockPath);
const migration = read(migrationPath);
const migrationsMod = read(migrationsModPath);
const readTracking = read(readTrackingPath);
const readTrackingAudience = read(readTrackingAudiencePath);
const service = read(servicePath);
const servicesMod = read(servicesModPath);
const test = read(testPath);
const plan = read(planPath);
const verifier = read(verifierPath);

assert.equal(contract.contract, "forum_topic_merge_read_state_reconciliation_v1");
assert.equal(contract.task, "FORUM-21D");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.depends_on, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_post_merge_conservative_read_state_discard");
assert.equal(
  contract.owner_service,
  "ForumTopicMergeReadStateReconciliationService",
);
assert.equal(contract.input, "ReconcileForumTopicMergeReadStatesInput");
assert.equal(contract.result, "ForumTopicMergeReadStateReconciliationResult");
assert.equal(
  contract.migration,
  "m20260801_000012_add_forum_topic_merge_read_state_reconciliations",
);
assert.equal(
  contract.receipt_table,
  "forum_topic_merge_read_state_reconciliations",
);
assert.equal(contract.source_merge_receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  source_read_state_rows_max: 500,
  one_merge_receipt_per_reconciliation: true,
  one_reconciliation_per_merge_receipt: true,
});
assert.deepEqual(contract.conservative_target_authority_policy, {
  target_rows: "byte_semantic_unchanged",
  source_only: "delete_source_without_creating_target_state",
  source_and_target_overlap: "delete_source_and_preserve_target",
  source_after_reconciliation: "zero_rows",
  rationale:
    "one_monotonic_target_high_water_cannot_represent_discontiguous_source_and_target_read_history_without_marking_unread_target_content_read",
});
assert.equal(
  contract.semantic_event.event_type,
  "forum.topic.merge_read_states_reconciled",
);
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.aggregate_is_retained_target, true);
assert.equal(contract.semantic_event.event_id_equals_operation_id, true);
assert.equal(contract.semantic_event.shared_rustok_events_contract_changed, false);
assert.equal(contract.transactional_invariants.length, 16);
assert.equal(contract.ordinary_write_hardening.length, 4);
assert.equal(contract.database_guards.length, 9);
assert.equal(contract.test, testPath);
assert.equal(contract.verifier, verifierPath);
assert.equal(contract.documentation, docsPath);
assert.deepEqual(contract.maintainer_commands, [
  `node ${verifierPath}`,
  "cargo test -p rustok-forum --test topic_merge_read_state_reconciliation_sqlite -- --nocapture",
]);

includesAll(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_read_state_reconciliations")]',
    "pub merge_operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub source_read_state_count: i32",
    "pub discarded_source_only_count: i32",
    "pub discarded_target_overlap_count: i32",
    "pub event_id: Uuid",
  ],
  "reconciliation entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_read_state_reconciliation;",
    "ForumTopicMergeReadStateReconciliationEntity",
  ],
  "entity exports",
);
includesAll(
  error,
  [
    "TopicMergeReadStateReconciliationConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_READ_STATE_RECONCILIATION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260801_000012_add_forum_topic_merge_read_state_reconciliations;",
    "m20260801_000012_add_forum_topic_merge_read_state_reconciliations::Migration,",
  ],
  "migration registry",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_read_state_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_read_state_reconciliation_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_read_state_reconciliations",
    "FOREIGN KEY (tenant_id, merge_operation_id)",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "UNIQUE (tenant_id, merge_operation_id)",
    "source_read_state_count BETWEEN 0 AND 500",
    "source_read_state_count = discarded_source_only_count",
    "forum_lock_topic_read_state_mutation",
    "forum_00_topic_read_state_scope",
    "forum-topic-read-state:%s:%s",
    "forum topic read-state identity cannot change",
    "forum_reject_archived_topic_read_state_write",
    "forum_10_topic_read_states_active_write",
    "forum_topic_read_states_active_insert",
    "forum_topic_read_states_active_update",
    "forum topic merge read-state reconciliations are append-only",
    "forum_topic_merge_read_state_reconciliation_update",
    "forum_topic_merge_read_state_reconciliation_delete",
  ],
  "reconciliation migration",
);

includesAll(
  lock,
  [
    "lock_active_topic_read_state_write_in_tx",
    "lock_active_topic_read_state_writes_in_tx",
    "lock_topic_rows_for_read_state_in_tx",
    "lock_topic_read_state_scopes_in_tx",
    "FOR SHARE",
    "forum-topic-read-state:",
    "forum_topic_read_state_locks",
    "TopicStatus::Archived",
    "cannot be changed after topic archival",
  ],
  "read-state locks",
);

includesAll(
  readTracking,
  [
    "lock_active_topic_read_state_write_in_tx(&txn, tenant_id, topic_id).await?;",
    "lock_topic_read_state_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;",
    "forum_topic::Column::DeletedAt.is_null()",
    "forum_topic::Column::Status.ne(TopicStatus::Archived)",
    "lock_active_topic_read_state_writes_in_tx(&txn, tenant_id, &topic_ids).await?;",
    "lock_topic_read_state_scopes_in_tx(&txn, tenant_id, &topic_ids).await?;",
  ],
  "raw read tracking",
);
const singleTopicLock = readTracking.indexOf(
  "lock_active_topic_read_state_write_in_tx(&txn, tenant_id, topic_id).await?;",
);
const singleScopeLock = readTracking.indexOf(
  "lock_topic_read_state_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;",
);
const singleHighWater = readTracking.indexOf("let latest_public_position", singleScopeLock);
assert.ok(singleTopicLock >= 0 && singleTopicLock < singleScopeLock);
assert.ok(singleScopeLock < singleHighWater);
const bulkTopicLock = readTracking.indexOf(
  "lock_active_topic_read_state_writes_in_tx(&txn, tenant_id, &topic_ids).await?;",
);
const bulkScopeLock = readTracking.indexOf(
  "lock_topic_read_state_scopes_in_tx(&txn, tenant_id, &topic_ids).await?;",
  bulkTopicLock,
);
const bulkHighWater = readTracking.indexOf(
  "latest_public_positions_in_tx(&txn, tenant_id, &topic_ids)",
  bulkScopeLock,
);
assert.ok(bulkTopicLock >= 0 && bulkTopicLock < bulkScopeLock);
assert.ok(bulkScopeLock < bulkHighWater);

includesAll(
  readTrackingAudience,
  [
    "forum_topic::Column::DeletedAt.is_null()",
    "forum_topic::Column::Status.ne(TopicStatus::Archived)",
    "lock_active_topic_read_state_writes_in_tx(",
    "lock_topic_read_state_scopes_in_tx(&write_txn, tenant_id, &visible_topic_ids)",
    "latest_public_positions_in_tx(&write_txn, tenant_id, &visible_topic_ids)",
  ],
  "visibility-scoped read tracking",
);
assert.ok(
  readTrackingAudience.indexOf("lock_active_topic_read_state_writes_in_tx(") <
    readTrackingAudience.indexOf(
      "lock_topic_read_state_scopes_in_tx(&write_txn, tenant_id, &visible_topic_ids)",
    ),
);
assert.ok(
  readTrackingAudience.indexOf(
    "lock_topic_read_state_scopes_in_tx(&write_txn, tenant_id, &visible_topic_ids)",
  ) <
    readTrackingAudience.indexOf(
      "latest_public_positions_in_tx(&write_txn, tenant_id, &visible_topic_ids)",
    ),
);

includesAll(
  service,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_READ_STATES: u64 = 500;",
    "pub struct ReconcileForumTopicMergeReadStatesInput",
    "pub struct ForumTopicMergeReadStateReconciliationResult",
    "pub struct ForumTopicMergeReadStateReconciliationService",
    "pub async fn reconcile_merge_read_states(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_read_state_reconciliation::Entity::find_by_id",
    "TopicMergeReadStateReconciliationConflict",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_merge_event_in_tx",
    "lock_topic_rows_for_read_state_in_tx",
    "lock_topic_read_state_scopes_in_tx",
    "source.status != TopicStatus::Archived || !source.is_locked",
    "target.status == TopicStatus::Archived",
    "MAX_FORUM_TOPIC_MERGE_READ_STATES + 1",
    "discarded_source_only_count",
    "discarded_target_overlap_count",
    "forum_topic_read_state::Entity::delete_many()",
    "ensure_source_read_states_empty_in_tx",
    '"forum.topic.merge_read_states_reconciled"',
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_read_state_reconciliation::ActiveModel",
    "txn.commit().await?;",
  ],
  "reconciliation owner",
);
assert.ok(
  service.indexOf("forum_topic_merge_read_state_reconciliation::Entity::find_by_id") <
    service.indexOf("forum_topic_merge_operation::Entity::find_by_id"),
  "exact replay must precede current merge lookup",
);
assert.ok(
  service.indexOf("lock_topic_rows_for_read_state_in_tx") <
    service.lastIndexOf("lock_topic_read_state_scopes_in_tx"),
  "first execution must lock topic rows before read-state scopes",
);
const firstExecutionEmptiness = service.indexOf(
  "ensure_source_read_states_empty_in_tx(&txn, tenant_id, merge.source_topic_id).await?;",
);
const evidenceInsert = service.indexOf("forum_domain_event::ActiveModel");
assert.ok(
  firstExecutionEmptiness >= 0 && firstExecutionEmptiness < evidenceInsert,
  "first-execution source emptiness must precede evidence insertion",
);
for (const forbidden of [
  "forum_topic_read_state::Entity::insert",
  "forum_topic_read_state::Entity::update_many",
  "position_offset +",
  "max_read_position",
  "combine_read_state",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!service.includes(forbidden), `service contains forbidden marker: ${forbidden}`);
}

includesAll(
  servicesMod,
  [
    "mod topic_merge_read_state_reconciliation;",
    "mod topic_read_state_lock;",
    "ForumTopicMergeReadStateReconciliationResult",
    "ForumTopicMergeReadStateReconciliationService",
    "ReconcileForumTopicMergeReadStatesInput",
    "MAX_FORUM_TOPIC_MERGE_READ_STATES",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeReadStateReconciliationResult",
    "ForumTopicMergeReadStateReconciliationService",
    "ReconcileForumTopicMergeReadStatesInput",
    "MAX_FORUM_TOPIC_MERGE_READ_STATES",
  ],
  "crate exports",
);

includesAll(
  test,
  [
    "merge_read_state_reconciliation_is_conservative_atomic_and_idempotent",
    "merge_read_state_reconciliation_requires_a_real_merge_receipt",
    "ForumTopicMergeService",
    "ForumTopicMergeReadStateReconciliationService",
    "source_read_state_count, 3",
    "discarded_source_only_count, 1",
    "discarded_target_overlap_count, 2",
    "read_state_snapshots(&db, tenant_id, target_topic_id).await?, target_before",
    "bulk.processed, 1",
    "TopicMergeReadStateReconciliationConflict",
    "UPDATE forum_topic_merge_read_state_reconciliations",
    "DELETE FROM forum_topic_merge_read_state_reconciliations",
    '"forum.topic.merge_read_states_reconciled"',
  ],
  "SQLite regression",
);
includesAll(
  docs,
  [
    "# FORUM-21D topic merge read-state reconciliation",
    "`source_ready_maintainer_execution_pending`",
    contractPath,
    "Why source high-water is not translated",
    "target rows are byte-semantic authoritative",
    "At most 500 source read-state rows",
    "exclude deleted and archived candidates",
    "FORUM_TOPIC_MERGE_READ_STATE_RECONCILIATION_CONFLICT",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21D handoff",
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
  "FORUM-21D conservative topic merge read-state reconciliation source is ready and canonical FORUM-21 remains planned.",
);
