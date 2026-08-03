#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json",
  policyContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-policy.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21l-topic-merge-solution-resolution.md",
  policyDocs: "crates/rustok-forum/docs/forum-21h-topic-merge-solution-policy.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  entity: "crates/rustok-forum/src/entities/forum_topic_merge_solution_resolution.rs",
  entitiesMod: "crates/rustok-forum/src/entities/mod.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260803_000018_add_forum_topic_merge_solution_resolution.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_merge.rs",
  stats: "crates/rustok-forum/src/services/user_stats.rs",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  sqliteTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
  graphqlTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
  subscriptionReconciliation:
    "crates/rustok-forum/src/services/topic_merge_subscription_reconciliation.rs",
  readStateReconciliation:
    "crates/rustok-forum/src/services/topic_merge_read_state_reconciliation.rs",
  tagReconciliation: "crates/rustok-forum/src/services/topic_merge_tag_reconciliation.rs",
  voteReconciliation: "crates/rustok-forum/src/services/topic_merge_vote_reconciliation.rs",
  audienceReconciliation:
    "crates/rustok-forum/src/services/topic_merge_audience_reconciliation.rs",
  readme: "crates/rustok-forum/README.md",
  docsIndex: "crates/rustok-forum/docs/README.md",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const policyContract = JSON.parse(read(paths.policyContract));
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const policyDocs = read(paths.policyDocs);
const cumulativeDocs = read(paths.cumulativeDocs);
const entity = read(paths.entity);
const entitiesMod = read(paths.entitiesMod);
const migration = read(paths.migration);
const migrationsMod = read(paths.migrationsMod);
const owner = read(paths.owner);
const stats = read(paths.stats);
const graphql = read(paths.graphql);
const graphqlMod = read(paths.graphqlMod);
const sqliteTest = read(paths.sqliteTest);
const graphqlTest = read(paths.graphqlTest);
const readme = read(paths.readme);
const docsIndex = read(paths.docsIndex);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_merge_solution_resolution_v1");
assert.equal(contract.task, "FORUM-21L");
assert.equal(contract.parent_task, "FORUM-21");
assert.deepEqual(contract.extends, ["FORUM-21B", "FORUM-21H", "FORUM-21K"]);
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.ordinary_owner_method, "merge_topic");
assert.equal(contract.resolution_owner_method, "merge_topic_resolving_solution");
assert.equal(contract.shared_private_transaction_owner, "merge_topic_internal");
assert.equal(contract.required_permission, "forum_topics:manage");
assert.equal(contract.selection.requires_exactly_two_valid_competing_solutions, true);
assert.equal(contract.selection.must_equal_source_or_target_solution_reply_id, true);
assert.equal(
  contract.selection.ordinary_merge_with_two_solutions,
  "fail_before_mutation_with_FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
);
assert.equal(contract.source_winner.preserve_marked_by_user_id, true);
assert.equal(contract.source_winner.preserve_marked_at, true);
assert.equal(contract.source_winner.decrement_target_solution_author_exactly_once, true);
assert.equal(contract.target_winner.preserve_target_marker, true);
assert.equal(contract.target_winner.decrement_source_solution_author_exactly_once, true);
assert.equal(contract.statistics.winner_solution_count_delta, 0);
assert.equal(contract.statistics.loser_solution_count_delta, -1);
assert.equal(contract.statistics.negative_solution_transitions_use_atomic_exact_decrement, true);
assert.equal(
  contract.audit.migration,
  "m20260803_000018_add_forum_topic_merge_solution_resolution",
);
assert.equal(contract.audit.table, "forum_topic_merge_solution_resolutions");
assert.deepEqual(contract.audit.primary_key, ["tenant_id", "operation_id"]);
assert.equal(contract.audit.append_only_on_postgresql_and_sqlite, true);
assert.equal(contract.audit.selection_pair_consistency_is_database_checked, true);
assert.equal(contract.semantic_event_compatibility.event_type, "forum.topic.merged");
assert.equal(contract.semantic_event_compatibility.schema_version, 1);
assert.equal(contract.semantic_event_compatibility.payload_changed, false);
assert.equal(
  contract.semantic_event_compatibility.subscription_reconciliation_remains_compatible,
  true,
);
assert.equal(
  contract.semantic_event_compatibility.read_state_reconciliation_remains_compatible,
  true,
);
assert.equal(contract.semantic_event_compatibility.tag_reconciliation_remains_compatible, true);
assert.equal(contract.semantic_event_compatibility.vote_reconciliation_remains_compatible, true);
assert.equal(
  contract.semantic_event_compatibility.audience_reconciliation_remains_compatible,
  true,
);
assert.equal(contract.idempotency.receipt_table_changed, false);
assert.equal(contract.idempotency.replay_validates_exact_schema_one_event, true);
assert.equal(contract.idempotency.replay_loads_optional_append_only_resolution_audit, true);
assert.equal(contract.idempotency.same_selection_returns_same_receipt, true);
assert.equal(contract.idempotency.selection_drift, "FORUM_TOPIC_MERGE_OPERATION_CONFLICT");
assert.equal(contract.graphql.field, "mergeForumTopicResolvingSolution");
assert.equal(contract.graphql.required_permission, "forum_topics:manage");
assert.equal(contract.graphql.canonical_source_alias_resolution, false);
assert.equal(contract.compatibility.audit_migration_added, true);
assert.equal(contract.compatibility.event_schema_or_payload_changed, false);
assert.equal(contract.compatibility.post_merge_reconciliation_owner_changed, false);

assert.equal(policyContract.latest_resolution_slice, "FORUM-21L");
assert.equal(policyContract.explicit_resolution_operation, "merge_topic_resolving_solution");
assert.equal(policyContract.compatibility.forum_topic_merged_event_changed, false);
assert.equal(policyContract.compatibility.forum_topic_merged_schema_version, 1);
assert.equal(policyContract.compatibility.resolution_audit_migration_added, true);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21L");
assert.equal(cumulativeContract.semantic_event.schema_version, 1);
assert.equal(cumulativeContract.semantic_event.payload_changed_by_solution_resolution, false);
assert.equal(cumulativeContract.solution_resolution.audit_migration_added, true);
assert.equal(cumulativeContract.solution_resolution.receipt_schema_changed, false);
assert.equal(cumulativeContract.solution_resolution.event_contract_changed, false);

includesAll(
  entity,
  [
    '#[sea_orm(table_name = "forum_topic_merge_solution_resolutions")]',
    "pub tenant_id: Uuid",
    "pub operation_id: Uuid",
    "pub source_solution_reply_id: Uuid",
    "pub target_solution_reply_id: Uuid",
    "pub selected_solution_reply_id: Uuid",
    "pub rejected_solution_reply_id: Uuid",
    "pub rejected_solution_author_id: Option<Uuid>",
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
  "solution-resolution entity registration",
);
includesAll(
  migrationsMod,
  [
    "mod m20260803_000018_add_forum_topic_merge_solution_resolution;",
    "Box::new(m20260803_000018_add_forum_topic_merge_solution_resolution::Migration)",
  ],
  "solution-resolution migration registration",
);
includesAll(
  migration,
  [
    "CREATE TABLE IF NOT EXISTS forum_topic_merge_solution_resolutions",
    "PRIMARY KEY (tenant_id, operation_id)",
    "REFERENCES forum_topic_merge_operations (tenant_id, operation_id)",
    "REFERENCES forum_replies (tenant_id, id)",
    "REFERENCES users (tenant_id, id)",
    "selected_solution_reply_id = source_solution_reply_id",
    "selected_solution_reply_id = target_solution_reply_id",
    "forum topic merge solution resolutions are append-only",
    "BEFORE UPDATE ON forum_topic_merge_solution_resolutions",
    "BEFORE DELETE ON forum_topic_merge_solution_resolutions",
  ],
  "solution-resolution migration",
);

includesAll(
  owner,
  [
    "const FORUM_TOPIC_MERGED_SCHEMA_VERSION: i16 = 1;",
    "struct ForumTopicMergeSolutionCandidate",
    "struct ForumTopicMergeSolutionResolutionAudit",
    "struct ForumTopicMergeSolutionPlan",
    "pub async fn merge_topic(",
    "pub async fn merge_topic_resolving_solution(",
    "async fn merge_topic_internal(",
    "self.merge_topic_internal(tenant_id, target_topic_id, security, None, input)",
    "Some(selected_solution_reply_id)",
    "validate_existing_semantic_event_in_tx(&txn, &existing)",
    "load_solution_resolution_audit_in_tx(",
    "TopicMergeOperationConflict(input.operation_id)",
    "plan_solution_merge(",
    "TopicMergeSolutionConflict(operation_id)",
    "selected == source.reply_id",
    "selected == target.reply_id",
    "delete_solution_in_tx(&txn, tenant_id, source.id, \"source\")",
    "delete_solution_in_tx(&txn, tenant_id, target.id, \"target\")",
    "UserStatsService::adjust_solution_count_in_tx",
    "solution_plan.losing_solution_author_id",
    "insert_transferred_solution_in_tx",
    "schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION)",
    "forum_topic_merge_operation::ActiveModel",
    "forum_topic_merge_solution_resolution::ActiveModel",
    "validate_solution_resolution_audit",
  ],
  "merge solution-resolution owner",
);
assert.equal((owner.match(/self\.db\.begin\(\)\.await\?/g) ?? []).length, 1);
assert.ok(!owner.includes("FORUM_TOPIC_MERGED_SOLUTION_RESOLUTION_SCHEMA_VERSION"));
assert.ok(!owner.includes('"solution_resolution"'));
const replayLookup = owner.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const replayEvent = owner.indexOf("validate_existing_semantic_event_in_tx(&txn, &existing)");
const replayAudit = owner.indexOf("load_solution_resolution_audit_in_tx(");
const preliminaryRead = owner.indexOf("let preliminary_source =");
const solutionPlan = owner.indexOf("let solution_plan = plan_solution_merge");
const sourceDelete = owner.indexOf("delete_solution_in_tx(&txn, tenant_id, source.id");
const statChange = owner.indexOf("UserStatsService::adjust_solution_count_in_tx");
const replyMove = owner.indexOf("move_replies_in_tx(", sourceDelete);
const eventInsert = owner.indexOf("forum_domain_event::ActiveModel");
const receiptInsert = owner.indexOf("forum_topic_merge_operation::ActiveModel");
const auditInsert = owner.indexOf("forum_topic_merge_solution_resolution::ActiveModel");
const invalidations = owner.indexOf("publish_forum_topic_projection_in_tx(");
assert.ok(replayLookup < replayEvent && replayEvent < replayAudit && replayAudit < preliminaryRead);
assert.ok(preliminaryRead < solutionPlan && solutionPlan < sourceDelete);
assert.ok(sourceDelete < statChange && statChange < replyMove);
assert.ok(replyMove < eventInsert && eventInsert < receiptInsert);
assert.ok(receiptInsert < auditInsert && auditInsert < invalidations);

for (const forbidden of [
  "ForumTopicMoveService",
  "resolve_canonical_topic",
  "forum_topic_alias",
  "forum_topic_redirects",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!owner.includes(forbidden), `owner contains forbidden marker: ${forbidden}`);
}

includesAll(
  stats,
  [
    "pub(crate) async fn adjust_solution_count_in_tx(",
    "if delta == -1",
    "decrement_solution_count_exact_in_tx",
    "solution_count = solution_count - 1",
    "solution_count > 0",
    "rows_affected() != 1",
    "Forum solution author statistic is inconsistent",
  ],
  "exact solution statistic owner",
);

includesAll(
  graphql,
  [
    "async fn merge_forum_topic(",
    "async fn merge_forum_topic_resolving_solution(",
    "ResolveForumTopicMergeSolutionGraphqlInput",
    "GqlForumTopicMergeSolutionResolution",
    "selected_solution_reply_id = input.selected_solution_reply_id",
    ".merge_topic_resolving_solution(",
    "Permission::FORUM_TOPICS_MANAGE",
    "Permission denied: tenant scope mismatch",
  ],
  "solution-resolution GraphQL adapter",
);
includesAll(
  graphqlMod,
  [
    "GqlForumTopicMergeSolutionResolution",
    "ResolveForumTopicMergeSolutionGraphqlInput",
    "topic_merge_mutation::ForumTopicMergeMutation",
  ],
  "GraphQL exports",
);
for (const forbidden of [
  "resolve_canonical_topic",
  "forum_topic_merge_operations",
  "forum_topic_merge_solution_resolutions",
  "forum_solutions::",
  "TopicService::new",
]) {
  assert.ok(!graphql.includes(forbidden), `GraphQL adapter contains forbidden marker: ${forbidden}`);
}

for (const [label, path] of [
  ["subscription reconciliation", paths.subscriptionReconciliation],
  ["read-state reconciliation", paths.readStateReconciliation],
  ["tag reconciliation", paths.tagReconciliation],
  ["vote reconciliation", paths.voteReconciliation],
  ["audience reconciliation", paths.audienceReconciliation],
]) {
  const source = read(path);
  includesAll(
    source,
    ["event.schema_version != 1", "event.payload != expected_payload"],
    label,
  );
  assert.ok(!source.includes("schema_version != 2"), `${label} contains schema-2 fallback`);
  assert.ok(!source.includes("solution_resolution"), `${label} reads resolution audit`);
}

includesAll(
  sqliteTest,
  [
    "manager_can_select_source_solution_and_replay_exact_audit",
    "manager_can_select_target_solution_and_invalid_selection_is_atomic",
    "ordinary merge must keep competing solutions fail-closed",
    "assert_merge_event_and_resolution_audit",
    'assert_eq!(event.try_get::<i16>("", "schema_version")?, 1);',
    'assert!(payload.get("solution_resolution").is_none());',
    "forum_topic_merge_solution_resolutions",
    "assert_eq!(replay, merged);",
    "UPDATE forum_topic_merge_solution_resolutions",
    "DELETE FROM forum_topic_merge_solution_resolutions",
    "TopicMergeOperationConflict",
    "FORUM_VALIDATION_FAILED",
  ],
  "solution-resolution SQLite regression",
);
includesAll(
  graphqlTest,
  [
    "graphql_schema_exposes_explicit_solution_resolution_command",
    '"mergeForumTopicResolvingSolution"',
    '"ResolveForumTopicMergeSolutionGraphqlInput"',
    '"GqlForumTopicMergeSolutionResolution"',
    '"selectedSolutionReplyId"',
    "ordinary_and_resolved_commands_share_one_private_transaction_owner",
    "resolution_audit_is_append_only_and_keeps_merge_event_schema_one",
  ],
  "solution-resolution GraphQL contract",
);

includesAll(
  docs,
  [
    "# FORUM-21L competing accepted-solution resolution",
    "`source_ready_maintainer_execution_pending`",
    "merge_topic_resolving_solution",
    "forum_topic_merge_solution_resolutions",
    "forum.topic.merged / schema version 1",
    "append-only",
    "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
    "FORUM-21` entry remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21L handoff",
);
includesAll(
  policyDocs,
  [
    "FORUM-21L",
    "## Explicit competing-solution resolution",
    "## Append-only resolution audit",
    "schema version 1",
  ],
  "solution policy handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21A through FORUM-21L",
    "mergeForumTopicResolvingSolution",
    "Resolution audit ledger",
    "schema version 1",
    "native/admin merge command composition and merge/resolution UI",
  ],
  "cumulative merge handoff",
);
includesAll(
  readme,
  [
    "`mergeForumTopicResolvingSolution`",
    "`forum_topic_merge_solution_resolutions`",
    "`forum.topic.merged` schema-1",
    "`ForumTopicMergeService::merge_topic_resolving_solution`",
  ],
  "Forum README",
);
includesAll(
  docsIndex,
  [
    "explicit manager-selected reply identity",
    "FORUM-21L competing solution resolution",
  ],
  "Forum docs index",
);

assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("## `FORUM-21` — move, merge, split and fork topics"));
assert.ok(plan.includes("**Status:** `planned`"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));

console.log(
  "FORUM-21L competing solution resolution source is ready; canonical FORUM-21 remains planned.",
);
