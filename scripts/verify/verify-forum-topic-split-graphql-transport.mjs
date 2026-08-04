#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-split-graphql-transport.json",
  ownerContract: "crates/rustok-forum/contracts/forum-topic-split-owner.json",
  docs: "crates/rustok-forum/docs/forum-21r-topic-split-graphql-transport.md",
  ownerDocs: "crates/rustok-forum/docs/forum-21p-topic-split-owner.md",
  graphql: "crates/rustok-forum/src/graphql/topic_split_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_split.rs",
  schemaTest: "crates/rustok-forum/tests/topic_split_graphql_contract.rs",
  ownerRuntimeTest: "crates/rustok-forum/tests/topic_split_sqlite.rs",
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
const ownerContract = JSON.parse(read(paths.ownerContract));
const docs = read(paths.docs);
const ownerDocs = read(paths.ownerDocs);
const graphql = read(paths.graphql);
const graphqlMod = read(paths.graphqlMod);
const owner = read(paths.owner);
const schemaTest = read(paths.schemaTest);
const ownerRuntimeTest = read(paths.ownerRuntimeTest);
const docsIndex = read(paths.docsIndex);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_split_graphql_transport_v1");
assert.equal(contract.task, "FORUM-21R");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21P");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.transport, "graphql");
assert.equal(contract.owner_service, "ForumTopicSplitService");
assert.equal(contract.authorization.module_must_be_enabled, "forum");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.tenant_is_derived_from_routed_TenantContext, true);
assert.equal(contract.authorization.optional_tenant_argument_must_equal_routed_tenant, true);
assert.equal(contract.command.field, "splitForumTopicReplies");
assert.equal(contract.command.input_type, "SplitForumTopicRepliesGraphqlInput");
assert.equal(contract.command.result_type, "GqlForumTopicSplit");
assert.equal(contract.command.calls_owner_method, "split_selected_replies");
assert.equal(contract.receipt_result.returns_immutable_owner_receipt, true);
assert.equal(contract.receipt_result.exact_replay_returns_same_result, true);
assert.equal(contract.composition.resolver_contains_no_split_business_logic, true);
assert.equal(contract.composition.raw_split_receipt_or_item_table_access_in_resolver, false);
assert.equal(contract.compatibility.owner_command_changed, false);
assert.equal(contract.compatibility.receipt_schema_changed, false);
assert.equal(contract.compatibility.semantic_event_schema_changed, false);
assert.equal(contract.compatibility.graphql_field_is_additive, true);
assert.equal(ownerContract.contract, "forum_topic_split_owner_v1");
assert.equal(ownerContract.task, "FORUM-21P");
assert.equal(ownerContract.command, "split_selected_replies");
assert.equal(ownerContract.audit.semantic_event, "forum.topic.split");

includesAll(
  graphql,
  [
    "pub(crate) struct ForumTopicSplitMutation",
    "async fn split_forum_topic_replies(",
    "require_module_enabled(ctx, MODULE_SLUG).await?;",
    "ctx.data::<DatabaseConnection>()?",
    "ctx.data::<TransactionalEventBus>()?",
    "ctx.data::<AuthContext>()",
    "ctx.data::<TenantContext>()?",
    "Permission::FORUM_TOPICS_MANAGE",
    "Permission denied: forum_topics:manage required",
    "Permission denied: tenant scope mismatch",
    "SecurityContext::from_permission_snapshot",
    "ForumTopicSplitService::new(db.clone(), event_bus.clone())",
    ".split_selected_replies(",
    "pub struct SplitForumTopicRepliesGraphqlInput",
    "pub struct GqlForumTopicSplit",
    "source_resulting_published_reply_count",
    "target_resulting_published_reply_count",
    "solution_reply_id",
    "split_at: value.split_at.to_rfc3339()",
  ],
  "topic split GraphQL adapter",
);

for (const forbidden of [
  "resolve_canonical_topic",
  "forum_topic_split_operations",
  "forum_topic_split_reply_items",
  "ForumTopicMoveService",
  "ForumTopicMergeService",
  "TopicService::new",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(!graphql.includes(forbidden), `GraphQL adapter contains forbidden marker: ${forbidden}`);
}

includesAll(
  graphqlMod,
  [
    "mod topic_split_mutation;",
    "GqlForumTopicSplit",
    "SplitForumTopicRepliesGraphqlInput",
    "topic_split_mutation::ForumTopicSplitMutation",
  ],
  "GraphQL root registration",
);
includesAll(
  owner,
  [
    "pub struct ForumTopicSplitService",
    "pub async fn split_selected_replies(",
    "Resource::ForumTopics, Action::Manage",
    "validate_split_boundary_in_tx",
    "clone_topic_access_in_tx",
    "insert_split_operation_in_tx",
    'const FORUM_TOPIC_SPLIT_EVENT_TYPE: &str = "forum.topic.split"',
  ],
  "topic split owner",
);
includesAll(
  schemaTest,
  [
    "graphql_schema_exposes_idempotent_topic_split_command",
    '"splitForumTopicReplies"',
    '"SplitForumTopicRepliesGraphqlInput"',
    '"GqlForumTopicSplit"',
    '"replyIds"',
    '"sourceResultingPublishedReplyCount"',
    "graphql_split_adapter_uses_routed_tenant_manage_scope_and_owner_service",
  ],
  "GraphQL schema contract",
);
includesAll(
  ownerRuntimeTest,
  [
    "selected_reply_split_is_atomic_idempotent_and_append_only",
    "selected_reply_split_rejects_cross_boundary_parent_edges",
    "forum_topic_split_operations",
    "forum.topic.split",
  ],
  "owner runtime contract source",
);
includesAll(
  docs,
  [
    "# FORUM-21R topic split GraphQL transport",
    "`source_ready_maintainer_execution_pending`",
    "splitForumTopicReplies",
    "forum_topics:manage",
    "ForumTopicSplitService::split_selected_replies",
    "immutable owner receipt",
    "FORUM-21` entry remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21R handoff",
);
includesAll(
  ownerDocs,
  [
    "# FORUM-21P selected-reply topic split owner",
    "parent-closed in both directions",
    "forum.topic.split",
  ],
  "FORUM-21P owner handoff",
);
includesAll(
  docsIndex,
  [
    "FORUM-21R topic split GraphQL transport",
    "splitForumTopicReplies",
  ],
  "Forum docs index",
);
includesAll(
  plan,
  [
    "### Delivered through `FORUM-21R`",
    "FORUM-21P adds the idempotent selected-reply split owner",
    "FORUM-21Q adds the idempotent reply-branch fork owner",
    "FORUM-21R adds the routed-tenant, manager-only GraphQL split transport",
    "public admin composition for split and fork workflows",
    "bounded reply-range movement",
  ],
  "canonical FORUM-21 plan",
);
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(plan.includes("**Status:** `planned`"));
assert.ok(!plan.includes("idempotent split-selected-replies workflow with immutable receipt"));
assert.ok(!plan.includes("idempotent reply-branch fork workflow with explicit copy/identity policy"));

console.log(
  "FORUM-21R topic split GraphQL transport source is ready; maintainer execution and admin composition remain pending.",
);
