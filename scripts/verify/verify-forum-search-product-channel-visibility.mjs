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
  contract:
    "crates/rustok-forum/contracts/forum-search-product-channel-visibility.json",
  note: "crates/rustok-forum/docs/forum-23b2e2-product-channel-visibility.md",
  predicate: "crates/rustok-search/src/storefront_product_channel_visibility.rs",
  projector: "crates/rustok-search/src/projector_legacy.rs",
  bootstrap: "crates/rustok-search/src/projector.rs",
  reconciler: "crates/rustok-search/src/product_channel_reconciliation.rs",
  serverWorker:
    "apps/server/src/services/search_product_channel_reconciliation.rs",
  serverServices: "apps/server/src/services/mod.rs",
  serverBootstrap: "apps/server/src/services/server_bootstrap.rs",
  engine: "crates/rustok-search/src/pg_engine.rs",
  dictionaries: "crates/rustok-search/src/dictionaries.rs",
  suggestions: "crates/rustok-search/src/suggestions.rs",
  graphql: "crates/rustok-search/src/graphql/query.rs",
  native: "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
  forumExecution: "crates/rustok-search/src/forum_storefront_execution.rs",
  searchLib: "crates/rustok-search/src/lib.rs",
  previousVerifier:
    "scripts/verify/verify-forum-search-trusted-channel-authority.mjs",
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
const predicate = read(paths.predicate);
const projector = read(paths.projector);
const bootstrap = read(paths.bootstrap);
const reconciler = read(paths.reconciler);
const serverWorker = read(paths.serverWorker);
const serverServices = read(paths.serverServices);
const serverBootstrap = read(paths.serverBootstrap);
const engine = read(paths.engine);
const dictionaries = read(paths.dictionaries);
const suggestions = read(paths.suggestions);
const graphql = read(paths.graphql);
const native = read(paths.native);
const forumExecution = read(paths.forumExecution);
const searchLib = read(paths.searchLib);
const previousVerifier = read(paths.previousVerifier);

requireAll(
  predicate,
  [
    "pub(crate) fn product_channel_visibility_sql",
    "pub(crate) fn product_payload_visible_for_storefront",
    "channel_visibility,allowed_channel_slugs",
    "jsonb_typeof",
    "jsonb_array_length",
    "entity_type_column} <> 'product'",
    "unwrap_or_else(|| \"FALSE\".to_string())",
    "missing_or_malformed_projection_fails_closed",
    "restricted_product_requires_matching_normalized_slug",
  ],
  paths.predicate,
);
rejectAll(
  predicate,
  ["rustok_product", "rustok_channel", "ProductService", "ChannelService"],
  paths.predicate,
);

requireAll(
  projector,
  [
    "'channel_visibility', jsonb_build_object(",
    "'allowed_channel_slugs'",
    "WHEN NOT (p.metadata ? 'channel_visibility') THEN '[]'::jsonb",
    "p.metadata #> '{channel_visibility,allowed_channel_slugs}'",
    "ELSE 'null'::jsonb",
  ],
  paths.projector,
);

requireAll(
  bootstrap,
  [
    "PRODUCT_CHANNEL_VISIBILITY_LEGACY_COUNT_SQL",
    "entity_type = 'product'",
    "IS NULL",
    "self.rebuild_product_scope(tenant_id).await?",
    "product_channel_visibility_legacy_projection_is_detected",
  ],
  paths.bootstrap,
);

requireAll(
  reconciler,
  [
    "pub struct ProductChannelProjectionReconciler",
    "DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT",
    "LEGACY_PRODUCT_CHANNEL_TENANTS_SQL",
    "allowed_channel_slugs}' IS NULL",
    "self.projector.rebuild_product_scope(tenant_id).await?",
    "reconciliation_selects_only_missing_legacy_projection",
  ],
  paths.reconciler,
);
requireAll(
  serverWorker,
  [
    "start_product_channel_projection_reconciliation_if_ready",
    "ProductChannelProjectionReconciler::new",
    "sweep_due(DEFAULT_PRODUCT_CHANNEL_REPAIR_TENANT_LIMIT)",
    "Product Search channel projection reconciliation completed",
  ],
  paths.serverWorker,
);
requireAll(
  serverServices,
  ["pub mod search_product_channel_reconciliation;"],
  paths.serverServices,
);
requireAll(
  serverBootstrap,
  ["start_product_channel_projection_reconciliation_if_ready"],
  paths.serverBootstrap,
);

requireAll(
  engine,
  [
    "pub async fn search_storefront(",
    "search_with_storefront_channel(query, Some(channel))",
    "product_channel_visibility_sql(",
    "run_fts_search(",
    "run_typo_tolerant_search(",
    "build_filter_clause(query, 4, storefront_channel)",
    "self.search_with_storefront_channel(query, None).await",
  ],
  paths.engine,
);

requireAll(
  dictionaries,
  [
    "pub async fn apply_storefront_query_rules(",
    "apply_query_rules_with_storefront_channel",
    "product_payload_visible_for_storefront(&payload, channel)",
    "SELECT document_id, entity_type, source_module, locale, status, is_public, title, payload",
  ],
  paths.dictionaries,
);

requireAll(
  suggestions,
  [
    "pub async fn storefront_suggestions(",
    "suggestions_with_storefront_channel",
    "let query_rows = if storefront_channel.is_some()",
    "let document_rows = fetch_document_suggestions(",
    "product_channel_visibility_sql(",
    "AND {product_scope}",
  ],
  paths.suggestions,
);

requireAll(
  graphql,
  [
    "run_storefront_search_with_dictionaries(",
    ".search_storefront(search_query.clone(), channel)",
    "apply_storefront_query_rules",
    "SearchSuggestionService::storefront_suggestions(",
    "&trusted_channel",
  ],
  paths.graphql,
);

requireAll(
  native,
  [
    ".search_storefront(search_query.clone(), &trusted_channel)",
    "apply_storefront_query_rules(",
    "SearchSuggestionService::storefront_suggestions(",
    "resolve_trusted_storefront_channel(&request_context, tenant.id, None)",
  ],
  paths.native,
);

requireAll(
  forumExecution,
  [
    "trusted_channel: TrustedStorefrontChannel",
    ".search_storefront(scan_query.clone(), trusted_channel)",
    "apply_storefront_query_rules(",
  ],
  paths.forumExecution,
);

requireAll(
  searchLib,
  [
    "mod product_channel_reconciliation;",
    "mod storefront_product_channel_visibility;",
    "ProductChannelProjectionReconciler",
  ],
  paths.searchLib,
);

requireAll(
  forumPlan,
  [
    "FORUM-23B2E2",
    "Product channel visibility",
    "verify-forum-search-product-channel-visibility.mjs",
  ],
  paths.forumPlan,
);
requireAll(
  searchPlan,
  [
    "FORUM-23B2E2",
    "source_complete_execution_pending",
    "Product channel visibility",
  ],
  paths.searchPlan,
);
requireAll(
  note,
  [
    "# FORUM-23B2E2 Product storefront Search channel visibility",
    "One storefront predicate",
    "does **not** claim completion",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23B2E2") {
    failures.push(`${paths.contract}: unexpected task ${contract.task}`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status ${contract.status}`);
  }
  if (!contract.storefront_predicate?.missing_or_malformed_projection_fails_closed) {
    failures.push(`${paths.contract}: fail-closed projection invariant is missing`);
  }
  if (!contract.covered_surfaces?.query_rule_pins) {
    failures.push(`${paths.contract}: query-rule pin coverage is missing`);
  }
  if (!contract.covered_surfaces?.document_suggestions) {
    failures.push(`${paths.contract}: document suggestion coverage is missing`);
  }
  if (!contract.covered_surfaces?.storefront_query_text_suggestions_disabled) {
    failures.push(`${paths.contract}: storefront query suggestion fail-closed invariant is missing`);
  }
  if (contract.covered_surfaces?.admin_preview_changed !== false) {
    failures.push(`${paths.contract}: admin preview must remain unchanged`);
  }
  if (contract.compatibility?.database_schema_changed !== false) {
    failures.push(`${paths.contract}: database schema claim drift`);
  }
  if (!contract.reconciliation?.startup_worker_detects_missing_legacy_projection) {
    failures.push(`${paths.contract}: startup repair invariant is missing`);
  }
  if (!contract.reconciliation?.malformed_explicit_owner_projection_is_not_rebuilt_forever) {
    failures.push(`${paths.contract}: malformed owner loop guard is missing`);
  }
  if (contract.reconciliation?.manual_backfill_required !== false) {
    failures.push(`${paths.contract}: manual backfill claim drift`);
  }
}

// The earlier B2E1 verifier must not forbid the later, separately contracted work.
rejectAll(
  previousVerifier,
  [
    "product visibility predicate moved into B2E1 unexpectedly",
    "B2E1 intentionally does not claim the Product projection/filter work",
  ],
  paths.previousVerifier,
);

if (failures.length > 0) {
  console.error("FORUM-23B2E2 Product channel visibility verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("FORUM-23B2E2 Product channel visibility source contract is consistent.");
