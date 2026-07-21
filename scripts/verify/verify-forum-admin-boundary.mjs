#!/usr/bin/env node
// RusTok forum admin FFA boundary guardrails.
// Fast source-level checks for the module-owned core/transport/ui split.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const libPath = "crates/rustok-forum/admin/src/lib.rs";
const corePath = "crates/rustok-forum/admin/src/core.rs";
const modelPath = "crates/rustok-forum/admin/src/model.rs";
const uiPath = "crates/rustok-forum/admin/src/ui/leptos.rs";
const categoryDndPath = "crates/rustok-forum/admin/src/ui/category_dnd.rs";
const transportPath = "crates/rustok-forum/admin/src/transport.rs";
const legacyApiPath = "crates/rustok-forum/admin/src/api.rs";
const graphqlAdapterPath = "crates/rustok-forum/admin/src/transport/graphql_adapter.rs";
const restAdapterPath = "crates/rustok-forum/admin/src/transport/rest_adapter.rs";
const categoryTreeGraphqlAdapterPath = "crates/rustok-forum/admin/src/transport/category_tree_graphql_adapter.rs";
const categoryTreeRestAdapterPath = "crates/rustok-forum/admin/src/transport/category_tree_rest_adapter.rs";
const implementationPlanPath = "crates/rustok-forum/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";
const packagePath = "package.json";
const verifierTestPath = "scripts/verify/verify-forum-admin-boundary.test.mjs";

for (const filePath of [
  libPath,
  corePath,
  modelPath,
  uiPath,
  categoryDndPath,
  transportPath,
  graphqlAdapterPath,
  restAdapterPath,
  categoryTreeGraphqlAdapterPath,
  categoryTreeRestAdapterPath,
  implementationPlanPath,
  registryPath,
  packagePath,
  verifierTestPath,
]) {
  assertExists(filePath, `${filePath}: expected forum admin FFA boundary file`);
}
if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: forum admin legacy api.rs must stay removed; transport/rest_adapter.rs owns REST fallback operations`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const model = readRepo(modelPath);
const ui = readRepo(uiPath);
const categoryDnd = readRepo(categoryDndPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const restAdapter = readRepo(restAdapterPath);
const categoryTreeGraphqlAdapter = readRepo(categoryTreeGraphqlAdapterPath);
const categoryTreeRestAdapter = readRepo(categoryTreeRestAdapterPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);
const packageJson = JSON.parse(readRepo(packagePath));
const verifierTest = readRepo(verifierTestPath);

assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::leptos::ForumAdmin;", `${libPath}: crate root must re-export ForumAdmin`);
assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api adapter`);

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "CategoryFormSnapshot",
  "TopicFormSnapshot",
  "ForumAdminRouteQueryIntent",
  "ForumAdminDeleteOutcome",
  "forum_admin_delete_outcome",
  "forum_admin_busy_key",
  "ForumAdminBusySurface",
  "ForumAdminFormErrorLabels",
  "ForumAdminCategorySelectOption",
  "category_select_options",
  "forum_admin_topic_tag_count_label",
  "forum_admin_editing_thread_label",
  "forum_admin_position_value",
  "forum_admin_sidebar_category_class",
  "forum_admin_status_badge_class",
  "forum_admin_tag_chips",
  "forum_admin_title_envelope_view_model",
  "forum_admin_placeholder_policy",
  "forum_admin_seo_copy_labels",
  "forum_admin_form_error_message",
  "forum_admin_transport_error_message",
  "selected_category_filter_label",
  "forum_admin_collection_state",
  "category_card_view_model",
  "topic_card_view_model",
  "ForumAdminModeratorNotesLabels",
  "forum_admin_moderator_notes_copy_labels",
  "ForumAdminSidebarLabels",
  "forum_admin_sidebar_copy_labels",
  "ForumAdminMetricSurface",
  "forum_admin_metric_accent_class",
  "ForumAdminActionButtonKind",
  "forum_admin_action_button_class",
  "ForumAdminCategoryMatrixLabels",
  "forum_admin_category_matrix_labels",
  "ForumAdminCategoryFormLabels",
  "forum_admin_category_form_labels",
  "ForumAdminTopicStreamLabels",
  "forum_admin_topic_stream_labels",
  "ForumAdminTopicFormLabels",
  "forum_admin_topic_form_labels",
  "ForumAdminReplyPreviewLabels",
  "forum_admin_reply_preview_labels",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

for (const marker of [
  "CategoryTreeResponse",
  "into_flat_items",
  "CategoryDropPlacement",
  "CategoryMoveRequest",
  "category_drop_move_request",
]) {
  assertContains(model, marker, `${modelPath}: expected canonical category tree/drop model ${marker}`);
}
assertNotContains(model, "leptos::", `${modelPath}: category tree/drop planning must remain UI-framework free`);

assertContains(ui, "use crate::core::{", `${uiPath}: Leptos adapter must import core-owned helpers`);
assertContains(ui, "use crate::transport;", `${uiPath}: Leptos adapter must call the module-owned transport facade`);
assertContains(ui, "CategoryDndGrid", `${uiPath}: category admin must mount the owner-command DnD component`);
assertContains(ui, "transport::fetch_category_tree", `${uiPath}: category admin must load the canonical category tree`);
assertNotContains(ui, "transport::fetch_categories(token_value", `${uiPath}: category hierarchy must not be reconstructed from the flat compatibility list`);
assertContains(ui, "forum_admin_category_matrix_labels", `${uiPath}: UI must consume core-owned category matrix labels`);
assertContains(ui, "forum_admin_category_form_labels", `${uiPath}: UI must consume core-owned category form labels`);
assertContains(ui, "forum_admin_topic_stream_labels", `${uiPath}: UI must consume core-owned topic stream labels`);
assertContains(ui, "forum_admin_topic_form_labels", `${uiPath}: UI must consume core-owned topic form labels`);
assertContains(ui, "forum_admin_reply_preview_labels", `${uiPath}: UI must consume core-owned reply preview labels`);
assertContains(ui, "forum_admin_busy_key", `${uiPath}: UI must consume core-owned busy-key construction`);
assertContains(ui, "forum_admin_form_error_message", `${uiPath}: UI must consume core-owned form error policy`);
assertContains(ui, "forum_admin_transport_error_message", `${uiPath}: UI must consume core-owned transport error formatting`);
assertContains(ui, "category_select_options", `${uiPath}: UI must consume core-owned category select options`);
assertContains(ui, "forum_admin_topic_tag_count_label", `${uiPath}: UI must consume core-owned tag count label policy`);
assertContains(ui, "forum_admin_editing_thread_label", `${uiPath}: UI must consume core-owned editing thread label policy`);
assertContains(ui, "forum_admin_position_value", `${uiPath}: UI must consume core-owned position parsing policy`);
assertContains(ui, "forum_admin_sidebar_category_class", `${uiPath}: UI must consume core-owned sidebar class policy`);
assertContains(ui, "forum_admin_status_badge_class", `${uiPath}: UI must consume core-owned status badge class policy`);
assertContains(ui, "forum_admin_tag_chips", `${uiPath}: UI must consume core-owned tag chip parsing policy`);
assertContains(ui, "forum_admin_title_envelope_view_model", `${uiPath}: UI must consume core-owned title envelope policy`);
assertContains(ui, "forum_admin_placeholder_policy", `${uiPath}: UI must consume core-owned placeholder policy`);
assertContains(ui, "forum_admin_seo_copy_labels", `${uiPath}: UI must consume core-owned SEO copy mapping`);
assertContains(ui, "forum_admin_delete_outcome", `${uiPath}: UI must consume core-owned delete outcome policy`);
assertContains(ui, "CategoryFormSnapshot", `${uiPath}: UI must consume core-owned category form snapshots`);
assertContains(ui, "TopicFormSnapshot", `${uiPath}: UI must consume core-owned topic form snapshots`);
assertContains(ui, "forum_admin_moderator_notes_copy_labels", `${uiPath}: UI must consume core-owned moderator notes copy policy`);
assertContains(ui, "forum_admin_sidebar_copy_labels", `${uiPath}: UI must consume core-owned sidebar copy policy`);
assertContains(ui, "forum_admin_metric_accent_class", `${uiPath}: UI must consume core-owned metric accent policy`);
assertContains(ui, "forum_admin_action_button_class", `${uiPath}: UI must consume core-owned action button style policy`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "ForumService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}
for (const rawBusyMarker of ["category:edit", "category:save", "category:delete", "topic:edit", "topic:save", "topic:delete"]) {
  assertNotContains(ui, rawBusyMarker, `${uiPath}: busy-key strings must stay in core helper (${rawBusyMarker})`);
}
for (const rawMetricColor of ["bg-sky-500", "bg-amber-500", "bg-emerald-500"]) {
  assertNotContains(ui, `accent_class="${rawMetricColor}"`, `${uiPath}: metric accent colors must stay in core policy (${rawMetricColor})`);
}
for (const rawActionButtonClass of [
  "rounded-full border border-border px-4 py-2 text-sm font-medium transition hover:bg-muted",
  "rounded-full border border-destructive/30 bg-destructive/10 px-4 py-2 text-sm font-medium text-destructive transition hover:bg-destructive/15",
]) {
  assertNotContains(ui, rawActionButtonClass, `${uiPath}: action button styles must stay in core policy (${rawActionButtonClass})`);
}
assertNotContains(ui, /format!\("\{\}: \{err\}"/, `${uiPath}: transport error message composition must stay in core helper`);

for (const marker of [
  "category_drop_move_request",
  "CategoryDropPlacement::Before",
  "CategoryDropPlacement::Inside",
  "CategoryDropPlacement::RootEnd",
  "on:dragstart",
  "on:drop",
  "transport::move_category",
]) {
  assertContains(categoryDnd, marker, `${categoryDndPath}: expected owner-command drag-and-drop marker ${marker}`);
}
for (const marker of ["crate::api", "ForumService", "update_category("]) {
  assertNotContains(categoryDnd, marker, `${categoryDndPath}: DnD must not bypass the transport owner move command (${marker})`);
}

for (const marker of ["fetch_category_tree", "fetch_categories", "fetch_category", "create_category", "update_category", "move_category", "reorder_category_siblings", "delete_category", "fetch_topics", "fetch_topic", "create_topic", "update_topic", "delete_topic", "fetch_replies"]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod category_tree_graphql_adapter;", `${transportPath}: transport facade must wire canonical tree GraphQL adapter`);
assertContains(transport, "mod category_tree_rest_adapter;", `${transportPath}: transport facade must wire canonical tree REST fallback`);
assertContains(transport, "category_tree_graphql_adapter::fetch_category_tree", `${transportPath}: canonical tree must prefer GraphQL`);
assertContains(transport, "category_tree_rest_adapter::fetch_category_tree", `${transportPath}: canonical tree must keep REST fallback`);
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must wire GraphQL adapter`);
assertContains(transport, "mod rest_adapter;", `${transportPath}: transport facade must wire REST fallback adapter`);
assertContains(transport, "graphql_adapter::fetch_categories", `${transportPath}: flat compatibility read must prefer GraphQL`);
assertContains(transport, "rest_adapter::fetch_categories", `${transportPath}: flat compatibility read must keep REST fallback`);
assertContains(transport, "graphql_adapter::move_category", `${transportPath}: placement must prefer the GraphQL owner command`);
assertContains(transport, "rest_adapter::move_category", `${transportPath}: placement must keep the REST owner-command fallback`);
assertContains(transport, "graphql_adapter::reorder_category_siblings", `${transportPath}: sibling reorder must prefer GraphQL`);
assertContains(transport, "rest_adapter::reorder_category_siblings", `${transportPath}: sibling reorder must keep REST fallback`);
assertNotContains(transport, "use crate::api", `${transportPath}: transport facade must not delegate to legacy api module`);

assertContains(categoryTreeGraphqlAdapter, "forumCategoryTree", `${categoryTreeGraphqlAdapterPath}: GraphQL adapter must query canonical category tree`);
assertContains(categoryTreeGraphqlAdapter, "MAX_CATEGORY_TREE_DEPTH", `${categoryTreeGraphqlAdapterPath}: GraphQL tree selection must cover owner depth bound`);
assertContains(categoryTreeGraphqlAdapter, "archived_at: archivedAt", `${categoryTreeGraphqlAdapterPath}: GraphQL tree must project lifecycle state`);
assertContains(categoryTreeRestAdapter, "/categories/tree", `${categoryTreeRestAdapterPath}: REST fallback must call canonical tree endpoint`);
assertContains(graphqlAdapter, "moveForumCategory", `${graphqlAdapterPath}: GraphQL adapter must call moveForumCategory`);
assertContains(graphqlAdapter, "reorderForumCategorySiblings", `${graphqlAdapterPath}: GraphQL adapter must call reorderForumCategorySiblings`);
assertContains(restAdapter, "reqwest", `${restAdapterPath}: forum admin REST adapter must keep the REST transport contract`);
assertContains(restAdapter, "/categories/{id}/move", `${restAdapterPath}: REST adapter must call the category move endpoint`);
assertContains(restAdapter, "/categories/reorder", `${restAdapterPath}: REST adapter must call the sibling reorder endpoint`);
for (const adapter of [graphqlAdapter, restAdapter]) {
  assertNotContains(adapter, "position: Some(draft.position)", "forum admin adapters must not bypass placement owner commands through generic update");
}

assertContains(implementationPlan, "verify-forum-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the forum fast boundary guardrail`);
assertContains(implementationPlan, "interactive admin drag-and-drop", `${implementationPlanPath}: canonical plan must record the DnD owner-command integration`);
assertContains(registry, "verify-forum-admin-boundary.mjs", `${registryPath}: central readiness board must mention the forum fast boundary guardrail`);
assertContains(registry, "forum-wave1-rollout-evidence.json", `${registryPath}: central readiness board must mention Wave 1 rollout evidence`);

const scripts = packageJson.scripts ?? {};
if (scripts["test:verify:forum:admin-boundary"] !== "node scripts/verify/verify-forum-admin-boundary.test.mjs") {
  fail(`${packagePath}: package scripts must expose forum boundary fixture tests`);
}
assertContains(
  scripts["test:verify:ffa:ui:migration"] ?? "",
  "npm run test:verify:forum:admin-boundary",
  `${packagePath}: aggregate FFA fixture tests must include forum boundary fixtures`,
);
for (const marker of [
  "passes canonical fixture",
  "rejects Leptos-specific core",
  "rejects raw api calls from UI",
  "rejects legacy admin api module",
  "rejects raw busy-key strings from UI",
  "rejects flat category hierarchy reads",
  "rejects DnD generic update bypass",
  "rejects missing package fixture script",
]) {
  assertContains(verifierTest, marker, `${verifierTestPath}: expected fixture coverage marker ${marker}`);
}

if (failures.length > 0) {
  console.error("forum admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum admin boundary verification passed");
