#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  core: "crates/rustok-forum/storefront/src/core.rs",
  packageLib: "crates/rustok-forum/storefront/src/lib.rs",
  transport: "crates/rustok-forum/storefront/src/transport/mod.rs",
  host: "apps/storefront/src/forum_category_route.rs",
  hostLib: "apps/storefront/src/lib.rs",
  contract:
    "crates/rustok-forum/contracts/forum-category-route-storefront-mount.json",
  contractTest:
    "crates/rustok-forum/tests/category_route_storefront_mount_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24o-category-route-storefront-mount.md",
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const source = Object.fromEntries(
  Object.entries(paths).map(([key, value]) => [key, read(value)]),
);

let contract = null;
try {
  contract = JSON.parse(source.contract);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}

for (const marker of [
  "pub fn category_href(locale: &str, slug: &str) -> Option<String>",
  "item.effective_locale.as_str()",
  "item.slug.as_str()",
  "Some(format!(\"/{locale}/forum/c/{slug}\"))",
  ".unwrap_or_else(|| module_route_base.to_string())",
]) {
  requireText(source.core, marker, paths.core);
}
forbidText(source.core, "?category={category_id}", paths.core);

for (const marker of [
  "StorefrontForumCategoryRouteDescriptor",
  "StorefrontForumCategoryRouteDisposition",
  "StorefrontForumCategoryRouteResolution",
  "resolve_storefront_category_route",
]) {
  requireText(source.packageLib, marker, paths.packageLib);
}

for (const marker of [
  "pub async fn resolve_storefront_category_route(",
  "if use_native_transport()",
  "resolve_storefront_category_route_server",
  "resolve_storefront_category_route_graphql",
]) {
  requireText(source.transport, marker, paths.transport);
}

for (const marker of [
  "rustok_forum_storefront::resolve_storefront_category_route(",
  "ForumCategoryHostAction::Render",
  "ForumCategoryHostAction::Redirect",
  "ForumCategoryHostAction::Invalid",
  "StatusCode::NOT_FOUND",
  "StatusCode::SERVICE_UNAVAILABLE",
  "private_permanent_redirect(location.as_str())",
  "query_params.insert(\"category\".to_string(), category_id)",
  "query_params.remove(\"topic\")",
  "fn safe_owner_path(path: &str) -> bool",
  "path.starts_with('/')",
  "!path.starts_with(\"//\")",
  "!path.chars().any(char::is_control)",
]) {
  requireText(source.host, marker, paths.host);
}
for (const marker of [
  "ForumCategoryRouteService",
  "ForumCategoryAudienceReadService",
  "forum_category_route_aliases",
  "Statement::from_sql_and_values",
  "SELECT ",
  "StatusCode::GONE",
  "fetch_seo_page_context",
  "hreflang",
  "schema.org",
]) {
  forbidText(source.host, marker, paths.host);
}

for (const marker of [
  "mod forum_category_route;",
  "\"/{locale}/forum/c/{slug}\"",
  "forum_category_route::render_forum_category_route_response(",
  "original_uri: axum::extract::OriginalUri",
  "original_uri.0.path().to_string()",
  "const PRIVATE_NO_STORE: &str = \"private, no-store\"",
]) {
  requireText(source.hostLib, marker, paths.hostLib);
}

for (const marker of [
  "category_cards_emit_canonical_locale_slug_routes",
  "rust_storefront_mount_executes_transport_decision_without_storage_access",
  "mount_contract_keeps_private_fail_closed_http_policy",
  "topic_mount_and_seo_boundaries_remain_outside_this_slice",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "private `308 Permanent Redirect`",
  "private `404 Not Found`",
  "private `503 Service Unavailable`",
  "There is no category `GONE` decision",
  "protocol-relative paths",
  "No tests, verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24O") {
    failures.push(`${paths.contract}: task must be FORUM-24O`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.route?.mounted_in_rust_storefront !== true) {
    failures.push(`${paths.contract}: Rust storefront mount is required`);
  }
  if (contract.route?.redirect_or_noncanonical_raw_path_status !== 308) {
    failures.push(`${paths.contract}: redirects must use 308`);
  }
  if (contract.route?.gone_supported !== false) {
    failures.push(`${paths.contract}: category gone must remain unsupported`);
  }
  if (contract.host_composition?.host_reads_forum_storage !== false) {
    failures.push(`${paths.contract}: host must not read Forum storage`);
  }
  if (contract.host_composition?.host_reauthorizes_category !== false) {
    failures.push(`${paths.contract}: host must not reauthorize categories`);
  }
  if (contract.host_composition?.owner_target_must_be_local_absolute_path !== true) {
    failures.push(`${paths.contract}: owner target must remain same-origin local`);
  }
  if (
    contract.host_composition?.protocol_relative_or_control_character_target_rejected !==
    true
  ) {
    failures.push(`${paths.contract}: unsafe Location targets must fail closed`);
  }
  if (contract.http?.redirect_cache_control !== "private, no-store") {
    failures.push(`${paths.contract}: redirects must be private no-store`);
  }
  if (contract.http?.not_found_status !== 404) {
    failures.push(`${paths.contract}: hidden and missing routes must remain 404`);
  }
  if (contract.http?.transport_or_malformed_status !== 503) {
    failures.push(`${paths.contract}: unsafe failures must remain 503`);
  }
  if (contract.category_links?.uuid_query_links_emitted !== false) {
    failures.push(`${paths.contract}: category cards must not emit UUID query links`);
  }
  if (contract.category_links?.legacy_module_query_route_retained !== true) {
    failures.push(`${paths.contract}: generic module compatibility route must remain`);
  }
  if (contract.transport?.automatic_fallback !== false) {
    failures.push(`${paths.contract}: transport failure fallback must remain disabled`);
  }
  if (contract.compatibility?.topic_route_changed !== false) {
    failures.push(`${paths.contract}: topic route must remain unchanged`);
  }
  if (contract.compatibility?.seo_or_hreflang_changed !== false) {
    failures.push(`${paths.contract}: SEO and hreflang must remain unchanged`);
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

if (failures.length > 0) {
  console.error("forum category route storefront mount verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category route storefront mount verification passed");
