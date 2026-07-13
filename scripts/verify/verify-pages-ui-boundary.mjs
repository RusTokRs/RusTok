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
  adminCargo: "crates/rustok-pages/admin/Cargo.toml",
  adminLib: "crates/rustok-pages/admin/src/lib.rs",
  adminBuilder: "crates/rustok-pages/admin/src/builder.rs",
  adminComposition: "crates/rustok-pages/admin/src/composition.rs",
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

const adminCargo = readRepo(files.adminCargo);
const adminLib = readRepo(files.adminLib);
const adminBuilder = readRepo(files.adminBuilder);
const adminComposition = readRepo(files.adminComposition);
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
const implementationPlan = readRepo(files.implementationPlan);
const registry = readRepo(files.registry);

assertNotContains(adminLib, "mod api;", `${files.adminLib}: admin crate root must not wire legacy api adapter`);
for (const marker of ["mod builder;", "mod composition;", "mod core;", "mod transport;"]) {
  assertContains(adminLib, marker, `${files.adminLib}: missing ${marker}`);
}
assertContains(
  adminLib,
  "pub use composition::PagesAdmin;",
  `${files.adminLib}: public entrypoint must be the Pages/Fly composition wrapper`,
);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn delete_/]) {
  assertNotContains(adminLib, marker, `${files.adminLib}: crate root must not expose public transport passthroughs (${marker})`);
}

for (const dependency of [
  'rustok-page-builder = { path = "../../rustok-page-builder"',
  'rustok-page-builder-admin = { path = "../../rustok-page-builder/admin"',
]) {
  assertContains(adminCargo, dependency, `${files.adminCargo}: missing ${dependency}`);
}

for (const marker of [
  "PagesBuilderFacade",
  "impl PageBuilderAdminFacade for PagesBuilderFacade",
  "PageBuilderCapabilityRequest::Publish",
  "transport::fetch_page",
  "transport::update_page",
  "REVISION_CONFLICT",
  "canonicalize_builder_project",
  "copy_frame_component",
  "synchronize_frame_component",
  "controller_from_project",
]) {
  assertContains(adminBuilder, marker, `${files.adminBuilder}: expected Page Builder consumer marker ${marker}`);
}
for (const forbidden of ["rustok_graphql", "GraphqlRequest", "#[server", "reqwest::"]) {
  assertNotContains(adminBuilder, forbidden, `${files.adminBuilder}: consumer facade must use the Pages transport facade, not ${forbidden}`);
}

for (const marker of [
  "pub fn PagesAdmin()",
  "use_route_query_value",
  "transport::fetch_page",
  "PagesBuilderFacade",
  "PageBuilderAdminHostContext",
  "provide_context",
  "PageBuilderAdmin",
  "crate::ui::leptos::PagesAdmin",
]) {
  assertContains(adminComposition, marker, `${files.adminComposition}: expected Fly composition marker ${marker}`);
}
assertNotContains(adminComposition, "graphql_adapter", `${files.adminComposition}: composition must not select a transport adapter`);

assertNotContains(storefrontLib, "mod api;", `${files.storefrontLib}: crate root must not wire legacy api adapter`);
assertContains(storefrontLib, "mod core;", `${files.storefrontLib}: crate root must wire core`);
assertContains(storefrontLib, "mod transport;", `${files.storefrontLib}: crate root must wire transport facade`);
assertContains(storefrontLib, "pub use ui::leptos::PagesView;", `${files.storefrontLib}: crate root must re-export only the public UI entrypoint`);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn delete_/]) {
  assertNotContains(storefrontLib, marker, `${files.storefrontLib}: crate root must not expose public transport passthroughs (${marker})`);
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
for (const marker of [
  "core::build_create_page_draft",
  "core::publish_state_view",
  "core::legacy_block_snapshot_label",
  "core::is_save_action_busy",
  "core::is_publish_action_disabled",
  "core::admin_page_list_item_view",
  "core::admin_page_row_action_state",
  "core::admin_page_row_action_labels",
  "core::issue_banner_view",
  "core::compatibility_warning_view",
  "core::page_properties_view",
  "banner.class_name",
  "transport::fetch_pages",
]) {
  assertContains(adminUi, marker, `${files.adminUi}: expected legacy metadata UI marker ${marker}`);
}
assertNotContains(adminUi, "core::issue_banner_class", `${files.adminUi}: admin UI must not bypass issue_banner_view for banner class policy`);

assertContains(storefrontUi, "use crate::core;", `${files.storefrontUi}: storefront UI must consume core layer`);
assertContains(storefrontUi, "use crate::transport;", `${files.storefrontUi}: storefront UI must consume transport layer`);
for (const marker of [
  "core::selected_page_title",
  "core::selected_page_empty_state",
  "core::load_error_message",
  "core::storefront_page_list_item_view",
  "core::published_pages_empty_state",
  "core::published_pages_header_view",
  "transport::fetch_pages",
]) {
  assertContains(storefrontUi, marker, `${files.storefrontUi}: expected storefront marker ${marker}`);
}

for (const [source, label] of [
  [adminUi, files.adminUi],
  [adminComposition, files.adminComposition],
  [storefrontUi, files.storefrontUi],
]) {
  for (const marker of ["crate::api", /(^|[^A-Za-z0-9_])api::/, "#[server", "PageService", "MenuService"]) {
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

assertContains(implementationPlan, "verify-pages-ui-boundary.mjs", `${files.implementationPlan}: local plan must mention the pages fast boundary guardrail`);
assertContains(registry, "verify-pages-ui-boundary.mjs", `${files.registry}: central readiness board must mention the pages fast boundary guardrail`);

if (failures.length > 0) {
  console.error("pages UI boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pages UI boundary verification passed");
