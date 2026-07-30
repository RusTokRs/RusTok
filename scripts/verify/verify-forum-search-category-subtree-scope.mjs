#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const servicePath = "crates/rustok-forum/src/services/category_search_scope.rs";
const exportPath = "crates/rustok-forum/src/services/mod.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-category-subtree-scope.json";
const exactContractPath = "crates/rustok-forum/contracts/forum-search-exact-category-filter.json";
const notePath = "crates/rustok-forum/docs/forum-23b2-category-subtree-scope.md";
const queryPath = "crates/rustok-search/src/engine.rs";
const verifierPath = "scripts/verify/verify-forum-search-category-subtree-scope.mjs";

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function requireAll(source, markers, label) {
  for (const marker of markers) requireMarker(source, marker, label);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const plan = read(planPath);
const service = read(servicePath);
const exports = read(exportPath);
const note = read(notePath);
const query = read(queryPath);
const contract = parseJson(contractPath);
const exactContract = parseJson(exactContractPath);

requireAll(
  service,
  [
    "pub const MAX_FORUM_SEARCH_CATEGORY_ROOTS: usize = 10",
    "pub struct ForumSearchCategoryScope",
    "pub struct ForumSearchCategoryScopeService",
    "pub async fn expand_visible_subtrees",
    "enforce_scope(&security, Resource::ForumCategories, Action::List)?",
    ".limit(MAX_FORUM_CATEGORY_TREE_NODES + 1)",
    "forum_category_lifecycle::Entity::find()",
    "hidden_category_ids_for_viewer",
    "normalize_requested_category_ids(category_ids)?",
    "category_ids.len() > MAX_FORUM_SEARCH_CATEGORY_ROOTS",
    "has_excluded_ancestor",
    "ForumError::CategoryNotFound(*category_id)",
    "append_visible_preorder",
    "overlapping_roots_expand_once_in_deterministic_preorder",
    "excluded_branch_is_pruned_with_all_descendants",
    "selected_descendant_of_excluded_ancestor_is_non_oracular",
    "raw_root_bound_is_checked_before_deduplication",
    "hierarchy_cycle_fails_closed",
  ],
  servicePath,
);
rejectMarker(service, "rustok_search", servicePath);
rejectMarker(service, "SearchQuery", servicePath);
rejectMarker(service, "search_documents", servicePath);

requireAll(
  exports,
  [
    "mod category_search_scope;",
    "ForumSearchCategoryScope, ForumSearchCategoryScopeService, MAX_FORUM_SEARCH_CATEGORY_ROOTS",
  ],
  exportPath,
);

requireAll(
  query,
  [
    "pub struct SearchQuery",
    "pub category_ids: Vec<Uuid>",
  ],
  queryPath,
);

requireAll(
  note,
  [
    "FORUM-23B2A",
    "source_complete_execution_pending",
    "requires `forum_categories:list`",
    "at most ten raw selected roots before deduplication",
    "canonical 512-node, depth-16 tenant category tree",
    "Search does not query Forum tables",
    "does not yet compose expansion into a public Search transport",
    "Complete role, trust, Channel, Groups and explicit-user audience composition remains open",
    "Not run by the implementation agent",
  ],
  notePath,
);

requireAll(
  plan,
  [
    "| `FORUM-23` | `in_progress` |",
    "**Status:** `in_progress`",
    "### Delivered in `FORUM-23B2A`",
    "ForumSearchCategoryScopeService",
    "verify-forum-search-category-subtree-scope.mjs",
    "cargo test -p rustok-forum category_search_scope -- --nocapture",
  ],
  planPath,
);

if (exactContract?.task !== "FORUM-23B1") {
  failures.push(`${exactContractPath}: exact-category predecessor drift`);
}

if (contract) {
  if (contract.task !== "FORUM-23B2A") failures.push(`${contractPath}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }

  const expectedPaths = {
    canonical_plan: planPath,
    owner_service: servicePath,
    owner_export: exportPath,
    exact_filter_contract: exactContractPath,
    search_query_contract: queryPath,
    owner_note: notePath,
    verifier: verifierPath,
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${contractPath}: ${key} drift`);
  }

  const expectedBounds = {
    raw_root_ids: 10,
    tenant_tree_nodes: 512,
    tree_depth: 16,
    expanded_ids: 512,
  };
  for (const [key, expected] of Object.entries(expectedBounds)) {
    if (contract.bounds?.[key] !== expected) failures.push(`${contractPath}: bound ${key} drift`);
  }

  for (const key of [
    "requires_forum_categories_list",
    "uses_existing_inherited_public_authenticated_visibility",
    "excludes_archived_categories",
    "missing_foreign_hidden_or_archived_root_is_not_found",
    "hidden_or_archived_branch_is_pruned",
    "excluded_ancestor_blocks_selected_descendant",
  ]) {
    if (contract.authorization?.[key] !== true) {
      failures.push(`${contractPath}: authorization ${key} drift`);
    }
  }

  for (const key of [
    "raw_root_bound_is_checked_before_deduplication",
    "requested_roots_preserve_first_occurrence_order",
    "children_follow_owner_position_then_id_order",
    "overlapping_roots_are_deduplicated",
    "expanded_output_is_deterministic_preorder",
  ]) {
    if (contract.ordering?.[key] !== true) {
      failures.push(`${contractPath}: ordering ${key} drift`);
    }
  }

  for (const key of [
    "returns_ids_for_search_query_category_ids",
  ]) {
    if (contract.integration_boundary?.[key] !== true) {
      failures.push(`${contractPath}: integration ${key} drift`);
    }
  }
  for (const key of [
    "forum_imports_search",
    "search_imports_forum",
    "search_reads_forum_tree_at_query_time",
    "host_search_composition_complete",
  ]) {
    if (contract.integration_boundary?.[key] !== false) {
      failures.push(`${contractPath}: integration ${key} must remain false`);
    }
  }

  for (const key of [
    "graphql_schema_changed",
    "search_query_shape_changed",
    "forum_projection_shape_changed",
    "database_migration_added",
    "dependency_added",
    "cargo_lock_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${contractPath}: compatibility ${key} must remain false`);
    }
  }
  if (contract.compatibility?.exact_category_filter_preserved !== true) {
    failures.push(`${contractPath}: exact category compatibility drift`);
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${contractPath}: execution status drift`);
  }
}

if (failures.length > 0) {
  console.error("Forum Search category subtree scope verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search category subtree scope verification passed.");
