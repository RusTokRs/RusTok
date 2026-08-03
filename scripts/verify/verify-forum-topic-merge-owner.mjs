#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  solutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  canonicalContract: "crates/rustok-forum/contracts/forum-topic-canonical-resolution.json",
  graphqlContract: "crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json",
  docs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  graphqlDocs: "crates/rustok-forum/docs/forum-21k-topic-merge-graphql-transport.md",
  entity: "crates/rustok-forum/src/entities/forum_topic_merge_operation.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  lib: "crates/rustok-forum/src/lib.rs",
  receiptMigration:
    "crates/rustok-forum/src/migrations/m20260801_000010_add_forum_topic_merge_operations.rs",
  solutionMigration:
    "crates/rustok-forum/src/migrations/m20260803_000016_add_forum_topic_merge_solution_policy.rs",
  canonicalMigration:
    "crates/rustok-forum/src/migrations/m20260803_000017_add_forum_topic_canonical_resolution.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  service: "crates/rustok-forum/src/services/topic_merge.rs",
  canonicalService: "crates/rustok-forum/src/services/topic_canonical_resolution.rs",
  facade: "crates/rustok-forum/src/services/topic_facade.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  redirect: "crates/rustok-forum/src/controllers/topic_redirect.rs",
  controller: "crates/rustok-forum/src/controllers/mod.rs",
  openapi: "crates/rustok-forum/src/openapi.rs",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  test: "crates/rustok-forum/tests/topic_merge_sqlite.rs",
  canonicalTest: "crates/rustok-forum/tests/topic_canonical_resolution_sqlite.rs",
  graphqlTest: "crates/rustok-forum/tests/topic_merge_graphql_contract.rs",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  verifier: "scripts/verify/verify-forum-topic-merge-owner.mjs",
};

const read = (path) => readFileSync(path, "utf8");
function includesAll(text, markers, label) {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
}

const contract = JSON.parse(read(paths.contract));
const solutionContract = JSON.parse(read(paths.solutionContract));
const canonicalContract = JSON.parse(read(paths.canonicalContract));
const graphqlContract = JSON.parse(read(paths.graphqlContract));
const docs = read(paths.docs);
const graphqlDocs = read(paths.graphqlDocs);
const entity = read(paths.entity);
const entitiesMod = read(paths.entitiesMod);
const error = read(paths.error);
const lib = read(paths.lib);
const receiptMigration = read(paths.receiptMigration);
const solutionMigration = read(paths.solutionMigration);
const canonicalMigration = read(paths.canonicalMigration);
const migrationsMod = read(paths.migrationsMod);
const service = read(paths.service);
const canonicalService = read(paths.canonicalService);
const facade = read(paths.facade);
const servicesMod = read(paths.servicesMod);
const redirect = read(paths.redirect);
const controller = read(paths.controller);
const openapi = read(paths.openapi);
const graphql = read(paths.graphql);
const graphqlMod = read(paths.graphqlMod);
const test = read(paths.test);
const canonicalTest = read(paths.canonicalTest);
const graphqlTest = read(paths.graphqlTest);
const plan = read(paths.plan);
const verifier = read(paths.verifier);

assert.equal(contract.contract, "forum_topic_merge_owner_v1");
assert.equal(contract.task, "FORUM-21B");
assert.equal(contract.latest_policy_slice, "FORUM-21K");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.scope, "bounded_same_category_source_into_retained_target");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.input, "MergeForumTopicInput");
assert.equal(contract.result, "ForumTopicMergeResult");
assert.deepEqual(contract.migrations, [
  "m20260801_000010_add_forum_topic_merge_operations",
  "m20260803_000016_add_forum_topic_merge_solution_policy",
  "m20260803_000017_add_forum_topic_canonical_resolution",
]);
assert.equal(contract.receipt_table, "forum_topic_merge_operations");
assert.deepEqual(contract.required_permissions, ["forum_topics:manage"]);
assert.equal(contract.bounds.reason_max_characters, 500);
assert.equal(contract.bounds.source_reply_rows_max, 500);
assert.equal(contract.bounds.same_category_only, true);
assert.equal(contract.bounds.accepted_solutions_per_topic_max, 1);
assert.equal(contract.bounds.canonical_resolution_hops_max, 32);
assert.equal(contract.semantic_event.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.event_id_equals_operation_id, true);
assert.equal(contract.solution_policy.solution_count_delta_during_merge, 0);
assert.equal(
  contract.solution_policy.source_and_target,
  "fail_before_mutation_with_FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
);
assert.equal(contract.canonical_resolution.source_of_truth, "forum_topic_merge_operations");
assert.equal(contract.canonical_resolution.parallel_alias_store, false);
assert.equal(contract.canonical_resolution.rest_direct_target_returns_200, true);
assert.equal(contract.canonical_resolution.rest_merged_source_returns_308, true);
assert.equal(contract.canonical_resolution.rest_redirect_is_get_only, true);
assert.equal(contract.graphql_transport.task, "FORUM-21K");
assert.equal(contract.graphql_transport.field, "mergeForumTopic");
assert.equal(contract.graphql_transport.required_permission, "forum_topics:manage");
assert.equal(contract.graphql_transport.operation_id_is_idempotency_identity, true);
assert.equal(contract.graphql_transport.returns_immutable_owner_receipt, true);
assert.equal(contract.graphql_transport.exact_replay_returns_same_result, true);
assert.equal(contract.graphql_transport.event_id_equals_operation_id, true);
assert.equal(contract.graphql_transport.mutation_follows_canonical_source_alias, false);
assert.equal(contract.graphql_transport.target_topic_hydration_after_command, false);
assert.equal(solutionContract.task, "FORUM-21H");
assert.equal(solutionContract.extends, "FORUM-21B");
assert.equal(canonicalContract.task, "FORUM-21I");
assert.equal(canonicalContract.latest_transport_slice, "FORUM-21J");
assert.equal(canonicalContract.extends, "FORUM-21B");
assert.equal(
  canonicalContract.resolution.each_hop_topic_and_edges_share_one_statement_snapshot,
  true,
);
assert.equal(canonicalContract.http_redirect.status, 308);
assert.equal(canonicalContract.http_redirect.cache_control, "private, no-store");
assert.equal(graphqlContract.task, "FORUM-21K");
assert.equal(graphqlContract.extends, "FORUM-21B");
assert.equal(graphqlContract.field, "mergeForumTopic");
assert.equal(graphqlContract.authorization.required_permission, "forum_topics:manage");
assert.equal(graphqlContract.result.exact_replay_returns_same_result, true);

includesAll(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_operations")]',
    "pub tenant_id: Uuid",
    "pub operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub category_id: Uuid",
    "pub moved_reply_count: i32",
    "pub moved_published_reply_count: i32",
    "pub resulting_published_reply_count: i32",
    "pub position_offset: i64",
    "pub event_id: Uuid",
  ],
  "topic merge entity",
);
includesAll(
  entitiesMod,
  [
    "pub mod forum_topic_merge_operation;",
    "pub use forum_topic_merge_operation::Entity as ForumTopicMergeOperationEntity;",
  ],
  "entity exports",
);
includesAll(
  error,
  [
    "TopicMergeOperationConflict(Uuid)",
    "TopicMergeSolutionConflict(Uuid)",
    "TopicCanonicalResolutionConflict(Uuid)",
    '"FORUM_TOPIC_MERGE_OPERATION_CONFLICT"',
    '"FORUM_TOPIC_MERGE_SOLUTION_CONFLICT"',
    '"FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT"',
  ],
  "ForumError",
);
includesAll(
  migrationsMod,
  [
    "mod m20260801_000010_add_forum_topic_merge_operations;",
    "Box::new(m20260801_000010_add_forum_topic_merge_operations::Migration)",
    "mod m20260803_000016_add_forum_topic_merge_solution_policy;",
    "Box::new(m20260803_000016_add_forum_topic_merge_solution_policy::Migration)",
    "mod m20260803_000017_add_forum_topic_canonical_resolution;",
    "Box::new(m20260803_000017_add_forum_topic_canonical_resolution::Migration)",
  ],
  "migration registration",
);
includesAll(
  receiptMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_locks",
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_operations",
    "PRIMARY KEY (tenant_id, operation_id)",
    "source_topic_id <> target_topic_id",
    "moved_reply_count BETWEEN 0 AND 500",
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
    "forum solution requires an active topic and approved reply",
  ],
  "solution policy migration",
);
includesAll(
  canonicalMigration,
  [
    "uq_forum_topic_merge_operations_source",
    "forum_validate_topic_merge_redirect_edge",
    "forum_05_topic_merge_redirect_edge",
    "source.status::text = 'archived'",
    "source.is_locked = TRUE",
    "source.reply_count = 0",
    "target.status::text <> 'archived'",
  ],
  "canonical resolution migration",
);

includesAll(
  service,
  [
    "pub const MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500;",
    "pub const MAX_FORUM_TOPIC_MERGE_REPLIES: u64 = 500;",
    'const FORUM_TOPIC_MERGED_EVENT_TYPE: &str = "forum.topic.merged";',
    "pub struct MergeForumTopicInput",
    "pub struct ForumTopicMergeResult",
    "pub struct ForumTopicMergeService",
    "pub async fn merge_topic(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_topic_merge_tenant_in_tx(&txn, tenant_id).await?;",
    "forum_topic_merge_operation::Entity::find_by_id",
    "TopicMergeOperationConflict(input.operation_id)",
    "lock_merge_counter_scopes_in_tx(",
    "lock_topics_in_tx(&txn, tenant_id, input.source_topic_id, target_topic_id).await?;",
    "lock_topic_solution_scopes_in_tx(",
    "TopicMergeSolutionConflict(input.operation_id)",
    "delete_source_solution_in_tx",
    "move_replies_in_tx(",
    "insert_transferred_solution_in_tx",
    "source_active.status = Set(TopicStatus::Archived);",
    "source_active.is_locked = Set(true);",
    "source_active.reply_count = Set(0);",
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "publish_forum_topic_projection_in_tx",
    "publish_forum_category_projection_in_tx",
    "validate_existing_semantic_event_in_tx",
    "txn.commit().await?;",
  ],
  "topic merge service",
);
assert.ok(!service.includes("does not yet support a source accepted solution"));
assert.ok(!service.includes("UserStatsService"));
const receiptLookup = service.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const preliminaryRead = service.indexOf("let preliminary_source =");
const counterLocks = service.indexOf("lock_merge_counter_scopes_in_tx(");
const topicLocks = service.indexOf("lock_topics_in_tx(&txn");
const solutionLocks = service.indexOf("lock_topic_solution_scopes_in_tx(");
const solutionConflict = service.indexOf("TopicMergeSolutionConflict(input.operation_id)");
const solutionDelete = service.indexOf("delete_source_solution_in_tx(&txn");
const replyMove = service.indexOf("move_replies_in_tx(", solutionDelete);
const solutionInsert = service.indexOf("insert_transferred_solution_in_tx(&txn");
assert.ok(receiptLookup < preliminaryRead);
assert.ok(preliminaryRead < counterLocks && counterLocks < topicLocks);
assert.ok(topicLocks < solutionLocks && solutionLocks < solutionConflict);
assert.ok(solutionConflict < solutionDelete && solutionDelete < replyMove);
assert.ok(replyMove < solutionInsert);
assert.equal((service.match(/publish_forum_topic_projection_in_tx\(/g) ?? []).length, 2);
assert.equal((service.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length, 1);
for (const marker of [
  "forum_topic::Entity::delete",
  "forum_reply::Entity::delete",
  "category_active.topic_count",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
  "sourceCommitOverride",
  "bestEffort",
]) {
  assert.ok(!service.includes(marker), `service contains forbidden marker: ${marker}`);
}

includesAll(
  canonicalService,
  [
    "pub const MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS: usize = 32;",
    "pub struct ForumTopicCanonicalResolution",
    "pub(crate) async fn resolve_unchecked(",
    "load_resolution_step(",
    "match step.edges.as_slice()",
    "!visited.insert(edge.target_topic_id)",
    "TopicCanonicalResolutionConflict(requested_topic_id)",
    "EXISTS (",
    "AS topic_exists",
    "FROM forum_topic_merge_operations",
    "LEFT JOIN (",
    "LIMIT 2",
    "topic.deleted_at IS NULL",
  ],
  "canonical resolution service",
);
assert.ok(!canonicalService.includes("forum_topic_alias"));
assert.ok(!canonicalService.includes("forum_topic_redirects"));
includesAll(
  facade,
  [
    "pub async fn resolve_canonical_topic(",
    "pub async fn get_with_canonical_resolution_and_locale_fallback(",
    "resolution.canonical_topic_id",
    ".is_topic_visible(tenant_id, resolution.canonical_topic_id, &scope)",
  ],
  "canonical topic facade",
);
includesAll(
  servicesMod,
  [
    "mod topic_merge;",
    "mod topic_canonical_resolution;",
    "ForumTopicMergeResult, ForumTopicMergeService",
    "MergeForumTopicInput",
    "MAX_FORUM_TOPIC_MERGE_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_REPLIES",
    "ForumTopicCanonicalResolution, MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS",
  ],
  "service exports",
);
includesAll(
  lib,
  [
    "ForumTopicMergeResult, ForumTopicMergeService",
    "MergeForumTopicInput",
    "MAX_FORUM_TOPIC_MERGE_REASON_LEN",
    "MAX_FORUM_TOPIC_MERGE_REPLIES",
    "ForumTopicCanonicalResolution",
    "MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS",
    "mod contract_tests;",
  ],
  "crate exports",
);

includesAll(
  redirect,
  [
    "pub(crate) async fn redirect_merged_topic(",
    "StatusCode::PERMANENT_REDIRECT",
    '(CACHE_CONTROL, "private, no-store".to_string())',
    "merged_source_redirects_privately_while_target_uses_existing_handler",
    "assert_eq!(put_response.status(), StatusCode::NO_CONTENT)",
  ],
  "merged source redirect",
);
includesAll(
  controller,
  [
    "mod topic_redirect;",
    "get(topics::get_topic)",
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
    "require_module_enabled(ctx, MODULE_SLUG).await?;",
    "Permission::FORUM_TOPICS_MANAGE",
    "Permission denied: tenant scope mismatch",
    "ForumTopicMergeService::new(db.clone(), event_bus.clone())",
    ".merge_topic(",
    "pub struct MergeForumTopicGraphqlInput",
    "pub struct GqlForumTopicMerge",
    "merge_transport_enforces_scope_and_replays_one_receipt",
    "assert_eq!(first, replay)",
  ],
  "GraphQL merge transport",
);
includesAll(
  graphqlMod,
  [
    "mod topic_merge_mutation;",
    "GqlForumTopicMerge, MergeForumTopicGraphqlInput",
    "topic_merge_mutation::ForumTopicMergeMutation",
  ],
  "GraphQL merge registration",
);
for (const marker of [
  "ForumTopicMoveService",
  "resolve_canonical_topic",
  "forum_topic_merge_operations",
  "get_with_locale_fallback",
  "bestEffort",
]) {
  assert.ok(!graphql.includes(marker), `GraphQL merge transport contains forbidden marker: ${marker}`);
}

includesAll(
  test,
  [
    "topic_merge_is_atomic_idempotent_and_append_only",
    "topic_merge_transfers_source_only_solution_and_preserves_target_only_solution",
    "topic_merge_rejects_cross_category_and_competing_solutions_without_partial_state",
    "topic_solution_database_guard_requires_active_topic_and_approved_reply",
    "source_root_reply_id",
    "source_child_reply_id",
    "TopicMergeOperationConflict",
    "TopicMergeSolutionConflict",
    "UPDATE forum_topic_merge_operations",
    "DELETE FROM forum_topic_merge_operations",
    '"forum.topic.merged"',
  ],
  "merge SQLite regression",
);
includesAll(
  canonicalTest,
  [
    "merged_topic_ids_resolve_to_one_visible_canonical_target",
    "vec![operation_ab, operation_bc]",
    "assert_eq!(selected.id, topic_c)",
    "assert_eq!(storefront.id, topic_c)",
    "insert_direct_merge_receipt",
  ],
  "canonical resolution SQLite regression",
);
includesAll(
  graphqlTest,
  [
    "graphql_schema_exposes_idempotent_topic_merge_command",
    '"mergeForumTopic"',
    '"MergeForumTopicGraphqlInput"',
    '"GqlForumTopicMerge"',
    "graphql_merge_adapter_uses_routed_tenant_manage_scope_and_owner_service",
  ],
  "GraphQL merge schema contract",
);
includesAll(
  docs,
  [
    "# FORUM-21B idempotent topic merge owner",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    paths.solutionContract,
    paths.canonicalContract,
    paths.graphqlContract,
    "source-only accepted solution",
    "target-only accepted solution",
    "two accepted solutions",
    "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
    "FORUM_TOPIC_CANONICAL_RESOLUTION_CONFLICT",
    "308 Permanent Redirect",
    "## GraphQL merge command",
    "mergeForumTopic",
    "FORUM-21A through FORUM-21K",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21B handoff",
);
includesAll(
  graphqlDocs,
  [
    "# FORUM-21K topic merge GraphQL transport",
    "forum_topics:manage",
    "immutable owner receipt",
    "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
    "FORUM-21` entry remains `planned`",
  ],
  "FORUM-21K handoff",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("| `FORUM-24` | `planned` | Localized routes, canonical URLs and aliases. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-24` | `done` |"));
assert.ok(verifier.includes("source_ready_maintainer_execution_pending"));

console.log(
  "FORUM-21B/H/I/J/K topic merge owner source is ready; FORUM-21 and FORUM-24 remain planned.",
);
