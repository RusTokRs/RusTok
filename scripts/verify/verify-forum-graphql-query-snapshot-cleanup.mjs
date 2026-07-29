#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function absolute(relativePath) {
  return path.join(root, relativePath);
}

function read(relativePath) {
  const target = absolute(relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const snapshotPath = "crates/rustok-forum/src/graphql/query.rs";
const runtimePath = "crates/rustok-forum/src/graphql/query_runtime.rs";
const modulePath = "crates/rustok-forum/src/graphql/mod.rs";
const channelVerifierPath = "scripts/verify/verify-channel-proof-points.mjs";
const replyAudienceVerifierPath = "scripts/verify/verify-forum-reply-audience-read.mjs";
const replyLegacyVerifierPath = "scripts/verify/verify-forum-reply-legacy-cutover.mjs";
const categoryVerifierPath = "scripts/verify/verify-forum-category-audience-read.mjs";
const workflowPath = ".github/workflows/forum11-diagnostics.yml";
const contractPath = "crates/rustok-forum/contracts/forum-graphql-query-snapshot-cleanup.json";
const notePath = "crates/rustok-forum/docs/forum-20bn-graphql-query-snapshot-cleanup.md";
const replyLegacyContractPath = "crates/rustok-forum/contracts/forum-reply-legacy-cutover.json";
const categoryContractPath = "crates/rustok-forum/contracts/forum-category-audience-read.json";
const publicDiscoveryContractPath = "crates/rustok-forum/contracts/forum-public-discovery-seo.json";
const searchContractPath = "crates/rustok-forum/contracts/forum-search-projection.json";
const invalidationContractPath = "crates/rustok-forum/contracts/forum-projection-invalidation.json";
const visibilityContractPath = "crates/rustok-forum/contracts/forum-visibility-scoped-bulk-read.json";
const rebuildContractPath = "crates/rustok-forum/contracts/forum-search-rebuild-scope-preservation.json";

if (existsSync(absolute(snapshotPath))) {
  failures.push(`${snapshotPath}: legacy GraphQL query snapshot must be removed`);
}

const runtime = read(runtimePath);
const moduleSource = read(modulePath);
const channelVerifier = read(channelVerifierPath);
const replyAudienceVerifier = read(replyAudienceVerifierPath);
const replyLegacyVerifier = read(replyLegacyVerifierPath);
const categoryVerifier = read(categoryVerifierPath);
const workflow = read(workflowPath);
const note = read(notePath);

for (const marker of [
  "async fn forum_categories(",
  "async fn forum_category(",
  "async fn forum_replies(",
  "async fn forum_storefront_categories(",
  "async fn forum_storefront_replies(",
  "category_read_audience_port_context(",
  "reply_read_audience_port_context(",
  "list_authenticated_owner_visible_with_audience_context",
  "list_response_authenticated_owner_visible_with_audience_context",
  "list_authenticated_storefront_visible_with_audience_context",
  "list_public_storefront_visible_with_locale_fallback",
  "public_channel_slug(ctx)",
  "is_topic_visible_for_channel",
  "Some(&PUBLIC_REPLY_STATUSES)",
]) {
  requireMarker(runtime, marker, runtimePath);
}
requireMarker(moduleSource, '#[path = "query_runtime.rs"]', modulePath);
rejectMarker(moduleSource, 'mod query_runtime;', modulePath);

requireMarker(channelVerifier, runtimePath, channelVerifierPath);
for (const marker of [
  "public_channel_slug(ctx)",
  "is_topic_visible_for_channel",
  "async fn forum_storefront_replies(",
  "list_public_storefront_visible_with_locale_fallback",
  "Some(&PUBLIC_REPLY_STATUSES)",
]) {
  requireMarker(channelVerifier, marker, channelVerifierPath);
}
rejectMarker(channelVerifier, snapshotPath, channelVerifierPath);

requireMarker(replyAudienceVerifier, runtimePath, replyAudienceVerifierPath);
rejectMarker(replyAudienceVerifier, snapshotPath, replyAudienceVerifierPath);
for (const [source, label] of [
  [replyLegacyVerifier, replyLegacyVerifierPath],
  [categoryVerifier, categoryVerifierPath],
]) {
  requireMarker(source, runtimePath, label);
  requireMarker(source, snapshotPath, label);
  requireMarker(source, "exists(snapshotPath)", label);
  rejectMarker(source, "read(snapshotPath)", label);
}

requireMarker(workflow, runtimePath, workflowPath);
rejectMarker(workflow, snapshotPath, workflowPath);
rejectMarker(workflow, 'Path("crates/rustok-forum/src/graphql/query.rs")', workflowPath);

for (const marker of [
  "FORUM-20BN",
  "query_runtime.rs",
  "must not exist",
  "cannot recreate",
  "FORUM-20BO",
]) {
  requireMarker(note, marker, notePath);
}

let contract = null;
const upstreamContracts = [];
for (const [label, source, assign] of [
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [replyLegacyContractPath, read(replyLegacyContractPath), (value) => upstreamContracts.push([replyLegacyContractPath, value])],
  [categoryContractPath, read(categoryContractPath), (value) => upstreamContracts.push([categoryContractPath, value])],
  [publicDiscoveryContractPath, read(publicDiscoveryContractPath), (value) => upstreamContracts.push([publicDiscoveryContractPath, value])],
  [searchContractPath, read(searchContractPath), (value) => upstreamContracts.push([searchContractPath, value])],
  [invalidationContractPath, read(invalidationContractPath), (value) => upstreamContracts.push([invalidationContractPath, value])],
  [visibilityContractPath, read(visibilityContractPath), (value) => upstreamContracts.push([visibilityContractPath, value])],
  [rebuildContractPath, read(rebuildContractPath), (value) => upstreamContracts.push([rebuildContractPath, value])],
]) {
  try {
    assign(JSON.parse(source));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

if (contract) {
  if (contract.task !== "FORUM-20BN") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BM") {
    failures.push(`${contractPath}: unexpected upstream task`);
  }
  if (contract.downstream_task !== "FORUM-20BO") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
  if (contract.canonical_graphql_runtime !== runtimePath) {
    failures.push(`${contractPath}: canonical runtime drift`);
  }
  if (contract.removed_snapshot !== snapshotPath) {
    failures.push(`${contractPath}: removed snapshot drift`);
  }
  for (const key of [
    "legacy_query_snapshot_removed",
    "canonical_runtime_file_preserved",
    "module_selector_still_targets_query_runtime",
    "source_verifiers_read_only_canonical_runtime",
    "channel_proof_points_read_only_canonical_runtime",
    "forum11_diagnostics_formats_canonical_runtime",
    "reply_exact_owner_assertions_preserved",
    "category_exact_owner_assertions_preserved",
    "public_channel_reply_assertions_preserved",
  ]) {
    if (contract.cleanup_boundary?.[key] !== true) {
      failures.push(`${contractPath}: cleanup boundary ${key} drift`);
    }
  }
  for (const key of [
    "forum11_diagnostics_patches_legacy_snapshot",
    "removed_snapshot_may_be_recreated_by_workflow",
  ]) {
    if (contract.cleanup_boundary?.[key] !== false) {
      failures.push(`${contractPath}: cleanup boundary ${key} must remain false`);
    }
  }
  for (const key of [
    "existing_graphql_field_names_changed",
    "graphql_schema_composition_changed",
    "graphql_runtime_owner_changed",
    "forum_rest_routes_changed",
    "public_response_dto_changed",
    "workspace_dependency_changed",
    "cargo_lock_changed",
    "migration_added",
    "ffa_status_changed",
    "fba_status_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${contractPath}: compatibility ${key} must remain false`);
    }
  }
}

for (const [label, upstream] of upstreamContracts) {
  if (upstream.graphql_snapshot_cleanup_contract !== contractPath) {
    failures.push(`${label}: snapshot cleanup contract handoff drift`);
  }
  const removed =
    upstream.compatibility?.legacy_graphql_snapshot_removed === true ||
    upstream.compatibility?.legacy_query_snapshot_removed === true;
  if (!removed) failures.push(`${label}: snapshot removal compatibility not recorded`);
  if (upstream.remaining_scope?.includes("remove the uncompiled legacy GraphQL query snapshot after verifier migration")) {
    failures.push(`${label}: completed snapshot cleanup remains in remaining scope`);
  }
}

if (failures.length > 0) {
  console.error("forum GraphQL query snapshot cleanup verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum GraphQL query snapshot cleanup verified");
