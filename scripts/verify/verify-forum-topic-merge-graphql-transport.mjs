#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { containsMarker, ledgerRowProblems } from "./lib/forum-source-verification.mjs";

const paths = {
  contract: "crates/modules/rustok-forum/contracts/forum-topic-merge-graphql-transport.json",
  resolutionContract: "crates/modules/rustok-forum/contracts/forum-topic-merge-solution-resolution.json",
  cumulativeContract: "crates/modules/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/modules/rustok-forum/docs/forum-21k-topic-merge-graphql-transport.md",
  resolutionDocs: "crates/modules/rustok-forum/docs/forum-21l-topic-merge-solution-resolution.md",
  cumulativeDocs: "crates/modules/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  graphql: "crates/modules/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/modules/rustok-forum/src/graphql/mod.rs",
  owner: "crates/modules/rustok-forum/src/services/topic_merge.rs",
  ordinarySchemaTest: "crates/modules/rustok-forum/tests/topic_merge_graphql_contract.rs",
  resolutionSchemaTest:
    "crates/modules/rustok-forum/tests/topic_merge_solution_resolution_graphql_contract.rs",
  resolutionRuntimeTest: "crates/modules/rustok-forum/tests/topic_merge_solution_resolution_sqlite.rs",
  readme: "crates/modules/rustok-forum/README.md",
  docsIndex: "crates/modules/rustok-forum/docs/README.md",
  plan: "crates/modules/rustok-forum/docs/implementation-plan.md",
};

const read = (path) => readFileSync(path, "utf8");
// Layout-tolerant by policy: a marker describes an invariant, not where rustfmt or a prose editor put
// the newline. Tokens are unchanged, so an absent call or sentence is still absent.
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(containsMarker(text, marker), `${label} is missing marker: ${marker}`);
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

// The shared forum guard is the only authorization entry point for these mutations: it must keep
// reading AuthContext through rustok_api, the permission set, and both non-nil identity checks.
for (const needle of [
  "pub(crate) fn require_forum_permission",
  "rustok_api::graphql::require_graphql_auth(ctx, permissions, message)?",
  "if auth.tenant_id.is_nil()",
  "if auth.user_id.is_nil()",
  "pub(crate) fn resolve_tenant_scope(",
  "Permission denied: tenant scope mismatch",
]) {
  assert.ok(
    graphqlMod.includes(needle),
    `${paths.graphqlMod}: shared authorization guard is missing ${needle}`,
  );
}
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
    // Auth is obtained only through the shared forum guard, which pins AuthContext, non-nil
    // tenant/user and the permission set in one place; the tenant scope goes through
    // resolve_tenant_scope() instead of an in-resolver comparison.
    "super::require_forum_permission(",
    "super::resolve_tenant_scope(tenant, tenant_id)?",
    "ctx.data::<TenantContext>()?",
    "Permission::FORUM_TOPICS_MANAGE",
    "Permission denied: forum_topics:manage required",
    // The mismatch wording is produced by the shared scope resolver in graphql/mod.rs, which the
    // mutation must call; pinning the literal here only pinned where the string used to live.
    "resolve_tenant_scope(tenant, tenant_id)?",
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
const permission = graphql.indexOf("super::require_forum_permission(", resolutionResolver);
const tenantScope = graphql.indexOf("super::resolve_tenant_scope(tenant, tenant_id)?", resolutionResolver);
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
    "forum_topic_merge_solution_resolutions",
    "schema version 1",
    "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
    "FORUM-21` entry remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "GraphQL handoff",
);
// The handoff is prose: the phrase wraps differently as the document is edited, so it is compared
// with whitespace normalised instead of pinning one line break.
assert.ok(
  docs.replace(/\s+/g, " ").includes("one private `merge_topic_internal` transaction"),
  "GraphQL handoff must describe the single private merge_topic_internal transaction",
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
    // The index now states the wire contract positively instead of describing the command family.
    "`mergeForumTopic` and `mergeForumTopicResolvingSolution` remain the only admin merge command contracts",
    "FORUM-21L competing solution resolution",
  ],
  "Forum docs index",
);

assert.deepEqual(
  ledgerRowProblems(plan, "FORUM-21", {
    mentions: ["move/merge/split/fork/range owners"],
  }),
  [],
);

console.log(
  "FORUM-21K/L topic merge GraphQL transport source is ready; canonical FORUM-21 remains planned.",
);