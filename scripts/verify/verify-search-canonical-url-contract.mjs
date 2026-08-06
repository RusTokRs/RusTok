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
const forumProjectionPath = "crates/rustok-forum/src/search_projection.rs";
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
const forumProjection = read(forumProjectionPath);
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
  'const FORUM_SOURCE_MODULE: &str = "forum"',
  'const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category"',
  'const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic"',
  'const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply"',
  "canonical_forum_projected_result_url(value)",
  'value.payload.get("route")',
  "canonical_forum_category_route",
  "canonical_forum_topic_route",
  "rustok_api::normalize_locale_tag",
  "forum_topic_short_identity",
  "valid_forum_slug",
  "canonical_url_accepts_owner_projected_forum_category_topic_and_reply_routes",
  "canonical_url_rejects_stale_or_malformed_forum_route_projections",
]) {
  requireMarker(engine, marker, enginePath);
}
for (const marker of [
  'const FORUM_STOREFRONT_ROUTE: &str = "/modules/forum"',
  'format!("{FORUM_STOREFRONT_ROUTE}?category=',
  'format!("{FORUM_STOREFRONT_ROUTE}?topic=',
  "canonical_forum_reply_result_url",
]) {
  rejectMarker(engine, marker, enginePath);
}

for (const marker of [
  "ForumCategoryRouteService",
  "ForumTopicRouteService",
  "exact_category_route",
  "exact_topic_route",
  ".canonical_descriptor(",
  '"route": route',
  'format!("{topic_route}?reply={reply_id}")',
  "category.effective_locale != locale",
  "topic.effective_locale != locale",
]) {
  requireMarker(forumProjection, marker, forumProjectionPath);
}
for (const marker of [
  '"/modules/forum?category=',
  '"/modules/forum?topic=',
]) {
  rejectMarker(forumProjection, marker, forumProjectionPath);
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
    'const FORUM_REPLY_ENTITY_TYPE',
    '"/modules/blog"',
    '"/modules/forum"',
  ]) {
    rejectMarker(source, forbidden, sourcePath);
  }
}

for (const marker of [
  '("forum_category", "forum" | "rustok-forum")',
  "Permission::FORUM_CATEGORIES_READ",
  '("forum_topic", "forum" | "rustok-forum")',
  "Permission::FORUM_TOPICS_READ",
  '("forum_reply", "forum" | "rustok-forum")',
  "Permission::FORUM_REPLIES_READ",
  'required_admin_search_permission("forum_reply", "content")',
]) {
  requireMarker(adminShell, marker, adminShellPath);
}

requireMarker(
  adminNativeRoot,
  'include!("native_server_adapter/mapping.rs")',
  adminNativeRootPath,
);
for (const marker of ["mod navigation", "enrich_search_result_urls", "blog_result_url"]) {
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
    forum_projection_owner: forumProjectionPath,
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
    "forum_projection_owner_routes",
    "forum_category_topic_routes",
    "forum_reply_canonical_route",
    "forum_reply_fail_closed",
    "forum_stale_projection_fail_closed",
    "product_and_content_routes",
    "content_kind_injection",
    "graphql_owner_projection",
    "storefront_native_owner_projection",
    "admin_native_owner_projection",
    "admin_shell_owner_projection",
    "admin_forum_permission_gate",
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
