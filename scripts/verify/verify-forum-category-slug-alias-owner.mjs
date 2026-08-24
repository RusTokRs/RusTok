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
  retiredAlias: "crates/rustok-forum/src/services/category_route_alias.rs",
  owner: "crates/rustok-forum/src/services/category_projection_owner.rs",
  forumSync: "crates/rustok-forum/src/services/category_taxonomy_sync.rs",
  taxonomySync: "crates/rustok-taxonomy/src/owner_category_route_sync.rs",
  migration:
    "crates/rustok-forum/src/migrations/m20260806_000026_add_forum_category_route_aliases.rs",
  migrationsMod: "crates/rustok-forum/src/migrations/mod.rs",
  contractTest: "crates/rustok-forum/tests/category_slug_alias_contract.rs",
  ownedAliasTest:
    "crates/rustok-forum/tests/category_taxonomy_owned_alias_history.rs",
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
  Object.entries(paths)
    .filter(([key]) => key !== "retiredAlias")
    .map(([key, value]) => [key, read(value)]),
);

if (existsSync(path.join(repoRoot, paths.retiredAlias))) {
  failures.push(`${paths.retiredAlias}: legacy runtime alias helper must be retired`);
}

// Keep the historical migration available for installations upgrading through
// the old Forum-owned route schema. Runtime ownership is checked separately.
for (const marker of [
  "CREATE TABLE IF NOT EXISTS forum_category_route_aliases",
  "UNIQUE (tenant_id, locale, slug)",
  "FOREIGN KEY (tenant_id, category_id)",
]) {
  requireText(source.migration, marker, paths.migration);
}
requireText(
  source.migrationsMod,
  "m20260806_000026_add_forum_category_route_aliases",
  paths.migrationsMod,
);

for (const marker of [
  "TaxonomyOwnerCategoryReader",
  "resolve_term_route_for_module(",
  "pub alias_id: Option<Uuid>",
  "forum_category_taxonomy_binding",
  "ensure_active_category",
]) {
  requireText(source.route, marker, paths.route);
}
for (const marker of [
  "category_route_alias.rs",
  "forum_category_route_aliases",
  "load_alias_route_candidates",
]) {
  forbidText(source.route, marker, paths.route);
}

requireText(
  source.forumSync,
  "sync_module_category_with_owned_aliases_in_tx(",
  paths.forumSync,
);
requireText(source.forumSync, "aliases: Vec::new()", paths.forumSync);
for (const marker of [
  "forum_category_route_aliases",
  "load_aliases_for_locale_in_tx",
]) {
  forbidText(source.forumSync, marker, paths.forumSync);
}
for (const marker of [
  "record_slug_rename_alias_in_tx(",
  "prepare_slug_rename_in_tx(",
  "ensure_current_route_key_available_in_tx(",
  "FORUM_CATEGORY_RENAMED_ROUTE_REASON",
]) {
  forbidText(source.owner, marker, paths.owner);
}

for (const marker of [
  "taxonomy_term_alias::Entity::find()",
  "taxonomy_term_translation::Entity::find()",
  "aliases.extend(std::mem::take(&mut input.aliases))",
  "if previous_slug != next_slug",
  "aliases.insert(previous_slug)",
  "sync_module_category_in_tx(txn, tenant_id, input).await",
]) {
  requireText(source.taxonomySync, marker, paths.taxonomySync);
}

for (const marker of [
  "forum_route_reads_are_taxonomy_owned",
  "forum_category_writes_delegate_alias_history_to_taxonomy",
  "taxonomy_route_sync_preserves_and_extends_append_only_history",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}
for (const marker of [
  "DROP TABLE forum_category_route_aliases",
  'slug: Some("help".to_string())',
  'slug: Some("assistance".to_string())',
  'resolve(tenant_id, "en", "support", None)',
  'resolve(tenant_id, "en", "help", None)',
]) {
  requireText(source.ownedAliasTest, marker, paths.ownedAliasTest);
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
  forbidText(source.forumSync, marker, paths.forumSync);
  forbidText(source.taxonomySync, marker, paths.taxonomySync);
}

if (failures.length > 0) {
  console.error("forum category slug alias owner verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum category slug alias owner verification passed");
