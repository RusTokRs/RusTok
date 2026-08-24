#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-category-owner-read-visibility.json";
const contract = JSON.parse(read(contractPath) || "{}");
const visibilityOwner = read(contract.category_visibility_owner_file ?? "");
const categoryFacade = read(contract.category_facade_file ?? "");
const categorySelector = read(contract.category_selector_file ?? "");
const categoryTreeFilter = read(contract.category_tree_filter_file ?? "");
const services = read(contract.services_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("category owner visibility contract must use schema_version=1");
}
if (contract.task !== "FORUM-20E") {
  failures.push("category owner visibility contract must belong to FORUM-20E");
}
if (contract.canonical_plan_sync !== "included") {
  failures.push("FORUM-20E must be synchronized into the canonical plan");
}
if (contract.category_tree_bound !== 512 || contract.category_depth_bound !== 16) {
  failures.push("category owner visibility bounds must remain 512 nodes and depth 16");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted category-read evidence");
}
for (const residual of [
  "role visibility",
  "trust-level visibility",
  "channel membership visibility",
  "group membership visibility",
  "explicit allow and deny",
  "create reply and moderate audience policy",
  "remaining non-category-topic-reply read composition",
  "search notification SEO and deep-link migration to the owner scope",
  "visibility-scoped category and all-read mutations",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`category owner visibility contract must keep ${residual} open`);
  }
}

for (const marker of [
  "pub(crate) async fn hidden_category_ids_for_viewer(",
  "pub(crate) async fn is_category_visible_to_viewer(",
  "forum_category::Column::TenantId.eq(tenant_id)",
  "CategoryVisibilitySnapshot::load(&self.db, tenant_id)",
  "Err(ForumError::CategoryNotFound(_)) => Ok(false)",
  "ForumCategoryVisibility::Public",
]) {
  requireText(visibilityOwner, marker, `category visibility owner is missing ${marker}`);
}
for (const forbidden of [
  "rustok_profiles",
  "rustok_channels",
  "rustok_groups",
  "forum_category::Column::Metadata",
]) {
  rejectText(
    visibilityOwner,
    forbidden,
    `category visibility owner must not depend on premature policy input ${forbidden}`,
  );
}

const exactRead = between(
  categoryFacade,
  "pub async fn get_with_locale_fallback(",
  "/// Archive the complete category subtree",
  "category exact owner read",
);
for (const marker of [
  "enforce_scope(&security, Resource::ForumCategories, Action::Read)?",
  ".is_category_visible_to_viewer(",
  "!security.is_public_read()",
  "return Err(ForumError::CategoryNotFound(category_id))",
  ".get_with_locale_fallback(tenant_id, security, category_id, locale, fallback_locale)",
]) {
  requireText(exactRead, marker, `category exact owner read is missing ${marker}`);
}
const exactAuthIndex = exactRead.indexOf("enforce_scope(");
const exactVisibilityIndex = exactRead.indexOf(".is_category_visible_to_viewer(");
const exactHydrationIndex = exactRead.indexOf(".get_with_locale_fallback(");
if (
  exactAuthIndex < 0 ||
  exactVisibilityIndex < 0 ||
  exactHydrationIndex < 0 ||
  exactAuthIndex > exactVisibilityIndex ||
  exactVisibilityIndex > exactHydrationIndex
) {
  failures.push("category exact read must authorize, evaluate visibility, then hydrate");
}

const pageRead = between(
  categoryFacade,
  "pub async fn list_paginated_with_locale_fallback(",
  "pub(crate) async fn find_category_in_tx(",
  "category paginated owner read",
);
for (const marker of [
  "enforce_scope(&security, Resource::ForumCategories, Action::List)?",
  ".hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())",
  ".list_paginated_with_locale_fallback_and_hidden_categories(",
]) {
  requireText(pageRead, marker, `category paginated owner read is missing ${marker}`);
}

for (const marker of [
  "TaxonomyOwnerCategoryReader",
  "pub(in crate::services) async fn list_paginated_with_locale_fallback_and_hidden_categories(",
  "forum_category::Column::Id.is_not_in(hidden_category_ids.to_vec())",
  "let paginator = query",
  "let total = paginator.num_items().await?",
  "let categories = paginator.fetch_page",
]) {
  requireText(categorySelector, marker, `category selector is missing ${marker}`);
}
const categoryFilterIndex = categorySelector.indexOf(
  "forum_category::Column::Id.is_not_in(hidden_category_ids.to_vec())",
);
const categoryPaginatorIndex = categorySelector.indexOf("let paginator = query");
if (
  categoryFilterIndex < 0 ||
  categoryPaginatorIndex < 0 ||
  categoryFilterIndex > categoryPaginatorIndex
) {
  failures.push("category visibility must be filtered before count and pagination");
}

for (const marker of [
  "TaxonomyOwnerCategoryReader",
  "pub(super) async fn read_with_hidden_categories(",
  "let hidden = hidden_category_ids.iter().copied().collect::<HashSet<_>>()",
  "let (total_nodes, max_depth) = retain_visible_nodes(&mut roots, &hidden)",
  "nodes.retain(|node| !hidden.contains(&node.id))",
  "node.children_count = node.children.len() as u32",
  "node.has_children = !node.children.is_empty()",
]) {
  requireText(categoryTreeFilter, marker, `category tree visibility is missing ${marker}`);
}

requireText(
  services,
  'include!("category_visibility_list.rs");',
  "services composition must retain the Taxonomy read adapter include",
);
requireText(
  categoryFacade,
  '#[path = "category_taxonomy_tree_read.rs"]',
  "category facade must compose the Taxonomy tree reader",
);
for (const forbidden of [
  'include!("category_locale_enumeration.rs");',
  'include!("category_tree.rs");',
  'include!("category_tree_visibility.rs");',
]) {
  rejectText(services, forbidden, `services composition must retire ${forbidden}`);
}

for (const marker of [
  "inherited_authenticated_floor_guards_category_exact_page_and_tree_reads",
  "ForumCategoryVisibility::Authenticated",
  "SecurityContext::public_read()",
  "Err(ForumError::CategoryNotFound(id)) if id == restricted_child",
  "assert_eq!(public_total, 2)",
  "assert_eq!(authenticated_total, 4)",
  "assert_eq!(public_tree.total_nodes, 2)",
  "assert_eq!(public_tree.max_depth, 1)",
  "assert_eq!(public_tree.roots[0].children_count, 1)",
  "assert_eq!(authenticated_tree.total_nodes, 4)",
  "assert_eq!(authenticated_tree.max_depth, 2)",
]) {
  requireText(testSource, marker, `category owner SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "Delivered in `FORUM-20C`",
  "Delivered in `FORUM-20D`",
  "Delivered in `FORUM-20E`",
  "category_owner_visibility_sqlite",
  "verify-forum-category-owner-read-visibility.mjs",
]) {
  requireText(plan, marker, `canonical FORUM-20 plan is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum category owner read visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum category owner read visibility contract is source-ready.");
