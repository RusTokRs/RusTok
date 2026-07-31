#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];
const paths = {
  contract: "crates/rustok-forum/contracts/forum-search-kind-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f4-search-kind-filter.md",
  forumPlan: "crates/rustok-forum/docs/implementation-plan.md",
  searchPlan: "crates/rustok-search/docs/implementation-plan.md",
  filter: "crates/rustok-search/src/forum_document_filters.rs",
  execution: "crates/rustok-search/src/forum_storefront_execution.rs",
  graphqlOwner: "crates/rustok-search/src/graphql/forum_storefront.rs",
  graphqlTypes: "crates/rustok-search/src/graphql/types.rs",
  engine: "crates/rustok-search/src/engine.rs",
  storefrontModel: "crates/rustok-search/storefront/src/model.rs",
  graphqlAdapter:
    "crates/rustok-search/storefront/src/transport/forum_graphql_adapter.rs",
  nativeAdapter:
    "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs",
  transportFacade: "crates/rustok-search/storefront/src/transport/mod.rs",
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

const contract = parseJson(paths.contract);
const note = read(paths.note);
const forumPlan = read(paths.forumPlan);
const searchPlan = read(paths.searchPlan);
const filter = read(paths.filter);
const execution = read(paths.execution);
const graphqlOwner = read(paths.graphqlOwner);
const graphqlTypes = read(paths.graphqlTypes);
const engine = read(paths.engine);
const storefrontModel = read(paths.storefrontModel);
const graphqlAdapter = read(paths.graphqlAdapter);
const nativeAdapter = read(paths.nativeAdapter);
const transportFacade = read(paths.transportFacade);

requireAll(
  filter,
  [
    "pub kinds: Vec<String>",
    "self.kinds.is_empty()",
    "self.matches_kind(item)",
    '"forum_topic" => "topic"',
    '"forum_reply" => "reply"',
    "kind_filter_selects_exact_topic_or_reply_documents",
    "active_filters_intersect_and_exclude_non_forum_items",
  ],
  paths.filter,
);
rejectAll(filter, ["rustok_forum", "forum_topic::", "forum_reply::"], paths.filter);

requireAll(
  execution,
  [
    "pub kinds: Vec<String>",
    "kinds: normalize_forum_kinds(request.kinds)?",
    "fn normalize_forum_kinds(",
    'matches!(value.as_str(), "topic" | "reply")',
    "kinds exceeds the maximum size of 2 values",
    "kinds contains an unsupported value",
    "all_items.retain(|item| document_filters.matches(item));",
    "let raw_total =",
    "let candidates = all_items",
    "let total = visible_items.len() as u64;",
    "if document_filters.is_empty()",
  ],
  paths.execution,
);
if (
  execution.indexOf("let raw_total =") >
  execution.indexOf("all_items.retain(|item| document_filters.matches(item));")
) {
  failures.push(`${paths.execution}: raw candidate bound must precede kind narrowing`);
}
if (
  execution.indexOf("all_items.retain(|item| document_filters.matches(item));") >
  execution.indexOf("let candidates = all_items")
) {
  failures.push(`${paths.execution}: kind narrowing must precede owner candidates`);
}
rejectAll(
  execution,
  ["forum_storefront_kind_execution", "execute_forum_storefront_search_with_kinds"],
  `${paths.execution} single execution owner`,
);

requireAll(
  graphqlOwner,
  [
    "kinds: Option<Vec<String>>",
    "kinds: kinds.unwrap_or_default()",
    "execute_forum_storefront_search(",
  ],
  paths.graphqlOwner,
);
requireAll(
  graphqlAdapter,
  [
    "ForumStorefrontSearchByKinds",
    "$kinds: [String!]!",
    "kinds: $kinds",
    "KindSearchPreviewVariables",
    "fetch_search_with_kinds",
  ],
  paths.graphqlAdapter,
);
requireAll(
  nativeAdapter,
  [
    "fetch_search_with_kinds",
    'endpoint = "search/forum-storefront-search-by-kinds"',
    "kinds: Vec<String>",
    "kinds,",
  ],
  paths.nativeAdapter,
);
requireAll(
  transportFacade,
  [
    "pub async fn fetch_forum_search_by_kinds",
    "forum_native_server_adapter::fetch_search_with_kinds",
    "forum_graphql_adapter::fetch_search_with_kinds",
  ],
  paths.transportFacade,
);

requireAll(
  graphqlAdapter,
  [
    "ForumStorefrontSearch($input: SearchPreviewInput!)",
    "ForumStorefrontSearchByAuthors",
    "ForumStorefrontSearchByFilters",
    "ForumStorefrontSearchByDateWindow",
  ],
  `${paths.graphqlAdapter} existing operations`,
);
requireAll(
  nativeAdapter,
  [
    'endpoint = "search/forum-storefront-search"',
    'endpoint = "search/forum-storefront-search-by-authors"',
    'endpoint = "search/forum-storefront-search-by-filters"',
    'endpoint = "search/forum-storefront-search-by-date-window"',
  ],
  `${paths.nativeAdapter} existing endpoints`,
);

rejectAll(graphqlTypes, ["pub kinds:", "forumKinds"], `${paths.graphqlTypes} neutral input`);
rejectAll(engine, ["pub kinds:", "ForumStorefrontKind"], `${paths.engine} neutral query`);
rejectAll(
  storefrontModel,
  ["pub kinds:", "forum_kinds", "forumKinds"],
  `${paths.storefrontModel} shared DTO`,
);

requireAll(
  forumPlan,
  [
    "FORUM-23B2F4",
    "exact bounded Forum document-kind filter",
    "verify-forum-search-kind-filter.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2F4",
    "source_complete_execution_pending",
    "topic/reply kind filter",
  ],
  paths.searchPlan,
);
requireAll(
  note,
  [
    "# FORUM-23B2F4 exact Forum Search document kind filter",
    "topic",
    "reply",
    "does **not** add storefront UI controls",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2F4") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  if (JSON.stringify(contract.input?.allowed_values) !== JSON.stringify(["topic", "reply"])) {
    failures.push(`${paths.contract}: allowed kinds drift`);
  }
  if (contract.input?.maximum_values !== 2) failures.push(`${paths.contract}: kind bound drift`);
  if (!contract.evaluation?.raw_candidate_limit_is_checked_before_kind_narrowing) {
    failures.push(`${paths.contract}: raw ordering invariant missing`);
  }
  if (!contract.evaluation?.kind_intersects_author_tag_solved_and_date) {
    failures.push(`${paths.contract}: filter intersection invariant missing`);
  }
  if (contract.transport_compatibility?.existing_wire_signatures_changed !== false) {
    failures.push(`${paths.contract}: existing wire signatures changed`);
  }
  if (contract.compatibility?.forum_projection_shape_changed !== false) {
    failures.push(`${paths.contract}: projection compatibility drift`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2F4 kind filter verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2F4 kind filter source contract is consistent.");
