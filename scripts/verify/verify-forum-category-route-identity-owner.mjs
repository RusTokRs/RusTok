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
  owner: "crates/rustok-forum/src/services/category_route.rs",
  servicesMod: "crates/rustok-forum/src/services/mod.rs",
  error: "crates/rustok-forum/src/error.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260328_000001_create_forum_tables.rs",
  decision: "DECISIONS/2026-03-29-forum-slug-locale-contract.md",
  contract:
    "crates/rustok-forum/contracts/forum-category-route-identity-owner.json",
  contractTest: "crates/rustok-forum/tests/category_route_identity_contract.rs",
  sqliteTest: "crates/rustok-forum/tests/category_route_identity_sqlite.rs",
  docs: "crates/rustok-forum/docs/forum-24l-category-route-identity-owner.md",
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
  "pub struct ForumCategoryRouteService",
  "pub async fn canonical_descriptor(",
  "pub async fn resolve(",
  "format!(\"/{locale}/forum/c/{slug}\")",
  "resolve_by_locale_with_fallback(",
  "Some(PLATFORM_FALLBACK_LOCALE)",
  "MAX_FORUM_CATEGORY_ROUTE_CANDIDATES: u64 = 64",
  ".limit(MAX_FORUM_CATEGORY_ROUTE_CANDIDATES + 1)",
  "forum_category_lifecycle::Entity::find()",
  "if category_ids.len() != 1",
  "ForumCategoryRouteDisposition::Canonical",
  "ForumCategoryRouteDisposition::Redirect",
]) {
  requireText(source.owner, marker, paths.owner);
}

for (const marker of [
  "mod category_route;",
  "ForumCategoryRouteDescriptor",
  "ForumCategoryRouteDisposition",
  "ForumCategoryRouteResolution",
  "ForumCategoryRouteService",
]) {
  requireText(source.servicesMod, marker, paths.servicesMod);
}

for (const marker of [
  "CategoryRouteNotFound",
  "FORUM_CATEGORY_ROUTE_NOT_FOUND",
  "CategoryRouteResolutionConflict",
  "FORUM_CATEGORY_ROUTE_RESOLUTION_CONFLICT",
]) {
  requireText(source.error, marker, paths.error);
}

for (const marker of [
  "idx_forum_category_translations_tenant_locale_slug",
  ".col(ForumCategoryTranslations::TenantId)",
  ".col(ForumCategoryTranslations::Locale)",
  ".col(ForumCategoryTranslations::Slug)",
  ".unique()",
]) {
  requireText(source.migration, marker, paths.migration);
}

for (const marker of [
  "Category slug is a locale-aware translation field",
  "same locale fallback contract",
]) {
  requireText(source.decision, marker, paths.decision);
}

for (const marker of [
  "localized_routes_follow_exact_and_shared_fallback_precedence",
  "exact_archived_route_does_not_fall_through_to_another_locale",
  "first_available_reverse_lookup_fails_closed_across_category_identities",
]) {
  requireText(source.sqliteTest, marker, paths.sqliteTest);
}

for (const marker of [
  "owner_uses_locale_aware_category_slug_and_existing_unique_route_key",
  "resolver_is_bounded_lifecycle_safe_and_fail_closed_on_ambiguity",
  "owner_is_exported_without_transport_storage_or_visibility_policy",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "`/{locale}/forum/c/{slug}`",
  "exact requested locale and slug belongs to an archived category",
  "Route identity is not storefront authorization",
  "No tests, verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}

for (const marker of [
  "async_graphql",
  "axum::",
  "#[server",
  "ForumCategoryAudienceVisibilityService",
  "ChannelService",
  "require_module_enabled",
  "forum_category_route_aliases",
  "INSERT INTO",
  "UPDATE ",
  "DELETE FROM",
  "StatusCode::",
]) {
  forbidText(source.owner, marker, paths.owner);
}

if (contract) {
  if (contract.task !== "FORUM-24L") {
    failures.push(`${paths.contract}: task must be FORUM-24L`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.owner !== "ForumCategoryRouteService") {
    failures.push(`${paths.contract}: unexpected owner`);
  }
  if (contract.route?.canonical_shape !== "/{locale}/forum/c/{slug}") {
    failures.push(`${paths.contract}: unexpected canonical route`);
  }
  if (contract.persistence?.new_migration !== false) {
    failures.push(`${paths.contract}: new migration must remain false`);
  }
  if (contract.resolution?.maximum_slug_candidates !== 64) {
    failures.push(`${paths.contract}: candidate bound must remain 64`);
  }
  if (contract.resolution?.exact_archived_route_falls_through !== false) {
    failures.push(`${paths.contract}: archived exact route must not fall through`);
  }
  if (contract.resolution?.first_available_requires_single_category_identity !== true) {
    failures.push(`${paths.contract}: residual lookup must require one identity`);
  }
  if (contract.authorization?.owner_authorizes_storefront_disclosure !== false) {
    failures.push(`${paths.contract}: route owner must not authorize disclosure`);
  }
  if (contract.compatibility?.storefront_route_mounted !== false) {
    failures.push(`${paths.contract}: host mount must remain out of scope`);
  }
  if (contract.compatibility?.seo_or_hreflang_changed !== false) {
    failures.push(`${paths.contract}: SEO and hreflang must remain out of scope`);
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

if (failures.length > 0) {
  console.error("forum category route identity owner verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category route identity owner verification passed");
