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
  contract: "crates/rustok-forum/contracts/forum-search-result-eligibility.json",
  note: "crates/rustok-forum/docs/forum-23b2d-search-result-eligibility.md",
  searchPort: "crates/rustok-search/src/storefront_result_eligibility.rs",
  searchExecution: "crates/rustok-search/src/forum_storefront_execution.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  forumOwner: "crates/rustok-forum/src/services/search_result_eligibility.rs",
  forumServices: "crates/rustok-forum/src/services/mod.rs",
  forumLib: "crates/rustok-forum/src/lib.rs",
  forumVisibility: "crates/rustok-forum/src/services/topic_audience_visibility.rs",
  forumContext: "crates/rustok-forum/src/category_read_transport.rs",
  serverAdapter: "apps/server/src/services/forum_search_result_eligibility.rs",
  serverComposition: "apps/server/src/services/mod.rs",
  graphqlOwner: "crates/rustok-search/src/graphql/forum_storefront.rs",
  nativeAdapter:
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  storefrontSelector: "crates/rustok-search/storefront/src/transport/mod.rs",
  existingGraphql: "crates/rustok-search/src/graphql/query.rs",
  existingNative:
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
  verifier: "scripts/verify/verify-forum-search-result-eligibility.mjs",
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
const searchPort = read(paths.searchPort);
const searchExecution = read(paths.searchExecution);
const searchLib = read(paths.searchLib);
const forumOwner = read(paths.forumOwner);
const forumServices = read(paths.forumServices);
const forumLib = read(paths.forumLib);
const forumVisibility = read(paths.forumVisibility);
const forumContext = read(paths.forumContext);
const serverAdapter = read(paths.serverAdapter);
const serverComposition = read(paths.serverComposition);
const graphqlOwner = read(paths.graphqlOwner);
const nativeAdapter = read(paths.nativeAdapter);
const storefrontSelector = read(paths.storefrontSelector);
const existingGraphql = read(paths.existingGraphql);
const existingNative = read(paths.existingNative);

requireAll(
  searchPort,
  [
    "pub const MAX_FORUM_SEARCH_RESULT_CANDIDATES: usize = 100",
    "pub enum StorefrontSearchResultCandidateKind",
    "ForumTopic",
    "ForumReply",
    "pub trait StorefrontSearchResultEligibilityPort",
    "pub type SharedStorefrontSearchResultEligibilityPort",
    "pub async fn resolve_storefront_search_result_candidates",
    "forum.search_result_eligibility.owner_unavailable",
    "owner_scope_invalid",
    "non_empty_scope_requires_owner_port",
  ],
  paths.searchPort,
);
rejectAll(
  searchPort,
  [
    "rustok_forum",
    "ForumTopicAudienceVisibilityService",
    "forum_reply::Entity",
    "forum_topic::Entity",
  ],
  paths.searchPort,
);

requireAll(
  forumOwner,
  [
    "pub const MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES: usize = 100",
    "pub struct ForumSearchResultEligibilityService",
    "filter_public_storefront_visible",
    "filter_authenticated_storefront_visible",
    "ForumTopicAudienceVisibilityService",
    "is_topic_visible",
    "forum_reply::Column::Status.eq(ReplyStatus::Approved)",
    "reply_topics.get(&candidate.document_id)",
    "visible_topics.contains",
    "seen.insert(*candidate)",
  ],
  paths.forumOwner,
);
rejectAll(
  forumOwner,
  ["rustok_search", "search_documents", "SearchResultItem", "SearchQuery"],
  paths.forumOwner,
);
requireAll(
  forumVisibility,
  [
    "pub async fn is_topic_visible",
    "ForumTopicVisibilityScope::storefront_for_viewer",
    "self.policy_allows",
    "inherited_category_layers",
    "configured_constraints",
  ],
  paths.forumVisibility,
);
requireAll(
  forumContext,
  [
    "SearchResultEligibility",
    'Self::SearchResultEligibility => "search-result-eligibility"',
    "with_deadline(FORUM_CATEGORY_READ_FACTS_DEADLINE)",
    "with_claim(permission.to_string())",
    "with_channel(channel_slug.to_string())",
  ],
  paths.forumContext,
);
requireAll(
  forumServices,
  [
    "mod search_result_eligibility;",
    "ForumSearchResultEligibilityService",
    "MAX_FORUM_SEARCH_RESULT_ELIGIBILITY_CANDIDATES",
  ],
  paths.forumServices,
);
requireAll(
  forumLib,
  [
    "ForumSearchResultCandidate",
    "ForumSearchResultCandidateKind",
    "ForumSearchResultEligibilityService",
  ],
  paths.forumLib,
);

requireAll(
  searchExecution,
  [
    "SharedStorefrontSearchResultEligibilityPort",
    "execute_result_eligible_search",
    "FORUM_RESULT_SCAN_PAGE_SIZE: usize = 50",
    "MAX_FORUM_SEARCH_RESULT_CANDIDATES",
    "narrow the query or category scope",
    "candidate snapshot changed during bounded eligibility evaluation",
    "resolve_storefront_search_result_candidates",
    '"forum_category" => true',
    '"forum_topic" | "forum_reply"',
    ".skip(query.offset)",
    ".take(query.limit)",
    "build_forum_result_facets(&visible_items)",
    "SearchDictionaryService::apply_query_rules",
  ],
  paths.searchExecution,
);
rejectAll(
  searchExecution,
  [
    "rustok_forum",
    "ForumTopicAudienceVisibilityService",
    "forum_reply::Entity",
    "forum_topic::Entity",
  ],
  paths.searchExecution,
);
requireAll(
  searchLib,
  [
    "pub mod storefront_result_eligibility;",
    "SharedStorefrontSearchResultEligibilityPort",
    "resolve_storefront_search_result_candidates",
  ],
  paths.searchLib,
);

requireAll(
  serverAdapter,
  [
    "impl StorefrontSearchResultEligibilityPort for ServerForumSearchResultEligibilityPort",
    "ForumSearchResultEligibilityService",
    "ForumCategoryReadOperation::SearchResultEligibility",
    "Permission::FORUM_CATEGORIES_LIST",
    "filter_authenticated_storefront_visible",
    "filter_public_storefront_visible",
    "is_tenant_module_enabled",
    "Forum Search result eligibility is unavailable",
  ],
  paths.serverAdapter,
);
requireAll(
  serverComposition,
  [
    'include!("forum_search_result_eligibility.rs")',
    "ServerForumSearchResultEligibilityPort::shared",
    "extensions.insert(result_eligibility)",
    "SharedStorefrontSearchResultEligibilityPort",
  ],
  paths.serverComposition,
);

requireAll(
  graphqlOwner,
  [
    "SharedStorefrontSearchResultEligibilityPort",
    "result_eligibility_port",
    "execute_forum_storefront_search(",
    "StorefrontSearchTransport::Graphql",
    "forum_category_scope_result_eligibility_then_fts",
  ],
  paths.graphqlOwner,
);
requireAll(
  nativeAdapter,
  [
    "SharedStorefrontSearchResultEligibilityPort",
    "result_eligibility_port",
    "execute_forum_storefront_search(",
    "StorefrontSearchTransport::NativeServer",
  ],
  paths.nativeAdapter,
);
requireAll(
  storefrontSelector,
  [
    "fn is_explicit_forum_category_scope",
    "forum_native_server_adapter::fetch_search",
    "forum_graphql_adapter::fetch_search",
    "native_server_adapter::fetch_search",
    "graphql_adapter::fetch_search",
  ],
  paths.storefrontSelector,
);
rejectAll(
  existingGraphql,
  [
    "SharedStorefrontSearchResultEligibilityPort",
    "ForumSearchResultEligibilityService",
    "filter_forum_result_candidates",
  ],
  paths.existingGraphql,
);
rejectAll(
  existingNative,
  [
    "SharedStorefrontSearchResultEligibilityPort",
    "ForumSearchResultEligibilityService",
    "filter_forum_result_candidates",
  ],
  paths.existingNative,
);

requireAll(
  note,
  [
    "FORUM-23B2D",
    "StorefrontSearchResultEligibilityPort",
    "ForumSearchResultEligibilityService",
    "before visible",
    "currently `approved`",
    "capped at 100",
    "No command above was run",
  ],
  paths.note,
);
requireAll(
  forumPlan,
  [
    "## `FORUM-23` — search/index integration",
    "FORUM-23B2D",
    "result eligibility",
    "trusted channel authority",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "StorefrontSearchResultEligibilityPort",
    "FORUM-23B2D",
    "100",
    "source_complete_execution_pending",
  ],
  paths.searchPlan,
);

if (contract) {
  if (contract.task !== "FORUM-23B2D") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  const expectedPaths = {
    canonical_forum_plan: paths.forumPlan,
    canonical_search_plan: paths.searchPlan,
    forum_owner: paths.forumOwner,
    topic_visibility_owner: paths.forumVisibility,
    neutral_search_port: paths.searchPort,
    search_execution_owner: paths.searchExecution,
    host_adapter: paths.serverAdapter,
    host_composition: paths.serverComposition,
    graphql_transport: paths.graphqlOwner,
    native_transport: paths.nativeAdapter,
    owner_note: paths.note,
    verifier: paths.verifier,
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${paths.contract}: ${key} drift`);
  }
  if (contract.bounds?.maximum_raw_result_rows !== 100) {
    failures.push(`${paths.contract}: raw result bound drift`);
  }
  if (contract.bounds?.maximum_owner_candidates !== 100) {
    failures.push(`${paths.contract}: owner candidate bound drift`);
  }
  if (contract.bounds?.raw_search_page_size !== 50) {
    failures.push(`${paths.contract}: raw page-size drift`);
  }
  for (const key of [
    "topic_reuses_exact_storefront_topic_audience_visibility",
    "topic_must_be_currently_open",
    "topic_route_channel_is_rechecked",
    "topic_local_audience_narrowing_is_rechecked",
    "reply_must_be_currently_approved",
    "reply_inherits_parent_topic_decision",
    "missing_or_denied_targets_are_omitted_non_oracularly",
    "missing_required_external_facts_fail_closed",
  ]) {
    if (contract.owner_decision?.[key] !== true) {
      failures.push(`${paths.contract}: owner_decision ${key} drift`);
    }
  }
  for (const key of [
    "eligibility_is_applied_before_visible_offset_and_limit",
    "visible_total_is_computed_after_eligibility",
    "visible_facets_are_computed_after_eligibility",
    "raw_ranking_order_is_preserved_for_allowed_rows",
    "query_rules_run_only_after_eligibility",
  ]) {
    if (contract.search_semantics?.[key] !== true) {
      failures.push(`${paths.contract}: search_semantics ${key} drift`);
    }
  }
  for (const key of [
    "mixed_product_blog_content_paths_changed",
    "projection_shape_changed",
    "migration_required",
    "dependency_added",
    "cargo_lock_changed",
  ]) {
    if (contract.search_semantics?.[key] !== false) {
      failures.push(`${paths.contract}: search_semantics ${key} must remain false`);
    }
  }
}

if (failures.length > 0) {
  console.error("Forum Search result eligibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search result eligibility verification passed.");
