#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json",
  resolutionContract: "crates/rustok-forum/contracts/forum-topic-merge-solution-resolution.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21k-topic-merge-graphql-transport.md",
  resolutionDocs: "crates/rustok-forum/docs/forum-21l-topic-merge-solution-resolution.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_merge.rs",
  ordinarySchemaTest: "crates/rustok-forum/tests/topic_merge_graphql_contract.rs",
  resolutionSchemaTest:
    "crates/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
  resolutionRuntimeTest: "crates/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
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
const resolutionContract = JSON.parse(read(paths.resolutionContract));
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const resolutionDocs = read(paths.resolutionDocs);
const cumulativeDocs = read(paths.cumulativeDocs);
const graphql = read(paths.graphql);
const graphqlMod = read(paths.graphqlMod);
const owner = read(paths.owner);
const ordinarySchemaTest = read(paths.ordinarySchemaTest);
const resolutionSchemaTest = read(paths.resolutionSchemaTest);
const resolutionRuntimeTest = read(paths.resolutionRuntimeTest);
const readme = read(paths.readme);
const docsIndex = read(paths.docsIndex);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_merge_graphql_transport_v1");
assert.equal(contract.task, "FORUM-21K");
assert.equal(contract.latest_resolution_slice, "FORUM-21L");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.transport, "graphql");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.authorization.module_must_be_enabled, "forum");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.tenant_is_derived_from_routed_TenantContext, true);
assert.equal(contract.authorization.optional_tenant_argument_must_equal_routed_tenant, true);
assert.equal(contract.ordinary_command.field, "mergeForumTopic");
assert.equal(contract.ordinary_command.input_type, "MergeForumTopicGraphqlInput");
assert.equal(contract.ordinary_command.result_type, "GqlForumTopicMerge");
assert.equal(
  contract.ordinary_command.two_solutions_without_selection,
  "FORUM_TOPIC_MERGE_SOLUTION_CONFLICT",
);
assert.equal(
  contract.solution_resolution_command.field,
  "mergeForumTopicResolvingSolution",
);
assert.equal(
  contract.solution_resolution_command.input_type,
  "ResolveForumTopicMergeSolutionGraphqlInput",
);
assert.equal(
  contract.solution_resolution_command.result_type,
  "GqlForumTopicMergeSolutionResolution",
);
assert.equal(
  contract.solution_resolution_command.calls_owner_method,
  "merge_topic_resolving_solution",
);
assert.equal(
  contract.solution_resolution_command.decision_audit_table,
  "forum_topic_merge_solution_resolutions",
);
assert.equal(contract.receipt_result.returns_immutable_owner_receipt, true);
assert.equal(contract.receipt_result.exact_replay_returns_same_result, true);
assert.equal(contract.composition.resolvers_contain_no_merge_business_logic, true);
assert.equal(
  contract.composition.ordinary_and_resolution_commands_share_one_owner_service_and_private_transaction,
  true,
);
assert.equal(contract.composition.topic_body_hydration_after_merge, false);
assert.equal(contract.composition.canonical_source_alias_resolution_for_mutation, false);
assert.equal(contract.composition.raw_solution_or_audit_table_access_in_resolver, false);
assert.equal(contract.compatibility.ordinary_field_changed, false);
assert.equal(contract.compatibility.ordinary_input_changed, false);
assert.equal(contract.compatibility.receipt_result_changed, false);
assert.equal(contract.compatibility.merge_event_schema_or_payload_changed, false);
assert.equal(contract.compatibility.post_merge_reconciliation_owner_changed, false);
assert.equal(contract.compatibility.solution_resolution_field_is_additive, true);
assert.equal(resolutionContract.task, "FORUM-21L");
assert.equal(resolutionContract.graphql.field, "mergeForumTopicResolvingSolution");
assert.equal(resolutionContract.semantic_event_compatibility.schema_version, 1);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21L");
assert.equal(
  cumulativeContract.graphql_transport.solution_resolution_field,
  "mergeForumTopicResolvingSolution",
);
assert.equal(cumulativeContract.semantic_event.schema_version, 1);

includesAll(
  graphql,
  [
    "pub(crate) struct ForumTopicMergeMutation",
    "async fn merge_forum_topic(",
    "async fn merge_forum_topic_resolving_solution(",
    "require_module_enabled(ctx, MODULE_SLUG).await?;",
    "ctx.data::<DatabaseConnection>()?",
    "ctx.data::<TransactionalEventBus>()?",
    "ctx.data::<AuthContext>()",
    "ctx.data::<TenantContext>()?",
    "Permission::FORUM_TOPICS_MANAGE",
    "Permission denied: forum_topics:manage required",
    "Permission denied: tenant scope mismatch",
    "SecurityContext::from_permission_snapshot",
    "ForumTopicMergeService::new(db.clone(), event_bus.clone())",
    ".merge_topic(",
    ".merge_topic_resolving_solution(",
    "selected_solution_reply_id = input.selected_solution_reply_id",
    "pub struct MergeForumTopicGraphqlInput",
    "pub struct ResolveForumTopicMergeSolutionGraphqlInput",
    "pub struct GqlForumTopicMerge",
    "pub struct GqlForumTopicMergeSolutionResolution",
  ],
  "topic merge GraphQL adapter",
);
assert.equal(
  (graphql.match(/pub\(crate\) struct ForumTopicMergeMutation/g) ?? []).length,
  1,
);
const ordinaryResolver = graphql.indexOf("async fn merge_forum_topic(");
const resolutionResolver = graphql.indexOf("async fn merge_forum_topic_resolving_solution(");
const permission = graphql.indexOf("require_topic_manage_permission(auth)?;", resolutionResolver);
const tenantScope = graphql.indexOf("resolve_tenant_scope(tenant, requested_tenant_id)?;", resolutionResolver);
const resolutionOwner = graphql.indexOf(".merge_topic_resolving_solution(", resolutionResolver);
assert.ok(ordinaryResolver >= 0 && ordinaryResolver < resolutionResolver);
assert.ok(permission > resolutionResolver && permission < tenantScope && tenantScope < resolutionOwner);
for (const forbidden of [
  "resolve_canonical_topic",
  "forum_topic_merge_operations",
  "forum_topic_merge_solution_resolutions",
  "forum_solutions::",
  "TopicService::new",
  "get_with_locale_fallback",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!graphql.includes(forbidden), `GraphQL adapter contains forbidden marker: ${forbidden}`);
}

includesAll(
  graphqlMod,
  [
    "mod topic_merge_mutation;",
    "GqlForumTopicMergeSolutionResolution",
    "ResolveForumTopicMergeSolutionGraphqlInput",
    "topic_merge_mutation::ForumTopicMergeMutation",
  ],
  "GraphQL root registration",
);
includesAll(
  owner,
  [
    "pub struct ForumTopicMergeService",
    "pub async fn merge_topic(",
    "pub async fn merge_topic_resolving_solution(",
    "async fn merge_topic_internal(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "TopicMergeSolutionConflict(operation_id)",
    "TopicMergeOperationConflict(input.operation_id)",
    "schema_version: Set(FORUM_TOPIC_MERGED_SCHEMA_VERSION)",
    "forum_topic_merge_solution_resolution::ActiveModel",
  ],
  "topic merge owner",
);
assert.equal((owner.match(/self\.db\.begin\(\)\.await\?/g) ?? []).length, 1);
assert.ok(!owner.includes("FORUM_TOPIC_MERGED_SOLUTION_RESOLUTION_SCHEMA_VERSION"));
assert.ok(!owner.includes('"solution_resolution"'));

includesAll(
  ordinarySchemaTest,
  [
    "graphql_schema_exposes_idempotent_topic_merge_command",
    '"mergeForumTopic"',
    '"MergeForumTopicGraphqlInput"',
    '"GqlForumTopicMerge"',
    '"operationId"',
    '"positionOffset"',
  ],
  "ordinary GraphQL schema contract",
);
includesAll(
  resolutionSchemaTest,
  [
    "graphql_schema_exposes_explicit_solution_resolution_command",
    '"mergeForumTopicResolvingSolution"',
    '"ResolveForumTopicMergeSolutionGraphqlInput"',
    '"GqlForumTopicMergeSolutionResolution"',
    '"selectedSolutionReplyId"',
    "resolution_adapter_uses_routed_manager_context_and_same_owner",
    "ordinary_and_resolved_commands_share_one_private_transaction_owner",
    "resolution_audit_is_append_only_and_keeps_merge_event_schema_one",
  ],
  "resolution GraphQL schema contract",
);
includesAll(
  resolutionRuntimeTest,
  [
    "manager_can_select_source_solution_and_replay_exact_audit",
    "manager_can_select_target_solution_and_invalid_selection_is_atomic",
    "forum_topic_merge_solution_resolutions",
  ],
  "resolution owner runtime test",
);

includesAll(
  docs,
  [
    "# FORUM-21K topic merge GraphQL transport",
    "`source_ready_maintainer_execution_pending`",
    "mergeForumTopic",
    "mergeForumTopicResolvingSolution",
    "selectedSolutionReplyId",
    "one private `merge_topic_internal` transaction",
    "forum_topic_merge_solution_resolutions",
    "schema version 1",
    "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
    "FORUM-21` entry remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "GraphQL handoff",
);
includesAll(
  resolutionDocs,
  [
    "# FORUM-21L competing accepted-solution resolution",
    "mergeForumTopicResolvingSolution",
    "forum_topics:manage",
    "forum_topic_merge_solution_resolutions",
  ],
  "resolution handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21L",
    "mergeForumTopicResolvingSolution",
    "Both require the `forum` module",
    "schema-version-1 event",
  ],
  "cumulative merge handoff",
);
includesAll(
  readme,
  [
    "`mergeForumTopic`",
    "`mergeForumTopicResolvingSolution`",
    "`forum_topic_merge_solution_resolutions`",
    "`graphql::ResolveForumTopicMergeSolutionGraphqlInput`",
    "`graphql::GqlForumTopicMergeSolutionResolution`",
  ],
  "Forum README",
);
includesAll(
  docsIndex,
  [
    "routed-tenant, `forum_topics:manage` GraphQL commands",
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
  "FORUM-21K/L topic merge GraphQL transport source is ready; canonical FORUM-21 remains planned.",
);