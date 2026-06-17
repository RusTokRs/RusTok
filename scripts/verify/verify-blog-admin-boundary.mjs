#!/usr/bin/env node
// RusTok blog admin FFA boundary guardrails.
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

const libPath = "crates/rustok-blog/admin/src/lib.rs";
const corePath = "crates/rustok-blog/admin/src/core.rs";
const uiPath = "crates/rustok-blog/admin/src/ui/leptos.rs";
const transportPath = "crates/rustok-blog/admin/src/transport/mod.rs";
const graphqlAdapterPath = "crates/rustok-blog/admin/src/transport/graphql_adapter.rs";
const legacyApiPath = "crates/rustok-blog/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-blog/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

if (existsSync(repoPath(legacyApiPath))) {
  fail(`${legacyApiPath}: legacy GraphQL api adapter must live under transport/graphql_adapter.rs`);
}

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  graphqlAdapterPath,
  implementationPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected blog admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertNotContains(lib, "mod api;", `${libPath}: crate root must not wire legacy api.rs after GraphQL adapter moved under transport/`);
assertContains(lib, "mod core;", `${libPath}: crate root must wire core`);
assertContains(lib, "mod transport;", `${libPath}: crate root must wire transport facade`);
assertContains(lib, "mod ui;", `${libPath}: crate root must wire UI adapters`);
assertContains(lib, "pub use ui::BlogAdmin;", `${libPath}: crate root must re-export BlogAdmin`);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn archive_/, /pub async fn delete_/]) {
  assertNotContains(lib, marker, `${libPath}: crate root must not expose public transport passthroughs (${marker})`);
}

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
  assertNotContains(core, marker, `${corePath}: core must stay Leptos/server-function free (${marker})`);
}
for (const marker of [
  "BlogPostFormInput",
  "build_blog_post_draft",
  "BlogPostSaveOperation",
  "BlogPostSaveCommand",
  "prepare_blog_post_save_command",
  "BlogPostLoadResultViewModel",
  "blog_post_load_result_view",
  "blog_post_transport_failure_issue",
  "BlogPostSaveResultViewModel",
  "blog_post_save_result_view",
  "BlogPostEditorFormState",
  "BlogPostAdminTableRowViewModel",
  "blog_post_admin_table_row_view",
  "BlogPostAdminTableViewModel",
  "blog_post_admin_table_view",
  "BlogPostAdminFormViewModel",
  "blog_post_admin_form_view",
  "BlogPostAdminEditBannerViewModel",
  "blog_post_admin_edit_banner_view",
  "BlogPostAdminRawBodyWarningViewModel",
  "blog_post_admin_raw_body_warning_view",
  "BlogPostAdminPostsLoadViewModel",
  "blog_post_admin_posts_load_view",
  "blog_post_admin_posts_load_view_from_list",
  "show_archive_action",
  "archive_label",
  "delete_label",
  "selected_post_request",
  "issue_banner_class_or_hidden",
  "BlogPostAdminIssueBannerViewModel",
  "blog_post_admin_issue_banner_view",
  "BlogPostStatusCommand",
  "prepare_blog_post_status_command",
  "BlogPostArchiveCommand",
  "prepare_blog_post_archive_command",
  "BlogPostDeleteCommand",
  "prepare_blog_post_delete_command",
  "BlogPostAdminRouteQueryIntent",
  "blog_post_admin_open_post_query_intent",
  "blog_post_admin_saved_post_query_intent",
  "blog_post_admin_clear_post_query_intent",
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::{core, transport};", `${uiPath}: Leptos adapter must consume core and transport layers`);
assertContains(ui, "core::prepare_blog_post_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "core::BlogPostSaveOperation", `${uiPath}: UI must dispatch core-owned save operations`);
assertContains(ui, "core::blog_post_admin_edit_banner_view", `${uiPath}: UI must use core-owned edit-banner view policy`);
assertContains(ui, "core::blog_post_admin_raw_body_warning_view", `${uiPath}: UI must use core-owned raw-body warning view policy`);
assertContains(ui, "core::blog_post_admin_posts_load_view_from_list", `${uiPath}: UI must use core-owned posts load result view-list normalization policy`);
assertContains(ui, "core::blog_post_load_result_view", `${uiPath}: UI must use core-owned load result policy`);
assertContains(ui, "core::blog_post_transport_failure_issue", `${uiPath}: UI must use core-owned transport failure issue mapping`);
assertContains(ui, "core::blog_post_save_result_view", `${uiPath}: UI must use core-owned save result policy`);
assertContains(ui, "apply_blog_post_admin_route_query_intent", `${uiPath}: UI must apply core-owned route/query intents through the Leptos writer adapter`);
assertContains(ui, "core::blog_post_admin_open_post_query_intent", `${uiPath}: UI must use core-owned open-post query intent`);
assertContains(ui, "core::blog_post_admin_clear_post_query_intent", `${uiPath}: UI must use core-owned clear-post query intent`);
assertContains(ui, "transport::is_posts_contract_unavailable", `${uiPath}: UI must use transport-owned posts contract-unavailable classification`);
assertContains(ui, "core::prepare_blog_post_status_command", `${uiPath}: UI must use core-owned status command preparation`);
assertContains(ui, "core::prepare_blog_post_archive_command", `${uiPath}: UI must use core-owned archive command preparation`);
assertContains(ui, "core::prepare_blog_post_delete_command", `${uiPath}: UI must use core-owned delete command preparation`);
assertContains(ui, "transport::fetch_posts", `${uiPath}: UI must call the module-owned transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "PostService", "CategoryService", "TagService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of [
  "fetch_posts",
  "is_posts_contract_unavailable",
  "fetch_post",
  "create_post",
  "update_post",
  "publish_post",
  "unpublish_post",
  "archive_post",
  "delete_post",
]) {
  assertContains(transport, marker, `${transportPath}: transport facade must expose ${marker}`);
}
assertContains(transport, "mod graphql_adapter;", `${transportPath}: transport facade must own the GraphQL adapter module`);
assertContains(transport, "graphql_adapter::", `${transportPath}: transport facade must delegate through transport/graphql_adapter.rs`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the blog admin transport facade`);
assertContains(graphqlAdapter, "GraphqlRequest", `${graphqlAdapterPath}: blog admin GraphQL adapter must keep the GraphQL transport contract`);
assertContains(graphqlAdapter, "BLOG_POSTS_QUERY", `${graphqlAdapterPath}: GraphQL adapter must own blog posts query text`);
assertNotContains(graphqlAdapter, "Err(error) if is_posts_contract_unavailable", `${graphqlAdapterPath}: GraphQL adapter must not swallow posts contract-unavailable errors before the UI parity branch can classify them`);

assertContains(implementationPlan, "verify-blog-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the blog fast boundary guardrail`);
assertContains(registry, "verify-blog-admin-boundary.mjs", `${registryPath}: central readiness board must mention the blog fast boundary guardrail`);

if (failures.length > 0) {
  console.error("blog admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("blog admin boundary verification passed");
