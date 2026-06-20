#!/usr/bin/env node
// RusTok pages UI FFA boundary guardrails.
// Fast source-level checks for module-owned admin/storefront core/transport/ui splits.

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

const files = {
  adminLib: "crates/rustok-pages/admin/src/lib.rs",
  adminCore: "crates/rustok-pages/admin/src/core.rs",
  adminUi: "crates/rustok-pages/admin/src/ui/leptos.rs",
  adminTransport: "crates/rustok-pages/admin/src/transport/mod.rs",
  adminApi: "crates/rustok-pages/admin/src/api.rs",
  storefrontLib: "crates/rustok-pages/storefront/src/lib.rs",
  storefrontCore: "crates/rustok-pages/storefront/src/core.rs",
  storefrontUi: "crates/rustok-pages/storefront/src/ui/leptos.rs",
  storefrontTransport: "crates/rustok-pages/storefront/src/transport.rs",
  storefrontApi: "crates/rustok-pages/storefront/src/api.rs",
  implementationPlan: "crates/rustok-pages/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
};

for (const [name, filePath] of Object.entries(files)) {
  assertExists(filePath, `${name}: expected pages FFA boundary file at ${filePath}`);
}

const adminLib = readRepo(files.adminLib);
const adminCore = readRepo(files.adminCore);
const adminUi = readRepo(files.adminUi);
const adminTransport = readRepo(files.adminTransport);
const adminApi = readRepo(files.adminApi);
const storefrontLib = readRepo(files.storefrontLib);
const storefrontCore = readRepo(files.storefrontCore);
const storefrontUi = readRepo(files.storefrontUi);
const storefrontTransport = readRepo(files.storefrontTransport);
const storefrontApi = readRepo(files.storefrontApi);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);

for (const [source, label, exportMarker] of [
  [adminLib, files.adminLib, "pub use ui::leptos::PagesAdmin;"],
  [storefrontLib, files.storefrontLib, "pub use ui::leptos::PagesView;"],
]) {
  assertContains(source, "mod api;", `${label}: crate root must wire api adapter privately`);
  assertContains(source, "mod core;", `${label}: crate root must wire core`);
  assertContains(source, "mod transport;", `${label}: crate root must wire transport facade`);
  assertContains(source, exportMarker, `${label}: crate root must re-export only the public UI entrypoint`);
  for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn delete_/]) {
    assertNotContains(source, marker, `${label}: crate root must not expose public transport passthroughs (${marker})`);
  }
}

for (const [source, label] of [
  [adminCore, files.adminCore],
  [storefrontCore, files.storefrontCore],
]) {
  for (const marker of ["leptos::", "leptos_", "#[component]", "#[server", "LocalResource", "WriteSignal", "web_sys::"]) {
    assertNotContains(source, marker, `${label}: core must stay Leptos/server-function free (${marker})`);
  }
}

for (const marker of [
  "PageDraftFormInput",
  "build_create_page_draft",
  "missing_required_page_field",
  "write_path_issue_with_context",
  "builder_host_fallback_surface",
  "publish_state_view",
  "channel_count_label",
  "legacy_block_snapshot_label",
  "is_save_action_busy",
  "is_publish_action_disabled",
  "admin_page_list_item_view",
]) {
  assertContains(adminCore, marker, `${files.adminCore}: expected admin core-owned helper ${marker}`);
}
for (const marker of [
  "selected_page_title",
  "selected_page_slug",
  "summarize_page_content",
  "storefront_builder_fallback_read_contract",
  "count_label",
  "page_link_href",
  "page_status_label",
  "selected_page_empty_state",
  "load_error_message",
  "storefront_page_list_item_view",
]) {
  assertContains(storefrontCore, marker, `${files.storefrontCore}: expected storefront core-owned helper ${marker}`);
}

assertContains(adminUi, "use crate::core;", `${files.adminUi}: admin UI must consume core layer`);
assertContains(adminUi, "use crate::transport;", `${files.adminUi}: admin UI must consume transport layer`);
assertContains(adminUi, "core::build_create_page_draft", `${files.adminUi}: admin UI must use core-owned draft preparation`);
assertContains(adminUi, "core::publish_state_view", `${files.adminUi}: admin UI must use core-owned publish state mapping`);
assertContains(adminUi, "core::legacy_block_snapshot_label", `${files.adminUi}: admin UI must use core-owned legacy block labels`);
assertContains(adminUi, "core::is_save_action_busy", `${files.adminUi}: admin UI must use core-owned save busy state mapping`);
assertContains(adminUi, "core::is_publish_action_disabled", `${files.adminUi}: admin UI must use core-owned publish disabled mapping`);
assertContains(adminUi, "core::admin_page_list_item_view", `${files.adminUi}: admin UI must use core-owned table item view mapping`);
assertContains(adminUi, "transport::fetch_pages", `${files.adminUi}: admin UI must call transport facade`);
assertContains(storefrontUi, "use crate::core;", `${files.storefrontUi}: storefront UI must consume core layer`);
assertContains(storefrontUi, "use crate::transport;", `${files.storefrontUi}: storefront UI must consume transport layer`);
assertContains(storefrontUi, "core::selected_page_title", `${files.storefrontUi}: storefront UI must use core-owned selected page view helpers`);
assertContains(storefrontUi, "core::selected_page_empty_state", `${files.storefrontUi}: storefront UI must use core-owned selected-page empty state`);
assertContains(storefrontUi, "core::load_error_message", `${files.storefrontUi}: storefront UI must use core-owned load error composition`);
assertContains(storefrontUi, "core::storefront_page_list_item_view", `${files.storefrontUi}: storefront UI must use core-owned list item view mapping`);
assertContains(storefrontUi, "transport::fetch_pages", `${files.storefrontUi}: storefront UI must call transport facade`);
for (const [source, label] of [
  [adminUi, files.adminUi],
  [storefrontUi, files.storefrontUi],
]) {
  for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "PageService", "MenuService"] ) {
    assertNotContains(source, marker, `${label}: UI adapter must not call raw transport or services (${marker})`);
  }
}

for (const marker of ["fetch_pages", "fetch_page", "create_page", "update_page", "publish_page", "unpublish_page", "delete_page"]) {
  assertContains(adminTransport, marker, `${files.adminTransport}: admin transport facade must expose ${marker}`);
}
assertContains(storefrontTransport, "fetch_pages", `${files.storefrontTransport}: storefront transport facade must expose fetch_pages`);
for (const [source, label] of [
  [adminTransport, files.adminTransport],
  [storefrontTransport, files.storefrontTransport],
]) {
  assertContains(source, "use crate::api", `${label}: transport facade may delegate to current api adapter`);
  assertNotContains(source, "#[server", `${label}: server/native endpoints must not live in the transport facade`);
}
for (const [source, label] of [
  [adminApi, files.adminApi],
  [storefrontApi, files.storefrontApi],
]) {
  assertContains(source, "GraphqlRequest", `${label}: api adapter must keep the GraphQL transport contract`);
}
assertContains(storefrontApi, "#[server", `${files.storefrontApi}: storefront api adapter must keep the native server-function path`);

assertContains(implementationPlan, "verify-pages-ui-boundary.mjs", `${files.implementationPlan}: local plan must mention the pages fast boundary guardrail`);
assertContains(registry, "verify-pages-ui-boundary.mjs", `${files.registry}: central readiness board must mention the pages fast boundary guardrail`);

if (failures.length > 0) {
  console.error("pages UI boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pages UI boundary verification passed");
