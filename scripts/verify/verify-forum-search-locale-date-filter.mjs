#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-locale-date-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f3-search-locale-date-filter.md",
  projection: "crates/rustok-forum/src/search_projection.rs",
  filters: "crates/rustok-search/src/forum_document_filters.rs",
  execution: "crates/rustok-search/src/forum_storefront_execution.rs",
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
const forbiddenParallelPaths = [
  "crates/rustok-search/src/forum_storefront_date_execution.rs",
  "crates/rustok-search/src/forum_storefront_date_execution/types_and_execute.rs",
  "crates/rustok-search/src/forum_storefront_date_execution/result_scan.rs",
  "crates/rustok-search/src/forum_storefront_date_execution/normalization.rs",
  "crates/rustok-search/src/forum_storefront_locale_date_filters.rs",
];

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

for (const relativePath of forbiddenParallelPaths) {
  if (existsSync(path.join(root, relativePath))) {
    failures.push(`${relativePath}: parallel Forum date execution path is forbidden`);
  }
}

const contract = parseJson(paths.contract);
const note = read(paths.note);
const projection = read(paths.projection);
const filters = read(paths.filters);
const execution = read(paths.execution);
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
requireAll(filters, [
  "pub exact_locale: Option<String>",
  "pub published_from: Option<DateTime<Utc>>",
  "pub published_to: Option<DateTime<Utc>>",
  "has_date_window",
  "DateTime::parse_from_rfc3339",
  "exact_locale_preserves_categories_and_fails_closed_on_missing_locale",
  "published_window_is_inclusive_and_excludes_categories",
  "malformed_or_missing_published_projection_fails_closed",
], paths.filters);
rejectAll(filters, ["rustok_forum", "forum_topic::", "forum_reply::"], paths.filters);
requireAll(execution, [
  "pub published_from: Option<String>",
  "pub published_to: Option<String>",
  "pub async fn execute_forum_storefront_search",
  "locale: Some(effective_locale.clone())",
  "all_items.retain(|item| document_filters.matches(item))",
  "let raw_total =",
  "let candidates = all_items",
  "let total = visible_items.len() as u64",
  'normalize_optional_rfc3339("published_from"',
  'normalize_optional_rfc3339("published_to"',
  "published_from must not be after published_to",
], paths.execution);
if (execution.indexOf("let raw_total =") > execution.indexOf("document_filters.matches(item)")) {
  failures.push(`${paths.execution}: raw bound must precede document narrowing`);
}
rejectAll(execution, [
  "execute_forum_storefront_search_with_date_window",
  "ForumStorefrontSearchDateWindowRequest",
], paths.execution);
rejectAll(searchLib, [
  "forum_storefront_date_execution",
  "forum_storefront_locale_date_filters",
  "execute_forum_storefront_search_with_date_window",
], paths.searchLib);
requireAll(graphqlOwner, [
  "published_from: Option<String>",
  "published_to: Option<String>",
  "execute_forum_storefront_search",
], paths.graphqlOwner);
rejectAll(graphqlOwner, [
  "ForumStorefrontSearchDateWindowRequest",
  "execute_forum_storefront_search_with_date_window",
], paths.graphqlOwner);
requireAll(graphqlAdapter, [
  "ForumStorefrontSearchByDateWindow",
  "$publishedFrom: String",
  "$publishedTo: String",
  "fetch_search_with_date_window",
], paths.graphqlAdapter);
requireAll(nativeAdapter, [
  'endpoint = "search/forum-storefront-search-by-date-window"',
  "fetch_search_with_date_window",
  "execute_forum_storefront_search_native",
], paths.nativeAdapter);
rejectAll(nativeAdapter, [
  "execute_forum_storefront_search_date_window_native",
  "ForumStorefrontSearchDateWindowRequest",
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
requireAll(note, [
  "# FORUM-23B2F3 exact Forum Search locale and date filters",
  "does not create a second execution",
  "Locale-only execution preserves Forum categories",
  "does **not** add storefront UI controls",
], paths.note);

if (contract) {
  if (contract.task !== "FORUM-23B2F3") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") failures.push(`${paths.contract}: unexpected status`);
  if (contract.architecture?.single_execution_owner !== true) failures.push(`${paths.contract}: single execution owner is not locked`);
  if (contract.architecture?.separate_date_execution_module_allowed !== false) failures.push(`${paths.contract}: duplicate date owner is not forbidden`);
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
