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
const transportPath = "crates/rustok-blog/admin/src/transport.rs";
const apiPath = "crates/rustok-blog/admin/src/api.rs";
const implementationPlanPath = "crates/rustok-blog/docs/implementation-plan.md";
const registryPath = "docs/modules/registry.md";

for (const filePath of [
  libPath,
  corePath,
  uiPath,
  transportPath,
  apiPath,
  implementationPlanPath,
  registryPath,
]) {
  assertExists(filePath, `${filePath}: expected blog admin FFA boundary file`);
}

const lib = readRepo(libPath);
const core = readRepo(corePath);
const ui = readRepo(uiPath);
const transport = readRepo(transportPath);
const api = readRepo(apiPath);
const implementationPlan = readRepo(implementationPlanPath);
const registry = readRepo(registryPath);

assertContains(lib, "mod api;", `${libPath}: crate root must wire current GraphQL/api adapter privately`);
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
  "BlogPostEditorFormState",
  "BlogPostAdminTableRowViewModel",
  "blog_post_admin_table_row_view",
  "BlogPostAdminTableViewModel",
  "blog_post_admin_table_view",
  "BlogPostAdminFormViewModel",
  "blog_post_admin_form_view",
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
]) {
  assertContains(core, marker, `${corePath}: expected core-owned FFA helper ${marker}`);
}

assertContains(ui, "use crate::{core, transport};", `${uiPath}: Leptos adapter must consume core and transport layers`);
assertContains(ui, "core::prepare_blog_post_save_command", `${uiPath}: UI must use core-owned save command preparation`);
assertContains(ui, "core::BlogPostSaveOperation", `${uiPath}: UI must dispatch core-owned save operations`);
assertContains(ui, "core::prepare_blog_post_status_command", `${uiPath}: UI must use core-owned status command preparation`);
assertContains(ui, "core::prepare_blog_post_archive_command", `${uiPath}: UI must use core-owned archive command preparation`);
assertContains(ui, "core::prepare_blog_post_delete_command", `${uiPath}: UI must use core-owned delete command preparation`);
assertContains(ui, "transport::fetch_posts", `${uiPath}: UI must call the module-owned transport facade`);
for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "PostService", "CategoryService", "TagService"]) {
  assertNotContains(ui, marker, `${uiPath}: UI adapter must not call raw transport or services (${marker})`);
}

for (const marker of [
  "fetch_posts",
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
assertContains(transport, "use crate::api", `${transportPath}: transport facade may delegate to the current GraphQL/api adapter`);
assertNotContains(transport, "#[server", `${transportPath}: server/native endpoints must not live in the blog admin transport facade`);
assertContains(api, "GraphqlRequest", `${apiPath}: blog admin api adapter must keep the GraphQL transport contract`);

assertContains(implementationPlan, "verify-blog-admin-boundary.mjs", `${implementationPlanPath}: local plan must mention the blog fast boundary guardrail`);
assertContains(registry, "verify-blog-admin-boundary.mjs", `${registryPath}: central readiness board must mention the blog fast boundary guardrail`);

if (failures.length > 0) {
  console.error("blog admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("blog admin boundary verification passed");
