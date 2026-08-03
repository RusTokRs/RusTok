#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  crossContract: "crates/rustok-forum/contracts/forum-topic-merge-cross-category.json",
  solutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  resolutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json",
  canonicalContract: "crates/rustok-forum/contracts/forum-topic-canonical-resolution.json",
  graphqlContract: "crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json",
  adminContract: "crates/rustok-forum/contracts/forum-topic-merge-admin-ui.json",
  docs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  crossDocs: "crates/rustok-forum/docs/forum-21m-topic-merge-cross-category.md",
  adminDocs: "crates/rustok-forum/docs/forum-21n-topic-merge-admin-ui.md",
  receiptEntity: "crates/rustok-forum/src/entities/forum_topic_merge_operation.rs",
  resolutionEntity:
    "crates/rustok-forum/src/entities/forum_topic_merge_solution_resolution.rs",
  error: "crates/rustok-forum/src/error.rs",
  receiptMigration:
    "crates/rustok-forum/src/migrations/m20260801_000010_add_forum_topic_merge_operations.rs",
  solutionMigration:
    "crates/rustok-forum/src/migrations/m20260803_000016_add_forum_topic_merge_solution_policy.rs",
  canonicalMigration:
    "crates/rustok-forum/src/migrations/m20260803_000017_add_forum_topic_canonical_resolution.rs",
  resolutionMigration:
    "crates/rustok-forum/src/migrations/m20260803_000018_add_forum_topic_merge_solution_resolution.rs",
  crossMigration:
    "crates/rustok-forum/src/migrations/m20260803_000019_allow_cross_category_topic_merge_redirect_edges.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  service: "crates/rustok-forum/src/services/topic_merge.rs",
  stats: "crates/rustok-forum/src/services/user_stats.rs",
  canonicalService: "crates/rustok-forum/src/services/topic_canonical_resolution.rs",
  facade: "crates/rustok-forum/src/services/topic_facade.rs",
  redirect: "crates/rustok-forum/src/controllers/topic_redirect.rs",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  ordinaryTest: "crates/rustok-forum/tests/topic_merge_sqlite.rs",
  crossTest: "crates/rustok-forum/tests/topic_merge_cross_category_sqlite.rs",
  resolutionTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
  canonicalTest: "crates/rustok-forum/tests/topic_canonical_resolution_sqlite.rs",
  graphqlTest: "crates/rustok-forum/tests/topic_merge_graphql_contract.rs",
  resolutionGraphqlTest:
    "crates/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
  leptosModel: "crates/rustok-forum/admin/src/topic_merge_model.rs",
  leptosUi: "crates/rustok-forum/admin/src/ui/topic_merge.rs",
  nextCore: "apps/next-admin/packages/forum/src/core/topic-merge.ts",
  nextUi: "apps/next-admin/packages/forum/src/components/forum-topic-merge.tsx",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const crossContract = JSON.parse(read(paths.crossContract));
const solutionContract = JSON.parse(read(paths.solutionContract));
const resolutionContract = JSON.parse(read(paths.resolutionContract));
const canonicalContract = JSON.parse(read(paths.canonicalContract));
const graphqlContract = JSON.parse(read(paths.graphqlContract));
const adminContract = JSON.parse(read(paths.adminContract));
const docs = read(paths.docs);
const crossDocs = read(paths.crossDocs);
const adminDocs = read(paths.adminDocs);
const receiptEntity = read(paths.receiptEntity);
const resolutionEntity = read(paths.resolutionEntity);
const error = read(paths.error);
const receiptMigration = read(paths.receiptMigration);
const solutionMigration = read(paths.solutionMigration);
const canonicalMigration = read(paths.canonicalMigration);
const resolutionMigration = read(paths.resolutionMigration);
const crossMigration = read(paths.crossMigration);
const migrationsMod = read(paths.migrationsMod);
const service = read(paths.service);
const stats = read(paths.stats);
const canonicalService = read(paths.canonicalService);
const facade = read(paths.facade);
const redirect = read(paths.redirect);
const graphql = read(paths.graphql);
const ordinaryTest = read(paths.ordinaryTest);
const crossTest = read(paths.crossTest);
const resolutionTest = read(paths.resolutionTest);
const canonicalTest = read(paths.canonicalTest);
const graphqlTest = read(paths.graphqlTest);
const resolutionGraphqlTest = read(paths.resolutionGraphqlTest);
const leptosModel = read(paths.leptosModel);
const leptosUi = read(paths.leptosUi);
const nextCore = read(paths.nextCore);
const nextUi = read(paths.nextUi);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_merge_owner_v1");
assert.equal(contract.task, "FORUM-21B");
assert.equal(contract.latest_policy_slice, "FORUM-21L");
assert.equal(contract.latest_category_slice, "FORUM-21M");
assert.equal(contract.latest_admin_ui_slice, "FORUM-21N");
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
  "m20260803_000019_allow_cross_category_topic_merge_redirect_edges",
]);
assert.equal(contract.semantic_event.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event.schema_version, 1);
assert.equal(contract.semantic_event.payload_changed_by_solution_resolution, false);
assert.equal(contract.semantic_event.payload_changed_by_cross_category_merge, false);
assert.equal(contract.semantic_event.category_id_is_retained_target_category, true);
assert.equal(contract.bounds.same_category_only, false);
assert.equal(contract.bounds.cross_category_supported, true);
assert.equal(contract.solution_resolution.task, "FORUM-21L");
assert.equal(contract.solution_resolution.audit_table, "forum_topic_merge_solution_resolutions");
assert.equal(contract.solution_resolution.event_contract_changed, false);
assert.equal(contract.cross_category_merge.task, "FORUM-21M");
assert.equal(contract.cross_category_merge.source_tombstone_retains_source_category, true);
assert.equal(contract.cross_category_merge.target_retains_target_category, true);
assert.equal(contract.cross_category_merge.source_category_topic_count_delta, 0);
assert.equal(contract.cross_category_merge.target_category_topic_count_delta, 0);
assert.equal(
  contract.cross_category_merge.source_category_reply_count_delta,
  "-moved_published_reply_count",
);
assert.equal(
  contract.cross_category_merge.target_category_reply_count_delta,
  "+moved_published_reply_count",
);
assert.equal(
  contract.cross_category_merge.redirect_edge_migration,
  "m20260803_000019_allow_cross_category_topic_merge_redirect_edges",
);
assert.equal(contract.cross_category_merge.source_category_may_differ_from_receipt_category, true);
assert.equal(contract.cross_category_merge.target_category_must_equal_receipt_category, true);
assert.equal(contract.cross_category_merge.receipt_schema_changed, false);
assert.equal(contract.cross_category_merge.event_contract_changed, false);
assert.equal(contract.cross_category_merge.migration_added, true);
assert.equal(contract.canonical_resolution.source_of_truth, "forum_topic_merge_operations");
assert.equal(contract.canonical_resolution.rest_merged_source_returns_308, true);
assert.equal(contract.graphql_transport.merge_field, "mergeForumTopic");
assert.equal(
  contract.graphql_transport.solution_resolution_field,
  "mergeForumTopicResolvingSolution",
);
assert.equal(
  contract.graphql_transport.commands_inherit_cross_category_owner_policy_without_schema_change,
  true,
);
assert.equal(contract.admin_ui.task, "FORUM-21N");
assert.equal(contract.admin_ui.leptos_route, "/modules/forum/merge");
assert.equal(contract.admin_ui.next_admin_route, "/dashboard/forum/merge");
assert.equal(contract.admin_ui.required_permission, "forum_topics:manage");
assert.equal(contract.admin_ui.candidate_limit, 100);
assert.equal(contract.admin_ui.exact_retry_reuses_operation_id, true);
assert.equal(contract.admin_ui.both_solved_require_explicit_winner, true);
assert.equal(contract.admin_ui.native_leptos_owner_path_claimed, false);
assert.equal(contract.admin_ui.backend_owner_or_schema_changed, false);

assert.equal(crossContract.contract, "forum_topic_merge_cross_category_v1");
assert.equal(crossContract.task, "FORUM-21M");
assert.equal(crossContract.semantic_event_compatibility.schema_version, 1);
assert.equal(crossContract.semantic_event_compatibility.payload_changed, false);
assert.equal(crossContract.semantic_event_compatibility.receipt_schema_changed, false);
assert.equal(
  crossContract.redirect_edge_migration.migration,
  "m20260803_000019_allow_cross_category_topic_merge_redirect_edges",
);
assert.equal(crossContract.migration_added, true);
assert.equal(solutionContract.latest_resolution_slice, "FORUM-21L");
assert.equal(solutionContract.compatibility.forum_topic_merged_event_changed, false);
assert.equal(resolutionContract.task, "FORUM-21L");
assert.equal(resolutionContract.semantic_event_compatibility.schema_version, 1);
assert.equal(resolutionContract.semantic_event_compatibility.payload_changed, false);
assert.equal(canonicalContract.task, "FORUM-21I");
assert.equal(canonicalContract.latest_transport_slice, "FORUM-21J");
assert.equal(graphqlContract.latest_resolution_slice, "FORUM-21L");
assert.equal(adminContract.task, "FORUM-21N");
assert.equal(adminContract.compatibility.backend_owner_changed, false);
assert.equal(adminContract.compatibility.graphql_schema_changed, false);
assert.equal(adminContract.compatibility.migration_added, false);

includesAll(
  receiptEntity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_operations")]',
    "pub operation_id: Uuid",
    "pub source_topic_id: Uuid",
    "pub target_topic_id: Uuid",
    "pub category_id: Uuid",
    "pub event_id: Uuid",
  ],
  "merge receipt entity",
);
includesAll(
  resolutionEntity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_solution_resolutions")]',
    "pub selected_solution_reply_id: Uuid",
    "pub rejected_solution_reply_id: Uuid",
  ],
  "solution-resolution entity",
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
    "m20260803_000019_allow_cross_category_topic_merge_redirect_edges",
    "m20260803_000019_allow_cross_category_topic_merge_redirect_edges::Migration",
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
    "forum_validate_topic_solution_target",
    "31",
  ],
  "solution migration",
);
includesAll(
  canonicalMigration,
  [
    "uq_forum_topic_merge_operations_source",
    "forum_validate_topic_merge_redirect_edge",
    "source.category_id = NEW.category_id",
    "target.category_id = NEW.category_id",
  ],
  "historical canonical migration",
);
includesAll(
  resolutionMigration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_solution_resolutions",
    "forum topic merge solution resolutions are append-only",
  ],
  "solution-resolution migration",
);
includesAll(
  crossMigration,
  [
    "CREATE OR REPLACE FUNCTION forum_validate_topic_merge_redirect_edge()",
    "source.id = NEW.source_topic_id",
    "source.status::text = 'archived'",
    "source.is_locked = TRUE",
    "source.reply_count = 0",
    "target.category_id = NEW.category_id",
    "DROP TRIGGER IF EXISTS forum_05_topic_merge_redirect_edge",
    "const POSTGRES_DOWN",
    "const SQLITE_DOWN",
  ],
  "cross-category redirect migration",
);
const postgresUp = crossMigration.slice(
  crossMigration.indexOf('const POSTGRES_UP: &str = r#"'),
  crossMigration.indexOf('const POSTGRES_DOWN: &str = r#"'),
);
const sqliteUp = crossMigration.slice(
  crossMigration.indexOf('const SQLITE_UP: &str = r#"'),
  crossMigration.indexOf('const SQLITE_DOWN: &str = r#"'),
);
assert.ok(!postgresUp.includes("source.category_id = NEW.category_id"));
assert.ok(!sqliteUp.includes("source.category_id = NEW.category_id"));
assert.ok(postgresUp.includes("target.category_id = NEW.category_id"));
assert.ok(sqliteUp.includes("target.category_id = NEW.category_id"));

includesAll(
  service,
  [
    "pub async fn merge_topic(",
    "pub async fn merge_topic_resolving_solution(",
    "async fn merge_topic_internal(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "lock_topic_merge_tenant_in_tx(&txn, tenant_id).await?;",
    "validate_existing_semantic_event_in_tx(&txn, &existing)",
    "load_solution_resolution_audit_in_tx(",
    "lock_merge_counter_scopes_in_tx(",
    "categories.sort();",
    "categories.dedup();",
    "lock_topics_in_tx(&txn",
    "ensure_categories_active_in_tx(",
    "transfer_cross_category_reply_counters_in_tx(",
    "source.reply_count < moved_published_reply_count",
    ".checked_sub(moved_published_reply_count)",
    ".checked_add(moved_published_reply_count)",
    "plan_solution_merge(",
    "UserStatsService::adjust_solution_count_in_tx",
    "move_replies_in_tx(",
    "source_active.status = Set(TopicStatus::Archived);",
    "source_active.is_locked = Set(true);",
    "schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION)",
    "forum_domain_event::ActiveModel",
    "forum_topic_merge_operation::ActiveModel",
    "forum_topic_merge_solution_resolution::ActiveModel",
    "if target_category_id != source_category_id",
    "txn.commit().await?;",
  ],
  "merge owner",
);
assert.equal((service.match(/self\.db\.begin\(\)\.await\?/g) ?? []).length, 1);
assert.equal((service.match(/publish_forum_topic_projection_in_tx\(/g) ?? []).length, 2);
assert.equal((service.match(/publish_forum_category_projection_in_tx\(/g) ?? []).length, 2);
assert.ok(!service.includes("Forum topic merge requires source and target topics in the same category"));
assert.ok(!service.includes("FORUM_TOPIC_MERGED_CROSS_CATEGORY_SCHEMA_VERSION"));
assert.ok(!service.includes('"source_category_id"'));

const receiptLookup = service.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const preliminaryRead = service.indexOf("let preliminary_source =");
const solutionPlan = service.indexOf("let solution_plan = plan_solution_merge");
const counterTransfer = service.indexOf("transfer_cross_category_reply_counters_in_tx(");
const replyMove = service.indexOf("move_replies_in_tx(", counterTransfer);
const eventInsert = service.indexOf("forum_domain_event::ActiveModel", replyMove);
const receiptInsert = service.indexOf("forum_topic_merge_operation::ActiveModel", eventInsert);
const auditInsert = service.indexOf("forum_topic_merge_solution_resolution::ActiveModel", receiptInsert);
const invalidation = service.indexOf("publish_forum_topic_projection_in_tx(", receiptInsert);
assert.ok(receiptLookup < preliminaryRead);
assert.ok(preliminaryRead < solutionPlan && solutionPlan < counterTransfer);
assert.ok(counterTransfer < replyMove && replyMove < eventInsert);
assert.ok(eventInsert < receiptInsert && receiptInsert < auditInsert && auditInsert < invalidation);

includesAll(
  stats,
  [
    "if delta == -1",
    "solution_count = solution_count - 1",
    "solution_count > 0",
    "rows_affected() != 1",
  ],
  "solution statistics",
);
includesAll(
  canonicalService,
  [
    "MAX_FORUM_TOPIC_CANONICAL_REDIRECT_HOPS: usize = 32",
    "load_resolution_step(",
    "LIMIT 2",
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
  redirect,
  [
    "StatusCode::PERMANENT_REDIRECT",
    '(CACHE_CONTROL, "private, no-store".to_string())',
  ],
  "REST redirect",
);
includesAll(
  graphql,
  [
    "async fn merge_forum_topic(",
    "async fn merge_forum_topic_resolving_solution(",
    "Permission::FORUM_TOPICS_MANAGE",
    ".merge_topic(",
    ".merge_topic_resolving_solution(",
  ],
  "GraphQL transport",
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
  ordinaryTest,
  [
    "topic_merge_is_atomic_idempotent_and_append_only",
    "topic_merge_transfers_source_only_solution_and_preserves_target_only_solution",
    "topic_merge_rejects_competing_solutions_without_partial_state",
  ],
  "ordinary merge regression",
);
includesAll(
  crossTest,
  [
    "cross_category_topic_merge_transfers_published_reply_counters_once",
    "cross_category_topic_merge_rolls_back_on_source_counter_drift",
    "assert_eq!(new_projection_ids.len(), 4);",
    '!payload_object.contains_key("source_category_id")',
    'error.stable_code(), "FORUM_VALIDATION_FAILED"',
  ],
  "cross-category merge regression",
);
includesAll(
  resolutionTest,
  [
    "manager_can_select_source_solution_and_replay_exact_audit",
    "manager_can_select_target_solution_and_invalid_selection_is_atomic",
    "forum_topic_merge_solution_resolutions",
  ],
  "solution-resolution regression",
);
includesAll(
  canonicalTest,
  [
    "merged_topic_ids_resolve_to_one_visible_canonical_target",
    "assert_eq!(selected.id, topic_c)",
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
    "resolution_audit_is_append_only_and_keeps_merge_event_schema_one",
  ],
  "resolution GraphQL contract",
);
includesAll(
  leptosModel,
  [
    "build_forum_topic_merge_command",
    "new_forum_topic_merge_operation_id",
    "Choose which accepted solution must remain",
  ],
  "Leptos admin merge policy",
);
includesAll(
  leptosUi,
  ["transport::merge_topic", "solution_choice_required", "set_refresh_nonce.update"],
  "Leptos admin merge UI",
);
includesAll(
  nextCore,
  [
    "buildForumTopicMergeCommand",
    "newForumTopicMergeOperationId",
    "Choose which accepted solution must remain",
  ],
  "Next admin merge policy",
);
includesAll(
  nextUi,
  ["mergeForumTopics", "commandShapeChanged", "router.refresh()"],
  "Next admin merge UI",
);

includesAll(
  docs,
  [
    "# FORUM-21B idempotent topic merge owner",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    paths.crossContract,
    paths.adminContract,
    "FORUM-21A through FORUM-21N",
    "Cross-category category counters",
    "Admin composition",
    "mergeForumTopicResolvingSolution",
    "forum.topic.merged / schema version 1",
    "No command above was run by the implementation agent",
  ],
  "merge owner handoff",
);
includesAll(
  crossDocs,
  [
    "# FORUM-21M checked cross-category topic merge",
    "m20260803_000019_allow_cross_category_topic_merge_redirect_edges",
    "forum.topic.merged / schema version 1",
  ],
  "cross-category handoff",
);
includesAll(
  adminDocs,
  [
    "# FORUM-21N admin topic merge workflow",
    "single-adapter GraphQL state",
    "/modules/forum/merge",
    "/dashboard/forum/merge",
  ],
  "admin merge handoff",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("### Delivered through `FORUM-21N`"));
assert.ok(plan.includes("checked cross-category merge"));
assert.ok(plan.includes("admin topic merge workflow"));
assert.ok(plan.includes("m20260803_000019_allow_cross_category_topic_merge_redirect_edges"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));

console.log(
  "FORUM-21B/H/I/J/K/L/M/N topic merge owner and admin composition source are ready; FORUM-21 and FORUM-24 remain planned.",
);
