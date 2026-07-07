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

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

const docs = [
  "docs/UI/README.md",
  "docs/verification/platform-frontend-surfaces-verification-plan.md",
  "apps/admin/src/widgets/app_shell/core.rs",
  "apps/admin/src/widgets/app_shell/sidebar.rs",
  "apps/storefront/src/widgets/header/core.rs",
  "apps/storefront/src/widgets/header/mod.rs",
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

if (failures.length > 0) {
  console.error("Frontend host FFA contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Frontend host FFA contract verification passed");