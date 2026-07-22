#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function read(relativePath) {
  const target = repoPath(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const enginePath = "crates/rustok-search/src/engine.rs";
const libPath = "crates/rustok-search/src/lib.rs";
const graphqlPath = "crates/rustok-search/src/graphql/types.rs";
const storefrontNativePath =
  "crates/rustok-search/storefront/src/transport/native_server_adapter.rs";
const storefrontFacadePath = "crates/rustok-search/storefront/src/transport/mod.rs";
const adminNativeRootPath = "crates/rustok-search/admin/src/transport/native_server_adapter.rs";
const adminNativeMappingPath =
  "crates/rustok-search/admin/src/transport/native_server_adapter/mapping.rs";
const adminShellPath = "apps/admin/src/widgets/app_shell/native_server_adapter.rs";
const removedCompatibilityPath =
  "crates/rustok-search/storefront/src/transport/navigation.rs";
const evidencePath = "crates/rustok-search/contracts/evidence/search-canonical-url-contract.json";
const planPath = "crates/rustok-search/docs/implementation-plan.md";

const engine = read(enginePath);
const lib = read(libPath);
const graphql = read(graphqlPath);
const storefrontNative = read(storefrontNativePath);
const storefrontFacade = read(storefrontFacadePath);
const adminNativeRoot = read(adminNativeRootPath);
const adminNativeMapping = read(adminNativeMappingPath);
const adminShell = read(adminShellPath);
const plan = read(planPath);
let evidence = null;
try {
  evidence = JSON.parse(read(evidencePath));
} catch (error) {
  failures.push(`${evidencePath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "pub fn canonical_search_result_url",
  'const BLOG_SOURCE_MODULE: &str = "blog"',
  'const BLOG_ENTITY_TYPE: &str = "blog_post"',
  'const BLOG_STOREFRONT_ROUTE: &str = "/modules/blog"',
  "value.source_module == BLOG_SOURCE_MODULE",
  'payload.get("slug")',
  "MAX_BLOG_SLUG_LEN",
  "ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')",
  "content_kind_query",
]) {
  requireMarker(engine, marker, enginePath);
}

requireMarker(lib, "canonical_search_result_url", libPath);

for (const [source, sourcePath, marker] of [
  [graphql, graphqlPath, "crate::canonical_search_result_url(&value)"],
  [storefrontNative, storefrontNativePath, "rustok_search::canonical_search_result_url(&value)"],
  [adminNativeMapping, adminNativeMappingPath, "rustok_search::canonical_search_result_url(&item)"],
  [adminShell, adminShellPath, "rustok_search::canonical_search_result_url(&item)"],
]) {
  requireMarker(source, marker, sourcePath);
  for (const forbidden of [
    "fn derive_search_result_url",
    "fn derive_admin_search_result_url",
    'const BLOG_STOREFRONT_ROUTE',
    '"/modules/blog"',
  ]) {
    rejectMarker(source, forbidden, sourcePath);
  }
}

requireMarker(
  adminNativeRoot,
  'include!("native_server_adapter/mapping.rs")',
  adminNativeRootPath,
);
for (const marker of ["mod navigation", "enrich_search_result_urls", "blog_result_url"] ) {
  rejectMarker(storefrontFacade, marker, storefrontFacadePath);
}
if (existsSync(repoPath(removedCompatibilityPath))) {
  failures.push(`${removedCompatibilityPath}: compatibility implementation must be deleted`);
}

if (evidence) {
  if (evidence.schema_version !== 1) failures.push(`${evidencePath}: schema_version must be 1`);
  if (evidence.module !== "search" || evidence.surface !== "canonical_result_url") {
    failures.push(`${evidencePath}: module/surface identity drift`);
  }
  if (evidence.status !== "source_verified_no_compile") {
    failures.push(`${evidencePath}: status drift`);
  }
  if (evidence.compile_policy !== "not_run_by_request") {
    failures.push(`${evidencePath}: compile policy drift`);
  }
  const contract = evidence.production_contract ?? {};
  for (const [key, expected] of Object.entries({
    normalized_result: enginePath,
    public_export: libPath,
    graphql_projection: graphqlPath,
    storefront_native_projection: storefrontNativePath,
    storefront_transport_facade: storefrontFacadePath,
    admin_native_root: adminNativeRootPath,
    admin_native_mapping: adminNativeMappingPath,
    admin_shell_projection: adminShellPath,
  })) {
    if (contract[key] !== expected) failures.push(`${evidencePath}: ${key} path drift`);
  }
  if ("compatibility_fallback" in contract) {
    failures.push(`${evidencePath}: compatibility_fallback must be removed`);
  }

  const cases = new Set((evidence.cases ?? []).map((entry) => entry.name));
  for (const requiredCase of [
    "blog_canonical_route",
    "blog_fail_closed",
    "product_and_content_routes",
    "content_kind_injection",
    "graphql_owner_projection",
    "storefront_native_owner_projection",
    "admin_native_owner_projection",
    "admin_shell_owner_projection",
    "no_transport_fallback",
  ]) {
    if (!cases.has(requiredCase)) failures.push(`${evidencePath}: missing case ${requiredCase}`);
  }
}

for (const marker of [
  "search-canonical-url-contract.json",
  "canonical_search_result_url",
  "single owner policy",
  "no transport fallback",
]) {
  requireMarker(plan, marker, planPath);
}
for (const marker of ["compatibility fallback", "rolling compatibility", "admin native cutover"]) {
  rejectMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("search canonical URL contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("search canonical URL contract verification passed");
