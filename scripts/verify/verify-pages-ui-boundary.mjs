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
  adminLegacyApi: "crates/rustok-pages/admin/src/api.rs",
  adminGraphqlAdapter: "crates/rustok-pages/admin/src/transport/graphql_adapter.rs",
  storefrontLib: "crates/rustok-pages/storefront/src/lib.rs",
  storefrontCore: "crates/rustok-pages/storefront/src/core.rs",
  storefrontUi: "crates/rustok-pages/storefront/src/ui/leptos.rs",
  storefrontTransport: "crates/rustok-pages/storefront/src/transport/mod.rs",
  storefrontLegacyApi: "crates/rustok-pages/storefront/src/api.rs",
  storefrontGraphqlAdapter: "crates/rustok-pages/storefront/src/transport/graphql_adapter.rs",
  storefrontNativeServerAdapter: "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
  storefrontCargo: "crates/rustok-pages/storefront/Cargo.toml",
  implementationPlan: "crates/rustok-pages/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
};

for (const [name, filePath] of Object.entries(files)) {
  if (name === "adminLegacyApi" || name === "storefrontLegacyApi") continue;
  assertExists(filePath, `${name}: expected pages FFA boundary file at ${filePath}`);
}
if (existsSync(repoPath(files.adminLegacyApi))) {
  fail(`${files.adminLegacyApi}: admin legacy api.rs must stay removed; transport/graphql_adapter.rs owns GraphQL operations`);
}
if (existsSync(repoPath(files.storefrontLegacyApi))) {
  fail(`${files.storefrontLegacyApi}: storefront legacy api.rs must stay removed; transport/{graphql_adapter,native_server_adapter}.rs own raw transport operations`);
}

const adminLib = readRepo(files.adminLib);
const adminCore = readRepo(files.adminCore);
const adminUi = readRepo(files.adminUi);
const adminTransport = readRepo(files.adminTransport);
const adminGraphqlAdapter = readRepo(files.adminGraphqlAdapter);
const storefrontLib = readRepo(files.storefrontLib);
const storefrontCore = readRepo(files.storefrontCore);
const storefrontUi = readRepo(files.storefrontUi);
const storefrontTransport = readRepo(files.storefrontTransport);
const storefrontGraphqlAdapter = readRepo(files.storefrontGraphqlAdapter);
const storefrontNativeServerAdapter = readRepo(files.storefrontNativeServerAdapter);
const storefrontCargo = readRepo(files.storefrontCargo);
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);

for (const [source, label, exportMarker] of [
  [storefrontLib, files.storefrontLib, "pub use ui::leptos::PagesView;"],
]) {
  assertNotContains(source, "mod api;", `${label}: crate root must not wire legacy api adapter`);
  assertContains(source, "mod core;", `${label}: crate root must wire core`);
  assertContains(source, "mod transport;", `${label}: crate root must wire transport facade`);
  assertContains(source, exportMarker, `${label}: crate root must re-export only the public UI entrypoint`);
  for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn delete_/]) {
    assertNotContains(source, marker, `${label}: crate root must not expose public transport passthroughs (${marker})`);
  }
}
assertNotContains(adminLib, "mod api;", `${files.adminLib}: admin crate root must not wire legacy api adapter`);
assertContains(adminLib, "mod core;", `${files.adminLib}: admin crate root must wire core`);
assertContains(adminLib, "mod transport;", `${files.adminLib}: admin crate root must wire transport facade`);
assertContains(adminLib, "pub use ui::leptos::PagesAdmin;", `${files.adminLib}: admin crate root must re-export only the public UI entrypoint`);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn delete_/]) {
  assertNotContains(adminLib, marker, `${files.adminLib}: crate root must not expose public transport passthroughs (${marker})`);
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
  "admin_page_row_action_state",
  "admin_page_row_action_labels",
  "issue_banner_view",
  "compatibility_warning_view",
  "page_properties_view",
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
  "published_pages_empty_state",
  "published_pages_header_view",
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
assertContains(adminUi, "core::admin_page_row_action_state", `${files.adminUi}: admin UI must use core-owned table action busy mapping`);
assertContains(adminUi, "core::admin_page_row_action_labels", `${files.adminUi}: admin UI must use core-owned table action label mapping`);
assertContains(adminUi, "core::issue_banner_view", `${files.adminUi}: admin UI must use core-owned issue banner view mapping`);
assertContains(adminUi, "core::compatibility_warning_view", `${files.adminUi}: admin UI must use core-owned compatibility warning view mapping`);
assertContains(adminUi, "core::page_properties_view", `${files.adminUi}: admin UI must use core-owned properties panel view mapping`);
assertContains(adminUi, "banner.class_name", `${files.adminUi}: admin UI must render core-owned issue banner class from the view model`);
assertNotContains(adminUi, "core::issue_banner_class", `${files.adminUi}: admin UI must not bypass issue_banner_view for banner class policy`);
assertContains(adminUi, "transport::fetch_pages", `${files.adminUi}: admin UI must call transport facade`);
assertContains(storefrontUi, "use crate::core;", `${files.storefrontUi}: storefront UI must consume core layer`);
assertContains(storefrontUi, "use crate::transport;", `${files.storefrontUi}: storefront UI must consume transport layer`);
assertContains(storefrontUi, "core::selected_page_title", `${files.storefrontUi}: storefront UI must use core-owned selected page view helpers`);
assertContains(storefrontUi, "core::selected_page_empty_state", `${files.storefrontUi}: storefront UI must use core-owned selected-page empty state`);
assertContains(storefrontUi, "core::load_error_message", `${files.storefrontUi}: storefront UI must use core-owned load error composition`);
assertContains(storefrontUi, "core::storefront_page_list_item_view", `${files.storefrontUi}: storefront UI must use core-owned list item view mapping`);
assertContains(storefrontUi, "core::published_pages_empty_state", `${files.storefrontUi}: storefront UI must use core-owned published-pages empty state`);
assertContains(storefrontUi, "core::published_pages_header_view", `${files.storefrontUi}: storefront UI must use core-owned published-pages header mapping`);
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
assertContains(adminTransport, "mod graphql_adapter;", `${files.adminTransport}: admin transport must own GraphQL adapter module`);
assertContains(adminTransport, "graphql_adapter::fetch_pages", `${files.adminTransport}: admin transport must delegate through GraphQL adapter`);
assertNotContains(adminTransport, "use crate::api", `${files.adminTransport}: admin transport facade must not delegate to legacy api module`);
assertContains(storefrontTransport, "mod graphql_adapter;", `${files.storefrontTransport}: storefront transport must own GraphQL adapter module`);
assertContains(storefrontTransport, "mod native_server_adapter;", `${files.storefrontTransport}: storefront transport must own native server adapter module`);
assertContains(storefrontTransport, "native_server_adapter::fetch_storefront_pages_server", `${files.storefrontTransport}: storefront transport facade must delegate through native server adapter first`);
assertContains(storefrontTransport, "graphql_adapter::fetch_storefront_pages", `${files.storefrontTransport}: storefront transport facade must keep GraphQL fallback`);
assertNotContains(storefrontTransport, "use crate::api", `${files.storefrontTransport}: storefront transport facade must not delegate to legacy api module`);
assertNotContains(storefrontTransport, "#[server", `${files.storefrontTransport}: server/native endpoints must not live in the transport facade`);
assertNotContains(adminTransport, "#[server", `${files.adminTransport}: server/native endpoints must not live in the transport facade`);
for (const [source, label] of [
  [adminGraphqlAdapter, files.adminGraphqlAdapter],
  [storefrontGraphqlAdapter, files.storefrontGraphqlAdapter],
]) {
  assertContains(source, "GraphqlRequest", `${label}: api adapter must keep the GraphQL transport contract`);
}
assertContains(storefrontNativeServerAdapter, "#[server", `${files.storefrontNativeServerAdapter}: storefront native server adapter must keep the native server-function path`);
assertContains(storefrontNativeServerAdapter, "expect_context::<HostRuntimeContext>()", `${files.storefrontNativeServerAdapter}: storefront native server adapter must use the host runtime context`);
assertContains(storefrontNativeServerAdapter, "shared_get::<TransactionalEventBus>()", `${files.storefrontNativeServerAdapter}: storefront native server adapter must receive the event bus through the host runtime context`);
assertContains(storefrontNativeServerAdapter, "runtime_ctx.db_clone()", `${files.storefrontNativeServerAdapter}: storefront native server adapter must receive DB through the host runtime context`);
assertNotContains(storefrontNativeServerAdapter, "loco_rs", `${files.storefrontNativeServerAdapter}: storefront native server adapter must not depend on Loco AppContext`);
assertNotContains(storefrontNativeServerAdapter, "rustok_outbox::loco", `${files.storefrontNativeServerAdapter}: storefront native server adapter must not use the outbox Loco adapter`);
assertNotContains(storefrontCargo, "loco-rs", `${files.storefrontCargo}: pages storefront must not depend on Loco`);
assertNotContains(storefrontCargo, "loco-adapter", `${files.storefrontCargo}: pages storefront must not enable the outbox Loco adapter feature`);

assertContains(implementationPlan, "verify-pages-ui-boundary.mjs", `${files.implementationPlan}: local plan must mention the pages fast boundary guardrail`);
assertContains(registry, "verify-pages-ui-boundary.mjs", `${files.registry}: central readiness board must mention the pages fast boundary guardrail`);

if (failures.length > 0) {
  console.error("pages UI boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pages UI boundary verification passed");
