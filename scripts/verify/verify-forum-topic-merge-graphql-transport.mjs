#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-graphql-transport.json",
  cumulativeContract: "crates/rustok-forum/contracts/forum-topic-merge-owner.json",
  docs: "crates/rustok-forum/docs/forum-21k-topic-merge-graphql-transport.md",
  cumulativeDocs: "crates/rustok-forum/docs/forum-21b-topic-merge-owner.md",
  graphql: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_merge.rs",
  schemaTest: "crates/rustok-forum/tests/topic_merge_graphql_contract.rs",
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
const cumulativeContract = JSON.parse(read(paths.cumulativeContract));
const docs = read(paths.docs);
const cumulativeDocs = read(paths.cumulativeDocs);
const graphql = read(paths.graphql);
const graphqlMod = read(paths.graphqlMod);
const owner = read(paths.owner);
const schemaTest = read(paths.schemaTest);
const readme = read(paths.readme);
const docsIndex = read(paths.docsIndex);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_merge_graphql_transport_v1");
assert.equal(contract.task, "FORUM-21K");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21B");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.transport, "graphql");
assert.equal(contract.field, "mergeForumTopic");
assert.equal(contract.owner_service, "ForumTopicMergeService");
assert.equal(contract.authorization.module_must_be_enabled, "forum");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.authenticated_human_actor_required_by_owner, true);
assert.equal(contract.authorization.tenant_is_derived_from_routed_TenantContext, true);
assert.equal(contract.authorization.optional_tenant_argument_must_equal_routed_tenant, true);
assert.equal(contract.input.type, "MergeForumTopicGraphqlInput");
assert.deepEqual(contract.input.fields, ["operation_id", "source_topic_id", "reason"]);
assert.equal(contract.input.operation_id_is_idempotency_identity, true);
assert.equal(contract.result.type, "GqlForumTopicMerge");
assert.equal(contract.result.returns_immutable_owner_receipt, true);
assert.equal(contract.result.exact_replay_returns_same_result, true);
assert.equal(contract.result.event_id_equals_operation_id, true);
assert.equal(contract.composition.resolver_contains_no_merge_business_logic, true);
assert.equal(contract.composition.security_context_uses_authenticated_permission_snapshot, true);
assert.equal(contract.composition.domain_errors_preserve_ForumGraphqlErrorExtension_mapping, true);
assert.equal(contract.composition.topic_body_hydration_after_merge, false);
assert.equal(contract.composition.canonical_source_alias_resolution_for_mutation, false);
assert.equal(contract.compatibility.domain_input_changed, false);
assert.equal(contract.compatibility.domain_result_changed, false);
assert.equal(contract.compatibility.receipt_schema_changed, false);
assert.equal(contract.compatibility.event_schema_changed, false);
assert.equal(contract.compatibility.rest_contract_changed, false);
assert.equal(contract.compatibility.graphql_existing_fields_changed, false);
assert.equal(cumulativeContract.latest_policy_slice, "FORUM-21K");
assert.equal(cumulativeContract.graphql_transport.task, "FORUM-21K");
assert.equal(cumulativeContract.graphql_transport.required_permission, "forum_topics:manage");
assert.equal(cumulativeContract.graphql_transport.exact_replay_returns_same_result, true);
assert.equal(cumulativeContract.graphql_transport.mutation_follows_canonical_source_alias, false);
assert.equal(cumulativeContract.graphql_transport.target_topic_hydration_after_command, false);

includesAll(
  graphql,
  [
    "#[Object]",
    "async fn merge_forum_topic(",
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
    "operation_id: input.operation_id",
    "source_topic_id: input.source_topic_id",
    "reason: input.reason",
    "pub struct MergeForumTopicGraphqlInput",
    "pub struct GqlForumTopicMerge",
    "impl From<ForumTopicMergeResult> for GqlForumTopicMerge",
    "merge_transport_enforces_scope_and_replays_one_receipt",
    "assert_eq!(first, replay)",
    "assert_eq!(first.event_id, operation_id)",
  ],
  "topic merge GraphQL adapter",
);

const moduleEnablement = graphql.indexOf("require_module_enabled(ctx, MODULE_SLUG).await?;");
const permission = graphql.indexOf("require_topic_manage_permission(auth)?;");
const tenantScope = graphql.indexOf("resolve_tenant_scope(tenant, requested_tenant_id)?;");
const ownerCall = graphql.indexOf("ForumTopicMergeService::new(db.clone(), event_bus.clone())");
assert.ok(moduleEnablement >= 0);
assert.ok(permission >= 0 && permission < tenantScope && tenantScope < ownerCall);

for (const forbidden of [
  "ForumTopicMoveService",
  "resolve_canonical_topic",
  "forum_topic_merge_operations",
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
    "GqlForumTopicMerge, MergeForumTopicGraphqlInput",
    "topic_merge_mutation::ForumTopicMergeMutation",
  ],
  "GraphQL root registration",
);
includesAll(
  owner,
  [
    "pub struct ForumTopicMergeService",
    "pub async fn merge_topic(",
    "enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;",
    "Forum topic merge requires a human actor",
    "TopicMergeOperationConflict(input.operation_id)",
  ],
  "topic merge owner",
);
includesAll(
  schemaTest,
  [
    "graphql_schema_exposes_idempotent_topic_merge_command",
    '"mergeForumTopic"',
    '"MergeForumTopicGraphqlInput"',
    '"GqlForumTopicMerge"',
    '"operationId"',
    '"positionOffset"',
    "graphql_merge_adapter_uses_routed_tenant_manage_scope_and_owner_service",
  ],
  "GraphQL schema contract test",
);

includesAll(
  docs,
  [
    "# FORUM-21K topic merge GraphQL transport",
    "`source_ready_maintainer_execution_pending`",
    "mergeForumTopic",
    "forum_topics:manage",
    "immutable owner receipt",
    "PERMISSION_DENIED",
    "FORUM_TOPIC_MERGE_OPERATION_CONFLICT",
    "does not follow merged source identities",
    "FORUM-21` entry remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21K handoff",
);
includesAll(
  cumulativeDocs,
  [
    "FORUM-21K",
    "## GraphQL merge command",
    "mergeForumTopic",
    "FORUM-21A through FORUM-21K",
  ],
  "cumulative merge handoff",
);
includesAll(
  readme,
  [
    "manager-only GraphQL mutation `mergeForumTopic`",
    "`graphql::MergeForumTopicGraphqlInput`",
    "`graphql::GqlForumTopicMerge`",
  ],
  "Forum README",
);
includesAll(
  docsIndex,
  [
    "manager-only merge GraphQL command",
    "FORUM-21K topic merge GraphQL transport",
  ],
  "Forum docs index",
);

assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("## `FORUM-21` — move, merge, split and fork topics"));
assert.ok(plan.includes("**Status:** `planned`"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));
assert.ok(!plan.includes("| `FORUM-21` | `in_progress` |"));

console.log(
  "FORUM-21K topic merge GraphQL transport source is ready; canonical FORUM-21 remains planned.",
);
