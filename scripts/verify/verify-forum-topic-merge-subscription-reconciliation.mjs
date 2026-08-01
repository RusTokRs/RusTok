#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-subscription-reconciliation.json",
  docs: "crates/rustok-forum/docs/forum-21c-topic-merge-subscription-reconciliation.md",
  entity: "crates/rustok-forum/src/entities/forum_topic_merge_subscription_reconciliation.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  lock: "crates/rustok-forum/src/services/topic_subscription_lock.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260801_000011_add_forum_topic_merge_subscription_reconciliations.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  postgresEvents:
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/postgres_up/events.rs",
  sqliteEvents:
    "crates/rustok-forum/src/migrations/m20260713_000013_add_forum_subscription_levels/sqlite_up/events.rs",
  service:
    "crates/rustok-forum/src/services/topic_merge_subscription_reconciliation.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  subscriptionWrite: "crates/rustok-forum/src/services/subscription/topic.rs",
  test:
    "crates/rustok-forum/tests/topic_merge_subscription_reconciliation_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier:
    "scripts/verify/verify-forum-topic-merge-subscription-reconciliation.mjs",
};

const read = (path) => readFileSync(path, "utf8");
const source = Object.fromEntries(
  Object.entries(paths).map(([key, path]) => [key, read(path)]),
);
const contract = JSON.parse(source.contract);

function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

function indexes(text, marker) {
  const result = [];
  let offset = 0;
  while (true) {
    const index = text.indexOf(marker, offset);
    if (index < 0) return result;
    result.push(index);
    offset = index + marker.length;
  }
}

assert.equal(contract.contract, "forum_topic_merge_subscription_reconciliation_v1");
assert.equal(contract.task, "FORUM-21C");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.depends_on, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_post_merge_topic_subscription_union");
assert.equal(
  contract.owner_service,
  "ForumTopicMergeSubscriptionReconciliationService",
);
assert.equal(contract.input, "ReconcileForumTopicMergeSubscriptionsInput");
assert.equal(
  contract.result,
  "ForumTopicMergeSubscriptionReconciliationResult",
);
assert.equal(
  contract.migration,
  "m20260801_000011_add_forum_topic_merge_subscription_reconciliations",
);
assert.equal(
  contract.receipt_table,
  "forum_topic_merge_subscription_reconciliations",
);
assert.equal(contract.source_merge_receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  source_subscription_rows_max: 500,
  one_merge_receipt_per_reconciliation: true,
  one_reconciliation_per_merge_receipt: true,
});
assert.deepEqual(contract.existing_subscription_event, {
  event_type: "forum.subscription.changed.v1",
  emitted_by_existing_table_triggers: true,
  same_owner_transaction: true,
});
assert.equal(
  contract.target_authority_policy.source_only,
  "move_row_to_target_preserving_delivery_state_last_notified_and_created_at_while_incrementing_revision_once_and_refreshing_updated_at",
);
assert.equal(contract.semantic_event.event_type, "forum.topic.merge_subscriptions_reconciled");
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
  "cargo test -p rustok-forum --test topic_merge_subscription_reconciliation_sqlite -- --nocapture",
]);

includesAll(
  source.entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_subscription_reconciliations")]',
    "pub merge_operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub source_subscription_count: i32",
    "pub moved_source_only_count: i32",
    "pub deduplicated_equal_count: i32",
    "pub target_authority_conflict_count: i32",
    "pub event_id: Uuid",
  ],
  "reconciliation entity",
);
includesAll(
  source.entitiesMod,
  [
    "pub mod forum_topic_merge_subscription_reconciliation;",
    "ForumTopicMergeSubscriptionReconciliationEntity",
  ],
  "entity exports",
);
includesAll(
  source.error,
  [
    "TopicMergeSubscriptionReconciliationConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_SUBSCRIPTION_RECONCILIATION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  source.migrationsMod,
  [
    "mod m20260801_000011_add_forum_topic_merge_subscription_reconciliations;",
    "m20260801_000011_add_forum_topic_merge_subscription_reconciliations::Migration,",
  ],
  "migration registration",
);
includesAll(
  source.migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_subscription_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_subscription_reconciliation_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_subscription_reconciliations",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "UNIQUE (tenant_id, merge_operation_id)",
    "source_subscription_count BETWEEN 0 AND 500",
    "source_subscription_count = moved_source_only_count",
    "forum_lock_topic_subscription_mutation",
    "forum_00_topic_subscription_scope",
    "forum-topic-subscription:%s:%s",
    "forum_reject_archived_topic_subscription_write",
    "forum_10_topic_subscriptions_active_write",
    "forum_topic_subscriptions_active_insert",
    "forum_topic_subscriptions_active_update",
    "forum topic subscriptions cannot target archived topics",
    "forum topic merge subscription reconciliations are append-only",
    "forum_topic_merge_subscription_reconciliation_update",
    "forum_topic_merge_subscription_reconciliation_delete",
  ],
  "reconciliation migration",
);
includesAll(
  source.postgresEvents,
  [
    "forum_emit_topic_subscription_event",
    "forum.subscription.changed.v1",
    "AFTER INSERT OR UPDATE OR DELETE ON forum_topic_subscriptions",
  ],
  "PostgreSQL subscription events",
);
includesAll(
  source.sqliteEvents,
  [
    "forum_80_topic_subscription_update_event AFTER UPDATE",
    "forum_80_topic_subscription_delete_event AFTER DELETE",
    "forum.subscription.changed.v1",
  ],
  "SQLite subscription events",
);

includesAll(
  source.lock,
  [
    "lock_active_topic_subscription_write_in_tx",
    "lock_topic_rows_for_subscription_in_tx",
    "lock_topic_subscription_scopes_in_tx",
    "FOR SHARE",
    "forum-topic-subscription:",
    "forum_topic_subscription_locks",
    "TopicStatus::Archived",
    "cannot be changed after topic archival",
  ],
  "shared subscription locks",
);

const normalTopicLock = source.subscriptionWrite.indexOf(
  "lock_active_topic_subscription_write_in_tx(&txn, tenant_id, topic_id).await?;",
);
const normalScopeLock = source.subscriptionWrite.indexOf(
  "lock_topic_subscription_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;",
);
const normalLookup = source.subscriptionWrite.indexOf(
  "let existing = forum_topic_subscription::Entity::find()",
  normalScopeLock,
);
assert.ok(normalTopicLock >= 0 && normalScopeLock >= 0 && normalLookup >= 0);
assert.ok(normalTopicLock < normalScopeLock);
assert.ok(normalScopeLock < normalLookup);
includesAll(
  source.subscriptionWrite,
  ["validate_expected_revision", "ensure_revision_update"],
  "ordinary subscription writes",
);

includesAll(
  source.service,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS: u64 = 500;",
    "pub struct ReconcileForumTopicMergeSubscriptionsInput",
    "pub struct ForumTopicMergeSubscriptionReconciliationResult",
    "pub struct ForumTopicMergeSubscriptionReconciliationService",
    "pub async fn reconcile_merge_subscriptions(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_subscription_reconciliation::Entity::find_by_id",
    "TopicMergeSubscriptionReconciliationConflict(input.operation_id)",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_merge_event_in_tx",
    "source.status != TopicStatus::Archived || !source.is_locked",
    "target.status == TopicStatus::Archived",
    "MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS + 1",
    "delivery_state_equal",
    "move_source_row_in_tx",
    "delete_source_row_in_tx",
    "ensure_source_subscriptions_empty_in_tx",
    "source.revision.checked_add(1)",
    "revision: Set(next_revision)",
    "updated_at: Set(Utc::now().into())",
    '"forum.topic.merge_subscriptions_reconciled"',
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_subscription_reconciliation::ActiveModel",
    "txn.commit().await?;",
  ],
  "reconciliation owner",
);
assert.ok(
  source.service.indexOf(
    "forum_topic_merge_subscription_reconciliation::Entity::find_by_id",
  ) < source.service.indexOf("forum_topic_merge_operation::Entity::find_by_id"),
);

const topicRowCalls = indexes(
  source.service,
  "lock_topic_rows_for_subscription_in_tx(\n",
);
const scopeCalls = indexes(
  source.service,
  "lock_topic_subscription_scopes_in_tx(\n",
);
const emptyProofs = indexes(
  source.service,
  "ensure_source_subscriptions_empty_in_tx(\n",
);
assert.equal(topicRowCalls.length, 1, "only first execution requires current topic rows");
assert.equal(scopeCalls.length, 2, "replay and first execution require subscription scopes");
assert.equal(emptyProofs.length, 2, "replay and first execution require source emptiness proof");
assert.ok(scopeCalls[0] < emptyProofs[0], "replay scope must precede emptiness proof");
assert.ok(topicRowCalls[0] < scopeCalls[1], "first execution must lock topics before scopes");
assert.ok(
  emptyProofs[1] < source.service.indexOf("forum_domain_event::ActiveModel"),
  "first execution must prove emptiness before evidence insertion",
);
for (const forbidden of [
  "usize::MAX",
  "combine_preferences",
  "merge_preferences",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
  "sourceCommitOverride",
]) {
  assert.ok(!source.service.includes(forbidden), `forbidden service marker: ${forbidden}`);
}

includesAll(
  source.servicesMod,
  [
    "mod topic_merge_subscription_reconciliation;",
    "mod topic_subscription_lock;",
    "ForumTopicMergeSubscriptionReconciliationResult",
    "ForumTopicMergeSubscriptionReconciliationService",
    "ReconcileForumTopicMergeSubscriptionsInput",
    "MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS",
  ],
  "service exports",
);
includesAll(
  source.lib,
  [
    "ForumTopicMergeSubscriptionReconciliationResult",
    "ForumTopicMergeSubscriptionReconciliationService",
    "ReconcileForumTopicMergeSubscriptionsInput",
    "MAX_FORUM_TOPIC_MERGE_SUBSCRIPTIONS",
  ],
  "crate exports",
);

includesAll(
  source.test,
  [
    "merge_subscription_reconciliation_is_atomic_idempotent_and_target_authoritative",
    "merge_subscription_reconciliation_requires_a_real_merge_receipt",
    "source_subscription_count, 4",
    "moved_source_only_count, 1",
    "deduplicated_equal_count, 2",
    "target_authority_conflict_count, 1",
    "subscription_count(&db, tenant_id, source_topic_id).await?, 0",
    "subscription_count(&db, tenant_id, target_topic_id).await?, 5",
    "assert_archived_subscription_database_guards",
    "UNIQUE (tenant_id, id)",
    '"muted"',
    "TopicMergeSubscriptionReconciliationConflict",
    "UPDATE forum_topic_merge_subscription_reconciliations",
    "DELETE FROM forum_topic_merge_subscription_reconciliations",
    '"forum.topic.merge_subscriptions_reconciled"',
  ],
  "SQLite regression",
);
assert.ok(source.test.includes('"immediate",\n        8,'));

includesAll(
  source.docs,
  [
    "# FORUM-21C topic merge subscription reconciliation",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "target subscription is always authoritative",
    "At most 500 source topic subscription rows",
    "topic row FOR SHARE",
    "BEFORE INSERT OR UPDATE OR DELETE",
    "revision from 7 to 8",
    "forum.subscription.changed.v1",
    "4 = 1 moved + 2 equal + 1 conflict",
    "automatic source/target topic-author subscriptions",
    "FORUM_TOPIC_MERGE_SUBSCRIPTION_RECONCILIATION_CONFLICT",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21C handoff",
);
assert.ok(
  source.plan.includes(
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
  ),
);
assert.ok(!source.plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!source.plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(source.verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21C topic merge subscription reconciliation source is ready and canonical FORUM-21 remains planned.",
);
