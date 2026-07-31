#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const paths = {
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
  contract: "crates/rustok-forum/contracts/forum-search-storefront-scope.json",
  note: "crates/rustok-forum/docs/forum-23b2c-storefront-search-scope.md",
  scopePort: "crates/rustok-search/src/storefront_category_scope.rs",
  execution: "crates/rustok-search/src/forum_storefront_execution.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  graphqlOwner: "crates/rustok-search/src/graphql/forum_storefront.rs",
  graphqlMod: "crates/rustok-search/src/graphql/mod.rs",
  serverAdapter: "apps/server/src/services/forum_search_category_scope.rs",
  serverComposition: "apps/server/src/services/mod.rs",
  serverSchema: "apps/server/src/graphql/schema.rs",
  storefrontSelector: "crates/rustok-search/storefront/src/transport/mod.rs",
  nativeAdapter:
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  graphqlAdapter:
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
  existingGraphql: "crates/rustok-search/src/graphql/query.rs",
  existingNative:
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
  verifier: "scripts/verify/verify-forum-search-storefront-scope.mjs",
};

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

const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);
const contract = parseJson(paths.contract);
const note = read(paths.note);
const scopePort = read(paths.scopePort);
const execution = read(paths.execution);
const searchLib = read(paths.searchLib);
const graphqlOwner = read(paths.graphqlOwner);
const graphqlMod = read(paths.graphqlMod);
const serverAdapter = read(paths.serverAdapter);
const serverComposition = read(paths.serverComposition);
const serverSchema = read(paths.serverSchema);
const storefrontSelector = read(paths.storefrontSelector);
const nativeAdapter = read(paths.nativeAdapter);
const graphqlAdapter = read(paths.graphqlAdapter);
const existingGraphql = read(paths.existingGraphql);
const existingNative = read(paths.existingNative);

requireAll(
  scopePort,
  [
    "pub trait StorefrontSearchCategoryScopePort",
    "pub type SharedStorefrontSearchCategoryScopePort",
    "pub async fn resolve_storefront_search_category_ids",
    "request.category_ids.is_empty() || !request.is_explicit_forum_only()",
    "forum.search_category_scope.owner_unavailable",
    "explicit_forum_only_scope_requires_owner_port",
    "mixed_scope_preserves_exact_categories_without_owner_call",
  ],
  paths.scopePort,
);
rejectAll(
  scopePort,
  ["rustok_forum", "ForumSearchCategoryAudienceScopeService", "forum_category::Entity"],
  paths.scopePort,
);

requireAll(
  execution,
  [
    "pub async fn execute_forum_storefront_search",
    "Forum storefront Search requires source_modules: [forum]",
    "Forum storefront Search requires at least one category_id",
    "resolve_storefront_search_category_ids",
    "published_only: true",
    "SearchDictionaryService::transform_query",
    "SearchFilterPresetService::resolve",
    "SearchRankingProfile::resolve",
    "SearchDictionaryService::apply_query_rules",
    "SearchAnalyticsService::record_query",
    "category_ids,",
  ],
  paths.execution,
);
rejectAll(
  execution,
  ["rustok_forum", "ForumSearchCategoryAudienceScopeService", "forum_category"],
  paths.execution,
);

requireAll(
  serverAdapter,
  [
    "impl StorefrontSearchCategoryScopePort for ServerForumSearchCategoryScopePort",
    "is_tenant_module_enabled",
    "Permission::FORUM_CATEGORIES_LIST",
    "category_read_audience_port_context",
    "ForumSearchCategoryAudienceScopeService",
    "expand_authenticated_visible_subtrees",
    "expand_public_visible_subtrees",
    "Forum category expansion requires an explicit Forum-only source scope",
    "Forum category scope is unavailable",
  ],
  paths.serverAdapter,
);
requireAll(
  serverComposition,
  [
    'include!("forum_search_category_scope.rs")',
    "ServerForumSearchCategoryScopePort::shared",
    "extensions.insert(category_scope)",
    "SharedStorefrontSearchCategoryScopePort",
  ],
  paths.serverComposition,
);

requireAll(
  graphqlOwner,
  [
    "pub struct ForumStorefrontSearchQuery",
    "async fn forum_storefront_search",
    "require_module_enabled(ctx, FORUM_MODULE_SLUG)",
    "enforce_rate_limit(ctx)",
    "SharedStorefrontSearchCategoryScopePort",
    "execute_forum_storefront_search",
    "tenantId does not match the authenticated request tenant",
    "StorefrontSearchTransport::Graphql",
  ],
  paths.graphqlOwner,
);
requireAll(
  graphqlMod,
  ["mod forum_storefront;", "pub use forum_storefront::ForumStorefrontSearchQuery;"],
  paths.graphqlMod,
);
requireAll(
  serverSchema,
  [
    "use rustok_search::graphql::ForumStorefrontSearchQuery;",
    "ForumStorefrontSearchQuery,",
  ],
  paths.serverSchema,
);

requireAll(
  nativeAdapter,
  [
    'endpoint = "search/forum-storefront-search"',
    "SharedStorefrontSearchCategoryScopePort",
    "execute_forum_storefront_search",
    "StorefrontSearchTransport::NativeServer",
    "OptionalAuthContext",
    "RequestContext",
  ],
  paths.nativeAdapter,
);
requireAll(
  graphqlAdapter,
  [
    "query ForumStorefrontSearch",
    "forumStorefrontSearch(input: $input)",
    "source_modules: filters.source_modules",
    "category_ids: filters.category_ids",
  ],
  paths.graphqlAdapter,
);
requireAll(
  storefrontSelector,
  [
    "fn is_explicit_forum_category_scope",
    "!filters.category_ids.is_empty()",
    "filters.source_modules.len() == 1",
    'eq_ignore_ascii_case("forum")',
    "forum_native_server_adapter::fetch_search",
    "forum_graphql_adapter::fetch_search",
    "native_server_adapter::fetch_search",
    "graphql_adapter::fetch_search",
    "only_explicit_forum_category_scope_selects_owner_path",
  ],
  paths.storefrontSelector,
);

requireAll(
  existingGraphql,
  ["async fn storefront_search", "STOREFRONT_SEARCH_SURFACE"],
  paths.existingGraphql,
);
requireAll(
  existingNative,
  [
    'endpoint = "search/storefront-search"',
    "async fn storefront_search_native",
  ],
  paths.existingNative,
);
rejectAll(
  existingGraphql,
  ["ForumSearchCategoryAudienceScopeService", "expand_authenticated_visible_subtrees"],
  paths.existingGraphql,
);
rejectAll(
  existingNative,
  ["ForumSearchCategoryAudienceScopeService", "expand_authenticated_visible_subtrees"],
  paths.existingNative,
);

requireAll(
  searchLib,
  [
    "pub mod forum_storefront_execution;",
    "pub mod storefront_category_scope;",
    "execute_forum_storefront_search",
    "SharedStorefrontSearchCategoryScopePort",
  ],
  paths.searchLib,
);

requireAll(
  note,
  [
    "FORUM-23B2C",
    "forumStorefrontSearch",
    "search/forum-storefront-search",
    "exactly one normalized source module: `forum`",
    "Missing owner composition",
    "Product category filtering remains exact",
    "Not run by the implementation agent",
  ],
  paths.note,
);
requireAll(
  forumPlan,
  [
    "## `FORUM-23` — search/index integration",
    "FORUM-23B2C",
    "forumStorefrontSearch",
    "topic-local audience narrowing",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "Forum-only storefront Search",
    "StorefrontSearchCategoryScopePort",
    "forumStorefrontSearch",
    "source_complete_execution_pending",
  ],
  paths.searchPlan,
);

if (contract) {
  if (contract.task !== "FORUM-23B2C") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  const expectedPaths = {
    canonical_forum_plan: paths.forumPlan,
    canonical_search_plan: paths.searchPlan,
    neutral_search_port: paths.scopePort,
    search_execution_owner: paths.execution,
    host_adapter: paths.serverAdapter,
    host_composition: paths.serverComposition,
    graphql_owner: paths.graphqlOwner,
    graphql_host_mount: paths.serverSchema,
    native_adapter: paths.nativeAdapter,
    graphql_adapter: paths.graphqlAdapter,
    storefront_selector: paths.storefrontSelector,
    owner_note: paths.note,
    verifier: paths.verifier,
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${paths.contract}: ${key} drift`);
  }
  for (const key of [
    "tenant_is_derived_from_request_context",
    "tenant_override_is_rejected",
    "tenant_forum_module_must_be_enabled",
    "richer_role_trust_channel_group_allow_deny_rules_are_reused",
    "missing_required_owner_facts_fail_closed",
    "missing_scope_port_fails_closed",
  ]) {
    if (contract.authorization?.[key] !== true) {
      failures.push(`${paths.contract}: authorization ${key} drift`);
    }
  }
  for (const key of [
    "graphql_and_native_share_one_search_execution_owner",
    "published_only",
    "expanded_ids_use_existing_search_category_ids",
    "server_is_only_cross_owner_adapter",
  ]) {
    if (contract.execution?.[key] !== true) {
      failures.push(`${paths.contract}: execution ${key} drift`);
    }
  }
  for (const key of [
    "existing_storefront_search_field_changed",
    "existing_storefront_native_endpoint_changed",
    "search_query_shape_changed",
    "forum_projection_shape_changed",
    "database_migration_added",
    "dependency_added",
    "cargo_lock_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${paths.contract}: compatibility ${key} must remain false`);
    }
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${paths.contract}: execution status drift`);
  }
}

if (failures.length > 0) {
  console.error("Forum storefront Search scope verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum storefront Search scope verification passed.");
