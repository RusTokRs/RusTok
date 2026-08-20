#!/usr/bin/env node

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const paths = {
  contract:
    "crates/rustok-forum/contracts/forum-topic-slug-rename-admin-ui.json",
  ownerContract: "crates/rustok-forum/contracts/forum-topic-slug-rename-owner.json",
  transportContract:
    "crates/rustok-forum/contracts/forum-topic-slug-rename-graphql-transport.json",
  docs: "crates/rustok-forum/docs/forum-24g-topic-slug-rename-admin-ui.md",
  docsIndex: "crates/rustok-forum/docs/README.md",
  adminReadme: "crates/rustok-forum/admin/README.md",
  manifest: "crates/rustok-forum/rustok-module.toml",
  leptosModel: "crates/rustok-forum/admin/src/topic_slug_rename_model.rs",
  leptosTransport: "crates/rustok-forum/admin/src/transport.rs",
  leptosGraphql:
    "crates/rustok-forum/admin/src/transport/topic_slug_rename_graphql_adapter.rs",
  leptosRoot: "crates/rustok-forum/admin/src/ui/root.rs",
  leptosUi: "crates/rustok-forum/admin/src/ui/topic_slug_rename.rs",
  leptosEn: "crates/rustok-forum/admin/locales/en.json",
  leptosRu: "crates/rustok-forum/admin/locales/ru.json",
  nextCore:
    "apps/next-admin/packages/forum/src/core/topic-slug-rename.ts",
  nextApi: "apps/next-admin/packages/forum/src/api/forum.ts",
  nextUi:
    "apps/next-admin/packages/forum/src/components/forum-topic-slug-rename.tsx",
  nextIndex: "apps/next-admin/packages/forum/src/index.ts",
  nextNav: "apps/next-admin/packages/forum/src/nav.ts",
  nextPage: "apps/next-admin/src/app/dashboard/forum/rename-slug/page.tsx",
  nextEn: "apps/next-admin/packages/forum/src/locales/en.json",
  nextRu: "apps/next-admin/packages/forum/src/locales/ru.json",
};

const read = (path) => readFileSync(path, "utf8");
const includesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(text.includes(marker), `${label} is missing marker: ${marker}`);
  }
};
const excludesAll = (text, markers, label) => {
  for (const marker of markers) {
    assert.ok(!text.includes(marker), `${label} contains forbidden marker: ${marker}`);
  }
};

const contract = JSON.parse(read(paths.contract));
const ownerContract = JSON.parse(read(paths.ownerContract));
const transportContract = JSON.parse(read(paths.transportContract));
const docs = read(paths.docs);
const docsIndex = read(paths.docsIndex);
const adminReadme = read(paths.adminReadme);
const manifest = read(paths.manifest);
const leptosModel = read(paths.leptosModel);
const leptosTransport = read(paths.leptosTransport);
const leptosGraphql = read(paths.leptosGraphql);
const leptosRoot = read(paths.leptosRoot);
const leptosUi = read(paths.leptosUi);
const leptosEn = JSON.parse(read(paths.leptosEn));
const leptosRu = JSON.parse(read(paths.leptosRu));
const nextCore = read(paths.nextCore);
const nextApi = read(paths.nextApi);
const nextUi = read(paths.nextUi);
const nextIndex = read(paths.nextIndex);
const nextNav = read(paths.nextNav);
const nextPage = read(paths.nextPage);
const nextEn = JSON.parse(read(paths.nextEn));
const nextRu = JSON.parse(read(paths.nextRu));

assert.equal(contract.contract, "forum_topic_slug_rename_admin_ui_v1");
assert.equal(contract.task, "FORUM-24G");
assert.deepEqual(contract.extends, ["FORUM-24D", "FORUM-24F"]);
assert.equal(contract.backend_owner, "TopicService::rename_slug");
assert.equal(contract.backend_graphql_field, "renameForumTopicSlug");
assert.equal(contract.authorization.required_permission, "forum_topics:update");
assert.equal(contract.leptos_admin.route, "/modules/forum/rename-slug");
assert.equal(contract.next_admin.route, "/dashboard/forum/rename-slug");
assert.equal(contract.command_policy.short_identity_is_not_computed_by_ui, true);
assert.equal(contract.command_policy.route_normalization_is_not_implemented_by_ui, true);
assert.equal(contract.result_policy.does_not_read_route_alias_table, true);
assert.equal(contract.compatibility.backend_owner_changed, false);
assert.equal(contract.compatibility.graphql_schema_changed, false);
assert.equal(ownerContract.task, "FORUM-24D");
assert.equal(ownerContract.command, "TopicService::rename_slug");
assert.equal(transportContract.task, "FORUM-24F");
assert.equal(transportContract.command.field, "renameForumTopicSlug");

includesAll(
  manifest,
  [
    'subpath = "rename-slug"',
    'title = "Rename Forum Topic Route"',
    'nav_label = "Rename Topic Route"',
  ],
  "Forum module manifest",
);

includesAll(
  leptosModel,
  [
    "pub struct ForumTopicSlugRenameCandidate",
    "pub struct ForumTopicSlugRenameCommand",
    "pub struct ForumTopicSlugRenameReceipt",
    "pub fn build_forum_topic_slug_rename_command(",
    "exact_slug_replay_remains_available_to_the_owner",
    "MAX_FORUM_TOPIC_ROUTE_SLUG_LEN",
  ],
  "Leptos framework-neutral model",
);
excludesAll(
  leptosModel,
  ["use leptos", "ForumTopicRouteService", "forum_topic_route_aliases"],
  "Leptos framework-neutral model",
);

includesAll(
  leptosGraphql,
  [
    "RENAME_CANDIDATES_QUERY",
    "RENAME_TOPIC_SLUG_MUTATION",
    "renameForumTopicSlug",
    "RenameForumTopicSlugGraphqlInput",
    "limit: 100",
    "pub async fn rename_topic_slug(",
    "GraphqlRequest::new",
  ],
  "Leptos GraphQL adapter",
);
excludesAll(
  leptosGraphql,
  [
    "ForumTopicRouteService",
    "TopicService::",
    "forum_topic_route_aliases",
    "resolve_canonical_topic",
    "fetch(",
    "reqwest",
  ],
  "Leptos GraphQL adapter",
);

includesAll(
  leptosTransport,
  [
    "mod topic_slug_rename_graphql_adapter;",
    "pub async fn fetch_topic_slug_rename_candidates(",
    "pub async fn rename_topic_slug(",
    "topic_slug_rename_graphql_adapter::",
    "topic_slug_rename_uses_the_update_graphql_transport_without_fallback",
  ],
  "Leptos transport facade",
);
includesAll(
  leptosRoot,
  [
    "ForumTopicSlugRenameAdmin",
    'subpath_matches("rename-slug")',
  ],
  "Leptos route dispatcher",
);
includesAll(
  leptosUi,
  [
    '"FORUM-24G"',
    "fetch_topic_slug_rename_candidates",
    "build_forum_topic_slug_rename_command",
    "transport::rename_topic_slug",
    "receipt.previous_path",
    "receipt.canonical.path",
    "receipt.alias_id",
    "receipt.changed",
    'data-forum-route-identifier=""',
    'dir="ltr"',
    'spellcheck="false"',
    '<dd dir="ltr" class="break-all font-mono text-xs">{receipt.previous_path}</dd>',
    '<dd dir="ltr" class="break-all font-mono text-xs">{receipt.canonical.path}</dd>',
  ],
  "Leptos slug rename UI",
);
excludesAll(
  leptosUi,
  [
    "ForumTopicRouteService",
    "forum_topic_route_aliases",
    "short_identity",
    "resolve_canonical_topic",
    "record_redirect_alias",
  ],
  "Leptos slug rename UI",
);

for (const key of [
  "forum.slugRename.title",
  "forum.slugRename.subtitle",
  "forum.slugRename.topic",
  "forum.slugRename.slug",
  "forum.slugRename.submit",
  "forum.slugRename.complete",
  "forum.slugRename.replay",
  "forum.slugRename.warning",
]) {
  assert.equal(typeof leptosEn[key], "string", `English Leptos copy missing ${key}`);
  assert.equal(typeof leptosRu[key], "string", `Russian Leptos copy missing ${key}`);
}

includesAll(
  nextCore,
  [
    "export interface ForumTopicSlugRenameCandidate",
    "export interface ForumTopicSlugRenameCommand",
    "export interface ForumTopicSlugRenameReceipt",
    "export function buildForumTopicSlugRenameCommand(",
    "MAX_FORUM_TOPIC_ROUTE_SLUG_LENGTH",
  ],
  "Next framework-neutral model",
);
excludesAll(
  nextCore,
  ["react", "next/", "ForumTopicRouteService", "forum_topic_route_aliases"],
  "Next framework-neutral model",
);
includesAll(
  nextApi,
  [
    "export async function renameForumTopicSlug(",
    "RenameForumTopicSlugGraphqlInput",
    "renameForumTopicSlug(",
    "previousPath",
    "canonical {",
    "aliasId",
    "changed",
  ],
  "Next package GraphQL API",
);
includesAll(
  nextUi,
  [
    "export function ForumTopicSlugRename",
    "buildForumTopicSlugRenameCommand",
    "renameForumTopicSlug",
    "receipt.previousPath",
    "receipt.canonical.path",
    "receipt.aliasId",
    "receipt.changed",
  ],
  "Next slug rename UI",
);
excludesAll(
  nextUi,
  [
    "ForumTopicRouteService",
    "forum_topic_route_aliases",
    "shortIdentity",
    "resolveCanonicalTopic",
    "fetch(",
  ],
  "Next slug rename UI",
);
includesAll(
  nextIndex,
  [
    "ForumTopicSlugRename",
    "./core/topic-slug-rename",
  ],
  "Next package entry",
);
includesAll(
  nextNav,
  [
    "Rename Topic Route",
    "/dashboard/forum/rename-slug",
    "forum_topics:update",
  ],
  "Next registry navigation",
);
includesAll(
  nextPage,
  [
    "ForumTopicSlugRename",
    "listForumTopics",
    "const session = await auth();",
    "tenantId",
    "gqlOpts={{ tenantId, tenantSlug }}",
  ],
  "Next host composition page",
);
excludesAll(
  nextPage,
  ["renameForumTopicSlug(", "graphqlRequest", "useState", "use client"],
  "Next host composition page",
);

for (const key of [
  "renameTitle",
  "renameSubtitle",
  "renameTopic",
  "renameSlug",
  "renameSubmit",
  "renameSuccess",
  "renameReplay",
  "renameWarning",
  "renamePreviousPath",
  "renameCanonicalPath",
]) {
  assert.equal(typeof nextEn[key], "string", `English Next copy missing ${key}`);
  assert.equal(typeof nextRu[key], "string", `Russian Next copy missing ${key}`);
}

includesAll(
  docs,
  [
    "# FORUM-24G topic slug rename admin UI",
    "`renameForumTopicSlug`",
    "`forum_topics:update`",
    "`/modules/forum/rename-slug`",
    "`/dashboard/forum/rename-slug`",
    "No command above was run by the implementation agent",
  ],
  "FORUM-24G documentation",
);
includesAll(
  docsIndex,
  [
    "FORUM-24G composes that mutation",
    "FORUM-24G topic slug rename admin UI",
  ],
  "Forum documentation index",
);
includesAll(
  adminReadme,
  [
    "/modules/forum/rename-slug",
    "topic_slug_rename_model.rs",
    "topic_slug_rename_graphql_adapter.rs",
    "forum_topics:update",
  ],
  "Forum admin README",
);

console.log(
  "FORUM-24G topic slug rename admin composition is source-ready; maintainer execution and public localized route composition remain pending.",
);
