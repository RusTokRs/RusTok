#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { containsMarker, ledgerRowProblems } from "./lib/forum-source-verification.mjs";

const paths = {
  contract: "crates/modules/rustok-forum/contracts/forum-topic-fork-graphql-transport.json",
  ownerContract: "crates/modules/rustok-forum/contracts/forum-topic-fork-owner.json",
  docs: "crates/modules/rustok-forum/docs/forum-21u-topic-fork-graphql-transport.md",
  ownerDocs: "crates/modules/rustok-forum/docs/forum-21q-topic-fork-owner.md",
  graphql: "crates/modules/rustok-forum/src/graphql/topic_fork_mutation.rs",
  graphqlMod: "crates/modules/rustok-forum/src/graphql/mod.rs",
  owner: "crates/modules/rustok-forum/src/services/topic_fork_owner.rs",
  schemaTest: "crates/modules/rustok-forum/tests/topic_fork_graphql_contract.rs",
  ownerRuntimeTest: "crates/modules/rustok-forum/tests/topic_fork_sqlite.rs",
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
const ownerContract = JSON.parse(read(paths.ownerContract));
const docs = read(paths.docs);
const ownerDocs = read(paths.ownerDocs);
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
const schemaTest = read(paths.schemaTest);
const ownerRuntimeTest = read(paths.ownerRuntimeTest);
const docsIndex = read(paths.docsIndex);
const plan = read(paths.plan);

assert.equal(contract.contract, "forum_topic_fork_graphql_transport_v1");
assert.equal(contract.task, "FORUM-21U");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.extends, "FORUM-21Q");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.transport, "graphql");
assert.equal(contract.owner_service, "ForumTopicForkService");
assert.equal(contract.authorization.module_must_be_enabled, "forum");
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.authorization.tenant_is_derived_from_routed_TenantContext, true);
assert.equal(contract.authorization.optional_tenant_argument_must_equal_routed_tenant, true);
assert.equal(contract.command.field, "forkForumTopicReplyBranch");
assert.equal(contract.command.input_type, "ForkForumTopicReplyBranchGraphqlInput");
assert.equal(contract.command.result_type, "GqlForumTopicFork");
assert.equal(contract.command.calls_owner_method, "fork_reply_branch");
assert.equal(contract.receipt_result.returns_immutable_owner_receipt, true);
assert.equal(contract.receipt_result.exact_replay_returns_same_result, true);
assert.equal(contract.composition.resolver_contains_no_fork_business_logic, true);
assert.equal(contract.composition.raw_fork_receipt_or_mapping_table_access_in_resolver, false);
assert.equal(contract.composition.transport_local_copy_policy, false);
assert.equal(contract.compatibility.owner_command_changed, false);
assert.equal(contract.compatibility.receipt_schema_changed, false);
assert.equal(contract.compatibility.semantic_event_schema_changed, false);
assert.equal(contract.compatibility.graphql_field_is_additive, true);
assert.equal(ownerContract.contract, "forum_topic_fork_owner_v1");
assert.equal(ownerContract.task, "FORUM-21Q");
assert.equal(ownerContract.command, "fork_reply_branch");
assert.equal(ownerContract.audit.semantic_event, "forum.topic.forked");

includesAll(
  graphql,
  [
    "pub(crate) struct ForumTopicForkMutation",
    "async fn fork_forum_topic_reply_branch(",
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
    "ForumTopicForkService::new(db.clone(), event_bus.clone())",
    ".fork_reply_branch(",
    "pub struct ForkForumTopicReplyBranchGraphqlInput",
    "pub struct GqlForumTopicFork",
    "root_reply_id: input.root_reply_id",
    "copied_reply_revision_count",
    "copied_relation_revision_count",
    "copied_mention_count",
    "copied_quote_count",
    "forked_at: value.forked_at.to_rfc3339()",
  ],
  "topic fork GraphQL adapter",
);

for (const forbidden of [
  "resolve_canonical_topic",
  "forum_topic_fork_operations",
  "forum_topic_fork_reply_items",
  "forum_topic_fork_revision_items",
  "ForumTopicMoveService",
  "ForumTopicMergeService",
  "ForumTopicSplitService",
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
    "mod topic_fork_mutation;",
    "ForkForumTopicReplyBranchGraphqlInput",
    "GqlForumTopicFork",
    "topic_fork_mutation::ForumTopicForkMutation",
  ],
  "GraphQL root registration",
);
includesAll(
  owner,
  [
    "pub struct ForumTopicForkService",
    "pub async fn fork_reply_branch(",
    "Resource::ForumTopics, Action::Manage",
    "load_reply_branch_ids_in_tx",
    "derive_reply_id_map",
    "insert_fork_operation_in_tx",
    'const FORUM_TOPIC_FORK_EVENT_TYPE: &str = "forum.topic.forked"',
  ],
  "topic fork owner",
);
includesAll(
  schemaTest,
  [
    "graphql_schema_exposes_idempotent_topic_fork_command",
    '"forkForumTopicReplyBranch"',
    '"ForkForumTopicReplyBranchGraphqlInput"',
    '"GqlForumTopicFork"',
    '"rootReplyId"',
    '"copiedReplyRevisionCount"',
    '"copiedRelationRevisionCount"',
    "graphql_fork_adapter_uses_routed_tenant_manage_scope_and_owner_service",
  ],
  "GraphQL schema contract",
);
includesAll(
  ownerRuntimeTest,
  [
    "reply_branch_fork_is_atomic_idempotent_and_preserves_provenance",
    "reply_branch_fork_rejects_non_topological_source_positions_atomically",
    "forum_topic_fork_operations",
    "forum.topic.forked",
  ],
  "owner runtime contract source",
);
includesAll(
  docs,
  [
    "# FORUM-21U topic fork GraphQL transport",
    "`source_ready_maintainer_execution_pending`",
    "forkForumTopicReplyBranch",
    "forum_topics:manage",
    "ForumTopicForkService::fork_reply_branch",
    "immutable owner receipt",
    "FORUM-21` entry remains `planned`",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21U handoff",
);
includesAll(
  ownerDocs,
  [
    "# FORUM-21Q reply-branch fork owner",
    "deterministically derived",
    "forum.topic.forked",
  ],
  "FORUM-21Q owner handoff",
);
includesAll(
  docsIndex,
  [
    "FORUM-21U topic fork GraphQL transport",
    "forkForumTopicReplyBranch",
  ],
  "Forum docs index",
);
// The plan carries one ledger row per task: the per-slice sentences it used to hold now live in the
// slice documents this guard already pins.
assert.deepEqual(
  ledgerRowProblems(plan, "FORUM-21", {
    mentions: ["move/merge/split/fork/range owners"],
  }),
  [],
);
assert.ok(plan.includes("**Status:** `planned`"));
assert.ok(!plan.includes("plus an additive\n  manager transport for the fork owner"));
assert.ok(!plan.includes("bounded reply-range movement with deterministic positions"));

console.log(
  "FORUM-21U topic fork GraphQL transport source is ready; maintainer execution and admin composition remain pending.",
);
