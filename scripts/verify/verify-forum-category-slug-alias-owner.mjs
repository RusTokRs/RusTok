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
  route: "crates/rustok-forum/src/services/category_route.rs",
  alias: "crates/rustok-forum/src/services/category_route_alias.rs",
  owner: "crates/rustok-forum/src/services/category_projection_owner.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260806_000026_add_forum_category_route_aliases.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  contract:
    "crates/rustok-forum/contracts/forum-category-slug-alias-owner.json",
  contractTest: "crates/rustok-forum/tests/category_slug_alias_contract.rs",
  sqliteTest: "crates/rustok-forum/tests/category_slug_alias_sqlite.rs",
  docs: "crates/rustok-forum/docs/forum-24m-category-slug-alias-owner.md",
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

function occurrenceCount(content, marker) {
  return content.split(marker).length - 1;
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
  "CREATE TABLE IF NOT EXISTS forum_category_route_aliases",
  "UNIQUE (tenant_id, locale, slug)",
  "FOREIGN KEY (tenant_id, category_id)",
  "forum category route aliases are append-only",
  "forum category route is reserved by alias",
  "forum category route alias conflicts with current route",
  "forum_category_translation_route_alias_guard",
  "forum_category_route_alias_insert_guard",
]) {
  requireText(source.migration, marker, paths.migration);
}
requireText(
  source.migrationsMod,
  "m20260806_000026_add_forum_category_route_aliases",
  paths.migrationsMod,
);

for (const marker of [
  "include!(\"category_route_alias.rs\")",
  "pub alias_id: Option<Uuid>",
  "load_alias_route_candidates(db, tenant_id, slug)",
  "candidate.alias_id.is_none()",
  "alias_id: candidate.alias_id",
  "Exact-locale aliases therefore precede fallback-locale current",
  "exact_alias_precedes_fallback_current_route",
]) {
  requireText(source.route, marker, paths.route);
}

for (const marker of [
  "Historical route keys are never reusable",
  "ensure_current_route_key_available_in_tx(",
  "prepare_slug_rename_in_tx(",
  "record_slug_rename_alias_in_tx(",
  "pg_advisory_xact_lock",
  "keys.sort_unstable()",
  "ON CONFLICT (tenant_id, locale, slug) DO NOTHING",
  "MAX_FORUM_CATEGORY_ROUTE_ALIAS_REASON_LEN: usize = 500",
  "FORUM_CATEGORY_RENAMED_ROUTE_REASON",
]) {
  requireText(source.alias, marker, paths.alias);
}

for (const marker of [
  "let previous_slug = normalize_required_slug(&existing_translation.slug)?;",
  "Some(name) =>",
  "normalize_required_slug(name)?",
  "if slug_changed",
  "prepare_slug_rename_in_tx(",
  "record_slug_rename_alias_in_tx(",
  "publish_forum_projection_scope_direct_in_tx(",
]) {
  requireText(source.owner, marker, paths.owner);
}
if (occurrenceCount(source.owner, "ensure_current_route_key_available_in_tx(") !== 2) {
  failures.push(
    `${paths.owner}: create and new-translation paths must both reserve route keys`,
  );
}

for (const marker of [
  "explicit_and_name_derived_slug_changes_record_redirects_atomically",
  "historical_route_keys_cannot_be_reclaimed_inside_one_tenant",
  "archived_category_hides_current_and_historical_routes",
  "alias_rows_are_append_only_and_guard_direct_route_reuse",
]) {
  requireText(source.sqliteTest, marker, paths.sqliteTest);
}

for (const marker of [
  "migration_reserves_one_append_only_historical_route_namespace",
  "every_public_category_slug_write_path_uses_the_route_owner",
  "alias_owner_is_bounded_idempotent_and_never_reuses_history",
  "resolver_combines_current_and_alias_candidates_without_authorizing_visibility",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}

for (const marker of [
  "source-ready / maintainer execution pending",
  "Historical route keys are deliberately not reusable",
  "existing name-derived slug change",
  "exact-locale old route cannot be shadowed",
  "Alias ownership is not visibility authorization",
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
  "StatusCode::",
]) {
  forbidText(source.route, marker, paths.route);
  forbidText(source.alias, marker, paths.alias);
}

if (contract) {
  if (contract.task !== "FORUM-24M") {
    failures.push(`${paths.contract}: task must be FORUM-24M`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.aliases?.table !== "forum_category_route_aliases") {
    failures.push(`${paths.contract}: unexpected alias table`);
  }
  if (contract.aliases?.append_only !== true) {
    failures.push(`${paths.contract}: aliases must be append-only`);
  }
  if (contract.aliases?.historical_key_reusable_by_same_category !== false) {
    failures.push(`${paths.contract}: same-category route reuse must remain blocked`);
  }
  if (contract.aliases?.historical_key_reusable_by_other_category !== false) {
    failures.push(`${paths.contract}: cross-category route reuse must remain blocked`);
  }
  if (contract.resolution?.exact_alias_precedes_fallback_current_route !== true) {
    failures.push(`${paths.contract}: exact alias precedence is required`);
  }
  if (contract.authorization?.alias_owner_authorizes_storefront_disclosure !== false) {
    failures.push(`${paths.contract}: alias owner must not authorize disclosure`);
  }
  if (contract.compatibility?.storefront_route_mounted !== false) {
    failures.push(`${paths.contract}: category host mount must remain out of scope`);
  }
  if (contract.compatibility?.seo_or_hreflang_changed !== false) {
    failures.push(`${paths.contract}: SEO and hreflang must remain out of scope`);
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

if (failures.length > 0) {
  console.error("forum category slug alias owner verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category slug alias owner verification passed");
