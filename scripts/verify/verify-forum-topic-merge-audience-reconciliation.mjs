#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract:
    "crates/rustok-forum/contracts/forum-topic-merge-audience-reconciliation.json",
  docs: "crates/rustok-forum/docs/forum-21g-topic-merge-audience-reconciliation.md",
  entity:
    "crates/rustok-forum/src/entities/forum_topic_merge_audience_reconciliation.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000015_add_forum_topic_merge_audience_reconciliations.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_merge_audience_reconciliation.rs",
  audience: "crates/rustok-forum/src/services/topic_audience.rs",
  audienceOwner: "crates/rustok-forum/src/services/topic_audience_owner.rs",
  audienceLock: "crates/rustok-forum/src/services/topic_audience_lock.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  merge: "crates/rustok-forum/src/services/topic_merge.rs",
  test: "crates/rustok-forum/tests/topic_merge_audience_reconciliation_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-audience-reconciliation.mjs",
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
const audience = read(paths.audience);
const audienceOwner = read(paths.audienceOwner);
const audienceLock = read(paths.audienceLock);
const servicesMod = read(paths.servicesMod);
const merge = read(paths.merge);
const test = read(paths.test);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_merge_audience_reconciliation_v1");
assert.equal(contract.task, "FORUM-21G");
assert.equal(contract.parent_task, "FORUM-21");
assert.deepEqual(contract.depends_on, ["FORUM-20", "FORUM-21B"]);
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(
  contract.scope,
  "atomic_new_merge_audience_guard_and_fail_closed_historical_reconciliation",
);
assert.equal(
  contract.owner_service,
  "ForumTopicMergeAudienceReconciliationService",
);
assert.equal(contract.input, "ReconcileForumTopicMergeAudienceInput");
assert.equal(
  contract.result,
  "ForumTopicMergeAudienceReconciliationResult",
);
assert.equal(contract.outcome_type, "ForumTopicMergeAudienceOutcome");
assert.equal(
  contract.migration,
  "m20260803_000015_add_forum_topic_merge_audience_reconciliations",
);
assert.equal(
  contract.receipt_table,
  "forum_topic_merge_audience_reconciliations",
);
assert.equal(contract.source_merge_receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  roles_max: 4,
  channels_max: 32,
  groups_max: 32,
  allow_users_max: 100,
  deny_users_max: 100,
  trust_level_max: 100,
  one_reconciliation_per_merge_receipt: true,
});
assert.deepEqual(contract.new_merge_commit_guard.rollback_scope, [
  "reply_moves",
  "topic_status_and_counters",
  "forum.topic.merged_event",
  "merge_receipt",
  "projection_invalidations",
]);
assert.equal(
  contract.new_merge_commit_guard.boundary,
  "BEFORE INSERT ON forum_topic_merge_operations",
);
assert.equal(contract.new_merge_commit_guard.same_transaction_as_forum_21b, true);
assert.equal(contract.new_merge_commit_guard.shared_audience_scope_seed, 5);
assert.equal(
  contract.new_merge_commit_guard.stable_code,
  "FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT",
);
assert.deepEqual(Object.keys(contract.historical_safe_outcomes), [
  "both_unrestricted",
  "target_only_preserved",
  "source_only_moved",
  "equal_layers_deduplicated",
]);
assert.equal(
  contract.historical_policy_conflict.stable_code,
  "FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT",
);
assert.equal(contract.ordinary_write_hardening.length, 4);
assert.equal(contract.transactional_invariants.length, 17);
assert.equal(contract.database_guards.length, 9);
assert.deepEqual(
  contract.search_projection.successful_historical_first_execution_invalidations,
  ["source_topic", "target_topic"],
);
assert.equal(contract.search_projection.new_merge_guard_conflict_invalidations, 0);
assert.equal(contract.search_projection.historical_conflict_invalidations, 0);
assert.equal(contract.search_projection.exact_replay_invalidations, 0);
assert.equal(contract.test, paths.test);
assert.equal(contract.verifier, paths.verifier);
assert.equal(contract.documentation, paths.docs);
assert.deepEqual(contract.maintainer_commands, [
  `node ${paths.verifier}`,
  "cargo test -p rustok-forum --test topic_merge_audience_reconciliation_sqlite -- --nocapture",
]);

includesAll(
  entity,
  [
    "pub enum ForumTopicMergeAudienceOutcome",
    '#[sea_orm(string_value = "both_unrestricted")]',
    '#[sea_orm(string_value = "target_only_preserved")]',
    '#[sea_orm(string_value = "source_only_moved")]',
    '#[sea_orm(string_value = "equal_layers_deduplicated")]',
    '#[sea_orm(table_name = "forum_topic_merge_audience_reconciliations")]',
    "pub merge_operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub outcome: ForumTopicMergeAudienceOutcome",
    "pub event_id: Uuid",
  ],
  "audience reconciliation entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_audience_reconciliation;",
    "ForumTopicMergeAudienceReconciliationEntity",
    "ForumTopicMergeAudienceOutcome",
  ],
  "entity exports",
);
includesAll(
  error,
  [
    "TopicMergeAudienceReconciliationConflict(Uuid)",
    "TopicMergeAudiencePolicyConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_AUDIENCE_RECONCILIATION_CONFLICT"',
    '"FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT"',
    'message.contains("forum topic merge audience policy conflict")',
    "TopicMergeAudiencePolicyConflict(Uuid::nil())",
  ],
  "ForumError",
);
assert.ok(
  error.includes(
    "Forum category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA",
  ),
  "unrelated category color validation must remain unchanged",
);
includesAll(
  migrationsMod,
  [
    "mod m20260803_000015_add_forum_topic_merge_audience_reconciliations;",
    "m20260803_000015_add_forum_topic_merge_audience_reconciliations::Migration,",
  ],
  "migration registry",
);

includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_audience_reconciliation_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_audience_reconciliations",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "UNIQUE (tenant_id, merge_operation_id)",
    "both_unrestricted",
    "target_only_preserved",
    "source_only_moved",
    "equal_layers_deduplicated",
    "forum_lock_topic_audience_mutation",
    "forum_validate_topic_merge_audience_compatibility",
    "forum_05_topic_merge_audience_compatibility",
    "BEFORE INSERT ON forum_topic_merge_operations",
    "source_policy.minimum_trust_level",
    "IS NOT DISTINCT FROM target_policy.minimum_trust_level",
    "source_policy.minimum_trust_level IS target_policy.minimum_trust_level",
    "SELECT role",
    "SELECT channel_slug",
    "SELECT group_id",
    "SELECT user_id, effect",
    "EXCEPT",
    "forum topic merge audience policy conflict",
    "forum_00_topic_audience_policy_scope",
    "forum_00_topic_audience_role_scope",
    "forum_00_topic_audience_channel_scope",
    "forum_00_topic_audience_group_scope",
    "forum_00_topic_audience_user_scope",
    "forum_reject_archived_topic_audience_insert",
    "forum_10_topic_audience_policy_active_insert",
    "forum_10_topic_audience_role_active_insert",
    "forum_10_topic_audience_channel_active_insert",
    "forum_10_topic_audience_group_active_insert",
    "forum_10_topic_audience_user_active_insert",
    "forum topic audience cannot target archived or deleted topics",
    "forum topic merge audience reconciliations are append-only",
  ],
  "audience reconciliation migration",
);
assert.ok(migration.includes("format('%s:%s', NEW.tenant_id, first_topic_id)"));
assert.ok(migration.includes("        5\n    ));"));
assert.ok(
  migration.indexOf("forum_validate_topic_merge_audience_compatibility") <
    migration.indexOf("CREATE TRIGGER forum_05_topic_merge_audience_compatibility"),
);
for (const table of [
  "forum_topic_audience_policies",
  "forum_topic_audience_roles",
  "forum_topic_audience_channels",
  "forum_topic_audience_groups",
  "forum_topic_audience_users",
]) {
  assert.ok(migration.includes(`ON ${table}`), `migration does not guard ${table}`);
}

includesAll(
  audienceLock,
  [
    "lock_active_topic_audience_write_in_tx",
    "lock_topic_rows_for_audience_in_tx",
    "lock_topic_audience_scopes_in_tx",
    "deleted_at IS NULL FOR SHARE",
    "deleted_at IS NULL",
    "hashtextextended($1, 5)",
    'format!("{tenant_id}:{topic_id}")',
    "TopicStatus::Archived",
    "cannot be changed after topic archival",
  ],
  "topic audience lock owner",
);
includesAll(
  audienceOwner,
  [
    "ForumTopicAudiencePolicyOwnerService",
    "lock_category_tree_in_tx(&txn, tenant_id).await?;",
    "lock_active_topic_audience_write_in_tx",
    "lock_topic_audience_scopes_in_tx",
    "forum_topic_audience_policy::Entity::delete_many()",
    "publish_forum_topic_projection_direct_in_tx",
    "txn.commit().await?;",
  ],
  "public topic audience owner",
);
const ownerSet = audienceOwner.indexOf("pub async fn set(");
const categoryLock = audienceOwner.indexOf(
  "lock_category_tree_in_tx(&txn, tenant_id)",
  ownerSet,
);
const activeTopicLock = audienceOwner.indexOf(
  "lock_active_topic_audience_write_in_tx",
  ownerSet,
);
const audienceScopeLock = audienceOwner.indexOf(
  "lock_topic_audience_scopes_in_tx",
  ownerSet,
);
const ownerDelete = audienceOwner.indexOf(
  "forum_topic_audience_policy::Entity::delete_many()",
  ownerSet,
);
const ownerInvalidation = audienceOwner.indexOf(
  "publish_forum_topic_projection_direct_in_tx",
  ownerSet,
);
const ownerCommit = audienceOwner.indexOf("txn.commit().await?;", ownerSet);
assert.ok(ownerSet >= 0 && ownerSet < categoryLock);
assert.ok(categoryLock < activeTopicLock);
assert.ok(activeTopicLock < audienceScopeLock);
assert.ok(audienceScopeLock < ownerDelete);
assert.ok(ownerDelete < ownerInvalidation && ownerInvalidation < ownerCommit);
assert.ok(
  servicesMod.includes(
    "ForumTopicAudiencePolicyOwnerService as ForumTopicAudiencePolicyService",
  ),
  "public topic audience service must remain the transactional owner facade",
);
includesAll(
  audience,
  [
    "positive selectors form a union",
    "explicit deny always wins",
    "load_topic_layer",
    "constraints.normalize()?",
  ],
  "canonical audience semantics",
);

includesAll(
  owner,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN: usize = 500;",
    "pub struct ReconcileForumTopicMergeAudienceInput",
    "pub struct ForumTopicMergeAudienceReconciliationResult",
    "pub struct ForumTopicMergeAudienceReconciliationService",
    "pub async fn reconcile_merge_audience(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_audience_reconciliation::Entity::find_by_id",
    "TopicMergeAudienceReconciliationConflict",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_merge_event_in_tx",
    "lock_topic_rows_for_audience_in_tx",
    "lock_topic_audience_scopes_in_tx",
    "source.status != TopicStatus::Archived || !source.is_locked",
    "target.status == TopicStatus::Archived",
    "load_local_layer(&txn, tenant_id, merge.source_topic_id)",
    "load_local_layer(&txn, tenant_id, merge.target_topic_id)",
    "ForumTopicMergeAudienceOutcome::BothUnrestricted",
    "ForumTopicMergeAudienceOutcome::TargetOnlyPreserved",
    "ForumTopicMergeAudienceOutcome::SourceOnlyMoved",
    "ForumTopicMergeAudienceOutcome::EqualLayersDeduplicated",
    "source_constraints == target_constraints",
    "TopicMergeAudiencePolicyConflict",
    "insert_local_layer_in_tx",
    "delete_local_layer_in_tx",
    "ensure_source_audience_empty_in_tx",
    '"forum.topic.merge_audience_reconciled"',
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_audience_reconciliation::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "txn.commit().await?;",
  ],
  "historical merge audience reconciliation owner",
);
assert.ok(
  owner.indexOf("forum_topic_merge_audience_reconciliation::Entity::find_by_id") <
    owner.indexOf("forum_topic_merge_operation::Entity::find_by_id"),
  "exact replay must precede current merge lookup",
);
assert.equal(
  owner.match(/publish_forum_topic_projection_in_tx\(/g)?.length,
  2,
  "successful historical repair must publish source and target invalidations",
);
const semanticEvent = owner.indexOf("forum_domain_event::ActiveModel");
const receipt = owner.indexOf(
  "forum_topic_merge_audience_reconciliation::ActiveModel",
);
const firstInvalidation = owner.indexOf("publish_forum_topic_projection_in_tx");
assert.ok(owner.lastIndexOf("ensure_source_audience_empty_in_tx") < semanticEvent);
assert.ok(semanticEvent < receipt && receipt < firstInvalidation);
assert.ok(owner.indexOf("TopicMergeAudiencePolicyConflict") < semanticEvent);
for (const unsafe of [
  "roles_any.extend",
  "channel_members_any.extend",
  "group_members_any.extend",
  "allow_user_ids.extend",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!owner.includes(unsafe), `unsafe audience merge marker: ${unsafe}`);
}

includesAll(
  servicesMod,
  [
    "mod topic_audience_lock;",
    "mod topic_merge_audience_reconciliation;",
    "ForumTopicMergeAudienceReconciliationResult",
    "ForumTopicMergeAudienceReconciliationService",
    "ReconcileForumTopicMergeAudienceInput",
    "MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeAudienceReconciliationResult",
    "ForumTopicMergeAudienceReconciliationService",
    "ReconcileForumTopicMergeAudienceInput",
    "MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN",
  ],
  "crate exports",
);
includesAll(
  merge,
  [
    "ForumTopicMergeService",
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "txn.commit().await?;",
  ],
  "FORUM-21B merge transaction",
);
assert.ok(
  merge.indexOf("forum_domain_event::ActiveModel") <
    merge.indexOf("forum_topic_merge_operation::ActiveModel"),
  "receipt guard must run after in-transaction merge mutations but before commit",
);
assert.ok(
  merge.indexOf("forum_topic_merge_operation::ActiveModel") <
    merge.indexOf("txn.commit().await?;"),
);

includesAll(
  test,
  [
    "setup_before_forum_21g",
    "apply_forum_21g",
    "forum_migrations.pop()",
    "historical_merge_audience_reconciliation_moves_source_only_layer_and_is_idempotent",
    "historical_merge_audience_reconciliation_rejects_different_dual_layers_atomically",
    "topic_merge_rejects_incompatible_source_audience_before_commit",
    "merge_audience_reconciliation_requires_a_real_merge_receipt",
    "ForumTopicMergeService",
    "ForumTopicAudiencePolicyService",
    "ForumTopicMergeAudienceReconciliationService",
    "ForumTopicMergeAudienceOutcome::SourceOnlyMoved",
    "assert_archived_audience_database_guard",
    "policy_updated_at",
    "assert_source_audience_empty",
    "TopicMergeAudienceReconciliationConflict",
    "TopicMergeAudiencePolicyConflict",
    "UPDATE forum_topic_merge_audience_reconciliations",
    "DELETE FROM forum_topic_merge_audience_reconciliations",
    '"forum.topic.merge_audience_reconciled"',
    "new_projection_ids.len(), 2",
    "merge_receipt_count(&db, tenant_id, merge_operation_id).await?, 0",
    "merge_event_count(&db, tenant_id).await?, merge_event_count_before",
    "topic_status(&db, tenant_id, source_topic_id).await?, \"open\"",
    "topic_status(&db, tenant_id, target_topic_id).await?, \"open\"",
    "projection_root_ids(&db, tenant_id).await?, baseline_projection_ids",
  ],
  "SQLite handoff",
);
assert.ok(
  test.indexOf("ForumTopicMergeService::new", test.indexOf("historical_merge_audience")) <
    test.indexOf("apply_forum_21g", test.indexOf("historical_merge_audience")),
  "historical fixtures must commit before FORUM-21G migration",
);

includesAll(
  docs,
  [
    "# FORUM-21G topic merge audience reconciliation",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "Why arbitrary union is unsafe",
    "New merge commit guard",
    "`BEFORE INSERT` guard on",
    "No committed interval exists",
    "Historical safe outcome matrix",
    "`both_unrestricted`",
    "`target_only_preserved`",
    "`source_only_moved`",
    "`equal_layers_deduplicated`",
    "`FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT`",
    "creates no audience mutation",
    "preserving the policy row's `updated_at` value",
    "publishes source and retained-target topic invalidations",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21G handoff",
);

includesAll(
  plan,
  [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "## `FORUM-21` — move, merge, split and fork topics",
    "**Status:** `planned`",
    "revalidate solutions and ACL",
  ],
  "canonical FORUM-21 plan",
);
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21G merge audience commit guard and historical reconciliation source are ready; canonical FORUM-21 remains planned.",
);
