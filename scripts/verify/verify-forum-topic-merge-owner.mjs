#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  solutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  resolutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json",
  canonicalContract: "crates/rustok-forum/contracts/forum-topic-canonical-resolution.json",
  graphqlContract: "crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json",
  docs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  receiptEntity: "crates/rustok-forum/src/entities/forum_topic_merge_operation.rs",
  resolutionEntity:
    "crates/rustok-forum/src/entities/forum_topic_merge_solution_resolution.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  receiptMigration:
    "crates/rustok-forum/src/migrations/m20260801_000010_add_forum_topic_merge_operations.rs",
  solutionMigration:
    "crates/rustok-forum/src/migrations/m20260803_000016_add_forum_topic_merge_solution_policy.rs",
  canonicalMigration:
    "crates/rustok-forum/src/migrations/m20260803_000017_add_forum_topic_canonical_resolution.rs",
  resolutionMigration:
    "crates/rustok-forum/src/migrations/m20260803_000018_add_forum_topic_merge_solution_resolution.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  service: "crates/rustok-forum/src/services/topic_merge.rs",
  stats: "crates/rustok-forum/src/services/user_stats.rs",
  canonicalService: "crates/rustok-forum/src/services/topic_canonical_resolution.rs",
  facade: "crates/rustok-forum/src/services/topic_facade.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  redirect: "crates/rustok-forum/src/controllers/topic_redirect.rs",
  controller: "crates/rustok-forum/src/controllers/mod.rs",
  openapi: "crates/rustok-forum/src/openapi.rs",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  test: "crates/rustok-forum/tests/topic_merge_sqlite.rs",
  resolutionTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
  canonicalTest: "crates/rustok-forum/tests/topic_canonical_resolution_sqlite.rs",
  graphqlTest: "crates/rustok-forum/tests/topic_merge_graphql_contract.rs",
  resolutionGraphqlTest:
    "crates/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const solutionContract = JSON.parse(read(paths.solutionContract));
const resolutionContract = JSON.parse(read(paths.resolutionContract));
const canonicalContract = JSON.parse(read(paths.canonicalContract));
const graphqlContract = JSON.parse(read(paths.graphqlContract));
const docs = read(paths.docs);
const receiptEntity = read(paths.receiptEntity);
const resolutionEntity = read(paths.resolutionEntity);
const entitiesMod = read(paths.entitiesMod);
const error = read(paths.error);
const receiptMigration = read(paths.receiptMigration);
const solutionMigration = read(paths.solutionMigration);
const canonicalMigration = read(paths.canonicalMigration);
const resolutionMigration = read(paths.resolutionMigration);
const migrationsMod = read(paths.migrationsMod);
const service = read(paths.service);
const stats = read(paths.stats);
const canonicalService = read(paths.canonicalService);
const facade = read(paths.facade);
const servicesMod = read(paths.servicesMod);
const lib = read(paths.lib);
const redirect = read(paths.redirect);
const controller = read(paths.controller);
const openapi = read(paths.openapi);
const graphql = read(paths.graphql);
const graphqlMod = read(paths.graphqlMod);
const test = read(paths.test);
const resolutionTest = read(paths.resolutionTest);
const canonicalTest = read(paths.canonicalTest);
const graphqlTest = read(paths.graphqlTest);
const resolutionGraphqlTest = read(paths.resolutionGraphqlTest);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_merge_owner_v1");
assert.equal(contract.task, "FORUM-21B");
assert.equal(contract.latest_policy_slice, "FORUM-21L");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.deepEqual(contract.migrations, [
  "m20260801_000010_add_forum_topic_merge_operations",
  "m20260803_000016_add_forum_topic_merge_solution_policy",
  "m20260803_000017_add_forum_topic_canonical_resolution",
  "m20260803_000018_add_forum_topic_merge_solution_resolution",
]);
assert.equal(contract.semantic_event.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.payload_changed_by_solution_resolution, false);
assert.equal(contract.semantic_event.all_post_merge_reconciliation_owners_remain_compatible, true);
assert.equal(contract.solution_policy.winner_solution_count_delta, 0);
assert.equal(contract.solution_policy.loser_solution_count_delta, -1);
assert.equal(contract.solution_policy.negative_solution_count_transition_is_exact_and_fail_closed, true);
assert.equal(contract.solution_resolution.task, "FORUM-21L");
assert.equal(contract.solution_resolution.explicit_method, "merge_topic_resolving_solution");
assert.equal(contract.solution_resolution.audit_table, "forum_topic_merge_solution_resolutions");
assert.equal(contract.solution_resolution.audit_is_append_only_on_postgresql_and_sqlite, true);
assert.equal(contract.solution_resolution.receipt_schema_changed, false);
assert.equal(contract.solution_resolution.event_contract_changed, false);
assert.equal(contract.solution_resolution.audit_migration_added, true);
assert.equal(contract.canonical_resolution.source_of_truth, "forum_topic_merge_operations");
assert.equal(contract.canonical_resolution.rest_merged_source_returns_308, true);
assert.equal(contract.graphql_transport.merge_field, "mergeForumTopic");
assert.equal(
  contract.graphql_transport.solution_resolution_field,
  "mergeForumTopicResolvingSolution",
);
assert.equal(contract.graphql_transport.resolvers_call_same_owner_service, true);
assert.equal(contract.graphql_transport.mutation_follows_canonical_source_alias, false);

assert.equal(solutionContract.latest_resolution_slice, "FORUM-21L");
assert.equal(solutionContract.compatibility.forum_topic_merged_event_changed, false);
assert.equal(solutionContract.compatibility.resolution_audit_migration_added, true);
assert.equal(resolutionContract.task, "FORUM-21L");
assert.equal(resolutionContract.semantic_event_compatibility.schema_version, 1);
assert.equal(resolutionContract.semantic_event_compatibility.payload_changed, false);
assert.equal(canonicalContract.task, "FORUM-21I");
assert.equal(canonicalContract.latest_transport_slice, "FORUM-21J");
assert.equal(graphqlContract.latest_resolution_slice, "FORUM-21L");
assert.equal(
  graphqlContract.solution_resolution_command.field,
  "mergeForumTopicResolvingSolution",
);

includesAll(
  receiptEntity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_operations")]',
    "pub operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub category_id: Uuid",
    "pub actor_id: Uuid",
    "pub event_id: Uuid",
  ],
  "merge receipt entity",
);
includesAll(
  resolutionEntity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_solution_resolutions")]',
    "pub operation_id: Uuid",
    "pub selected_solution_reply_id: Uuid",
    "pub rejected_solution_reply_id: Uuid",
    "pub resolved_at: DateTimeWithTimeZone",
  ],
  "solution-resolution entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_solution_resolution;",
    "ForumTopicMergeSolutionResolutionEntity",
  ],
  "entity registration",
);
includesAll(
  error,
  [
    "TopicMergeOperationConflict(Uuid)",
    "TopicMergeSolutionConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_OPERATION_CONFLICT"',
    '"FORUM_TOPIC_MERGE_SOLUTION_CONFLICT"',
    '"FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT"',
  ],
  "Forum errors",
);
includesAll(
  migrationsMod,
  [
    "m20260801_000010_add_forum_topic_merge_operations",
    "m20260803_000016_add_forum_topic_merge_solution_policy",
    "m20260803_000017_add_forum_topic_canonical_resolution",
    "m20260803_000018_add_forum_topic_merge_solution_resolution",
    "Box::new(m20260803_000018_add_forum_topic_merge_solution_resolution::Migration)",
  ],
  "migration registration",
);
includesAll(
  receiptMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_operations",
    "PRIMARY KEY (tenant_id, operation_id)",
    "source_topic_id <> target_topic_id",
    "event_id = operation_id",
    "forum topic merge operations are append-only",
  ],
  "receipt migration",
);
includesAll(
  solutionMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_solution_locks",
    "forum_lock_topic_solution_mutation",
    "forum_validate_topic_solution_target",
    "hashtextextended(",
    "31",
  ],
  "solution migration",
);
includesAll(
  canonicalMigration,
  [
    "uq_forum_topic_merge_operations_source",
    "forum_validate_topic_merge_redirect_edge",
    "source.status::text = 'archived'",
    "source.is_locked = TRUE",
    "source.reply_count = 0",
  ],
  "canonical migration",
);
includesAll(
  resolutionMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_solution_resolutions",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "forum topic merge solution resolutions are append-only",
    "BEFORE UPDATE ON forum_topic_merge_solution_resolutions",
    "BEFORE DELETE ON forum_topic_merge_solution_resolutions",
  ],
  "solution-resolution migration",
);

includesAll(
  service,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500;",
    "pub const MAX_FORUM_TOPIC_MERGE_REPLIES: u64 = 500;",
    'const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";',
    "const FORUM_TOPIC_MERGED_SCHEMA_VERSION: i16 = 1;",
    "pub struct MergeForumTopicInput",
    "pub struct ForumTopicMergeResult",
    "pub struct ForumTopicMergeService",
    "pub async fn merge_topic(",
    "pub async fn merge_topic_resolving_solution(",
    "async fn merge_topic_internal(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_topic_merge_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_operation::Entity::find_by_id",
    "validate_existing_semantic_event_in_tx(&txn, &existing)",
    "load_solution_resolution_audit_in_tx(",
    "TopicMergeOperationConflict(input.operation_id)",
    "lock_merge_counter_scopes_in_tx(",
    "lock_topics_in_tx(&txn",
    "lock_topic_solution_scopes_in_tx(",
    "plan_solution_merge(",
    "TopicMergeSolutionConflict(operation_id)",
    "delete_solution_in_tx",
    "UserStatsService::adjust_solution_count_in_tx",
    "move_replies_in_tx(",
    "insert_transferred_solution_in_tx",
    "source_active.status = Set(TopicStatus::Archived);",
    "source_active.is_locked = Set(true);",
    "schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION)",
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "forum_topic_merge_solution_resolution::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "publish_forum_category_projection_in_tx",
    "txn.commit().await?;",
  ],
  "merge owner",
);
assert.equal((service.match(/self\.db\.begin\(\)\.await\?/g) ?? []).length, 1);
assert.equal((service.match(/publish_forum_topic_projection_in_tx\(/g) ?? []).length, 2);
assert.equal((service.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length, 1);
assert.ok(!service.includes("FORUM_TOPIC_MERGED_SOLUTION_RESOLUTION_SCHEMA_VERSION"));
assert.ok(!service.includes('"solution_resolution"'));
const receiptLookup = service.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const replayEvent = service.indexOf("validate_existing_semantic_event_in_tx(&txn, &existing)");
const replayAudit = service.indexOf("load_solution_resolution_audit_in_tx(");
const preliminaryRead = service.indexOf("let preliminary_source =");
const solutionPlan = service.indexOf("let solution_plan = plan_solution_merge");
const sourceDelete = service.indexOf("delete_solution_in_tx(&txn, tenant_id, source.id");
const statDelta = service.indexOf("UserStatsService::adjust_solution_count_in_tx");
const replyMove = service.indexOf("move_replies_in_tx(", sourceDelete);
const eventInsert = service.indexOf("forum_domain_event::ActiveModel");
const receiptInsert = service.indexOf("forum_topic_merge_operation::ActiveModel");
const auditInsert = service.indexOf("forum_topic_merge_solution_resolution::ActiveModel");
const invalidation = service.indexOf("publish_forum_topic_projection_in_tx(");
assert.ok(receiptLookup < replayEvent && replayEvent < replayAudit && replayAudit < preliminaryRead);
assert.ok(preliminaryRead < solutionPlan && solutionPlan < sourceDelete);
assert.ok(sourceDelete < statDelta && statDelta < replyMove);
assert.ok(replyMove < eventInsert && eventInsert < receiptInsert);
assert.ok(receiptInsert < auditInsert && auditInsert < invalidation);
for (const forbidden of [
  "forum_topic::Entity::delete",
  "forum_reply::Entity::delete",
  "category_active.topic_count",
  "ForumTopicMoveService",
  "resolve_canonical_topic",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!service.includes(forbidden), `merge owner contains forbidden marker: ${forbidden}`);
}

includesAll(
  stats,
  [
    "if delta == -1",
    "decrement_solution_count_exact_in_tx",
    "solution_count = solution_count - 1",
    "solution_count > 0",
    "rows_affected() != 1",
    "Forum solution author statistic is inconsistent",
  ],
  "solution statistics",
);

includesAll(
  canonicalService,
  [
    "MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS: usize = 32",
    "pub struct ForumTopicCanonicalResolution",
    "load_resolution_step(",
    "LIMIT 2",
    "TopicCanonicalResolutionConflict(requested_topic_id)",
  ],
  "canonical resolution owner",
);
includesAll(
  facade,
  [
    "pub async fn resolve_canonical_topic(",
    "pub async fn get_with_canonical_resolution_and_locale_fallback(",
  ],
  "topic facade",
);
includesAll(
  servicesMod,
  [
    "mod topic_merge;",
    "ForumTopicMergeResult, ForumTopicMergeService",
    "MergeForumTopicInput",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeResult, ForumTopicMergeService",
    "MergeForumTopicInput",
    "ForumTopicCanonicalResolution",
  ],
  "crate exports",
);

includesAll(
  redirect,
  [
    "StatusCode::PERMANENT_REDIRECT",
    '(CACHE_CONTROL, "private, no-store".to_string())',
    "merged_source_redirects_privately_while_target_uses_existing_handler",
  ],
  "REST redirect",
);
includesAll(
  controller,
  [
    "topic_redirect::redirect_merged_topic",
    ".put(content_commands::update_topic)",
    ".delete(topics::delete_topic)",
  ],
  "redirect wiring",
);
includesAll(openapi, ["crate::controllers::topic_redirect::redirect_merged_topic"], "OpenAPI");

includesAll(
  graphql,
  [
    "async fn merge_forum_topic(",
    "async fn merge_forum_topic_resolving_solution(",
    "Permission::FORUM_TOPICS_MANAGE",
    "Permission denied: tenant scope mismatch",
    ".merge_topic(",
    ".merge_topic_resolving_solution(",
    "ResolveForumTopicMergeSolutionGraphqlInput",
    "GqlForumTopicMergeSolutionResolution",
  ],
  "GraphQL transport",
);
includesAll(
  graphqlMod,
  [
    "GqlForumTopicMergeSolutionResolution",
    "ResolveForumTopicMergeSolutionGraphqlInput",
    "topic_merge_mutation::ForumTopicMergeMutation",
  ],
  "GraphQL registration",
);
for (const forbidden of [
  "resolve_canonical_topic",
  "forum_topic_merge_operations",
  "forum_topic_merge_solution_resolutions",
  "forum_solutions::",
  "TopicService::new",
]) {
  assert.ok(!graphql.includes(forbidden), `GraphQL transport contains forbidden marker: ${forbidden}`);
}

includesAll(
  test,
  [
    "topic_merge_is_atomic_idempotent_and_append_only",
    "topic_merge_transfers_source_only_solution_and_preserves_target_only_solution",
    "topic_merge_rejects_cross_category_and_competing_solutions_without_partial_state",
    "TopicMergeOperationConflict",
    "TopicMergeSolutionConflict",
  ],
  "ordinary merge regression",
);
includesAll(
  resolutionTest,
  [
    "manager_can_select_source_solution_and_replay_exact_audit",
    "manager_can_select_target_solution_and_invalid_selection_is_atomic",
    "assert_merge_event_and_resolution_audit",
    "forum_topic_merge_solution_resolutions",
    "TopicMergeOperationConflict",
    "FORUM_VALIDATION_FAILED",
  ],
  "solution-resolution regression",
);
includesAll(
  canonicalTest,
  [
    "merged_topic_ids_resolve_to_one_visible_canonical_target",
    "assert_eq!(selected.id, topic_c)",
    "assert_eq!(storefront.id, topic_c)",
  ],
  "canonical regression",
);
includesAll(
  graphqlTest,
  ["graphql_schema_exposes_idempotent_topic_merge_command", '"mergeForumTopic"'],
  "ordinary GraphQL contract",
);
includesAll(
  resolutionGraphqlTest,
  [
    "graphql_schema_exposes_explicit_solution_resolution_command",
    '"mergeForumTopicResolvingSolution"',
    "ordinary_and_resolved_commands_share_one_private_transaction_owner",
    "resolution_audit_is_append_only_and_keeps_merge_event_schema_one",
  ],
  "resolution GraphQL contract",
);

includesAll(
  docs,
  [
    "# FORUM-21B idempotent topic merge owner",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    paths.resolutionContract,
    "FORUM-21A through FORUM-21L",
    "mergeForumTopicResolvingSolution",
    "forum.topic.merged / schema version 1",
    "Resolution audit ledger",
    "No command above was run by the implementation agent",
  ],
  "merge owner handoff",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("| `FORUM-24` | `planned` | Localized routes, canonical URLs and aliases. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));

console.log(
  "FORUM-21B/H/I/J/K/L topic merge owner source is ready; FORUM-21 and FORUM-24 remain planned.",
);