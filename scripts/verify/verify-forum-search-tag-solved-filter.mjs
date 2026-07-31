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
  contract: "crates/rustok-forum/contracts/forum-search-tag-solved-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f2-search-tag-solved-filter.md",
  projection: "crates/rustok-forum/src/search_projection.rs",
  filter: "crates/rustok-search/src/forum_document_filters.rs",
  execution: "crates/rustok-search/src/forum_storefront_execution.rs",
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
const projection = read(paths.projection);
const filter = read(paths.filter);
const execution = read(paths.execution);
const graphqlOwner = read(paths.graphqlOwner);
const graphqlTypes = read(paths.graphqlTypes);
const storefrontModel = read(paths.storefrontModel);
const graphqlAdapter = read(paths.graphqlAdapter);
const nativeAdapter = read(paths.nativeAdapter);
const transportFacade = read(paths.transportFacade);
const engine = read(paths.engine);

requireAll(
  projection,
  [
    "let topic_tags = topic.tags.clone();",
    '"topic_tags": topic_tags',
    '"solution_reply_id": topic.solution_reply_id',
    '"is_solution": is_solution',
  ],
  paths.projection,
);

requireAll(
  filter,
  [
    "pub tags: Vec<String>",
    "pub solved: Option<bool>",
    '"forum_topic" => "tags"',
    '"forum_reply" => "topic_tags"',
    "self.tags.iter().all",
    '"solution_reply_id"',
    '"is_solution"',
    "tag_filter_requires_every_exact_topic_tag",
    "solved_filter_uses_topic_solution_and_exact_reply_marker",
    "active_filters_intersect_and_exclude_non_forum_items",
  ],
  paths.filter,
);
rejectAll(
  filter,
  ["rustok_forum", "forum_topic::", "forum_reply::"],
  `${paths.filter} owner-neutral boundary`,
);

requireAll(
  execution,
  [
    "pub tags: Vec<String>",
    "pub solved: Option<bool>",
    'normalize_tag_values("tags", request.tags)',
    "solved: request.solved",
    "all_items.retain(|item| document_filters.matches(item));",
    "let candidates = all_items",
    "let total = visible_items.len() as u64;",
    "if document_filters.is_empty()",
  ],
  paths.execution,
);
if (
  execution.indexOf("all_items.retain(|item| document_filters.matches(item));") >
  execution.indexOf("let candidates = all_items")
) {
  failures.push(`${paths.execution}: document filters must precede owner candidates`);
}
if (
  execution.indexOf("let raw_total =") >
  execution.indexOf("all_items.retain(|item| document_filters.matches(item));")
) {
  failures.push(`${paths.execution}: raw candidate bound must precede filter narrowing`);
}

requireAll(
  graphqlOwner,
  [
    "tags: Option<Vec<String>>",
    "solved: Option<bool>",
    "tags: tags.unwrap_or_default()",
    "solved,",
  ],
  paths.graphqlOwner,
);
requireAll(
  graphqlAdapter,
  [
    "ForumStorefrontSearchByFilters",
    "$tags: [String!]",
    "$solved: Boolean",
    "fetch_search_with_filters",
    "FilterSearchPreviewVariables",
  ],
  paths.graphqlAdapter,
);
requireAll(
  nativeAdapter,
  [
    "fetch_search_with_filters",
    'endpoint = "search/forum-storefront-search-by-filters"',
    "tags: Vec<String>",
    "solved: Option<bool>",
    "tags,",
    "solved,",
  ],
  paths.nativeAdapter,
);
requireAll(
  transportFacade,
  [
    "pub async fn fetch_forum_search_with_filters",
    "forum_native_server_adapter::fetch_search_with_filters",
    "forum_graphql_adapter::fetch_search_with_filters",
  ],
  paths.transportFacade,
);

requireAll(
  graphqlAdapter,
  [
    "ForumStorefrontSearch($input: SearchPreviewInput!)",
    "ForumStorefrontSearchByAuthors",
  ],
  `${paths.graphqlAdapter} legacy operations`,
);
requireAll(
  nativeAdapter,
  [
    'endpoint = "search/forum-storefront-search"',
    'endpoint = "search/forum-storefront-search-by-authors"',
  ],
  `${paths.nativeAdapter} legacy endpoints`,
);

rejectAll(
  graphqlTypes,
  ["tag_filters", "solved_filter", "author_ids"],
  `${paths.graphqlTypes} neutral SearchPreviewInput`,
);
rejectAll(
  storefrontModel,
  ["tag_filters", "solved_filter", "author_ids"],
  `${paths.storefrontModel} neutral shared filter DTO`,
);
rejectAll(
  engine,
  ["ForumStorefrontDocumentFilters", "topic_tags"],
  `${paths.engine} neutral SearchQuery`,
);

requireAll(
  forumPlan,
  [
    "FORUM-23B2F2",
    "exact bounded Forum tag and solved filters",
    "verify-forum-search-tag-solved-filter.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2F2",
    "source_complete_execution_pending",
    "tag and solved-state filters",
  ],
  paths.searchPlan,
);
requireAll(
  note,
  [
    "# FORUM-23B2F2 exact Forum Search tag and solved filters",
    "Tag matching is exact, case-sensitive and uses AND semantics",
    "does **not** add storefront UI controls",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2F2") {
    failures.push(`${paths.contract}: unexpected task ${contract.task}`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status ${contract.status}`);
  }
  if (contract.input?.maximum_tag_values !== 10) {
    failures.push(`${paths.contract}: tag bound must remain 10`);
  }
  if (contract.input?.maximum_tag_length !== 64) {
    failures.push(`${paths.contract}: tag length bound must remain 64`);
  }
  if (!contract.evaluation?.tag_values_use_and_semantics) {
    failures.push(`${paths.contract}: tag AND semantics are missing`);
  }
  if (!contract.projection?.legacy_reply_without_topic_tags_fails_closed_when_tag_filter_active) {
    failures.push(`${paths.contract}: legacy reply fail-closed invariant is missing`);
  }
  if (!contract.evaluation?.raw_candidate_limit_is_checked_before_filter_narrowing) {
    failures.push(`${paths.contract}: raw candidate ordering invariant is missing`);
  }
  if (contract.transport_compatibility?.existing_wire_signatures_changed !== false) {
    failures.push(`${paths.contract}: legacy wire signatures changed`);
  }
  if (
    contract.transport_compatibility?.additive_filter_native_endpoint !==
    "search/forum-storefront-search-by-filters"
  ) {
    failures.push(`${paths.contract}: additive native filter endpoint is missing`);
  }
  if (contract.compatibility?.public_search_preview_input_changed !== false) {
    failures.push(`${paths.contract}: shared GraphQL input must remain unchanged`);
  }
  if (contract.compatibility?.shared_storefront_filter_dto_changed !== false) {
    failures.push(`${paths.contract}: shared storefront DTO must remain unchanged`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2F2 Search tag/solved verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2F2 Search tag/solved source contract is consistent.");
