#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const servicePath = "crates/rustok-forum/src/services/category_search_audience_scope.rs";
const audienceReadPath = "crates/rustok-forum/src/services/category_audience_read.rs";
const audienceReadSearchPath =
  "crates/rustok-forum/src/services/category_audience_read_search.rs";
const visibilityPath = "crates/rustok-forum/src/services/category_audience_visibility.rs";
const baseScopePath = "crates/rustok-forum/src/services/category_search_scope.rs";
const visibleExpanderPath =
  "crates/rustok-forum/src/services/category_search_scope_visible.rs";
const exportPath = "crates/rustok-forum/src/services/mod.rs";
const contractPath =
  "crates/rustok-forum/contracts/forum-search-category-audience-scope.json";
const notePath = "crates/rustok-forum/docs/forum-23b2b-category-audience-scope.md";
const verifierPath = "scripts/verify/verify-forum-search-category-audience-scope.mjs";

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

function requireAll(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
  }
}

function rejectAll(source, markers, label) {
  for (const marker of markers) {
    if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
  }
}

const plan = read(planPath);
const service = read(servicePath);
const audienceRead = read(audienceReadPath);
const audienceReadSearch = read(audienceReadSearchPath);
const visibility = read(visibilityPath);
const baseScope = read(baseScopePath);
const visibleExpander = read(visibleExpanderPath);
const ownerExport = read(exportPath);
const note = read(notePath);
const contract = parseJson(contractPath);

requireAll(
  service,
  [
    "pub struct ForumSearchCategoryAudienceScopeService",
    "ForumCategoryAudienceReadService",
    "pub fn with_audience_facts",
    "pub async fn expand_public_visible_subtrees",
    "pub async fn expand_authenticated_visible_subtrees",
    "tree_public_storefront_visible_with_locale_fallback",
    "tree_authenticated_owner_visible_with_audience_context",
    "expand_search_category_scope_from_visible_tree",
    "CategoryTreeQuery",
  ],
  servicePath,
);
rejectAll(
  service,
  [
    "use rustok_search",
    "PgSearchEngine",
    "SearchQuery {",
    "search_documents",
    "ForumCategoryAudienceVisibilityService",
    "ForumCategoryAudienceViewer",
    "forum_category::Entity",
    "fn prune_category_nodes",
    "fn collect_category_ids",
  ],
  servicePath,
);

requireAll(
  audienceRead,
  [
    "pub struct ForumCategoryAudienceReadService",
    "tree_authenticated_owner_visible_with_audience_context",
    "ForumCategoryAudienceVisibilityService",
    ".is_category_visible(tenant_id, category_id, &viewer)",
    "prune_category_nodes",
    "category_tree_stats",
  ],
  audienceReadPath,
);
requireAll(
  audienceReadSearch,
  [
    "tree_public_storefront_visible_with_locale_fallback",
    "SecurityContext::public_read()",
    "Resource::ForumCategories, Action::List",
    "ForumCategoryAudienceViewer::public()",
    ".is_category_visible(tenant_id, category_id, &viewer)",
    "prune_category_nodes(tree.roots, &visible_ids)",
    "category_tree_stats(&tree.roots)",
  ],
  audienceReadSearchPath,
);
rejectAll(
  audienceReadSearch,
  ["use rustok_search", "SearchQuery {", "search_documents"],
  audienceReadSearchPath,
);

requireAll(
  visibility,
  [
    "pub struct ForumCategoryAudienceVisibilityService",
    "load_category_audience_policy",
    "resolve_for_constraints",
    "ForumAudienceEvaluator::decide",
  ],
  visibilityPath,
);

requireAll(
  baseScope,
  [
    "pub const MAX_FORUM_SEARCH_CATEGORY_ROOTS: usize = 10",
    "pub struct ForumSearchCategoryScope",
    "struct CategoryHierarchy",
    "fn normalize_requested_category_ids",
    "fn expand_visible_subtrees",
    "raw_root_bound_is_checked_before_deduplication",
  ],
  baseScopePath,
);
requireAll(
  visibleExpander,
  [
    "expand_search_category_scope_from_visible_tree",
    "normalize_requested_category_ids(category_ids)",
    "collect_active_visible_nodes",
    "if node.is_archived",
    "CategoryHierarchy::from_ordered_nodes",
    "hierarchy.expand_visible_subtrees",
    "HashSet::new()",
  ],
  visibleExpanderPath,
);
rejectAll(
  visibleExpander,
  [
    "use rustok_search",
    "ForumCategoryAudienceVisibilityService",
    "ForumCategoryAudienceViewer",
    "forum_category::Entity",
  ],
  visibleExpanderPath,
);

requireAll(
  ownerExport,
  [
    'include!("category_audience_read_search.rs")',
    "mod category_search_audience_scope;",
    'include!("category_search_scope_visible.rs")',
    'include!("category_search_scope.rs")',
    "pub use category_search_audience_scope::ForumSearchCategoryAudienceScopeService;",
  ],
  exportPath,
);

requireAll(
  plan,
  [
    "## `FORUM-23` — search/index integration",
    "FORUM-23B2B",
    "richer category audience",
    "Host composition remains open",
  ],
  planPath,
);
requireAll(
  note,
  [
    "FORUM-23B2B",
    "ForumCategoryAudienceReadService",
    "role",
    "trust",
    "Channel",
    "Groups",
    "explicit user allow/deny",
    "fails closed",
    "B2A `CategoryHierarchy`",
    "Forum-only Search entrypoint",
    "Not run by the implementation agent",
  ],
  notePath,
);

if (contract) {
  if (contract.task !== "FORUM-23B2B") {
    failures.push(`${contractPath}: unexpected task`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }

  const expectedPaths = {
    canonical_plan: planPath,
    base_scope_owner: baseScopePath,
    visible_tree_expander: visibleExpanderPath,
    audience_scope_owner: servicePath,
    audience_read_owner: audienceReadPath,
    audience_read_search_extension: audienceReadSearchPath,
    audience_visibility_owner: visibilityPath,
    owner_export: exportPath,
    owner_note: notePath,
    verifier: verifierPath,
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${contractPath}: ${key} drift`);
  }

  for (const key of [
    "requires_forum_categories_list",
    "public_viewer_supported",
    "authenticated_viewer_requires_exact_port_context",
    "reuses_canonical_category_audience_read_owner",
    "reuses_public_authenticated_floor",
    "reuses_inherited_richer_category_layers",
    "supports_role_selector",
    "supports_trust_selector",
    "supports_channel_selector",
    "supports_group_selector",
    "supports_explicit_allow_deny",
    "missing_required_owner_facts_fail_closed",
    "archived_category_is_excluded",
    "denied_ancestor_prunes_descendants",
    "denied_selected_root_is_not_found",
  ]) {
    if (contract.authorization?.[key] !== true) {
      failures.push(`${contractPath}: authorization ${key} drift`);
    }
  }

  for (const key of [
    "raw_root_bound_is_checked_before_deduplication",
    "requested_roots_preserve_first_occurrence_order",
    "visible_children_preserve_canonical_tree_order",
    "overlapping_roots_are_deduplicated",
    "expanded_output_is_deterministic_preorder",
    "shared_b2a_hierarchy_expander_is_reused",
  ]) {
    if (contract.ordering?.[key] !== true) {
      failures.push(`${contractPath}: ordering ${key} drift`);
    }
  }

  for (const key of [
    "graphql_schema_changed",
    "rest_schema_changed",
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
  if (contract.compatibility?.b2a_base_scope_preserved !== true) {
    failures.push(`${contractPath}: B2A base scope must remain preserved`);
  }
  if (contract.integration_boundary?.host_composition_complete !== false) {
    failures.push(`${contractPath}: host composition must remain an explicit non-claim`);
  }
  if (contract.integration_boundary?.search_execution_is_added !== false) {
    failures.push(`${contractPath}: Search execution must remain absent`);
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${contractPath}: execution status drift`);
  }
}

if (failures.length > 0) {
  console.error("Forum richer Search category audience scope verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum richer Search category audience scope verification passed.");
