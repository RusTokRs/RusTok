#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

const paths = {
  projection: "crates/rustok-forum/src/search_projection.rs",
  engine: "crates/rustok-search/src/engine.rs",
  searchEvidence:
    "crates/rustok-search/contracts/evidence/search-canonical-url-contract.json",
  contract:
    "crates/rustok-forum/contracts/forum-search-canonical-route-cutover.json",
  contractTest:
    "crates/rustok-forum/tests/search_canonical_route_cutover_contract.rs",
  docs: "crates/rustok-forum/docs/forum-24q-search-canonical-route-cutover.md",
  searchVerifier: "scripts/verify/verify-search-canonical-url-contract.mjs",
};

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(content, marker, label) {
  if (!content.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function forbidText(content, marker, label) {
  if (content.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const source = Object.fromEntries(
  Object.entries(paths).map(([key, value]) => [key, read(value)]),
);

let contract = null;
let searchEvidence = null;
try {
  contract = JSON.parse(source.contract);
} catch (error) {
  failures.push(`${paths.contract}: invalid JSON (${error.message})`);
}
try {
  searchEvidence = JSON.parse(source.searchEvidence);
} catch (error) {
  failures.push(`${paths.searchEvidence}: invalid JSON (${error.message})`);
}

for (const marker of [
  "ForumCategoryRouteService",
  "ForumTopicRouteService",
  "exact_category_route",
  "exact_topic_route",
  ".canonical_descriptor(",
  "category.effective_locale != locale",
  "topic.effective_locale != locale",
  "descriptor.category_id != category_id || descriptor.locale != locale",
  "descriptor.topic_id != topic_id || descriptor.locale != locale",
  'format!("{topic_route}?reply={reply_id}")',
  '"route": route',
]) {
  requireText(source.projection, marker, paths.projection);
}
for (const marker of [
  '"/modules/forum?category=',
  '"/modules/forum?topic=',
]) {
  forbidText(source.projection, marker, paths.projection);
}

for (const marker of [
  "canonical_forum_projected_result_url(value)",
  'value.payload.get("route")',
  "canonical_forum_category_route",
  "canonical_forum_topic_route",
  "rustok_api::normalize_locale_tag",
  "forum_topic_short_identity",
  "valid_forum_short_identity",
  "valid_forum_slug",
  'route.starts_with("//")',
  "route.contains('#')",
  "canonical_url_accepts_owner_projected_forum_category_topic_and_reply_routes",
  "canonical_url_rejects_stale_or_malformed_forum_route_projections",
]) {
  requireText(source.engine, marker, paths.engine);
}
for (const marker of [
  "const FORUM_STOREFRONT_ROUTE",
  "canonical_forum_reply_result_url",
  "{FORUM_STOREFRONT_ROUTE}?category=",
  "{FORUM_STOREFRONT_ROUTE}?topic=",
]) {
  forbidText(source.engine, marker, paths.engine);
}

for (const marker of [
  "forum_projection_owner",
  "forum_projection_owner_routes",
  "forum_stale_projection_fail_closed",
  "no compatibility fallback exists",
  "verify no UUID Forum query route is emitted after reindex",
]) {
  requireText(source.searchEvidence, marker, paths.searchEvidence);
}
for (const marker of [
  "forumProjectionPath",
  "ForumCategoryRouteService",
  "ForumTopicRouteService",
  "canonical_forum_projected_result_url(value)",
  "forum_stale_projection_fail_closed",
]) {
  requireText(source.searchVerifier, marker, paths.searchVerifier);
}
for (const marker of [
  "forum_projection_publishes_only_exact_owner_routes",
  "search_validates_owner_route_without_rebuilding_forum_identity",
  "contract_locks_reindex_fail_closed_and_transport_compatibility",
]) {
  requireText(source.contractTest, marker, paths.contractTest);
}
for (const marker of [
  "source-ready / maintainer execution pending",
  "No compatibility fallback is added",
  "full Forum Search projection rebuild",
  "A canonical route is not an authorization token",
  "No tests, Node verifiers, formatting, Cargo commands",
]) {
  requireText(source.docs, marker, paths.docs);
}

if (contract) {
  if (contract.task !== "FORUM-24Q") {
    failures.push(`${paths.contract}: task must be FORUM-24Q`);
  }
  if (contract.status !== "source_ready_maintainer_execution_pending") {
    failures.push(`${paths.contract}: unexpected source status`);
  }
  if (contract.projection_owner?.uuid_module_routes_projected !== false) {
    failures.push(`${paths.contract}: UUID module routes must not be projected`);
  }
  if (contract.search_boundary?.owner_projected_route_required !== true) {
    failures.push(`${paths.contract}: owner-projected route must be required`);
  }
  if (contract.search_boundary?.search_reconstructs_forum_slug !== false) {
    failures.push(`${paths.contract}: Search must not reconstruct Forum slugs`);
  }
  if (contract.search_boundary?.search_reconstructs_topic_short_id !== false) {
    failures.push(`${paths.contract}: Search must not reconstruct route identity`);
  }
  if (contract.reindex?.legacy_documents_fail_closed_until_reindexed !== true) {
    failures.push(`${paths.contract}: stale documents must fail closed`);
  }
  if (contract.reindex?.compatibility_fallback_added !== false) {
    failures.push(`${paths.contract}: compatibility fallback is forbidden`);
  }
  if (contract.compatibility?.new_migration !== false) {
    failures.push(`${paths.contract}: no migration is allowed`);
  }
  if (contract.verification?.executed_by_implementation_agent !== false) {
    failures.push(`${paths.contract}: execution must not be claimed`);
  }
}

if (searchEvidence) {
  if (
    searchEvidence.production_contract?.forum_projection_owner !== paths.projection
  ) {
    failures.push(`${paths.searchEvidence}: Forum projection owner path drift`);
  }
}

if (failures.length > 0) {
  console.error("forum Search canonical route cutover verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Search canonical route cutover verification passed");
