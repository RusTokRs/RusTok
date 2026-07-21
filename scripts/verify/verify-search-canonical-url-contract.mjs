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
const adminNativePath = "crates/rustok-search/admin/src/transport/native_server_adapter.rs";
const compatibilityPath = "crates/rustok-search/storefront/src/transport/navigation.rs";
const evidencePath = "crates/rustok-search/contracts/evidence/search-canonical-url-contract.json";
const planPath = "crates/rustok-search/docs/implementation-plan.md";

const engine = read(enginePath);
const lib = read(libPath);
const graphql = read(graphqlPath);
const storefrontNative = read(storefrontNativePath);
const adminNative = read(adminNativePath);
const compatibility = read(compatibilityPath);
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
  "payload.get(\"slug\")",
  "MAX_BLOG_SLUG_LEN",
  "ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_')",
  "content_kind_query",
]) {
  requireMarker(engine, marker, enginePath);
}

requireMarker(lib, "canonical_search_result_url", libPath);
requireMarker(graphql, "crate::canonical_search_result_url(&value)", graphqlPath);
rejectMarker(graphql, "fn derive_search_result_url", graphqlPath);
requireMarker(
  storefrontNative,
  "rustok_search::canonical_search_result_url(&value)",
  storefrontNativePath,
);
rejectMarker(storefrontNative, "fn derive_search_result_url", storefrontNativePath);

for (const marker of [
  "item.url.is_some()",
  "item.url = blog_result_url",
  "preserves_backend_url_and_rejects_invalid_slug",
]) {
  requireMarker(compatibility, marker, compatibilityPath);
}

// Admin native still carries the final transport-local switch. Keep the debt
// visible until it is migrated, but do not let it expand into another Blog
// implementation.
requireMarker(adminNative, "fn derive_search_result_url", adminNativePath);
rejectMarker(adminNative, '"blog_post"', adminNativePath);
rejectMarker(adminNative, '"/modules/blog"', adminNativePath);

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
  if (evidence.production_contract?.normalized_result !== enginePath) {
    failures.push(`${evidencePath}: normalized result owner drift`);
  }
  if (evidence.production_contract?.graphql_projection !== graphqlPath) {
    failures.push(`${evidencePath}: GraphQL projection path drift`);
  }
  if (evidence.production_contract?.storefront_native_projection !== storefrontNativePath) {
    failures.push(`${evidencePath}: storefront native projection path drift`);
  }
  if (evidence.production_contract?.admin_native_projection !== adminNativePath) {
    failures.push(`${evidencePath}: admin native projection path drift`);
  }
  if (evidence.production_contract?.compatibility_fallback !== compatibilityPath) {
    failures.push(`${evidencePath}: compatibility fallback path drift`);
  }
}

for (const marker of [
  "search-canonical-url-contract.json",
  "canonical_search_result_url",
  "compatibility fallback",
  "admin native",
]) {
  requireMarker(plan, marker, planPath);
}

if (failures.length > 0) {
  console.error("search canonical URL contract verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("search canonical URL contract verification passed");
