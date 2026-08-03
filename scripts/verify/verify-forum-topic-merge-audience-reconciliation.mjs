#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const p = {
  contract:
    "crates/rustok-forum/contracts/forum-topic-merge-audience-reconciliation.json",
  docs: "crates/rustok-forum/docs/forum-21g-topic-merge-audience-reconciliation.md",
  entity:
    "crates/rustok-forum/src/entities/forum_topic_merge_audience_reconciliation.rs",
  entities: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000015_add_forum_topic_merge_audience_reconciliations.rs",
  migrations: "crates/rustok-forum/src/migrations/mod.rs",
  service: "crates/rustok-forum/src/services/topic_merge_audience_reconciliation.rs",
  audience: "crates/rustok-forum/src/services/topic_audience.rs",
  audienceOwner: "crates/rustok-forum/src/services/topic_audience_owner.rs",
  audienceLock: "crates/rustok-forum/src/services/topic_audience_lock.rs",
  services: "crates/rustok-forum/src/services/mod.rs",
  merge: "crates/rustok-forum/src/services/topic_merge.rs",
  test: "crates/rustok-forum/tests/topic_merge_audience_reconciliation_sqlite.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-audience-reconciliation.mjs",
};

const read = (path) => readFileSync(path, "utf8");
const contains = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} missing: ${marker}`);
  }
};

const contract = JSON.parse(read(p.contract));
const docs = read(p.docs);
const entity = read(p.entity);
const entities = read(p.entities);
const error = read(p.error);
const lib = read(p.lib);
const migration = read(p.migration);
const migrations = read(p.migrations);
const service = read(p.service);
const audience = read(p.audience);
const audienceOwner = read(p.audienceOwner);
const audienceLock = read(p.audienceLock);
const services = read(p.services);
const merge = read(p.merge);
const test = read(p.test);
const plan = read(p.plan);
const verifier = read(p.verifier);

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
  contract.new_merge_commit_guard.boundary,
  "BEFORE INSERT ON forum_topic_merge_operations",
);
assert.equal(contract.new_merge_commit_guard.same_transaction_as_forum_21b, true);
assert.equal(contract.new_merge_commit_guard.shared_audience_scope_seed, 5);
assert.equal(
  contract.new_merge_commit_guard.stable_code,
  "FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT",
);
assert.deepEqual(contract.new_merge_commit_guard.rollback_scope, [
  "reply_moves",
  "topic_status_and_counters",
  "forum.topic.merged_event",
  "merge_receipt",
  "projection_invalidations",
]);
assert.deepEqual(Object.keys(contract.historical_safe_outcomes), [
  "both_unrestricted",
  "target_only_preserved",
  "source_only_moved",
  "equal_layers_deduplicated",
]);
assert.equal(contract.transactional_invariants.length, 17);
assert.equal(contract.database_guards.length, 9);
assert.equal(contract.test, p.test);
assert.equal(contract.verifier, p.verifier);
assert.equal(contract.documentation, p.docs);
assert.deepEqual(contract.maintainer_commands, [
  `node ${p.verifier}`,
  "cargo test -p rustok-forum --test topic_merge_audience_reconciliation_sqlite -- --nocapture",
]);

contains(
  entity,
  [
    "pub enum ForumTopicMergeAudienceOutcome",
    'string_value = "both_unrestricted"',
    'string_value = "target_only_preserved"',
    'string_value = "source_only_moved"',
    'string_value = "equal_layers_deduplicated"',
    'table_name = "forum_topic_merge_audience_reconciliations"',
    "pub outcome: ForumTopicMergeAudienceOutcome",
  ],
  "entity",
);
contains(
  entities,
  [
    "pub mod forum_topic_merge_audience_reconciliation;",
    "ForumTopicMergeAudienceReconciliationEntity",
    "ForumTopicMergeAudienceOutcome",
  ],
  "entity registry",
);
contains(
  error,
  [
    "TopicMergeAudienceReconciliationConflict(Uuid)",
    "TopicMergeAudiencePolicyConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_AUDIENCE_RECONCILIATION_CONFLICT"',
    '"FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT"',
    'message.contains("forum topic merge audience policy conflict")',
    "TopicMergeAudiencePolicyConflict(Uuid::nil())",
    "Forum category color must use #RGB, #RGBA, #RRGGBB, or #RRGGBBAA",
  ],
  "ForumError",
);
contains(
  migrations,
  [
    "mod m20260803_000015_add_forum_topic_merge_audience_reconciliations;",
    "m20260803_000015_add_forum_topic_merge_audience_reconciliations::Migration,",
  ],
  "migration registry",
);

contains(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_audience_reconciliations",
    "UNIQUE (tenant_id, merge_operation_id)",
    "forum_lock_topic_audience_mutation",
    "forum_validate_topic_merge_audience_compatibility",
    "forum_05_topic_merge_audience_compatibility",
    "BEFORE INSERT ON forum_topic_merge_operations",
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
    "forum topic audience cannot target archived or deleted topics",
    "forum topic merge audience reconciliations are append-only",
  ],
  "migration",
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
  assert.ok(migration.includes(`ON ${table}`), `missing table guard: ${table}`);
}

contains(
  audienceLock,
  [
    "lock_active_topic_audience_write_in_tx",
    "lock_topic_rows_for_audience_in_tx",
    "lock_topic_audience_scopes_in_tx",
    "deleted_at IS NULL FOR SHARE",
    "hashtextextended($1, 5)",
    'format!("{tenant_id}:{topic_id}")',
    "cannot be changed after topic archival",
  ],
  "audience lock",
);
contains(
  audienceOwner,
  [
    "lock_category_tree_in_tx(&txn, tenant_id).await?;",
    "lock_active_topic_audience_write_in_tx",
    "lock_topic_audience_scopes_in_tx",
    "forum_topic_audience_policy::Entity::delete_many()",
    "publish_forum_topic_projection_direct_in_tx",
    "txn.commit().await?;",
  ],
  "audience owner",
);
const setStart = audienceOwner.indexOf("pub async fn set(");
const treeLock = audienceOwner.indexOf("lock_category_tree_in_tx", setStart);
const topicLock = audienceOwner.indexOf("lock_active_topic_audience_write_in_tx", setStart);
const scopeLock = audienceOwner.indexOf("lock_topic_audience_scopes_in_tx", setStart);
const mutation = audienceOwner.indexOf(
  "forum_topic_audience_policy::Entity::delete_many()",
  setStart,
);
const invalidation = audienceOwner.indexOf(
  "publish_forum_topic_projection_direct_in_tx",
  setStart,
);
const commit = audienceOwner.indexOf("txn.commit().await?;", setStart);
assert.ok(setStart < treeLock && treeLock < topicLock);
assert.ok(topicLock < scopeLock && scopeLock < mutation);
assert.ok(mutation < invalidation && invalidation < commit);
assert.ok(
  services.includes(
    "ForumTopicAudiencePolicyOwnerService as ForumTopicAudiencePolicyService",
  ),
);
contains(
  audience,
  ["positive selectors form a union", "explicit deny always wins", "load_topic_layer"],
  "audience semantics",
);

contains(
  service,
  [
    "pub struct ForumTopicMergeAudienceReconciliationService",
    "pub async fn reconcile_merge_audience(",
    "lock_reconciliation_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_audience_reconciliation::Entity::find_by_id",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_merge_event_in_tx",
    "lock_topic_rows_for_audience_in_tx",
    "lock_topic_audience_scopes_in_tx",
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
  "historical repair owner",
);
assert.ok(
  service.indexOf("forum_topic_merge_audience_reconciliation::Entity::find_by_id") <
    service.indexOf("forum_topic_merge_operation::Entity::find_by_id"),
);
assert.equal(service.match(/publish_forum_topic_projection_in_tx\(/g)?.length, 2);
const eventInsert = service.indexOf("forum_domain_event::ActiveModel");
const receiptInsert = service.indexOf(
  "forum_topic_merge_audience_reconciliation::ActiveModel",
);
const projectionInsert = service.indexOf("publish_forum_topic_projection_in_tx");
const sourceProof = service.lastIndexOf(
  "ensure_source_audience_empty_in_tx(&txn",
  eventInsert,
);
assert.ok(sourceProof >= 0 && sourceProof < eventInsert);
assert.ok(eventInsert < receiptInsert && receiptInsert < projectionInsert);
assert.ok(service.indexOf("TopicMergeAudiencePolicyConflict") < eventInsert);
for (const unsafe of [
  "roles_any.extend",
  "channel_members_any.extend",
  "group_members_any.extend",
  "allow_user_ids.extend",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!service.includes(unsafe), `unsafe merge marker: ${unsafe}`);
}

contains(
  services,
  [
    "mod topic_audience_lock;",
    "mod topic_merge_audience_reconciliation;",
    "ForumTopicMergeAudienceReconciliationService",
    "ReconcileForumTopicMergeAudienceInput",
    "MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN",
  ],
  "service exports",
);
contains(
  lib,
  [
    "ForumTopicMergeAudienceReconciliationResult",
    "ForumTopicMergeAudienceReconciliationService",
    "ReconcileForumTopicMergeAudienceInput",
    "MAX_FORUM_TOPIC_MERGE_AUDIENCE_REASON_LEN",
  ],
  "crate exports",
);
contains(
  merge,
  [
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "txn.commit().await?;",
  ],
  "FORUM-21B transaction",
);
assert.ok(
  merge.indexOf("forum_domain_event::ActiveModel") <
    merge.indexOf("forum_topic_merge_operation::ActiveModel"),
);
assert.ok(
  merge.indexOf("forum_topic_merge_operation::ActiveModel") <
    merge.indexOf("txn.commit().await?;"),
);

contains(
  test,
  [
    "setup_before_forum_21g",
    "apply_forum_21g",
    ".pop()",
    "historical_merge_audience_reconciliation_moves_source_only_layer_and_is_idempotent",
    "historical_merge_audience_reconciliation_rejects_different_dual_layers_atomically",
    "topic_merge_rejects_incompatible_source_audience_before_commit",
    "merge_audience_reconciliation_requires_a_real_merge_receipt",
    "ForumTopicMergeAudienceOutcome::SourceOnlyMoved",
    "TopicMergeAudienceReconciliationConflict",
    "TopicMergeAudiencePolicyConflict",
    "new_projection_ids.len(), 2",
    "merge_receipt_count(&db, tenant_id, merge_operation_id).await?, 0",
    "merge_event_count(&db, tenant_id).await?, merge_event_count_before",
    "topic_status(&db, tenant_id, source_topic_id).await?, \"open\"",
    "topic_status(&db, tenant_id, target_topic_id).await?, \"open\"",
    "projection_root_ids(&db, tenant_id).await?, baseline_projection_ids",
  ],
  "SQLite handoff",
);
const historicalStart = test.indexOf(
  "historical_merge_audience_reconciliation_moves_source_only_layer_and_is_idempotent",
);
assert.ok(
  test.indexOf("ForumTopicMergeService::new", historicalStart) <
    test.indexOf("apply_forum_21g(&db", historicalStart),
  "historical receipt must predate migration 000015",
);

contains(
  docs,
  [
    "# FORUM-21G topic merge audience reconciliation",
    "`source_ready_maintainer_execution_pending`",
    p.contract,
    "Why arbitrary union is unsafe",
    "New merge commit guard",
    "No committed interval exists",
    "Historical safe outcome matrix",
    "`FORUM_TOPIC_MERGE_AUDIENCE_POLICY_CONFLICT`",
    "preserving the policy row's `updated_at` value",
    "No command above was run by the implementation agent",
  ],
  "handoff",
);
contains(
  plan,
  [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "## `FORUM-21` — move, merge, split and fork topics",
    "**Status:** `planned`",
    "revalidate solutions and ACL",
  ],
  "canonical plan",
);
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21G merge audience guard and historical reconciliation source are ready; FORUM-21 remains planned.",
);
