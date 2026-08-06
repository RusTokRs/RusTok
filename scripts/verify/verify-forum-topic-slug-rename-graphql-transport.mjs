#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract:
    "crates/rustok-forum/contracts/forum-topic-slug-rename-graphql-transport.json",
  ownerContract: "crates/rustok-forum/contracts/forum-topic-slug-rename-owner.json",
  docs: "crates/rustok-forum/docs/forum-24f-topic-slug-rename-graphql-transport.md",
  ownerDocs: "crates/rustok-forum/docs/forum-24d-topic-slug-rename-owner.md",
  graphql: "crates/rustok-forum/src/graphql/topic_slug_rename_mutation.rs",
  graphqlMod: "crates/rustok-forum/src/graphql/mod.rs",
  owner: "crates/rustok-forum/src/services/topic_owner.rs",
  routeOwner: "crates/rustok-forum/src/services/topic_route.rs",
  schemaTest: "crates/rustok-forum/tests/topic_slug_rename_graphql_contract.rs",
  ownerRuntimeTest: "crates/rustok-forum/tests/topic_slug_rename_sqlite.rs",
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
const routeOwner = read(paths.routeOwner);
const schemaTest = read(paths.schemaTest);
const ownerRuntimeTest = read(paths.ownerRuntimeTest);
const docsIndex = read(paths.docsIndex);

assert.equal(contract.contract, "forum_topic_slug_rename_graphql_transport_v1");
assert.equal(contract.task, "FORUM-24F");
assert.equal(contract.parent_task, "FORUM-24");
assert.equal(contract.extends, "FORUM-24D");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.transport, "graphql");
assert.equal(contract.owner_service, "TopicService");
assert.equal(contract.authorization.module_must_be_enabled, "forum");
assert.equal(contract.authorization.required_permission, "forum_topics:update");
assert.equal(contract.authorization.ownership_semantics_remain_owner_defined, true);
assert.equal(contract.authorization.tenant_is_derived_from_routed_TenantContext, true);
assert.equal(contract.authorization.optional_tenant_argument_must_equal_routed_tenant, true);
assert.equal(contract.command.field, "renameForumTopicSlug");
assert.equal(contract.command.input_type, "RenameForumTopicSlugGraphqlInput");
assert.equal(contract.command.result_type, "GqlForumTopicSlugRename");
assert.equal(contract.command.calls_owner_method, "rename_slug");
assert.equal(contract.result.exact_normalized_replay_reports_changed_false, true);
assert.equal(contract.result.changed_write_returns_immutable_alias_id, true);
assert.equal(contract.composition.resolver_contains_no_route_business_logic, true);
assert.equal(contract.composition.raw_route_alias_table_access_in_resolver, false);
assert.equal(contract.compatibility.owner_command_changed, false);
assert.equal(contract.compatibility.route_alias_schema_changed, false);
assert.equal(contract.compatibility.semantic_event_schema_changed, false);
assert.equal(contract.compatibility.graphql_field_is_additive, true);
assert.equal(ownerContract.task, "FORUM-24D");
assert.equal(ownerContract.command, "TopicService::rename_slug");
assert.equal(ownerContract.authorization.action, "update");

includesAll(
  graphql,
  [
    "pub(crate) struct ForumTopicSlugRenameMutation",
    "async fn rename_forum_topic_slug(",
    "require_module_enabled(ctx, MODULE_SLUG).await?;",
    "ctx.data::<DatabaseConnection>()?",
    "ctx.data::<TransactionalEventBus>()?",
    "ctx.data::<AuthContext>()",
    "ctx.data::<TenantContext>()?",
    "Permission::FORUM_TOPICS_UPDATE",
    "Permission denied: forum_topics:update required",
    "Permission denied: tenant scope mismatch",
    "SecurityContext::from_permission_snapshot",
    "TopicService::new(db.clone(), event_bus.clone())",
    ".rename_slug(",
    "pub struct RenameForumTopicSlugGraphqlInput",
    "pub struct GqlForumTopicRouteDescriptor",
    "pub struct GqlForumTopicSlugRename",
    "previous_path: value.previous_path",
    "canonical: value.canonical.into()",
  ],
  "topic slug rename GraphQL adapter",
);

for (const forbidden of [
  "ForumTopicRouteService::new",
  "rename_topic_slug_in_tx",
  "forum_topic_route_aliases",
  "resolve_canonical_topic",
  "ForumTopicMergeService",
  "ForumTopicMergeRouteBackfillService",
  "bestEffort",
  "ALLOW_MISSING",
  "SKIP_VALIDATION",
]) {
  assert.ok(
    !graphql.includes(forbidden),
    `GraphQL adapter contains forbidden marker: ${forbidden}`,
  );
}

includesAll(
  graphqlMod,
  [
    "mod topic_slug_rename_mutation;",
    "GqlForumTopicRouteDescriptor",
    "GqlForumTopicSlugRename",
    "RenameForumTopicSlugGraphqlInput",
    "topic_slug_rename_mutation::ForumTopicSlugRenameMutation",
  ],
  "GraphQL root registration",
);
includesAll(
  owner,
  [
    "pub async fn rename_slug(",
    "Resource::ForumTopics",
    "Action::Update",
    "ForumTopicRouteService::rename_topic_slug_in_tx",
    "publish_forum_topic_projection_in_tx",
  ],
  "topic owner",
);
includesAll(
  routeOwner,
  [
    "pub(crate) async fn rename_topic_slug_in_tx(",
    "FORUM_TOPIC_RENAMED_ROUTE_REASON",
    "record_redirect_alias_in_tx",
    "update_topic_route_slug_in_tx",
  ],
  "topic route owner",
);
includesAll(
  schemaTest,
  [
    "graphql_schema_exposes_topic_slug_rename_command",
    '"renameForumTopicSlug"',
    '"RenameForumTopicSlugGraphqlInput"',
    '"GqlForumTopicSlugRename"',
    '"GqlForumTopicRouteDescriptor"',
    '"previousPath"',
    '"shortId"',
    "graphql_rename_adapter_uses_routed_tenant_update_scope_and_owner_service",
  ],
  "GraphQL schema contract",
);
includesAll(
  ownerRuntimeTest,
  [
    "rename_records_one_alias_and_old_route_becomes_gone_after_delete",
    "forum_topic_route_aliases",
    "Topic slug changed",
  ],
  "owner runtime contract source",
);
includesAll(
  docs,
  [
    "# FORUM-24F topic slug rename GraphQL transport",
    "`source_ready_maintainer_execution_pending`",
    "renameForumTopicSlug",
    "forum_topics:update",
    "TopicService::rename_slug",
    "owner-defined",
    "No command above was run by the implementation agent",
  ],
  "FORUM-24F handoff",
);
includesAll(
  ownerDocs,
  [
    "# FORUM-24D topic slug rename owner",
    "TopicService::rename_slug",
    "Topic slug changed",
  ],
  "FORUM-24D owner handoff",
);
includesAll(
  docsIndex,
  [
    "FORUM-24F exposes the localized topic slug rename owner through an additive routed-tenant GraphQL mutation",
    "FORUM-24F topic slug rename GraphQL transport",
  ],
  "Forum docs index",
);

console.log(
  "FORUM-24F topic slug rename GraphQL transport source is ready; maintainer execution and UI/public route composition remain pending.",
);
