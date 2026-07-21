#!/usr/bin/env node
// RusTok Pages FFA guardrails for the current builder-only admin architecture.

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

function assertNotExists(relativePath, description) {
  if (existsSync(repoPath(relativePath))) fail(description);
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
  adminModel: "crates/rustok-pages/admin/src/model.rs",
  adminTransport: "crates/rustok-pages/admin/src/transport/mod.rs",
  adminGraphqlAdapter: "crates/rustok-pages/admin/src/transport/graphql_adapter.rs",
  storefrontLib: "crates/rustok-pages/storefront/src/lib.rs",
  storefrontCore: "crates/rustok-pages/storefront/src/core.rs",
  storefrontUi: "crates/rustok-pages/storefront/src/ui/leptos.rs",
  storefrontTransport: "crates/rustok-pages/storefront/src/transport/mod.rs",
  storefrontGraphqlAdapter: "crates/rustok-pages/storefront/src/transport/graphql_adapter.rs",
  storefrontNativeServerAdapter: "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
  implementationPlan: "crates/rustok-pages/docs/implementation-plan.md",
  registry: "docs/modules/registry.md",
};

for (const [name, filePath] of Object.entries(files)) {
  assertExists(filePath, `${name}: expected Pages boundary file at ${filePath}`);
}

for (const legacyPath of [
  "crates/rustok-pages/admin/src/api.rs",
  "crates/rustok-pages/admin/src/editor_sync.rs",
  "crates/rustok-pages/admin/src/ui/mod.rs",
  "crates/rustok-pages/admin/src/ui/leptos.rs",
  "crates/rustok-pages/storefront/src/api.rs",
]) {
  assertNotExists(legacyPath, `${legacyPath}: obsolete Pages surface must stay deleted`);
}

const adminCargo = readRepo(files.adminCargo);
const adminLib = readRepo(files.adminLib);
const adminBuilder = readRepo(files.adminBuilder);
const adminComposition = readRepo(files.adminComposition);
const adminCore = readRepo(files.adminCore);
const adminModel = readRepo(files.adminModel);
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

for (const marker of ["mod builder;", "mod composition;", "mod core;", "mod transport;"]) {
  assertContains(adminLib, marker, `${files.adminLib}: missing ${marker}`);
}
for (const forbidden of ["mod api;", "mod ui;", "mod editor_sync;", "pub mod ui"] ) {
  assertNotContains(adminLib, forbidden, `${files.adminLib}: obsolete admin module marker ${forbidden}`);
}
assertContains(
  adminLib,
  "pub use composition::PagesAdmin;",
  `${files.adminLib}: builder-first composition must be the sole admin entrypoint`,
);
for (const marker of [/pub async fn fetch_/, /pub async fn create_/, /pub async fn update_/, /pub async fn publish_/, /pub async fn delete_/]) {
  assertNotContains(adminLib, marker, `${files.adminLib}: crate root must not expose transport passthroughs (${marker})`);
}

for (const dependency of [
  'rustok-page-builder = { path = "../../rustok-page-builder"',
  'rustok-page-builder-admin = { path = "../../rustok-page-builder/admin"',
]) {
  assertContains(adminCargo, dependency, `${files.adminCargo}: missing ${dependency}`);
}
for (const obsoleteDependency of ["rustok-api", "rustok-seo-admin-support", "rustok-seo-targets"]) {
  assertNotContains(adminCargo, obsoleteDependency, `${files.adminCargo}: obsolete admin dependency ${obsoleteDependency}`);
}

for (const marker of [
  "PagesBuilderFacade",
  "impl PageBuilderAdminFacade for PagesBuilderFacade",
  "PageBuilderCapabilityRequest::Publish",
  "transport::fetch_page",
  "transport::update_page",
  "REVISION_CONFLICT",
  "canonicalize_builder_project",
  "pages[].component",
]) {
  assertContains(adminBuilder, marker, `${files.adminBuilder}: expected current builder marker ${marker}`);
}
for (const obsoleteBuilderMarker of [
  "copy_frame_component",
  "synchronize_frame_component",
  "canonical_component_refreshes_frame_snapshot",
]) {
  assertNotContains(adminBuilder, obsoleteBuilderMarker, `${files.adminBuilder}: legacy frame compatibility marker ${obsoleteBuilderMarker}`);
}
for (const forbidden of ["rustok_graphql", "GraphqlRequest", "#[server", "reqwest::"]) {
  assertNotContains(adminBuilder, forbidden, `${files.adminBuilder}: builder facade must use Pages transport, not ${forbidden}`);
}

for (const marker of [
  "pub fn PagesAdmin()",
  "CreatePageCard",
  "PagesNavigator",
  "PageWorkspace",
  "transport::fetch_pages",
  "transport::fetch_page",
  "transport::create_page",
  "transport::publish_page",
  "transport::unpublish_page",
  "transport::delete_page",
  "PageBuilderAdminHostContext",
  "PageBuilderAdmin",
]) {
  assertContains(adminComposition, marker, `${files.adminComposition}: expected builder workspace marker ${marker}`);
}
for (const obsoleteUiMarker of [
  "crate::ui",
  "Project data (grapesjs)",
  "preview_html",
  "project_tree",
  "Existing blocks",
  "compatibility_warning",
  "<textarea",
]) {
  assertNotContains(adminComposition, obsoleteUiMarker, `${files.adminComposition}: obsolete parallel UI marker ${obsoleteUiMarker}`);
}
assertNotContains(adminComposition, "graphql_adapter", `${files.adminComposition}: composition must not select a raw transport adapter`);

for (const marker of [
  "PageDraftFormInput",
  "build_create_page_draft",
  "edit_form_seed_from_page",
  "default_project_data",
  "parse_project_data",
  "status_badge_class",
]) {
  assertContains(adminCore, marker, `${files.adminCore}: expected current domain helper ${marker}`);
}
for (const obsoleteCoreMarker of [
  "PageBlock",
  "builder_host_fallback_surface",
  "preview_html",
  "project_tree",
  "legacy_block_snapshot_label",
  "compatibility_warning_view",
  "issue_banner_view",
]) {
  assertNotContains(adminCore, obsoleteCoreMarker, `${files.adminCore}: obsolete UI helper ${obsoleteCoreMarker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server", "LocalResource", "web_sys::"]) {
  assertNotContains(adminCore, marker, `${files.adminCore}: core must remain framework-neutral (${marker})`);
}

for (const obsoleteModelMarker of ["PageBlock", "blocks: Vec", "pub blocks"] ) {
  assertNotContains(adminModel, obsoleteModelMarker, `${files.adminModel}: obsolete block model marker ${obsoleteModelMarker}`);
}
for (const obsoleteTransportMarker of ["blocks {", "blocks: Option", "blocks: None"]) {
  assertNotContains(adminGraphqlAdapter, obsoleteTransportMarker, `${files.adminGraphqlAdapter}: obsolete block transport marker ${obsoleteTransportMarker}`);
}

for (const marker of ["fetch_pages", "fetch_page", "create_page", "update_page", "publish_page", "unpublish_page", "delete_page"]) {
  assertContains(adminTransport, marker, `${files.adminTransport}: transport facade must expose ${marker}`);
}
assertContains(adminTransport, "mod graphql_adapter;", `${files.adminTransport}: admin transport must own GraphQL adapter`);
assertNotContains(adminTransport, "#[server", `${files.adminTransport}: server functions must not live in transport facade`);

assertNotContains(storefrontLib, "mod api;", `${files.storefrontLib}: storefront legacy api module must stay removed`);
assertContains(storefrontLib, "pub use ui::leptos::PagesView;", `${files.storefrontLib}: storefront entrypoint must remain module-owned`);
for (const [source, label] of [
  [storefrontCore, files.storefrontCore],
  [adminCore, files.adminCore],
]) {
  for (const marker of ["#[server", "LocalResource", "WriteSignal"]) {
    assertNotContains(source, marker, `${label}: core must not own runtime adapters (${marker})`);
  }
}
for (const marker of [
  "core::selected_page_title",
  "core::selected_page_empty_state",
  "core::load_error_message",
  "transport::fetch_pages",
]) {
  assertContains(storefrontUi, marker, `${files.storefrontUi}: expected storefront marker ${marker}`);
}
assertContains(storefrontTransport, "mod graphql_adapter;", `${files.storefrontTransport}: storefront GraphQL adapter missing`);
assertContains(storefrontTransport, "mod native_server_adapter;", `${files.storefrontTransport}: storefront native adapter missing`);
assertNotContains(storefrontTransport, "#[server", `${files.storefrontTransport}: native endpoint must stay behind its adapter`);
assertContains(storefrontGraphqlAdapter, "GraphqlRequest", `${files.storefrontGraphqlAdapter}: GraphQL contract missing`);
assertContains(storefrontNativeServerAdapter, "#[server", `${files.storefrontNativeServerAdapter}: native server endpoint missing`);
assertContains(storefrontNativeServerAdapter, "expect_context::<HostRuntimeContext>()", `${files.storefrontNativeServerAdapter}: host runtime context missing`);

assertContains(implementationPlan, "verify-pages-ui-boundary.mjs", `${files.implementationPlan}: local plan must mention the guardrail`);
assertContains(implementationPlan, "no legacy", `${files.implementationPlan}: local plan must record the no-legacy policy`);
assertContains(registry, "verify-pages-ui-boundary.mjs", `${files.registry}: central readiness board must mention the guardrail`);

if (failures.length > 0) {
  console.error("pages UI boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("pages UI boundary verification passed");
