#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-tag-reconciliation.json",
  docs: "crates/rustok-forum/docs/forum-21e-topic-merge-tag-reconciliation.md",
  entity: "crates/rustok-forum/src/entities/forum_topic_merge_tag_reconciliation.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000013_add_forum_topic_merge_tag_reconciliations.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_merge_tag_reconciliation.rs",
  lock: "crates/rustok-forum/src/services/topic_tag_lock.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  topicInline: "crates/rustok-forum/src/services/topic_inline.rs",
  test: "crates/rustok-forum/tests/topic_merge_tag_reconciliation_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-tag-reconciliation.mjs",
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
const topicInline = read(paths.topicInline);
const test = read(paths.test);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_merge_tag_reconciliation_v1");
assert.equal(contract.task, "FORUM-21E");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.depends_on, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_post_merge_topic_tag_union");
assert.equal(contract.owner_service, "ForumTopicMergeTagReconciliationService");
assert.equal(contract.input, "ReconcileForumTopicMergeTagsInput");
assert.equal(contract.result, "ForumTopicMergeTagReconciliationResult");
assert.equal(
  contract.migration,
  "m20260803_000013_add_forum_topic_merge_tag_reconciliations",
);
assert.equal(contract.receipt_table, "forum_topic_merge_tag_reconciliations");
assert.equal(contract.source_merge_receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  ordinary_topic_tags_max: 100,
  source_tag_rows_max: 500,
  one_merge_receipt_per_reconciliation: true,
  one_reconciliation_per_merge_receipt: true,
});
assert.deepEqual(contract.set_union_policy, {
  source_only: "move_row_to_target_preserving_row_id_term_id_and_created_at",
  source_and_target_same_term: "delete_source_and_preserve_target_row",
  target_only: "unchanged",
  source_after_reconciliation: "zero_rows",
});
assert.deepEqual(contract.projection_repair, {
  source_topic_invalidation: true,
  target_topic_invalidation: true,
  same_owner_transaction: true,
  reason: "forum tags are search projection facets",
});
assert.equal(contract.semantic_event.event_type, "forum.topic.merge_tags_reconciled");
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
  "cargo test -p rustok-forum --test topic_merge_tag_reconciliation_sqlite -- --nocapture",
]);

includesAll(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_tag_reconciliations")]',
    "pub merge_operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub source_tag_count: i32",
    "pub moved_source_only_count: i32",
    "pub deduplicated_existing_count: i32",
    "pub event_id: Uuid",
  ],
  "tag reconciliation entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_tag_reconciliation;",
    "ForumTopicMergeTagReconciliationEntity",
  ],
  "entity exports",
);
includesAll(
  error,
  [
    "TopicMergeTagReconciliationConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_TAG_RECONCILIATION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260803_000013_add_forum_topic_merge_tag_reconciliations;",
    "m20260803_000013_add_forum_topic_merge_tag_reconciliations::Migration,",
  ],
  "migration registry",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_tag_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_tag_reconciliation_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_tag_reconciliations",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "UNIQUE (tenant_id, merge_operation_id)",
    "source_tag_count BETWEEN 0 AND 500",
    "source_tag_count = moved_source_only_count + deduplicated_existing_count",
    "forum_lock_topic_tag_mutation",
    "forum_00_topic_tag_scope",
    "forum-topic-tag:%s:%s",
    "forum_reject_archived_topic_tag_write",
    "forum_10_topic_tags_active_write",
    "forum_topic_tags_active_insert",
    "forum_topic_tags_active_update",
    "forum topic merge tag reconciliations are append-only",
    "forum_topic_merge_tag_reconciliation_update",
    "forum_topic_merge_tag_reconciliation_delete",
  ],
  "tag reconciliation migration",
);

includesAll(
  lock,
  [
    "lock_active_topic_tag_write_in_tx",
    "lock_topic_rows_for_tags_in_tx",
    "lock_topic_tag_scopes_in_tx",
    "FOR SHARE",
    "forum-topic-tag:",
    "forum_topic_tag_locks",
    "TopicStatus::Archived",
    "cannot be changed after topic archival",
  ],
  "tag lock owner",
);
includesAll(
  topicInline,
  [
    "pub const MAX_FORUM_TOPIC_TAGS: usize = 100;",
    "validate_normalized_topic_tags(&normalized_tags)?;",
    "lock_active_topic_tag_write_in_tx(&txn, tenant_id, topic_id).await?;",
    "lock_topic_tag_scopes_in_tx(&txn, tenant_id, &[topic_id]).await?;",
    "Forum topic tags must not exceed {MAX_FORUM_TOPIC_TAGS} entries",
  ],
  "canonical topic tag writes",
);
const updateStart = topicInline.indexOf("pub(crate) async fn update_with_inline_relations(");
const updateTagLock = topicInline.indexOf(
  "lock_active_topic_tag_write_in_tx(&txn, tenant_id, topic_id).await?;",
  updateStart,
);
const updateTopicMutation = topicInline.indexOf(
  "let mut active: forum_topic::ActiveModel",
  updateStart,
);
const updateTagSync = topicInline.indexOf(
  "self.sync_topic_tags_in_tx(&txn, tenant_id, topic_id, &locale, tags)",
  updateStart,
);
assert.ok(updateStart >= 0 && updateTagLock > updateStart);
assert.ok(updateTagLock < updateTopicMutation);
assert.ok(updateTopicMutation < updateTagSync);

includesAll(
  owner,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_TAGS: u64 = 500;",
    "pub const MAX_FORUM_TOPIC_MERGE_TAG_REASON_LEN: usize = 500;",
    "pub struct ReconcileForumTopicMergeTagsInput",
    "pub struct ForumTopicMergeTagReconciliationResult",
    "pub struct ForumTopicMergeTagReconciliationService",
    "pub async fn reconcile_merge_tags(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_tag_reconciliation::Entity::find_by_id",
    "TopicMergeTagReconciliationConflict",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_merge_event_in_tx",
    "lock_topic_rows_for_tags_in_tx",
    "lock_topic_tag_scopes_in_tx",
    "source.status != TopicStatus::Archived || !source.is_locked",
    "target.status == TopicStatus::Archived",
    "MAX_FORUM_TOPIC_MERGE_TAGS + 1",
    "order_by_asc(forum_topic_tag::Column::TermId)",
    "move_source_row_in_tx",
    "delete_source_row_in_tx",
    "ensure_source_tags_empty_in_tx",
    '"forum.topic.merge_tags_reconciled"',
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_tag_reconciliation::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "txn.commit().await?;",
  ],
  "tag reconciliation owner",
);
assert.ok(
  owner.indexOf("forum_topic_merge_tag_reconciliation::Entity::find_by_id") <
    owner.indexOf("forum_topic_merge_operation::Entity::find_by_id"),
  "exact replay must precede current merge lookup",
);
assert.ok(
  owner.indexOf("lock_topic_rows_for_tags_in_tx") <
    owner.lastIndexOf("lock_topic_tag_scopes_in_tx"),
  "first execution must lock topic rows before tag scopes",
);
const sourceEmptiness = owner.indexOf("ensure_source_tags_empty_in_tx");
const semanticEvent = owner.indexOf("forum_domain_event::ActiveModel");
const receipt = owner.indexOf("forum_topic_merge_tag_reconciliation::ActiveModel");
const firstInvalidation = owner.indexOf("publish_forum_topic_projection_in_tx");
assert.ok(sourceEmptiness >= 0 && sourceEmptiness < semanticEvent);
assert.ok(semanticEvent < receipt && receipt < firstInvalidation);
assert.equal(
  owner.match(/publish_forum_topic_projection_in_tx\(/g)?.length,
  2,
  "owner must publish exactly source and target topic invalidations",
);
for (const forbidden of [
  "TaxonomyService",
  "ensure_terms_for_module_in_tx",
  "taxonomy_term::Entity::delete",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!owner.includes(forbidden), `owner contains forbidden marker: ${forbidden}`);
}

includesAll(
  servicesMod,
  [
    "mod topic_merge_tag_reconciliation;",
    "mod topic_tag_lock;",
    "ForumTopicMergeTagReconciliationResult",
    "ForumTopicMergeTagReconciliationService",
    "ReconcileForumTopicMergeTagsInput",
    "MAX_FORUM_TOPIC_MERGE_TAG_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_TAGS",
    "MAX_FORUM_TOPIC_TAGS",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeTagReconciliationResult",
    "ForumTopicMergeTagReconciliationService",
    "ReconcileForumTopicMergeTagsInput",
    "MAX_FORUM_TOPIC_MERGE_TAG_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_TAGS",
    "MAX_FORUM_TOPIC_TAGS",
  ],
  "crate exports",
);

includesAll(
  test,
  [
    "merge_tag_reconciliation_is_atomic_idempotent_and_preserves_relation_identity",
    "merge_tag_reconciliation_requires_a_real_merge_receipt",
    "ForumTopicMergeService",
    "ForumTopicMergeTagReconciliationService",
    "source_tag_count, 3",
    "moved_source_only_count, 1",
    "deduplicated_existing_count, 2",
    "assert_eq!(moved_after.id, moved_before.id)",
    "assert_eq!(moved_after.created_at, moved_before.created_at)",
    "TopicMergeTagReconciliationConflict",
    "UPDATE forum_topic_merge_tag_reconciliations",
    "DELETE FROM forum_topic_merge_tag_reconciliations",
    '"forum.topic.merge_tags_reconciled"',
    "new_projection_ids.len(), 2",
  ],
  "SQLite regression",
);
includesAll(
  docs,
  [
    "# FORUM-21E topic merge tag reconciliation",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "Set-union policy",
    "same row `id`",
    "At most 500",
    "Tags are Forum Search facets",
    "FORUM_TOPIC_MERGE_TAG_RECONCILIATION_CONFLICT",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21E handoff",
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
  "FORUM-21E topic merge tag reconciliation source is ready and canonical FORUM-21 remains planned.",
);
