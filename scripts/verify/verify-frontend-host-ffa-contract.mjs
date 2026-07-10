#!/usr/bin/env node
// Fast source-level guardrail for Leptos hosts in the FFA migration.
// Next.js hosts use a separate package-ownership and contract-parity model.

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
  "apps/storefront/src/shared/context/enabled_modules_native_server_adapter.rs",
  "apps/storefront/src/shared/context/canonical_route_native_server_adapter.rs",
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
  "apps/admin/src/features/cache/mod.rs",
  "apps/admin/src/features/cache/model.rs",
  "apps/admin/src/features/cache/transport/mod.rs",
  "apps/admin/src/features/cache/transport/native_server_adapter.rs",
  "apps/admin/src/features/dashboard/mod.rs",
  "apps/admin/src/features/dashboard/model.rs",
  "apps/admin/src/features/dashboard/transport/mod.rs",
  "apps/admin/src/features/dashboard/transport/native_server_adapter.rs",
  "apps/admin/src/features/email/mod.rs",
  "apps/admin/src/features/email/model.rs",
  "apps/admin/src/features/email/transport/mod.rs",
  "apps/admin/src/features/email/transport/native_server_adapter.rs",
  "apps/admin/src/features/modules/mod.rs",
  "apps/admin/src/features/modules/transport/mod.rs",
  "apps/admin/src/features/modules/transport/client.rs",
  "apps/admin/src/features/modules/transport/native_server_adapter.rs",
  "apps/admin/build.rs",
  "apps/admin/docs/README.md",
  "apps/storefront/docs/README.md",
  "apps/next-admin/docs/README.md",
  "apps/next-admin/README.md",
  "apps/next-admin/docs/implementation-plan.md",
  "apps/next-admin/packages/blog/src/index.ts",
  "apps/next-admin/packages/cache/src/index.ts",
  "apps/next-admin/packages/commerce/src/index.ts",
  "apps/next-admin/packages/email/src/index.ts",
  "apps/next-admin/packages/events/src/index.ts",
  "apps/next-admin/packages/rbac/src/index.ts",
  "apps/next-admin/packages/rustok-product/src/index.ts",
  "apps/next-admin/packages/workflow/src/index.ts",
  "apps/next-admin/src/shared/api/modules.ts",
  "apps/next-admin/src/shared/api/oauth-apps.ts",
  "apps/next-admin/src/shared/api/index.ts",
  "apps/next-admin/src/modules/index.ts",
  "apps/next-frontend/docs/README.md",
  "apps/next-frontend/docs/implementation-plan.md",
  "apps/next-frontend/packages/rustok-blog/src/index.tsx",
  "apps/next-frontend/packages/rustok-blog/src/api/posts.ts",
  "apps/next-frontend/packages/rustok-product/src/index.ts",
  "apps/next-frontend/packages/search/src/index.tsx",
  "apps/next-frontend/src/modules/index.ts",
  "apps/next-frontend/src/modules/registry.ts",
  "apps/next-frontend/src/shared/lib/graphql.ts",
];

for (const doc of docs) assertExists(doc);

const uiReadme = readRepo("docs/UI/README.md");
const frontendPlan = readRepo("docs/verification/platform-frontend-surfaces-verification-plan.md");
const adminShellCore = readRepo("apps/admin/src/widgets/app_shell/core.rs");
const adminSidebar = readRepo("apps/admin/src/widgets/app_shell/sidebar.rs");
const storefrontHeaderCore = readRepo("apps/storefront/src/widgets/header/core.rs");
const storefrontHeader = readRepo("apps/storefront/src/widgets/header/mod.rs");
const storefrontEnabledModulesAdapter = readRepo("apps/storefront/src/shared/context/enabled_modules_native_server_adapter.rs");
const storefrontCanonicalRouteAdapter = readRepo("apps/storefront/src/shared/context/canonical_route_native_server_adapter.rs");
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
const adminCacheMod = readRepo("apps/admin/src/features/cache/mod.rs");
const adminCacheModel = readRepo("apps/admin/src/features/cache/model.rs");
const adminCacheTransport = readRepo("apps/admin/src/features/cache/transport/mod.rs");
const adminCacheNativeAdapter = readRepo("apps/admin/src/features/cache/transport/native_server_adapter.rs");
const adminCachePage = readRepo("apps/admin/src/pages/cache.rs");
const adminDashboardMod = readRepo("apps/admin/src/features/dashboard/mod.rs");
const adminDashboardModel = readRepo("apps/admin/src/features/dashboard/model.rs");
const adminDashboardTransport = readRepo("apps/admin/src/features/dashboard/transport/mod.rs");
const adminDashboardNativeAdapter = readRepo("apps/admin/src/features/dashboard/transport/native_server_adapter.rs");
const adminDashboardPage = readRepo("apps/admin/src/pages/dashboard.rs");
const adminEmailMod = readRepo("apps/admin/src/features/email/mod.rs");
const adminEmailModel = readRepo("apps/admin/src/features/email/model.rs");
const adminEmailTransport = readRepo("apps/admin/src/features/email/transport/mod.rs");
const adminEmailNativeAdapter = readRepo("apps/admin/src/features/email/transport/native_server_adapter.rs");
const adminEmailPage = readRepo("apps/admin/src/pages/email_settings.rs");
const adminInstallerPage = readRepo("apps/admin/src/pages/installer.rs");
const adminModulesMod = readRepo("apps/admin/src/features/modules/mod.rs");
const adminModulesTransport = readRepo("apps/admin/src/features/modules/transport/mod.rs");
const adminModulesTransportClient = readRepo("apps/admin/src/features/modules/transport/client.rs");
const adminModulesNativeAdapter = readRepo("apps/admin/src/features/modules/transport/native_server_adapter.rs");
const adminBuild = readRepo("apps/admin/build.rs");
const adminDocs = readRepo("apps/admin/docs/README.md");
const storefrontDocs = readRepo("apps/storefront/docs/README.md");
const nextAdminDocs = readRepo("apps/next-admin/docs/README.md");
const nextAdminReadme = readRepo("apps/next-admin/README.md");
const nextAdminPlan = readRepo("apps/next-admin/docs/implementation-plan.md");
const nextAdminPackageEntrypoints = [
  "apps/next-admin/packages/blog/src/index.ts",
  "apps/next-admin/packages/cache/src/index.ts",
  "apps/next-admin/packages/commerce/src/index.ts",
  "apps/next-admin/packages/email/src/index.ts",
  "apps/next-admin/packages/events/src/index.ts",
  "apps/next-admin/packages/rbac/src/index.ts",
  "apps/next-admin/packages/rustok-product/src/index.ts",
  "apps/next-admin/packages/workflow/src/index.ts",
].map((relativePath) => [relativePath, readRepo(relativePath)]);
const nextAdminSharedApiIndex = readRepo("apps/next-admin/src/shared/api/index.ts");
const nextAdminModulesIndex = readRepo("apps/next-admin/src/modules/index.ts");
const nextFrontendDocs = readRepo("apps/next-frontend/docs/README.md");
const nextFrontendPlan = readRepo("apps/next-frontend/docs/implementation-plan.md");
const nextFrontendBlogEntrypoint = readRepo("apps/next-frontend/packages/rustok-blog/src/index.tsx");
const nextFrontendBlogPosts = readRepo("apps/next-frontend/packages/rustok-blog/src/api/posts.ts");
const nextFrontendProductPackage = readRepo("apps/next-frontend/packages/rustok-product/src/index.ts");
const nextFrontendSearchPackage = readRepo("apps/next-frontend/packages/search/src/index.tsx");
const nextFrontendModulesIndex = readRepo("apps/next-frontend/src/modules/index.ts");
const nextFrontendRegistry = readRepo("apps/next-frontend/src/modules/registry.ts");

assertContains(
  uiReadme,
  "## FFA Status for Leptos Hosts",
  "docs/UI/README.md: must explicitly document Leptos host FFA status",
);
assertContains(
  uiReadme,
  "Leptos hosts only",
  "docs/UI/README.md: must scope FFA host status to Leptos hosts",
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
]) {
  assertContains(
    text,
    "FFA-compatible composition host",
    `${label}: must use the shared frontend-host FFA classification`,
  );
}

for (const [label, text] of [
  ["apps/next-admin README.md", nextAdminReadme],
  ["apps/next-admin docs/README.md", nextAdminDocs],
  ["apps/next-admin docs/implementation-plan.md", nextAdminPlan],
]) {
  assertContains(
    text,
    "packages/*",
    `${label}: Next admin documentation must describe package-owned module surfaces`,
  );
  assertNotContains(
    text,
    "legacy import paths",
    `${label}: Next admin documentation must not allow legacy import paths`,
  );
  assertNotContains(
    text,
    "temporary compatibility",
    `${label}: Next admin documentation must not describe temporary compatibility layers for package imports`,
  );
}

for (const [relativePath, source] of nextAdminPackageEntrypoints) {
  assertNotContains(
    source,
    "../../../src/features",
    `${relativePath}: package entrypoint must not re-export host src/features`,
  );
  assertNotContains(
    source,
    /from ['"]\.\.\/\.\.\/\.\.\/src\//,
    `${relativePath}: package entrypoint must use host aliases instead of relative src imports`,
  );
}

assertMissing(
  "apps/next-admin/src/features/modules/api.ts",
  "apps/next-admin/src/features/modules/api.ts: module management GraphQL contract belongs in src/shared/api/modules.ts",
);
assertMissing(
  "apps/next-admin/src/features/oauth-apps/api.ts",
  "apps/next-admin/src/features/oauth-apps/api.ts: OAuth app GraphQL contract belongs in src/shared/api/oauth-apps.ts",
);
assertContains(
  nextAdminSharedApiIndex,
  "export * from './modules';",
  "apps/next-admin/src/shared/api/index.ts: module management API must be exported from shared API",
);
assertContains(
  nextAdminSharedApiIndex,
  "export * from './oauth-apps';",
  "apps/next-admin/src/shared/api/index.ts: OAuth app API must be exported from shared API",
);

for (const packageImport of [
  "../../packages/blog/src",
  "../../packages/cache/src",
  "../../packages/commerce/src",
  "../../packages/email/src",
  "../../packages/events/src",
  "../../packages/rbac/src",
  "../../packages/rustok-product/src",
  "../../packages/workflow/src",
]) {
  assertContains(
    nextAdminModulesIndex,
    packageImport,
    `apps/next-admin/src/modules/index.ts: missing package registry import ${packageImport}`,
  );
}

for (const [label, text] of [
  ["apps/next-frontend docs/README.md", nextFrontendDocs],
  ["apps/next-frontend docs/implementation-plan.md", nextFrontendPlan],
]) {
  assertContains(
    text,
    "packages/rustok-blog",
    `${label}: must describe the blog-owned Next storefront package`,
  );
}

assertContains(
  nextFrontendModulesIndex,
  '../../packages/rustok-blog/src',
  "apps/next-frontend/src/modules/index.ts: must register the blog-owned package",
);
assertContains(
  nextFrontendBlogEntrypoint,
  'from "@/modules/registry"',
  "apps/next-frontend/packages/rustok-blog/src/index.tsx: must use the host registry alias",
);
assertNotContains(
  nextFrontendBlogEntrypoint,
  "src/features",
  "apps/next-frontend/packages/rustok-blog/src/index.tsx: package must not re-export a host feature",
);
assertContains(
  nextFrontendBlogPosts,
  'import type { storefrontGraphql } from "@/shared/lib/graphql"',
  "apps/next-frontend/packages/rustok-blog/src/api/posts.ts: blog transport must consume the shared GraphQL executor contract",
);
assertContains(
  nextFrontendRegistry,
  "graphql: typeof storefrontGraphql;",
  "apps/next-frontend/src/modules/registry.ts: host must pass the shared GraphQL executor to package surfaces",
);

for (const [relativePath, source] of [
  ["apps/next-frontend/packages/rustok-blog/src/api/posts.ts", nextFrontendBlogPosts],
  ["apps/next-frontend/packages/rustok-product/src/index.ts", nextFrontendProductPackage],
  ["apps/next-frontend/packages/search/src/index.tsx", nextFrontendSearchPackage],
]) {
  assertContains(
    source,
    'import type { storefrontGraphql } from "@/shared/lib/graphql"',
    `${relativePath}: package must consume the shared GraphQL executor contract`,
  );
  assertNotContains(
    source,
    "StorefrontGraphqlOptions",
    `${relativePath}: package must not duplicate the shared GraphQL options contract`,
  );
  assertNotContains(
    source,
    "StorefrontGraphqlResponse",
    `${relativePath}: package must not duplicate the shared GraphQL response contract`,
  );
}

for (const [relativePath, description] of [
  [
    "apps/next-frontend/src/features/blog/index.tsx",
    "apps/next-frontend/src/features/blog/index.tsx: blog UI belongs in packages/rustok-blog",
  ],
  [
    "apps/next-frontend/src/features/blog/api/posts.ts",
    "apps/next-frontend/src/features/blog/api/posts.ts: blog transport belongs in packages/rustok-blog",
  ],
  [
    "apps/next-frontend/src/lib/graphql.ts",
    "apps/next-frontend/src/lib/graphql.ts: duplicate host GraphQL client must stay removed",
  ],
]) {
  assertMissing(relativePath, description);
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
assertContains(storefrontEnabledModulesAdapter, "storefront/list-enabled-modules", "storefront enabled-modules adapter must own its server endpoint");
assertContains(storefrontCanonicalRouteAdapter, "storefront/resolve-canonical-route", "storefront canonical-route adapter must own its server endpoint");

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

assertContains(adminCacheMod, "pub mod model;", "apps/admin/src/features/cache/mod.rs: cache host feature must wire a model");
assertContains(adminCacheMod, "pub mod transport;", "apps/admin/src/features/cache/mod.rs: cache host feature must wire a transport facade");
for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]"]) {
  assertNotContains(adminCacheModel, marker, `apps/admin/src/features/cache/model.rs: cache model must stay framework/server-function free (${marker})`);
}
assertContains(adminCacheTransport, "UiTransportPath::NativeServer", "apps/admin/src/features/cache/transport/mod.rs: cache transport must select the native path");
assertContains(adminCacheTransport, "UiTransportPath::Graphql", "apps/admin/src/features/cache/transport/mod.rs: cache transport must keep the GraphQL path");
assertNotContains(adminCacheTransport, "#[server", "apps/admin/src/features/cache/transport/mod.rs: server functions belong in native_server_adapter.rs");
assertContains(adminCacheNativeAdapter, "#[server", "apps/admin/src/features/cache/transport/native_server_adapter.rs: native adapter must own server-function endpoints");
assertContains(adminCachePage, "transport::fetch_cache_health", "apps/admin/src/pages/cache.rs: cache page must use the transport facade");
for (const marker of ["CACHE_HEALTH_QUERY", "crate::shared::api::{request", "native_server_adapter::cache_health_native"]) {
  assertNotContains(adminCachePage, marker, `apps/admin/src/pages/cache.rs: cache page must not own raw transport (${marker})`);
}

assertContains(adminDashboardMod, "pub mod model;", "apps/admin/src/features/dashboard/mod.rs: dashboard host feature must wire a model");
assertContains(adminDashboardMod, "pub mod transport;", "apps/admin/src/features/dashboard/mod.rs: dashboard host feature must wire a transport facade");
for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]"]) {
  assertNotContains(adminDashboardModel, marker, `apps/admin/src/features/dashboard/model.rs: dashboard model must stay framework/server-function free (${marker})`);
}
assertContains(adminDashboardTransport, "UiTransportPath::NativeServer", "apps/admin/src/features/dashboard/transport/mod.rs: dashboard transport must select the native path");
assertContains(adminDashboardTransport, "UiTransportPath::Graphql", "apps/admin/src/features/dashboard/transport/mod.rs: dashboard transport must keep the GraphQL path");
assertContains(adminDashboardNativeAdapter, "#[server", "apps/admin/src/features/dashboard/transport/native_server_adapter.rs: native adapter must own server-function endpoints");
for (const marker of ["transport::fetch_dashboard_stats", "transport::fetch_recent_activity"]) {
  assertContains(adminDashboardPage, marker, `apps/admin/src/pages/dashboard.rs: dashboard page must use ${marker}`);
}
for (const marker of ["DASHBOARD_STATS_QUERY", "RECENT_ACTIVITY_QUERY", "load_period_count_snapshot", "load_order_stats_snapshot", "load_recent_activity"]) {
  assertNotContains(adminDashboardPage, marker, `apps/admin/src/pages/dashboard.rs: dashboard page must not own raw transport/runtime helpers (${marker})`);
}

assertContains(adminEmailMod, "pub mod model;", "apps/admin/src/features/email/mod.rs: email host feature must wire a model");
assertContains(adminEmailMod, "pub mod transport;", "apps/admin/src/features/email/mod.rs: email host feature must wire a transport facade");
for (const marker of ["leptos::", "leptos_", "#[component]", "#[server]"]) {
  assertNotContains(adminEmailModel, marker, `apps/admin/src/features/email/model.rs: email model must stay framework/server-function free (${marker})`);
}
assertContains(adminEmailTransport, "UiTransportPath::NativeServer", "apps/admin/src/features/email/transport/mod.rs: email transport must select the native read path");
assertContains(adminEmailTransport, "UiTransportPath::Graphql", "apps/admin/src/features/email/transport/mod.rs: email transport must keep the GraphQL read path");
assertNotContains(adminEmailTransport, "#[server", "apps/admin/src/features/email/transport/mod.rs: server functions belong in native_server_adapter.rs");
assertContains(adminEmailNativeAdapter, "#[server", "apps/admin/src/features/email/transport/native_server_adapter.rs: native adapter must own server-function endpoints");
for (const marker of ["transport::fetch_email_settings", "transport::update_email_settings"]) {
  assertContains(adminEmailPage, marker, `apps/admin/src/pages/email_settings.rs: email page must use ${marker}`);
}
for (const marker of ["PLATFORM_SETTINGS_QUERY", "UPDATE_PLATFORM_SETTINGS_MUTATION", "native_server_adapter::email_settings_native"]) {
  assertNotContains(adminEmailPage, marker, `apps/admin/src/pages/email_settings.rs: email page must not own raw transport (${marker})`);
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
