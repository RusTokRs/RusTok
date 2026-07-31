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
  contract: "crates/rustok-forum/contracts/forum-search-author-filter.json",
  note: "crates/rustok-forum/docs/forum-23b2f1-search-author-filter.md",
  projection: "crates/rustok-forum/src/search_projection.rs",
  authorProjection: "crates/rustok-forum/src/search_projection_author.rs",
  filter: "crates/rustok-search/src/forum_document_filters.rs",
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
const authorProjection = read(paths.authorProjection);
const filter = read(paths.filter);
const execution = read(paths.execution);
const searchLib = read(paths.searchLib);
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
    '"author": author::public_author_payload',
    '"author_id": author::public_author_id',
  ],
  paths.projection,
);
requireAll(
  authorProjection,
  [
    "pub(super) fn public_author_payload",
    '"user_id": summary.user_id',
    "absent_or_denied_author_is_not_serialized",
  ],
  paths.authorProjection,
);

requireAll(
  filter,
  [
    "pub struct ForumStorefrontDocumentFilters",
    "pub author_ids: Vec<Uuid>",
    'item.source_module != "forum"',
    '"forum_topic" | "forum_reply"',
    '.get("author")',
    '.get("user_id")',
    "Uuid::parse_str",
    "author_filter_matches_exact_public_topic_or_reply_author",
    "non_forum_items_never_match_active_author_filter",
  ],
  paths.filter,
);
rejectAll(
  filter,
  ["rustok_forum", "rustok_profiles", "ProfilePresentationService"],
  paths.filter,
);

requireAll(
  execution,
  [
    "pub author_ids: Vec<String>",
    "document_filters: ForumStorefrontDocumentFilters",
    'normalize_uuid_values("author_ids", request.author_ids)',
    "all_items.retain(|item| document_filters.matches(item));",
    "let candidates = all_items",
    "let total = visible_items.len() as u64;",
    ".skip(query.offset)",
    ".take(query.limit)",
    "if document_filters.is_empty()",
    "SearchDictionaryService::apply_storefront_query_rules",
  ],
  paths.execution,
);
if (
  execution.indexOf("all_items.retain(|item| document_filters.matches(item));") >
  execution.indexOf("let candidates = all_items")
) {
  failures.push(`${paths.execution}: author filter must precede owner candidates`);
}
if (
  execution.indexOf("let raw_total =") >
  execution.indexOf("all_items.retain(|item| document_filters.matches(item));")
) {
  failures.push(`${paths.execution}: raw candidate bound must precede author narrowing`);
}

requireAll(
  searchLib,
  [
    "pub mod forum_document_filters;",
    "pub use forum_document_filters::ForumStorefrontDocumentFilters;",
  ],
  paths.searchLib,
);

requireAll(
  graphqlOwner,
  [
    "author_ids: Option<Vec<String>>",
    "author_ids: author_ids.unwrap_or_default()",
    "ForumStorefrontSearchRequest",
  ],
  paths.graphqlOwner,
);
requireAll(
  graphqlAdapter,
  [
    "FORUM_STOREFRONT_SEARCH_QUERY",
    "FORUM_STOREFRONT_SEARCH_BY_AUTHORS_QUERY",
    "ForumStorefrontSearchByAuthors",
    "$authorIds: [String!]!",
    "authorIds: $authorIds",
    "fetch_search_with_authors",
    "struct SearchPreviewVariables",
    "struct AuthorSearchPreviewVariables",
    "author_ids: Vec<String>",
  ],
  paths.graphqlAdapter,
);
const legacyGraphqlQueryStart = graphqlAdapter.indexOf(
  "const FORUM_STOREFRONT_SEARCH_QUERY",
);
const authorGraphqlQueryStart = graphqlAdapter.indexOf(
  "const FORUM_STOREFRONT_SEARCH_BY_AUTHORS_QUERY",
);
if (legacyGraphqlQueryStart < 0 || authorGraphqlQueryStart < 0) {
  failures.push(`${paths.graphqlAdapter}: GraphQL operations are incomplete`);
} else {
  const legacyGraphqlQuery = graphqlAdapter.slice(
    legacyGraphqlQueryStart,
    authorGraphqlQueryStart,
  );
  if (legacyGraphqlQuery.includes("authorIds")) {
    failures.push(
      `${paths.graphqlAdapter}: existing GraphQL operation must not send authorIds`,
    );
  }
}
requireAll(
  nativeAdapter,
  [
    "fetch_search_with_authors",
    "author_ids: Vec<String>",
    "author_ids,",
    'endpoint = "search/forum-storefront-search"',
    'endpoint = "search/forum-storefront-search-by-authors"',
    "execute_forum_storefront_search_native",
  ],
  paths.nativeAdapter,
);
requireAll(
  transportFacade,
  [
    "pub async fn fetch_forum_search_by_authors",
    "forum_native_server_adapter::fetch_search(",
    "forum_graphql_adapter::fetch_search(",
    "forum_native_server_adapter::fetch_search_with_authors",
    "forum_graphql_adapter::fetch_search_with_authors",
  ],
  paths.transportFacade,
);
const legacyForumBranch = transportFacade.indexOf("if forum_category_scope");
const additiveAuthorFunction = transportFacade.indexOf(
  "pub async fn fetch_forum_search_by_authors",
);
if (legacyForumBranch < 0 || additiveAuthorFunction < 0) {
  failures.push(`${paths.transportFacade}: Forum transport functions are incomplete`);
} else {
  const legacySection = transportFacade.slice(legacyForumBranch, additiveAuthorFunction);
  if (legacySection.includes("fetch_search_with_authors")) {
    failures.push(
      `${paths.transportFacade}: existing Forum search must keep the legacy adapter path`,
    );
  }
}

rejectAll(
  graphqlTypes,
  ["author_ids", "authorIds"],
  `${paths.graphqlTypes} neutral SearchPreviewInput`,
);
rejectAll(
  storefrontModel,
  ["author_ids", "authorIds"],
  `${paths.storefrontModel} neutral shared filter DTO`,
);
rejectAll(
  engine,
  ["author_ids", "ForumStorefrontDocumentFilters"],
  `${paths.engine} neutral SearchQuery`,
);

requireAll(
  forumPlan,
  [
    "FORUM-23B2F1",
    "exact bounded Forum author filter",
    "verify-forum-search-author-filter.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2F1",
    "source_complete_execution_pending",
    "exact bounded Forum author filter",
  ],
  paths.searchPlan,
);
requireAll(
  note,
  [
    "# FORUM-23B2F1 exact Forum Search author filter",
    "Evaluation order",
    "does **not** add a storefront UI control",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2F1") {
    failures.push(`${paths.contract}: unexpected task ${contract.task}`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status ${contract.status}`);
  }
  if (contract.input?.maximum_values !== 10) {
    failures.push(`${paths.contract}: author bound must remain 10`);
  }
  if (!contract.author_source?.missing_denied_redacted_or_malformed_author_fails_closed) {
    failures.push(`${paths.contract}: fail-closed author source invariant is missing`);
  }
  if (!contract.evaluation?.raw_candidate_limit_is_checked_before_author_narrowing) {
    failures.push(`${paths.contract}: raw candidate ordering invariant is missing`);
  }
  if (!contract.evaluation?.query_rule_pins_disabled_when_filter_active) {
    failures.push(`${paths.contract}: query-rule pin invariant is missing`);
  }
  if (contract.compatibility?.public_search_preview_input_changed !== false) {
    failures.push(`${paths.contract}: shared GraphQL input must remain unchanged`);
  }
  if (contract.compatibility?.shared_storefront_filter_dto_changed !== false) {
    failures.push(`${paths.contract}: shared storefront DTO must remain unchanged`);
  }
  if (contract.transport_parity?.existing_graphql_operation_changed !== false) {
    failures.push(`${paths.contract}: existing GraphQL operation changed`);
  }
  if (
    contract.transport_parity?.additive_author_graphql_operation !==
    "ForumStorefrontSearchByAuthors"
  ) {
    failures.push(`${paths.contract}: additive GraphQL author operation is missing`);
  }
  if (contract.transport_parity?.existing_native_endpoint_signature_changed !== false) {
    failures.push(`${paths.contract}: existing native endpoint signature changed`);
  }
  if (
    contract.transport_parity?.additive_author_native_endpoint !==
    "search/forum-storefront-search-by-authors"
  ) {
    failures.push(`${paths.contract}: additive native author endpoint is missing`);
  }
}

if (failures.length > 0) {
  console.error("FORUM-23B2F1 Search author filter verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2F1 Search author filter source contract is consistent.");
