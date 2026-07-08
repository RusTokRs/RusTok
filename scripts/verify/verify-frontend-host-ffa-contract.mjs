#!/usr/bin/env node
// Fast source-level guardrail for frontend hosts in the FFA migration.
// Host apps are FFA-compatible composition roots, not module-owned UI packages.

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

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: expected file to exist`);
}

function assertMissing(relativePath, description) {
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

const docs = [
  "docs/UI/README.md",
  "docs/verification/platform-frontend-surfaces-verification-plan.md",
  "apps/admin/src/widgets/app_shell/core.rs",
  "apps/admin/src/widgets/app_shell/sidebar.rs",
  "apps/storefront/src/widgets/header/core.rs",
  "apps/storefront/src/widgets/header/mod.rs",
  "apps/admin/src/features/workflow/mod.rs",
  "apps/admin/src/features/workflow/model.rs",
  "apps/admin/src/features/workflow/transport/mod.rs",
  "apps/admin/src/features/workflow/transport/graphql_adapter.rs",
  "apps/admin/src/features/workflow/transport/native_server_adapter.rs",
  "apps/admin/src/features/oauth_apps/mod.rs",
  "apps/admin/src/features/oauth_apps/model.rs",
  "apps/admin/src/features/oauth_apps/transport/mod.rs",
  "apps/admin/src/features/oauth_apps/transport/graphql_adapter.rs",
  "apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs",
  "apps/admin/src/features/installer/mod.rs",
  "apps/admin/src/features/installer/model.rs",
  "apps/admin/src/features/installer/transport/mod.rs",
  "apps/admin/src/features/modules/mod.rs",
  "apps/admin/src/features/modules/transport/mod.rs",
  "apps/admin/src/features/modules/transport/client.rs",
  "apps/admin/src/features/modules/transport/native_server_adapter.rs",
  "apps/admin/build.rs",
  "apps/admin/docs/README.md",
  "apps/storefront/docs/README.md",
  "apps/next-admin/docs/README.md",
  "apps/next-frontend/docs/README.md",
];

for (const doc of docs) assertExists(doc);

const uiReadme = readRepo("docs/UI/README.md");
const frontendPlan = readRepo("docs/verification/platform-frontend-surfaces-verification-plan.md");
const adminShellCore = readRepo("apps/admin/src/widgets/app_shell/core.rs");
const adminSidebar = readRepo("apps/admin/src/widgets/app_shell/sidebar.rs");
const storefrontHeaderCore = readRepo("apps/storefront/src/widgets/header/core.rs");
const storefrontHeader = readRepo("apps/storefront/src/widgets/header/mod.rs");
const adminWorkflowMod = readRepo("apps/admin/src/features/workflow/mod.rs");
const adminWorkflowModel = readRepo("apps/admin/src/features/workflow/model.rs");
const adminWorkflowTransport = readRepo("apps/admin/src/features/workflow/transport/mod.rs");
const adminWorkflowGraphqlAdapter = readRepo("apps/admin/src/features/workflow/transport/graphql_adapter.rs");
const adminWorkflowNativeAdapter = readRepo("apps/admin/src/features/workflow/transport/native_server_adapter.rs");
const adminOauthAppsMod = readRepo("apps/admin/src/features/oauth_apps/mod.rs");
const adminOauthAppsModel = readRepo("apps/admin/src/features/oauth_apps/model.rs");
const adminOauthAppsTransport = readRepo("apps/admin/src/features/oauth_apps/transport/mod.rs");
const adminOauthAppsGraphqlAdapter = readRepo("apps/admin/src/features/oauth_apps/transport/graphql_adapter.rs");
const adminOauthAppsNativeAdapter = readRepo("apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs");
const adminInstallerMod = readRepo("apps/admin/src/features/installer/mod.rs");
const adminInstallerModel = readRepo("apps/admin/src/features/installer/model.rs");
const adminInstallerTransport = readRepo("apps/admin/src/features/installer/transport/mod.rs");
const adminInstallerPage = readRepo("apps/admin/src/pages/installer.rs");
const adminModulesMod = readRepo("apps/admin/src/features/modules/mod.rs");
const adminModulesTransport = readRepo("apps/admin/src/features/modules/transport/mod.rs");
const adminModulesTransportClient = readRepo("apps/admin/src/features/modules/transport/client.rs");
const adminModulesNativeAdapter = readRepo("apps/admin/src/features/modules/transport/native_server_adapter.rs");
const adminBuild = readRepo("apps/admin/build.rs");
const adminDocs = readRepo("apps/admin/docs/README.md");
const storefrontDocs = readRepo("apps/storefront/docs/README.md");
const nextAdminDocs = readRepo("apps/next-admin/docs/README.md");
const nextFrontendDocs = readRepo("apps/next-frontend/docs/README.md");

assertContains(
  uiReadme,
  "## FFA Status for Frontend Hosts",
  "docs/UI/README.md: must explicitly document frontend host FFA status",
);
assertContains(
  uiReadme,
  "FFA-compatible composition host",
  "docs/UI/README.md: host apps must be described as FFA-compatible composition hosts",
);
assertContains(
  uiReadme,
  "do not receive a module FFA status",
  "docs/UI/README.md: must state host apps do not receive module FFA status",
);

for (const [label, text] of [
  ["frontend plan", frontendPlan],
  ["apps/admin docs", adminDocs],
  ["apps/storefront docs", storefrontDocs],
  ["apps/next-admin docs", nextAdminDocs],
  ["apps/next-frontend docs", nextFrontendDocs],
]) {
  assertContains(
    text,
    "FFA-compatible composition host",
    `${label}: must use the shared frontend-host FFA classification`,
  );
}

assertContains(
  frontendPlan,
  "verify:frontend:host-ffa-contract",
  "frontend verification plan: must include the frontend host FFA gate",
);

for (const marker of ["leptos::", "leptos_", "leptos_router", "#[component]", "#[server]", "IntoView"]) {
  if (adminShellCore.includes(marker)) {
    fail(`apps/admin/src/widgets/app_shell/core.rs: host core must stay Leptos-free (${marker})`);
  }
  if (storefrontHeaderCore.includes(marker)) {
    fail(`apps/storefront/src/widgets/header/core.rs: host core must stay Leptos-free (${marker})`);
  }
}

for (const marker of [
  "build_module_nav_groups",
  "href_is_active",
  "module_group_icon",
]) {
  assertContains(
    adminShellCore,
    marker,
    `apps/admin/src/widgets/app_shell/core.rs: missing host navigation core helper ${marker}`,
  );
  assertContains(
    adminSidebar,
    marker,
    `apps/admin/src/widgets/app_shell/sidebar.rs: Leptos adapter must consume core helper ${marker}`,
  );
}

assertContains(
  storefrontHeaderCore,
  "build_header_links",
  "apps/storefront/src/widgets/header/core.rs: missing storefront header link core helper",
);
assertContains(
  storefrontHeader,
  "build_header_links",
  "apps/storefront/src/widgets/header/mod.rs: Leptos adapter must consume storefront header core helper",
);

assertMissing(
  "apps/admin/src/features/workflow/api.rs",
  "apps/admin/src/features/workflow/api.rs: removed workflow api facade must stay removed",
);
assertContains(adminWorkflowMod, "pub mod model;", "apps/admin/src/features/workflow/mod.rs: workflow host feature must wire model");
assertContains(adminWorkflowMod, "pub mod transport;", "apps/admin/src/features/workflow/mod.rs: workflow host feature must wire transport facade");
assertNotContains(adminWorkflowMod, "pub mod api;", "apps/admin/src/features/workflow/mod.rs: workflow host feature must not wire api facade");

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]"]) {
  assertNotContains(adminWorkflowModel, marker, `apps/admin/src/features/workflow/model.rs: workflow model must stay framework/server-function free (${marker})`);
}

assertContains(
  adminWorkflowTransport,
  "mod graphql_adapter;",
  "apps/admin/src/features/workflow/transport/mod.rs: workflow transport facade must wire GraphQL adapter",
);
assertContains(
  adminWorkflowTransport,
  "mod native_server_adapter;",
  "apps/admin/src/features/workflow/transport/mod.rs: workflow transport facade must wire native server adapter",
);
assertContains(
  adminWorkflowTransport,
  "UiTransportPath::NativeServer",
  "apps/admin/src/features/workflow/transport/mod.rs: workflow transport facade must select native path",
);
assertContains(
  adminWorkflowTransport,
  "UiTransportPath::Graphql",
  "apps/admin/src/features/workflow/transport/mod.rs: workflow transport facade must keep GraphQL selected path",
);
assertNotContains(
  adminWorkflowTransport,
  "#[server",
  "apps/admin/src/features/workflow/transport/mod.rs: server functions belong in native_server_adapter.rs",
);
assertContains(
  adminWorkflowNativeAdapter,
  "#[server",
  "apps/admin/src/features/workflow/transport/native_server_adapter.rs: native adapter must own server-function endpoints",
);
assertNotContains(
  adminWorkflowGraphqlAdapter,
  "#[server",
  "apps/admin/src/features/workflow/transport/graphql_adapter.rs: GraphQL adapter must not contain server-function endpoints",
);

const workflowHostCallers = [
  "apps/admin/src/pages/workflows.rs",
  "apps/admin/src/pages/workflow_detail.rs",
  "apps/admin/src/features/workflow/components/workflow_step_editor.rs",
  "apps/admin/src/features/workflow/components/template_gallery.rs",
  "apps/admin/src/features/workflow/components/version_history.rs",
].map((relativePath) => [relativePath, readRepo(relativePath)]);

for (const [relativePath, source] of workflowHostCallers) {
  assertContains(source, /workflow::(?:transport|\{[\s\S]*transport)/, `${relativePath}: workflow host caller must use the transport facade`);
  assertNotContains(source, "workflow::api", `${relativePath}: workflow host caller must not use the removed api facade`);
  assertNotContains(source, "native_server_adapter::", `${relativePath}: workflow host caller must not call native adapter directly`);
  assertNotContains(source, "graphql_adapter::", `${relativePath}: workflow host caller must not call GraphQL adapter directly`);
}

assertMissing(
  "apps/admin/src/features/oauth_apps/api.rs",
  "apps/admin/src/features/oauth_apps/api.rs: removed OAuth apps api facade must stay removed",
);
assertContains(adminOauthAppsMod, "pub mod model;", "apps/admin/src/features/oauth_apps/mod.rs: OAuth apps host feature must wire model");
assertContains(adminOauthAppsMod, "pub mod transport;", "apps/admin/src/features/oauth_apps/mod.rs: OAuth apps host feature must wire transport facade");
assertNotContains(adminOauthAppsMod, "pub mod api;", "apps/admin/src/features/oauth_apps/mod.rs: OAuth apps host feature must not wire api facade");

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]"]) {
  assertNotContains(adminOauthAppsModel, marker, `apps/admin/src/features/oauth_apps/model.rs: OAuth apps model must stay framework/server-function free (${marker})`);
}

assertContains(
  adminOauthAppsTransport,
  "mod graphql_adapter;",
  "apps/admin/src/features/oauth_apps/transport/mod.rs: OAuth apps transport facade must wire GraphQL adapter",
);
assertContains(
  adminOauthAppsTransport,
  "mod native_server_adapter;",
  "apps/admin/src/features/oauth_apps/transport/mod.rs: OAuth apps transport facade must wire native server adapter",
);
assertContains(
  adminOauthAppsTransport,
  "UiTransportPath::NativeServer",
  "apps/admin/src/features/oauth_apps/transport/mod.rs: OAuth apps transport facade must select native list path",
);
assertContains(
  adminOauthAppsTransport,
  "UiTransportPath::Graphql",
  "apps/admin/src/features/oauth_apps/transport/mod.rs: OAuth apps transport facade must keep GraphQL selected path",
);
assertNotContains(
  adminOauthAppsTransport,
  "#[server",
  "apps/admin/src/features/oauth_apps/transport/mod.rs: server functions belong in native_server_adapter.rs",
);
assertContains(
  adminOauthAppsNativeAdapter,
  "#[server",
  "apps/admin/src/features/oauth_apps/transport/native_server_adapter.rs: native adapter must own server-function endpoints",
);
assertNotContains(
  adminOauthAppsGraphqlAdapter,
  "#[server",
  "apps/admin/src/features/oauth_apps/transport/graphql_adapter.rs: GraphQL adapter must not contain server-function endpoints",
);

const oauthAppsHostCallers = [
  "apps/admin/src/features/oauth_apps/create_app.rs",
  "apps/admin/src/features/oauth_apps/edit_app.rs",
  "apps/admin/src/features/oauth_apps/rotate_secret.rs",
  "apps/admin/src/features/oauth_apps/revoke_app.rs",
].map((relativePath) => [relativePath, readRepo(relativePath)]);

for (const [relativePath, source] of oauthAppsHostCallers) {
  assertContains(source, /oauth_apps::(?:transport|\{[\s\S]*transport)/, `${relativePath}: OAuth apps host caller must use the transport facade`);
  assertNotContains(source, "oauth_apps::api", `${relativePath}: OAuth apps host caller must not use the removed api facade`);
  assertNotContains(source, "native_server_adapter::", `${relativePath}: OAuth apps host caller must not call native adapter directly`);
  assertNotContains(source, "graphql_adapter::", `${relativePath}: OAuth apps host caller must not call GraphQL adapter directly`);
  assertNotContains(source, "crate::shared::api::request", `${relativePath}: OAuth apps host caller must not execute raw GraphQL requests directly`);
}

assertMissing(
  "apps/admin/src/features/installer/api.rs",
  "apps/admin/src/features/installer/api.rs: removed installer api facade must stay removed",
);
assertContains(adminInstallerMod, "pub mod model;", "apps/admin/src/features/installer/mod.rs: installer host feature must wire model");
assertContains(adminInstallerMod, "pub mod transport;", "apps/admin/src/features/installer/mod.rs: installer host feature must wire transport facade");
assertNotContains(adminInstallerMod, "pub mod api;", "apps/admin/src/features/installer/mod.rs: installer host feature must not wire api facade");

for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]"]) {
  assertNotContains(adminInstallerModel, marker, `apps/admin/src/features/installer/model.rs: installer model must stay framework/server-function free (${marker})`);
  assertNotContains(adminInstallerTransport, marker, `apps/admin/src/features/installer/transport/mod.rs: installer transport must stay framework/server-function free (${marker})`);
}

for (const endpoint of [
  "/api/install/status",
  "/api/install/preflight",
  "/api/install/apply",
  "/api/install/jobs/{job_id}",
  "/api/install/sessions/{session_id}/receipts",
]) {
  assertContains(
    adminInstallerTransport,
    endpoint,
    `apps/admin/src/features/installer/transport/mod.rs: installer transport must own ${endpoint}`,
  );
  assertNotContains(
    adminInstallerPage,
    endpoint,
    `apps/admin/src/pages/installer.rs: installer page must not own raw endpoint ${endpoint}`,
  );
}

assertContains(
  adminInstallerPage,
  /installer::(?:transport|\{[\s\S]*transport)/,
  "apps/admin/src/pages/installer.rs: installer page must use the transport facade",
);
for (const marker of [
  "installer::api",
  "features::installer::api",
  "reqwest::Client",
  "api_base_url",
  "extract_http_error",
]) {
  assertNotContains(
    adminInstallerPage,
    marker,
    `apps/admin/src/pages/installer.rs: installer page must not use raw installer API wiring (${marker})`,
  );
}

assertMissing(
  "apps/admin/src/features/modules/api",
  "apps/admin/src/features/modules/api: modules host boundary must be named transport",
);
assertMissing(
  "apps/admin/src/features/modules/api.rs.bak",
  "apps/admin/src/features/modules/api.rs.bak: backup source artifact must stay removed",
);
assertContains(adminModulesMod, "pub mod transport;", "apps/admin/src/features/modules/mod.rs: modules host feature must wire transport boundary");
assertNotContains(adminModulesMod, "pub mod api;", "apps/admin/src/features/modules/mod.rs: modules host feature must not wire api boundary");
assertContains(adminModulesTransport, "pub mod client;", "apps/admin/src/features/modules/transport/mod.rs: modules transport must expose client helpers");
assertContains(
  adminModulesTransport,
  "pub mod native_server_adapter;",
  "apps/admin/src/features/modules/transport/mod.rs: modules transport must expose native server-function adapter",
);
assertContains(
  adminModulesTransportClient,
  "UiTransportPath",
  "apps/admin/src/features/modules/transport/client.rs: modules transport client must keep selected transport path logic",
);
assertContains(
  adminModulesNativeAdapter,
  "#[server",
  "apps/admin/src/features/modules/transport/native_server_adapter.rs: modules native adapter must own server-function endpoints",
);
assertContains(
  adminBuild,
  "child_pages: Vec<AdminNestedPageContract>",
  "apps/admin/build.rs: admin module registry must read canonical child_pages metadata",
);
assertNotContains(
  adminBuild,
  'alias = "pages"',
  "apps/admin/build.rs: admin module registry must not accept provides.admin_ui.pages",
);

const modulesHostCallers = [
  "apps/admin/src/pages/modules.rs",
  "apps/admin/src/shared/context/enabled_modules.rs",
  "apps/admin/src/features/modules/components/modules_list.rs",
  "apps/admin/src/features/modules/components/module_detail_panel.rs",
  "apps/admin/src/features/modules/components/detail/governance.rs",
  "apps/admin/src/features/modules/components/detail/governance_form.rs",
].map((relativePath) => [relativePath, readRepo(relativePath)]);

for (const [relativePath, source] of modulesHostCallers) {
  assertContains(source, /modules::(?:transport|\{[\s\S]*transport)/, `${relativePath}: modules host caller must use the transport boundary`);
  assertNotContains(source, "modules::api", `${relativePath}: modules host caller must not use the removed api boundary`);
}

if (failures.length > 0) {
  console.error("Frontend host FFA contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Frontend host FFA contract verification passed");
