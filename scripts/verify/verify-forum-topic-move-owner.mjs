#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = process.cwd();
const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-move-owner.json",
  migration:
    "crates/rustok-forum/src/migrations/m20260801_000009_add_forum_topic_move_operations.rs",
  migrationMod: "crates/rustok-forum/src/migrations/mod.rs",
  entity: "crates/rustok-forum/src/entities/forum_topic_move_operation.rs",
  entityMod: "crates/rustok-forum/src/entities/mod.rs",
  service: "crates/rustok-forum/src/services/topic_move.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  error: "crates/rustok-forum/src/error.rs",
  test: "crates/rustok-forum/tests/topic_move_sqlite.rs",
  doc: "crates/rustok-forum/docs/forum-21a-topic-move-owner.md",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(resolve(root, path), "utf8");
const requireIncludes = (source, fragments, label) => {
  for (const fragment of fragments) {
    assert.ok(source.includes(fragment), `${label} is missing: ${fragment}`);
  }
};
const requireArrayIncludes = (actual, expected, label) => {
  assert.ok(Array.isArray(actual), `${label} must be an array`);
  for (const value of expected) {
    assert.ok(actual.includes(value), `${label} is missing: ${value}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const migration = read(paths.migration);
const migrationMod = read(paths.migrationMod);
const entity = read(paths.entity);
const entityMod = read(paths.entityMod);
const service = read(paths.service);
const servicesMod = read(paths.servicesMod);
const lib = read(paths.lib);
const error = read(paths.error);
const test = read(paths.test);
const doc = read(paths.doc);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_move_owner_v1");
assert.equal(contract.task, "FORUM-21A");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(
  contract.canonical_plan_promotion_requires,
  "maintainer_verifier_and_sqlite_execution",
);
assert.equal(contract.scope, "single_active_topic_category_move_only");
assert.equal(contract.owner_service, "ForumTopicMoveService");
assert.equal(contract.input, "MoveForumTopicInput");
assert.equal(contract.result, "ForumTopicMoveResult");
assert.equal(
  contract.migration,
  "m20260801_000009_add_forum_topic_move_operations",
);
assert.equal(contract.receipt_table, "forum_topic_move_operations");
assert.deepEqual(contract.semantic_event, {
  journal: "forum_domain_events",
  event_type: "forum.topic.moved",
  schema_version: 1,
  event_id_equals_operation_id: true,
  shared_rustok_events_contract_changed: false,
});
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.bounds, {
  reason_max_characters: 500,
  one_topic_per_operation: true,
  one_source_category_per_operation: true,
  one_target_category_per_operation: true,
});
assert.equal(contract.test, paths.test);
assert.equal(contract.verifier, "scripts/verify/verify-forum-topic-move-owner.mjs");
assert.equal(contract.documentation, paths.doc);
assert.deepEqual(contract.maintainer_commands, [
  "node scripts/verify/verify-forum-topic-move-owner.mjs",
  "cargo test -p rustok-forum --test topic_move_sqlite -- --nocapture",
]);
requireArrayIncludes(
  contract.transactional_invariants,
  [
    "topic moves are serialized per tenant before receipt lookup or owner-state mutation",
    "the exact existing operation receipt is resolved before current topic or category state so a valid retry returns the original result",
    "reusing an operation identifier with a different topic target actor or normalized reason fails with FORUM_TOPIC_MOVE_OPERATION_CONFLICT",
    "the source and target categories must exist in the tenant and both remain active",
    "a current solution must still reference one approved reply owned by the moved topic",
    "source and target category topic and published-reply counters transfer with checked arithmetic and never clamp",
    "topic category mutation counter transfer semantic event receipt and three projection invalidations commit in one transaction",
    "projection invalidations are published for the topic then source category then target category",
    "the operation receipt and semantic event are immutable and exact replay creates no additional event invalidation or counter change",
  ],
  "transactional_invariants",
);
requireArrayIncludes(
  contract.non_claims,
  [
    "this source-ready slice does not claim that the verifier SQLite test Cargo checks formatting workflows or CI ran",
    "this source-ready slice does not promote the canonical FORUM-21 status before maintainer execution",
    "this slice does not add a public REST GraphQL native admin or storefront topic-move transport",
    "this slice does not create canonical URL aliases redirects or route tombstones",
    "this slice does not merge split fork or move reply ranges",
    "this slice does not provide PostgreSQL concurrency runtime evidence",
    "this slice is not sufficient to mark FORUM-21 done or LINK-FORUM-03 complete",
  ],
  "non_claims",
);

requireIncludes(
  migrationMod,
  [
    "mod m20260801_000009_add_forum_topic_move_operations;",
    "Box::new(m20260801_000009_add_forum_topic_move_operations::Migration)",
  ],
  "migration registry",
);
requireIncludes(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_move_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_move_operations",
    "PRIMARY KEY (tenant_id, operation_id)",
    "UNIQUE (event_id)",
    "FOREIGN KEY (tenant_id, topic_id)",
    "FOREIGN KEY (tenant_id, source_category_id)",
    "FOREIGN KEY (tenant_id, target_category_id)",
    "FOREIGN KEY (tenant_id, actor_id)",
    "event_id = operation_id",
    "source_category_id <> target_category_id",
    "length(reason) BETWEEN 1 AND 500",
    "published_reply_count >= 0",
    "forum topic move operations are append-only",
    "DatabaseBackend::Postgres",
    "DatabaseBackend::Sqlite",
  ],
  "topic move migration",
);
assert.equal(
  (migration.match(/CREATE TABLE IF NOT EXISTS forum_topic_move_locks/g) ?? []).length,
  2,
);
assert.equal(
  (migration.match(/CREATE TABLE IF NOT EXISTS forum_topic_move_operations/g) ?? []).length,
  2,
);
assert.ok(
  (migration.match(/forum_topic_move_operation_update/g) ?? []).length >= 4 &&
    (migration.match(/forum_topic_move_operation_delete/g) ?? []).length >= 4,
  "receipt mutation guards must exist for PostgreSQL and SQLite",
);

requireIncludes(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_move_operations")]',
    "pub tenant_id: Uuid",
    "pub operation_id: Uuid",
    "pub topic_id: Uuid",
    "pub source_category_id: Uuid",
    "pub target_category_id: Uuid",
    "pub actor_id: Uuid",
    "pub reason: String",
    "pub published_reply_count: i32",
    "pub event_id: Uuid",
    "pub moved_at: DateTimeWithTimeZone",
  ],
  "topic move entity",
);
requireIncludes(
  entityMod,
  ["pub mod forum_topic_move_operation;", "ForumTopicMoveOperationEntity"],
  "entity exports",
);

requireIncludes(
  service,
  [
    "pub const MAX_FORUM_TOPIC_MOVE_REASON_LEN: usize = 500;",
    'const FORUM_TOPIC_MOVED_EVENT_TYPE: &str = "forum.topic.moved";',
    "pub struct MoveForumTopicInput",
    "pub struct ForumTopicMoveResult",
    "pub struct ForumTopicMoveService",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_topic_move_tenant_in_tx(&txn, tenant_id).await?;",
    "SELECT pg_advisory_xact_lock(hashtextextended($1, 21))",
    "INSERT INTO forum_topic_move_locks (tenant_id, touched_at)",
    "ON CONFLICT(tenant_id) DO UPDATE SET touched_at = CURRENT_TIMESTAMP",
    "forum_topic_move_operation::Entity::find_by_id((",
    "validate_existing_semantic_event_in_tx(&txn, &existing).await?;",
    "return Ok(operation_to_result(existing));",
    "ForumError::TopicMoveOperationConflict(input.operation_id)",
    "AND deleted_at IS NULL FOR UPDATE",
    "if topic.status == TopicStatus::Archived",
    "Forum topic is already assigned to the target category",
    "validate_solution_in_tx(&txn, tenant_id, topic_id).await?;",
    "reply.status == ReplyStatus::Approved",
    "load_category_audience_policy(&txn, tenant_id, input.target_category_id)",
    "checked_sub(1)",
    ".checked_sub(published_reply_count)",
    "checked_add(1)",
    ".checked_add(published_reply_count)",
    "active.category_id = Set(input.target_category_id);",
    "load_policy_for_topic(&txn, tenant_id, &moved_topic).await?;",
    "forum_domain_event::ActiveModel",
    "event_id: Set(input.operation_id)",
    "forum_topic_move_operation::ActiveModel",
    "publish_forum_topic_projection_in_tx(",
    "publish_forum_category_projection_in_tx(",
    "txn.commit().await?;",
  ],
  "topic move service",
);
assert.ok(!service.includes(".max(0)") && !service.includes("saturating_"));
assert.ok(
  !service.includes("DomainEvent::ForumTopicMoved") &&
    !service.includes("CanonicalUrlChanged") &&
    !service.includes("UrlAliasPurged"),
);
const receiptLookup = service.indexOf("forum_topic_move_operation::Entity::find_by_id((");
const topicLock = service.indexOf("lock_topic_in_tx(&txn, tenant_id, topic_id)");
assert.ok(receiptLookup >= 0 && topicLock > receiptLookup);
const topicInvalidation = service.indexOf("publish_forum_topic_projection_in_tx(");
const sourceInvalidation = service.indexOf(
  "publish_forum_category_projection_in_tx(",
  topicInvalidation,
);
const targetInvalidation = service.indexOf(
  "publish_forum_category_projection_in_tx(",
  sourceInvalidation + 1,
);
assert.ok(
  topicInvalidation >= 0 &&
    sourceInvalidation > topicInvalidation &&
    targetInvalidation > sourceInvalidation,
);

requireIncludes(
  servicesMod,
  [
    "mod topic_move;",
    "ForumTopicMoveResult, ForumTopicMoveService, MAX_FORUM_TOPIC_MOVE_REASON_LEN",
    "MoveForumTopicInput",
  ],
  "service exports",
);
requireIncludes(
  lib,
  [
    "ForumTopicMoveResult, ForumTopicMoveService",
    "MAX_FORUM_TOPIC_MOVE_REASON_LEN",
    "MoveForumTopicInput",
  ],
  "crate exports",
);
requireIncludes(
  error,
  ["TopicMoveOperationConflict(Uuid)", '"FORUM_TOPIC_MOVE_OPERATION_CONFLICT"'],
  "typed move conflict",
);

requireIncludes(
  test,
  [
    "async fn topic_move_is_atomic_idempotent_and_append_only()",
    "async fn topic_move_rejects_foreign_and_archived_targets_without_partial_state()",
    "OutboxModule.migrations()",
    "ForumModule.migrations()",
    "create_approved_reply(",
    'assert_eq!(reply.status, "approved")',
    "assert_category_counters(&db, tenant_id, source_category_id, 1, 1)",
    "assert_category_counters(&db, tenant_id, source_category_id, 0, 0)",
    "assert_category_counters(&db, tenant_id, target_category_id, 1, 1)",
    "assert_semantic_event(&db, tenant_id, &moved).await?;",
    "assert_eq!(new_projection_ids.len(), 3);",
    "collect::<BTreeSet<_>>();",
    "assert_eq!(targets, expected_targets);",
    "assert_eq!(replay, moved);",
    "ForumError::TopicMoveOperationConflict(id)",
    "UPDATE forum_topic_move_operations SET reason = 'tampered'",
    "DELETE FROM forum_topic_move_operations",
    "INSERT INTO forum_category_lifecycle",
    "Err(ForumError::CategoryNotFound(id))",
    "assert_eq!(move_operation_count(&db, tenant_id).await?, 0);",
  ],
  "SQLite regression",
);

requireIncludes(
  doc,
  [
    "`source_ready_maintainer_execution_pending`",
    "Idempotency contract",
    "One atomic owner transaction",
    "Semantic event and projection invalidation",
    "Persistence guards",
    "Regression coverage",
    "canonical FORUM-21 ledger entry deliberately remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "handoff",
);
requireIncludes(
  plan,
  [
    "| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |",
    "## `FORUM-21` — move, merge, split and fork topics",
    "Each operation has an operation ID, reason, transactional state change and",
    "semantic event; retry produces the same result; partial moves are impossible;",
  ],
  "canonical pending plan",
);
assert.ok(
  !plan.includes("### Delivered in `FORUM-21A`") &&
    !plan.includes("| `FORUM-21` | `in_progress` | FORUM-21A"),
);

for (const forbidden of [
  "async_graphql",
  "axum",
  "server_fn",
  "TopicsMerged",
  "TopicSplit",
  "CanonicalUrlChanged",
  "UrlAliasPurged",
]) {
  assert.ok(!service.includes(forbidden), `future scope leaked into service: ${forbidden}`);
}

console.log("FORUM-21A topic move owner source contract verified");
