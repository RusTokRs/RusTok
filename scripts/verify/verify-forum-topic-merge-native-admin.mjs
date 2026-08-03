#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-native-admin.json",
  docs: "crates/rustok-forum/docs/forum-21o-topic-merge-native-admin.md",
  readme: "crates/rustok-forum/admin/README.md",
  packageCargo: "crates/rustok-forum/admin/Cargo.toml",
  hostCargo: "apps/admin/Cargo.toml",
  facade: "crates/rustok-forum/admin/src/transport.rs",
  native:
    "crates/rustok-forum/admin/src/transport/topic_merge_native_server_adapter.rs",
  graphql:
    "crates/rustok-forum/admin/src/transport/topic_merge_graphql_adapter.rs",
  ui: "crates/rustok-forum/admin/src/ui/topic_merge.rs",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const docs = read(paths.docs);
const readme = read(paths.readme);
const packageCargo = read(paths.packageCargo);
const hostCargo = read(paths.hostCargo);
const facade = read(paths.facade);
const native = read(paths.native);
const graphql = read(paths.graphql);
const ui = read(paths.ui);

assert.equal(contract.contract, "forum_topic_merge_native_admin_v1");
assert.equal(contract.task, "FORUM-21O");
assert.equal(contract.parent_task, "FORUM-21");
assert.deepEqual(contract.extends, ["FORUM-21N"]);
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.equal(contract.transport_selection.ssr, "native_server");
assert.equal(contract.transport_selection.hydrate, "native_server");
assert.equal(contract.transport_selection.csr, "graphql");
assert.equal(contract.transport_selection.headless_default, "graphql");
assert.equal(contract.transport_selection.fallback, false);
assert.equal(contract.native_server.module_slug, "forum");
assert.equal(
  contract.native_server.module_lifecycle_check,
  "rustok_api::is_tenant_module_enabled",
);
assert.equal(contract.native_server.module_must_be_enabled, true);
assert.equal(contract.native_server.candidate_limit, 100);
assert.equal(contract.native_server.candidate_permission, "forum_topics:list");
assert.equal(contract.native_server.merge_permission, "forum_topics:manage");
assert.equal(contract.native_server.auth_tenant_must_equal_routed_tenant, true);
assert.equal(contract.request_dto_policy.access_token, false);
assert.equal(contract.request_dto_policy.tenant_id, false);
assert.equal(contract.request_dto_policy.actor_id, false);
assert.equal(contract.graphql_parity.csr_and_headless_only, true);
assert.equal(contract.compatibility.owner_changed, false);
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(contract.compatibility.migration_added, false);
assert.equal(contract.compatibility.next_admin_changed, false);

includesAll(
  packageCargo,
  [
    'csr = ["leptos/csr"]',
    'hydrate = ["leptos/hydrate"]',
    '"leptos/ssr"',
    '"dep:leptos_axum"',
    '"rustok-api/server"',
    '"dep:rustok-core"',
    '"dep:rustok-forum"',
    '"dep:rustok-outbox"',
    'rustok-ui-transport.workspace = true',
    'rustok-forum = { path = "..", optional = true }',
  ],
  "Forum admin Cargo features",
);
assert.ok(!packageCargo.includes('leptos = { workspace = true, features = ["csr"] }'));

includesAll(
  hostCargo,
  [
    '"rustok-forum-admin/csr"',
    '"rustok-forum-admin/hydrate"',
    '"rustok-forum-admin/ssr"',
  ],
  "apps/admin feature forwarding",
);

includesAll(
  facade,
  [
    "mod topic_merge_native_server_adapter;",
    "selected_topic_merge_transport_path",
    'cfg(any(feature = "ssr", feature = "hydrate"))',
    "UiTransportPath::NativeServer",
    "UiTransportPath::Graphql",
    "execute_selected_transport",
    "topic_merge_native_server_adapter::fetch_topic_merge_candidates_native",
    "topic_merge_native_server_adapter::merge_topic_native",
    "topic_merge_graphql_adapter::fetch_candidates",
    "topic_merge_graphql_adapter::merge_topic",
  ],
  "Forum merge transport facade",
);
assert.ok(!facade.includes("or_else"));
assert.ok(!facade.includes("fallback_failed"));

includesAll(
  native,
  [
    '#[server(prefix = "/api/fn", endpoint = "forum/topic-merge-candidates")]',
    '#[server(prefix = "/api/fn", endpoint = "forum/topic-merge")]',
    "fetch_topic_merge_candidates_native(\n    locale: String,",
    "merge_topic_native(\n    command: ForumTopicMergeCommand,",
    "leptos_axum::extract::<rustok_api::AuthContext>()",
    "leptos_axum::extract::<rustok_api::TenantContext>()",
    "auth.tenant_id == tenant.id",
    "require_forum_module_enabled",
    "rustok_api::is_tenant_module_enabled(host.db(), tenant_id, \"forum\")",
    "Permission::FORUM_TOPICS_LIST",
    "Permission::FORUM_TOPICS_MANAGE",
    "shared_get::<rustok_outbox::TransactionalEventBus>()",
    "TopicService::new",
    "list_with_locale_fallback",
    "per_page: 100",
    "ForumTopicMergeService::new",
    "merge_topic_resolving_solution",
    ".merge_topic(tenant.id",
    "SecurityContext::from_permission_snapshot",
    "map_receipt",
  ],
  "Forum native merge adapter",
);
assert.ok(!native.includes("token: Option<String>"));
assert.ok(!native.includes("tenant_id: String"));
assert.ok(!native.includes("execute_graphql"));
assert.ok(!native.includes("GraphqlRequest"));

includesAll(
  graphql,
  [
    "MERGE_CANDIDATES_QUERY",
    "MERGE_TOPIC_MUTATION",
    "MERGE_TOPIC_RESOLVING_SOLUTION_MUTATION",
    "mergeForumTopic(",
    "mergeForumTopicResolvingSolution(",
    "limit: 100",
    "execute_graphql",
  ],
  "retained Forum GraphQL parity adapter",
);

includesAll(
  ui,
  [
    "transport::fetch_topic_merge_candidates",
    "transport::merge_topic",
    "build_forum_topic_merge_command",
  ],
  "Forum merge UI facade usage",
);
assert.ok(!ui.includes("topic_merge_native_server_adapter"));
assert.ok(!ui.includes("topic_merge_graphql_adapter"));
assert.ok(!ui.includes("ForumTopicMergeService"));

includesAll(
  readme,
  [
    "FORUM-21O",
    "ssr` and `hydrate` use native server functions",
    "csr` and headless/default builds use GraphQL",
    "never triggers cross-path fallback",
    paths.native,
  ],
  "Forum admin README",
);

includesAll(
  docs,
  [
    "# FORUM-21O native Leptos admin topic-merge transport",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "direct authenticated native server-function path",
    "never causes an implicit retry through the other transport",
    "rustok_api::is_tenant_module_enabled(..., \"forum\")",
    "A disabled module therefore fails closed",
    "No native DTO accepts an access token",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21O handoff",
);

console.log(
  "FORUM-21O native Leptos topic-merge transport source is ready; maintainer runtime evidence and remaining FORUM-21 workflows are pending.",
);
