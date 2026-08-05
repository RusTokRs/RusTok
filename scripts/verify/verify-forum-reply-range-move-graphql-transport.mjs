#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-reply-range-move-graphql-transport.json",
  ownerContract: "crates/rustok-forum/contracts/forum-reply-range-move-owner.json",
  docs: "crates/rustok-forum/docs/forum-21t-reply-range-move-graphql-transport.md",
  ownerDocs: "crates/rustok-forum/docs/forum-21s-reply-range-move-owner.md",
  graphql: "crates/rustok-forum/src/graphql/topic_reply_range_move_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_reply_range_move.rs",
  schemaTest: "crates/rustok-forum/tests/reply_range_move_graphql_contract.rs",
  ownerRuntimeTest: "crates/rustok-forum/tests/reply_range_move_sqlite.rs",
  docsIndex: "crates/rustok-forum/docs/README.md",
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

assert.equal(contract.contract, "forum_reply_range_move_graphql_transport_v1");
assert.equal(contract.task, "FORUM-21T");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21S");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.transport, "graphql");
assert.equal(contract.owner_service, "ForumReplyRangeMoveService");
assert.equal(contract.authorization.module_must_be_enabled, "forum");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.tenant_is_derived_from_routed_TenantContext, true);
assert.equal(contract.authorization.optional_tenant_argument_must_equal_routed_tenant, true);
assert.equal(contract.command.field, "moveForumTopicReplyRange");
assert.equal(contract.command.input_type, "MoveForumTopicReplyRangeGraphqlInput");
assert.equal(contract.command.result_type, "GqlForumReplyRangeMove");
assert.equal(contract.command.calls_owner_method, "move_reply_range");
assert.equal(contract.receipt_result.returns_immutable_owner_receipt, true);
assert.equal(contract.receipt_result.exact_replay_returns_same_result, true);
assert.equal(contract.composition.resolver_contains_no_range_move_business_logic, true);
assert.equal(contract.composition.raw_range_move_receipt_or_item_table_access_in_resolver, false);
assert.equal(contract.compatibility.owner_command_changed, false);
assert.equal(contract.compatibility.receipt_schema_changed, false);
assert.equal(contract.compatibility.semantic_event_schema_changed, false);
assert.equal(contract.compatibility.graphql_field_is_additive, true);
assert.equal(ownerContract.contract, "forum_reply_range_move_owner_v1");
assert.equal(ownerContract.task, "FORUM-21S");
assert.equal(ownerContract.command, "move_reply_range");
assert.equal(ownerContract.audit.semantic_event, "forum.topic.reply_range_moved");

includesAll(graphql, [
  "pub(crate) struct ForumTopicReplyRangeMoveMutation",
  "async fn move_forum_topic_reply_range(",
  "require_module_enabled(ctx, MODULE_SLUG).await?;",
  "Permission::FORUM_TOPICS_MANAGE",
  "Permission denied: tenant scope mismatch",
  "SecurityContext::from_permission_snapshot",
  "ForumReplyRangeMoveService::new(db.clone(), event_bus.clone())",
  ".move_reply_range(",
  "pub struct MoveForumTopicReplyRangeGraphqlInput",
  "pub struct GqlForumReplyRangeMove",
  "moved_at: value.moved_at.to_rfc3339()",
], "reply-range GraphQL adapter");

for (const forbidden of [
  "resolve_canonical_topic",
  "forum_reply_range_move_operations",
  "forum_reply_range_move_items",
  "ForumTopicMoveService",
  "ForumTopicMergeService",
  "ForumTopicSplitService",
  "ReplyService::new",
]) {
  assert.ok(!graphql.includes(forbidden), `GraphQL adapter contains forbidden marker: ${forbidden}`);
}

includesAll(graphqlMod, [
  "mod topic_reply_range_move_mutation;",
  "GqlForumReplyRangeMove",
  "MoveForumTopicReplyRangeGraphqlInput",
  "topic_reply_range_move_mutation::ForumTopicReplyRangeMoveMutation",
], "GraphQL root registration");

includesAll(owner, [
  "pub struct ForumReplyRangeMoveService",
  "pub async fn move_reply_range(",
  "Resource::ForumTopics, Action::Manage",
  "validate_parent_boundary_in_tx",
  "validate_equal_access_in_tx",
  "insert_operation_in_tx",
  'const FORUM_REPLY_RANGE_MOVE_EVENT_TYPE: &str = "forum.topic.reply_range_moved"',
], "reply-range owner");

includesAll(schemaTest, [
  "graphql_schema_exposes_idempotent_reply_range_move_command",
  '"moveForumTopicReplyRange"',
  '"MoveForumTopicReplyRangeGraphqlInput"',
  '"GqlForumReplyRangeMove"',
  '"startPosition"',
  "graphql_reply_range_adapter_uses_routed_tenant_manage_scope_and_owner_service",
], "GraphQL schema contract");

includesAll(ownerRuntimeTest, [
  "reply_range_move_is_atomic_idempotent_and_preserves_identity",
  "forum_reply_range_move_operations",
  "forum.topic.reply_range_moved",
], "owner runtime contract source");

includesAll(docs, [
  "# FORUM-21T reply-range move GraphQL transport",
  "`source_ready_maintainer_execution_pending`",
  "moveForumTopicReplyRange",
  "forum_topics:manage",
  "ForumReplyRangeMoveService::move_reply_range",
  "immutable owner receipt",
  "FORUM-21` entry remains `planned`",
  "No command above was run by the implementation agent",
], "FORUM-21T handoff");

includesAll(ownerDocs, [
  "# FORUM-21S bounded reply-range move owner",
  "forum.topic.reply_range_moved",
], "FORUM-21S owner handoff");

includesAll(docsIndex, [
  "FORUM-21T reply-range move GraphQL transport",
  "moveForumTopicReplyRange",
], "Forum docs index");

console.log(
  "FORUM-21T reply-range GraphQL transport source is ready; maintainer execution and admin composition remain pending.",
);
