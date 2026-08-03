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
  owner: "crates/rustok-forum/src/services/topic_merge.rs",
  stats: "crates/rustok-forum/src/services/user_stats.rs",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  sqliteTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
  graphqlTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
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
assert.equal(contract.audit.event_type, "forum.topic.merged");
assert.equal(contract.audit.ordinary_schema_version, 1);
assert.equal(contract.audit.resolved_schema_version, 2);
assert.equal(contract.audit.resolution_payload_key, "solution_resolution");
assert.deepEqual(contract.audit.fields, [
  "source_solution_reply_id",
  "target_solution_reply_id",
  "selected_solution_reply_id",
  "rejected_solution_reply_id",
  "rejected_solution_author_id",
]);
assert.equal(contract.audit.shared_rustok_events_contract_changed, false);
assert.equal(contract.idempotency.receipt_table_changed, false);
assert.equal(contract.idempotency.same_selection_returns_same_receipt, true);
assert.equal(contract.idempotency.selection_drift, "FORUM_TOPIC_MERGE_OPERATION_CONFLICT");
assert.equal(contract.graphql.field, "mergeForumTopicResolvingSolution");
assert.equal(contract.graphql.required_permission, "forum_topics:manage");
assert.equal(contract.graphql.canonical_source_alias_resolution, false);
assert.equal(contract.compatibility.migration_added, false);
assert.equal(contract.compatibility.ordinary_owner_method_changed, false);
assert.equal(contract.compatibility.ordinary_graphql_field_changed, false);

assert.equal(policyContract.latest_resolution_slice, "FORUM-21L");
assert.equal(policyContract.explicit_resolution_operation, "merge_topic_resolving_solution");
assert.equal(policyContract.compatibility.resolved_event_schema_version_added, 2);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21L");
assert.equal(cumulativeContract.semantic_event.ordinary_schema_version, 1);
assert.equal(cumulativeContract.semantic_event.solution_resolution_schema_version, 2);
assert.equal(cumulativeContract.solution_resolution.task, "FORUM-21L");
assert.equal(
  cumulativeContract.solution_resolution.selection_or_command_shape_drift,
  "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
);
assert.equal(cumulativeContract.solution_resolution.receipt_schema_changed, false);
assert.equal(cumulativeContract.solution_resolution.migration_added, false);

includesAll(
  owner,
  [
    "const FORUM_TOPIC_MERGED_SCHEMA_VERSION: i16 = 1;",
    "const FORUM_TOPIC_MERGED_SOLUTION_RESOLUTION_SCHEMA_VERSION: i16 = 2;",
    "struct ForumTopicMergeSolutionCandidate",
    "struct ForumTopicMergeSolutionResolutionAudit",
    "struct ForumTopicMergeSolutionPlan",
    "pub async fn merge_topic(",
    "pub async fn merge_topic_resolving_solution(",
    "async fn merge_topic_internal(",
    "self.merge_topic_internal(tenant_id, target_topic_id, security, None, input)",
    "Some(selected_solution_reply_id)",
    "validate_existing_semantic_event_in_tx(&txn, &existing)",
    "stored_resolution",
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
    "solution_resolution",
    "source_solution_reply_id",
    "target_solution_reply_id",
    "selected_solution_reply_id",
    "rejected_solution_reply_id",
    "rejected_solution_author_id",
    "validate_solution_resolution_audit",
  ],
  "merge solution-resolution owner",
);
assert.equal(owner.match(/self\.db\.begin\(\)\.await\?/g)?.length, 1);
const replayLookup = owner.indexOf("forum_topic_merge_operation::Entity::find_by_id");
const preliminaryRead = owner.indexOf("let preliminary_source =");
const solutionPlan = owner.indexOf("let solution_plan = plan_solution_merge");
const sourceDelete = owner.indexOf("delete_solution_in_tx(&txn, tenant_id, source.id");
const statChange = owner.indexOf("UserStatsService::adjust_solution_count_in_tx");
const replyMove = owner.indexOf("move_replies_in_tx(", sourceDelete);
const eventInsert = owner.indexOf("forum_domain_event::ActiveModel");
const receiptInsert = owner.indexOf("forum_topic_merge_operation::ActiveModel");
assert.ok(replayLookup >= 0 && replayLookup < preliminaryRead);
assert.ok(preliminaryRead < solutionPlan && solutionPlan < sourceDelete);
assert.ok(sourceDelete < statChange && statChange < replyMove);
assert.ok(replyMove < eventInsert && eventInsert < receiptInsert);

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
assert.ok(!stats.includes("solution_count = CASE"));

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
  "forum_solutions::",
  "TopicService::new",
]) {
  assert.ok(!graphql.includes(forbidden), `GraphQL adapter contains forbidden marker: ${forbidden}`);
}

includesAll(
  sqliteTest,
  [
    "manager_can_select_source_solution_and_replay_exact_audit",
    "manager_can_select_target_solution_and_invalid_selection_is_atomic",
    "ordinary merge must keep competing solutions fail-closed",
    "assert_resolution_event",
    "schema_version",
    "assert_eq!(first, replay)".replace("first", "replay"),
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
  ],
  "solution-resolution GraphQL contract",
);

includesAll(
  docs,
  [
    "# FORUM-21L competing accepted-solution resolution",
    "`source_ready_maintainer_execution_pending`",
    "merge_topic_resolving_solution",
    "mergeForumTopicResolvingSolution",
    "schema version 2",
    "one exact decrement",
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
    "Fail-closed statistics",
    "schema version 2",
  ],
  "solution policy handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21A through FORUM-21L",
    "mergeForumTopicResolvingSolution",
    "solution_resolution.selected_solution_reply_id",
    "native/admin merge command composition and merge/resolution UI",
  ],
  "cumulative merge handoff",
);
includesAll(
  readme,
  [
    "`mergeForumTopicResolvingSolution`",
    "`ForumTopicMergeService::merge_topic_resolving_solution`",
    "`graphql::ResolveForumTopicMergeSolutionGraphqlInput`",
    "`graphql::GqlForumTopicMergeSolutionResolution`",
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
