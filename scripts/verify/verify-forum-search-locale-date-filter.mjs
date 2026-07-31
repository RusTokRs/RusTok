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
  contract: "crates/rustok-forum/contracts/forum-search-locale-date-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f3-search-locale-date-filter.md",
  projection: "crates/rustok-forum/src/search_projection.rs",
  filter: "crates/rustok-search/src/forum_storefront_locale_date_filters.rs",
  execution: "crates/rustok-search/src/forum_storefront_date_execution.rs",
  executionTypes:
    "crates/rustok-search/src/forum_storefront_date_execution/types_and_execute.rs",
  executionScan:
    "crates/rustok-search/src/forum_storefront_date_execution/result_scan.rs",
  executionNormalization:
    "crates/rustok-search/src/forum_storefront_date_execution/normalization.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  graphqlOwner: "crates/rustok-search/src/graphql/forum_storefront.rs",
  graphqlTypes: "crates/rustok-search/src/graphql/types.rs",
  storefrontModel: "crates/rustok-search/storefront/src/model.rs",
  graphqlAdapter:
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
  nativeAdapter:
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  transportFacade: "crates/rustok-search/storefront/src/transport/mod.rs",
  engine: "crates/rustok-search/src/engine.rs",
};

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
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
function parseJson(relativePath) {
  try {
    return JSON.parse(read(relativePath));
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return null;
  }
}

const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);
const contract = parseJson(paths.contract);
const note = read(paths.note);
const projection = read(paths.projection);
const filter = read(paths.filter);
const executionMain = read(paths.execution);
const execution = [
  read(paths.executionTypes),
  read(paths.executionScan),
  read(paths.executionNormalization),
].join("\n");
const searchLib = read(paths.searchLib);
const graphqlOwner = read(paths.graphqlOwner);
const graphqlTypes = read(paths.graphqlTypes);
const storefrontModel = read(paths.storefrontModel);
const graphqlAdapter = read(paths.graphqlAdapter);
const nativeAdapter = read(paths.nativeAdapter);
const transportFacade = read(paths.transportFacade);
const engine = read(paths.engine);

requireAll(projection, [
  '"published_at": created_at.to_rfc3339()',
  '"topic_tags": topic_tags',
  '"is_solution": is_solution',
], paths.projection);
if (projection.split('"published_at": created_at.to_rfc3339()').length - 1 !== 2) {
  failures.push(`${paths.projection}: topic and reply timestamp projections are required`);
}
requireAll(filter, [
  "pub exact_locale: String",
  "pub published_from: Option<DateTime<Utc>>",
  "pub published_to: Option<DateTime<Utc>>",
  "has_date_window",
  "DateTime::parse_from_rfc3339",
  "exact_locale_preserves_categories_without_date_window",
  "published_window_is_inclusive_and_excludes_categories",
  "malformed_or_missing_projection_fails_closed",
], paths.filter);
rejectAll(filter, ["rustok_forum", "forum_topic::", "forum_reply::"], paths.filter);
requireAll(executionMain, [
  'include!("forum_storefront_date_execution/types_and_execute.rs")',
  'include!("forum_storefront_date_execution/result_scan.rs")',
  'include!("forum_storefront_date_execution/normalization.rs")',
], paths.execution);
requireAll(execution, [
  "pub struct ForumStorefrontSearchDateWindowRequest",
  "pub async fn execute_forum_storefront_search_with_date_window",
  "locale: Some(effective_locale.clone())",
  "locale_date_filters.matches(item) && document_filters.matches(item)",
  "let raw_total =",
  "let candidates = all_items",
  "let total = visible_items.len() as u64",
  'normalize_optional_date_window_rfc3339("published_from"',
  'normalize_optional_date_window_rfc3339("published_to"',
  "published_from must not be after published_to",
], "B2F3 execution owner");
if (execution.indexOf("let raw_total =") > execution.indexOf("locale_date_filters.matches(item)")) {
  failures.push("B2F3 execution owner: raw bound must precede date narrowing");
}
requireAll(searchLib, [
  "pub mod forum_storefront_date_execution;",
  "pub mod forum_storefront_locale_date_filters;",
  "execute_forum_storefront_search_with_date_window",
], paths.searchLib);
requireAll(graphqlOwner, [
  "published_from: Option<String>",
  "published_to: Option<String>",
  "ForumStorefrontSearchDateWindowRequest",
  "execute_forum_storefront_search_with_date_window",
], paths.graphqlOwner);
if (graphqlOwner.includes("execute_forum_storefront_search(")) {
  failures.push(`${paths.graphqlOwner}: GraphQL Forum transport must use the exact-locale owner`);
}
requireAll(graphqlAdapter, [
  "ForumStorefrontSearchByDateWindow",
  "$publishedFrom: String",
  "$publishedTo: String",
  "fetch_search_with_date_window",
], paths.graphqlAdapter);
requireAll(nativeAdapter, [
  'endpoint = "search/forum-storefront-search-by-date-window"',
  "execute_forum_storefront_search_date_window_native",
  "ForumStorefrontSearchDateWindowRequest",
  "execute_forum_storefront_search_date_window_native(",
], paths.nativeAdapter);
requireAll(transportFacade, [
  "pub async fn fetch_forum_search_with_date_window",
  "forum_native_server_adapter::fetch_search_with_date_window",
  "forum_graphql_adapter::fetch_search_with_date_window",
], paths.transportFacade);
requireAll(graphqlAdapter, [
  "ForumStorefrontSearch($input: SearchPreviewInput!)",
  "ForumStorefrontSearchByAuthors",
  "ForumStorefrontSearchByFilters",
], `${paths.graphqlAdapter} legacy operations`);
requireAll(nativeAdapter, [
  'endpoint = "search/forum-storefront-search"',
  'endpoint = "search/forum-storefront-search-by-authors"',
  'endpoint = "search/forum-storefront-search-by-filters"',
], `${paths.nativeAdapter} legacy endpoints`);
rejectAll(graphqlTypes, ["published_from", "published_to", "publishedFrom", "publishedTo"], paths.graphqlTypes);
rejectAll(storefrontModel, ["published_from", "published_to", "publishedFrom", "publishedTo"], paths.storefrontModel);
rejectAll(engine, ["published_from", "published_to", "ForumStorefrontLocaleDateFilters"], paths.engine);
requireAll(forumPlan, ["FORUM-23B2F3", "locale", "date", "verify-forum-search-locale-date-filter.mjs"], paths.forumPlan);
requireAll(searchPlan, ["FORUM-23B2F3", "source_complete_execution_pending", "published date-window"], paths.searchPlan);
requireAll(note, [
  "# FORUM-23B2F3 exact Forum Search locale and date filters",
  "Locale-only execution preserves Forum categories",
  "does **not** add storefront UI controls",
], paths.note);

if (contract) {
  if (contract.task !== "FORUM-23B2F3") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") failures.push(`${paths.contract}: unexpected status`);
  if (contract.input?.date_format !== "RFC3339") failures.push(`${paths.contract}: date format drift`);
  if (!contract.evaluation?.raw_candidate_limit_is_checked_before_date_narrowing) failures.push(`${paths.contract}: raw ordering invariant missing`);
  if (contract.transport_compatibility?.existing_wire_signatures_changed !== false) failures.push(`${paths.contract}: legacy wire signatures changed`);
  if (contract.compatibility?.search_query_shape_changed !== false) failures.push(`${paths.contract}: neutral SearchQuery changed`);
}

if (failures.length > 0) {
  console.error("FORUM-23B2F3 Search locale/date verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("FORUM-23B2F3 Search locale/date source contract is consistent.");
