#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const projectionPath = "crates/rustok-forum/src/search_projection.rs";
const queryContractPath = "crates/rustok-search/src/engine.rs";
const graphqlTypesPath = "crates/rustok-search/src/graphql/types.rs";
const graphqlQueryPath = "crates/rustok-search/src/graphql/query.rs";
const pgEnginePath = "crates/rustok-search/src/pg_engine.rs";
const searchReadmePath = "crates/rustok-search/README.md";
const contractPath = "crates/rustok-forum/contracts/forum-search-exact-category-filter.json";
const notePath = "crates/rustok-forum/docs/forum-23b1-exact-category-filter.md";
const verifierPath = "scripts/verify/verify-forum-search-exact-category-filter.mjs";

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

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function requireAll(source, markers, label) {
  for (const marker of markers) requireMarker(source, marker, label);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const current = source.indexOf(marker, previous + 1);
    if (current < 0) {
      failures.push(`${label}: missing ordered marker ${marker}`);
      return;
    }
    if (current <= previous) {
      failures.push(`${label}: marker out of order ${marker}`);
      return;
    }
    previous = current;
  }
}

const plan = read(planPath);
const projection = read(projectionPath);
const queryContract = read(queryContractPath);
const graphqlTypes = read(graphqlTypesPath);
const graphqlQuery = read(graphqlQueryPath);
const pgEngine = read(pgEnginePath);
const searchReadme = read(searchReadmePath);
const note = read(notePath);
const contract = parseJson(contractPath);

requireAll(
  plan,
  [
    "## `FORUM-23` — search/index integration",
    "Search filters include",
    "category subtree, author, tag, locale, date, solved, kind, channel/group and",
  ],
  planPath,
);

requireAll(
  projection,
  [
    'const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category"',
    'const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic"',
    'const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply"',
    '"category_id": topic.category_id',
    '"category_id": topic.category_id',
    'document_id: category.id',
    "Some(&[ReplyStatus::Approved])",
  ],
  projectionPath,
);

requireAll(
  queryContract,
  [
    "pub struct SearchQuery",
    "pub category_ids: Vec<Uuid>",
  ],
  queryContractPath,
);
requireAll(
  graphqlTypes,
  [
    "pub struct SearchPreviewInput",
    "pub category_ids: Option<Vec<String>>",
  ],
  graphqlTypesPath,
);
requireAll(
  graphqlQuery,
  [
    'normalize_uuid_values("category_ids", input.category_ids)?',
    "if values.len() > MAX_FILTER_VALUES",
    "Uuid::parse_str(value.trim())",
    "category_ids: input.category_ids",
    "published_only: policy.published_only",
  ],
  graphqlQueryPath,
);

requireAll(
  pgEngine,
  [
    "let category_params =",
    "bind_uuid_list(&query.category_ids, &mut values, &mut next_param)",
    ".map(ToString::to_string)",
    "bind_list(&category_facet_values, &mut values, &mut next_param)",
    "entity_type = 'product'",
    "FROM index_product_categories ipc",
    "ipc.category_id IN ({category_params})",
    "source_module = 'forum'",
    "entity_type = 'forum_category' AND id IN ({category_params})",
    "entity_type IN ('forum_topic', 'forum_reply')",
    "facets ->> 'category_id' IN ({category_facet_params})",
    "sd.facets AS facets",
    "category_filter_preserves_product_and_adds_exact_forum_scope",
    '"ipc.category_id IN ($4)"',
    '"facets ->> \'category_id\' IN ($5)"',
    "assert_eq!(filters.values.len(), 2)",
  ],
  pgEnginePath,
);
requireOrder(
  pgEngine,
  [
    "bind_uuid_list(&query.category_ids, &mut values, &mut next_param)",
    "bind_list(&category_facet_values, &mut values, &mut next_param)",
    "clauses.push(format!",
  ],
  pgEnginePath,
);
if ((pgEngine.match(/sd\.facets AS facets/g) ?? []).length !== 2) {
  failures.push(`${pgEnginePath}: both ranked CTEs must carry facets`);
}
rejectMarker(pgEngine, "format!(query.category_ids", pgEnginePath);
rejectMarker(pgEngine, "facets ->> 'category_id' = ANY('{", pgEnginePath);

requireAll(
  searchReadme,
  [
    "Interpret the bounded `category_ids` query field across owner projections",
    "Exact Forum category filtering reuses `category_ids`",
    "Search does not resolve descendants or copy Forum tree policy",
    "verify-forum-search-exact-category-filter.mjs",
  ],
  searchReadmePath,
);
requireAll(
  note,
  [
    "FORUM-23B1",
    "exact-category foundation",
    "source_modules: [\"forum\"]",
    "index_product_categories",
    "facets.category_id",
    "bound parameters",
    "not category-subtree completion",
    "FORUM-23B2",
    "Not run by the implementation agent",
  ],
  notePath,
);

if (contract) {
  if (contract.task !== "FORUM-23B1") failures.push(`${contractPath}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }
  const expectedPaths = {
    canonical_plan: planPath,
    forum_projection_owner: projectionPath,
    search_query_contract: queryContractPath,
    search_graphql_input: graphqlTypesPath,
    search_graphql_normalization: graphqlQueryPath,
    postgres_filter_owner: pgEnginePath,
    search_readme: searchReadmePath,
    owner_note: notePath,
    verifier: verifierPath,
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${contractPath}: ${key} drift`);
  }

  for (const key of [
    "existing_category_ids_input_is_reused",
    "category_id_count_remains_bounded_by_existing_limit",
    "invalid_category_uuid_is_rejected_before_execution",
    "storefront_published_only_policy_is_preserved",
    "caller_can_constrain_forum_only_results_with_source_modules",
  ]) {
    if (contract.query_boundary?.[key] !== true) {
      failures.push(`${contractPath}: query boundary ${key} drift`);
    }
  }
  if (contract.query_boundary?.graphql_field_added !== false) {
    failures.push(`${contractPath}: GraphQL field addition must remain false`);
  }

  for (const key of [
    "forum_category_matches_its_document_id",
    "forum_topic_projects_exact_category_id_facet",
    "forum_reply_projects_exact_topic_category_id_facet",
    "category_filter_does_not_copy_category_tree_policy",
    "pending_hidden_or_denied_documents_remain_absent_from_public_projection",
  ]) {
    if (contract.projection_boundary?.[key] !== true) {
      failures.push(`${contractPath}: projection boundary ${key} drift`);
    }
  }

  for (const key of [
    "product_category_exists_clause_is_preserved",
    "forum_branch_is_source_module_scoped",
    "forum_category_matches_uuid_document_id",
    "forum_topic_and_reply_match_category_id_jsonb_facet",
    "uuid_and_facet_values_use_bound_parameters",
    "fts_ranked_cte_carries_facets",
    "typo_ranked_cte_carries_facets",
    "total_items_and_facets_use_the_same_filter_clause",
  ]) {
    if (contract.postgres_boundary?.[key] !== true) {
      failures.push(`${contractPath}: PostgreSQL boundary ${key} drift`);
    }
  }

  for (const key of [
    "search_graphql_schema_changed",
    "search_query_rust_shape_changed",
    "product_category_filter_removed",
    "forum_projection_shape_changed",
    "database_migration_added",
    "dependency_added",
    "cargo_lock_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${contractPath}: compatibility ${key} must remain false`);
    }
  }

  for (const key of [
    "category_subtree_expansion_is_complete",
    "category_descendants_are_resolved_at_query_time",
    "category_facet_buckets_are_added",
    "all_forum_filters_are_complete",
    "runtime_verification_was_executed",
  ]) {
    if (contract.non_claims?.[key] !== true) {
      failures.push(`${contractPath}: non-claim ${key} drift`);
    }
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${contractPath}: execution status drift`);
  }
  if (contract.downstream_task !== "FORUM-23B2") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
}

if (failures.length > 0) {
  console.error("Forum exact category Search filter verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum exact category Search filter verification passed.");
