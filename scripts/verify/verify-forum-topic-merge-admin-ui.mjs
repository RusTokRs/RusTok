#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract: "crates/rustok-forum/contracts/forum-topic-merge-admin-ui.json",
  docs: "crates/rustok-forum/docs/forum-21n-topic-merge-admin-ui.md",
  nativeContract:
    "crates/rustok-forum/contracts/forum-topic-merge-native-admin.json",
  nativeDocs: "crates/rustok-forum/docs/forum-21o-topic-merge-native-admin.md",
  plan: "crates/rustok-forum/docs/implementation-plan.md",
  backend: "crates/rustok-forum/src/graphql/topic_merge_mutation.rs",
  manifest: "crates/rustok-forum/rustok-module.toml",
  leptosReadme: "crates/rustok-forum/admin/README.md",
  leptosModel: "crates/rustok-forum/admin/src/topic_merge_model.rs",
  leptosFacade: "crates/rustok-forum/admin/src/transport.rs",
  leptosNative:
    "crates/rustok-forum/admin/src/transport/topic_merge_native_server_adapter.rs",
  leptosGraphql:
    "crates/rustok-forum/admin/src/transport/topic_merge_graphql_adapter.rs",
  leptosRoot: "crates/rustok-forum/admin/src/ui/root.rs",
  leptosUi: "crates/rustok-forum/admin/src/ui/topic_merge.rs",
  leptosEn: "crates/rustok-forum/admin/locales/en.json",
  leptosRu: "crates/rustok-forum/admin/locales/ru.json",
  nextCore: "apps/next-admin/packages/forum/src/core/topic-merge.ts",
  nextApi: "apps/next-admin/packages/forum/src/api/forum.ts",
  nextUi: "apps/next-admin/packages/forum/src/components/forum-topic-merge.tsx",
  nextNav: "apps/next-admin/packages/forum/src/nav.ts",
  nextPage: "apps/next-admin/src/app/dashboard/forum/merge/page.tsx",
  nextProxy: "apps/next-admin/src/app/api/rustok/graphql/route.ts",
  nextAr: "apps/next-admin/packages/forum/src/locales/ar.json",
  nextEn: "apps/next-admin/packages/forum/src/locales/en.json",
  nextRu: "apps/next-admin/packages/forum/src/locales/ru.json",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const docs = read(paths.docs);
const nativeContract = JSON.parse(read(paths.nativeContract));
const nativeDocs = read(paths.nativeDocs);
const plan = read(paths.plan);
const backend = read(paths.backend);
const manifest = read(paths.manifest);
const leptosReadme = read(paths.leptosReadme);
const leptosModel = read(paths.leptosModel);
const leptosFacade = read(paths.leptosFacade);
const leptosNative = read(paths.leptosNative);
const leptosGraphql = read(paths.leptosGraphql);
const leptosRoot = read(paths.leptosRoot);
const leptosUi = read(paths.leptosUi);
const leptosEn = JSON.parse(read(paths.leptosEn));
const leptosRu = JSON.parse(read(paths.leptosRu));
const nextCore = read(paths.nextCore);
const nextApi = read(paths.nextApi);
const nextUi = read(paths.nextUi);
const nextNav = read(paths.nextNav);
const nextPage = read(paths.nextPage);
const nextProxy = read(paths.nextProxy);
const nextAr = JSON.parse(read(paths.nextAr));
const nextEn = JSON.parse(read(paths.nextEn));
const nextRu = JSON.parse(read(paths.nextRu));

assert.equal(contract.contract, "forum_topic_merge_admin_ui_v1");
assert.equal(contract.task, "FORUM-21N");
assert.equal(contract.parent_task, "FORUM-21");
assert.equal(contract.status, "source_ready_maintainer_execution_pending");
assert.equal(contract.canonical_plan_status, "planned");
assert.deepEqual(contract.backend_graphql_fields, [
  "mergeForumTopic",
  "mergeForumTopicResolvingSolution",
]);
assert.equal(contract.authorization.required_permission, "forum_topics:manage");
assert.equal(contract.leptos_admin.candidate_limit, 100);
assert.equal(contract.leptos_admin.transport_state, "single_adapter_graphql");
assert.equal(contract.leptos_admin.native_server_path_claimed, false);
assert.equal(contract.leptos_admin.access_token_in_server_function_dto, false);
assert.equal(contract.leptos_admin.rest_fallback, false);
assert.equal(contract.next_admin.candidate_limit, 100);
assert.equal(contract.next_admin.browser_transport, "same_origin_graphql_proxy");
assert.equal(contract.next_admin.proxy_session_authentication, true);
assert.equal(contract.next_admin.client_access_token_serialized, false);
assert.equal(contract.command_policy.exact_retry_reuses_operation_id, true);
assert.equal(
  contract.command_policy.source_target_reason_or_solution_change_rotates_operation_id,
  true,
);
assert.equal(contract.command_policy.both_solved_require_explicit_source_or_target_winner, true);
assert.equal(contract.compatibility.backend_owner_changed, false);
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(contract.compatibility.migration_added, false);

assert.equal(nativeContract.contract, "forum_topic_merge_native_admin_v1");
assert.equal(nativeContract.task, "FORUM-21O");
assert.deepEqual(nativeContract.extends, ["FORUM-21N"]);
assert.equal(nativeContract.transport_selection.ssr, "native_server");
assert.equal(nativeContract.transport_selection.hydrate, "native_server");
assert.equal(nativeContract.transport_selection.csr, "graphql");
assert.equal(nativeContract.transport_selection.fallback, false);
assert.equal(nativeContract.request_dto_policy.access_token, false);
assert.equal(nativeContract.request_dto_policy.tenant_id, false);

includesAll(
  backend,
  [
    "async fn merge_forum_topic(",
    "async fn merge_forum_topic_resolving_solution(",
    "forum_topics:manage required",
    "ForumTopicMergeService::new",
  ],
  "backend GraphQL owner composition",
);

includesAll(
  manifest,
  [
    'subpath = "merge"',
    'title = "Merge Forum Topics"',
    'nav_label = "Merge Topics"',
  ],
  "Forum admin manifest",
);
includesAll(
  leptosModel,
  [
    "MAX_FORUM_TOPIC_MERGE_REASON_LEN: usize = 500",
    "build_forum_topic_merge_command",
    "forum_topic_merge_requires_solution_choice",
    "new_forum_topic_merge_operation_id",
    "Source and retained target topics must be different",
    "Choose which accepted solution must remain",
  ],
  "Leptos merge policy",
);
includesAll(
  leptosGraphql,
  [
    "mergeForumTopic(",
    "mergeForumTopicResolvingSolution(",
    "MergeForumTopicGraphqlInput",
    "ResolveForumTopicMergeSolutionGraphqlInput",
    "limit: 100",
    "execute_graphql",
  ],
  "Leptos GraphQL parity adapter",
);
assert.ok(!leptosGraphql.includes("reqwest"));
assert.ok(!leptosGraphql.includes("rest_adapter"));
assert.ok(!leptosGraphql.includes("#[server"));
includesAll(
  leptosNative,
  [
    "fetch_topic_merge_candidates_native",
    "merge_topic_native",
    "ForumTopicMergeService::new",
    "TopicService::new",
    "shared_get::<rustok_outbox::TransactionalEventBus>()",
  ],
  "Leptos native owner adapter",
);
assert.ok(!leptosNative.includes("token: Option<String>"));
includesAll(
  leptosFacade,
  [
    "fetch_topic_merge_candidates",
    "pub async fn merge_topic(",
    "execute_selected_transport",
    "topic_merge_native_server_adapter::fetch_topic_merge_candidates_native",
    "topic_merge_native_server_adapter::merge_topic_native",
    "topic_merge_graphql_adapter::fetch_candidates",
    "topic_merge_graphql_adapter::merge_topic",
  ],
  "Leptos transport facade",
);
assert.ok(!leptosFacade.includes("fallback_failed"));
includesAll(
  leptosRoot,
  ["subpath_matches(\"merge\")", "ForumTopicMergeAdmin", "super::leptos::ForumAdmin"],
  "Leptos root dispatcher",
);
includesAll(
  leptosUi,
  [
    "build_forum_topic_merge_command",
    "new_forum_topic_merge_operation_id",
    "transport::merge_topic",
    "solution_choice_required",
    "set_refresh_nonce.update",
  ],
  "Leptos merge UI",
);
assert.ok(!leptosUi.includes("graphql_adapter"));
assert.ok(!leptosUi.includes("native_server_adapter"));
assert.ok(!leptosUi.includes("ForumTopicMergeService"));
assert.equal(leptosEn["forum.merge.title"], "Merge topics");
assert.equal(leptosRu["forum.merge.title"], "Объединение тем");
includesAll(
  leptosReadme,
  [
    "FORUM-21O",
    "ssr` and `hydrate` use native server functions",
    "csr` and headless/default builds use GraphQL",
    "never triggers cross-path fallback",
    "one operation ID stable across an exact retry",
  ],
  "Leptos package handoff",
);

includesAll(
  nextCore,
  [
    "MAX_FORUM_TOPIC_MERGE_REASON_LENGTH = 500",
    "buildForumTopicMergeCommand",
    "forumTopicMergeNeedsSolutionChoice",
    "newForumTopicMergeOperationId",
    "Source and retained target topics must be different",
  ],
  "Next merge policy",
);
includesAll(
  nextApi,
  [
    "solutionReplyId",
    "export async function mergeForumTopics",
    "mergeForumTopicResolvingSolution",
    "mergeForumTopic(",
    "selectedSolutionReplyId",
  ],
  "Next Forum API",
);
includesAll(
  nextUi,
  [
    "type ClientGqlOpts = Pick<GqlOpts, 'tenantId' | 'tenantSlug'>",
    "buildForumTopicMergeCommand",
    "commandShapeChanged",
    "mergeForumTopics",
    "router.refresh()",
    "forumTopicMergeNeedsSolutionChoice",
    "import ar from '../locales/ar.json';",
    "if (normalizedLocale.startsWith('ar')) return ar;",
    "{copy.operation}",
    "{copy.event}",
    "dir='ltr'",
  ],
  "Next merge UI",
);
assert.ok(!nextUi.includes("<p className='font-medium'>Operation</p>"));
assert.ok(!nextUi.includes("<p className='font-medium'>Event</p>"));
assert.ok(!nextUi.includes("fetch("));
assert.ok(!nextUi.includes("gqlOpts?: GqlOpts"));
includesAll(
  nextNav,
  [
    "Merge Topics",
    "/dashboard/forum/merge",
    "forum_topics:manage",
  ],
  "Next navigation",
);
includesAll(
  nextPage,
  ["auth", "listForumTopics", "ForumTopicMerge", "tenantId"],
  "Next host composition",
);
assert.ok(!nextPage.includes("mergeForumTopic"));
assert.ok(!nextPage.includes("gqlOpts={{ token"));
includesAll(
  nextProxy,
  [
    "import { auth } from '@/auth'",
    "if (!headers.has('authorization'))",
    "session?.user?.rustokToken",
    "headers.set('authorization'",
  ],
  "Next authenticated GraphQL proxy",
);
assert.deepEqual(
  Object.keys(nextAr).sort(),
  Object.keys(nextEn).sort(),
  "Arabic Next Forum copy key set must match English",
);
assert.deepEqual(
  Object.keys(nextRu).sort(),
  Object.keys(nextEn).sort(),
  "Russian Next Forum copy key set must match English",
);
assert.equal(nextAr.title, "دمج مواضيع المنتدى");
assert.equal(nextEn.title, "Merge forum topics");
assert.equal(nextRu.title, "Объединение тем форума");
for (const key of ["operation", "event"]) {
  assert.equal(typeof nextAr[key], "string", `Arabic Next copy missing ${key}`);
  assert.equal(typeof nextEn[key], "string", `English Next copy missing ${key}`);
  assert.equal(typeof nextRu[key], "string", `Russian Next copy missing ${key}`);
}

includesAll(
  docs,
  [
    "# FORUM-21N admin topic merge workflow",
    "`source_ready_maintainer_execution_pending`",
    paths.contract,
    "single-adapter GraphQL state",
    "never serializes that token into the client component",
    "No command above was run by the implementation agent",
  ],
  "historical FORUM-21N handoff",
);
includesAll(
  nativeDocs,
  [
    "# FORUM-21O native Leptos admin topic-merge transport",
    paths.nativeContract,
    "direct authenticated native server-function path",
    "No command above was run by the implementation agent",
  ],
  "FORUM-21O extension handoff",
);
assert.ok(
  plan.includes("### Delivered through `FORUM-21N`") ||
    plan.includes("### Delivered through `FORUM-21O`"),
);
assert.ok(plan.includes("admin topic merge workflow"));
assert.ok(plan.includes("| `FORUM-21` | `planned` | Move, merge, split and fork topic workflows. |"));
assert.ok(!plan.includes("| `FORUM-21` | `done` |"));

console.log(
  "FORUM-21N admin topic merge UI remains source-ready and is extended by FORUM-21O native Leptos transport selection; FORUM-21 completion remains pending.",
);
