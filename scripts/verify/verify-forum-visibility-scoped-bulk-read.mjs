#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function read(relativePath) {
  const target = path.join(root, relativePath);
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

const ownerPath = "crates/rustok-forum/src/services/read_tracking_audience.rs";
const storefrontOwnerPath = "crates/rustok-forum/src/services/storefront_read_state_bulk.rs";
const servicesPath = "crates/rustok-forum/src/services/mod.rs";
const libPath = "crates/rustok-forum/src/lib.rs";
const contextPath = "crates/rustok-forum/src/topic_read_transport.rs";
const restPath = "crates/rustok-forum/src/controllers/read_state.rs";
const graphqlPath = "crates/rustok-forum/src/graphql/read_state.rs";
const runtimePath = "crates/rustok-forum/src/graphql/runtime_data.rs";
const storefrontGraphqlPath = "crates/rustok-forum/src/graphql/storefront_read_state.rs";
const storefrontSelectorPath = "crates/rustok-forum/storefront/src/transport/mod.rs";
const storefrontGraphqlAdapterPath = "crates/rustok-forum/storefront/src/transport/graphql_adapter.rs";
const storefrontNativeAdapterPath = "crates/rustok-forum/storefront/src/transport/native_server_adapter_bulk.rs";
const sqliteTestPath = "crates/rustok-forum/tests/topic_visibility_bulk_read_state_sqlite.rs";
const transportTestPath = "crates/rustok-forum/tests/read_state_transport_contract.rs";
const storefrontTestPath = "crates/rustok-forum/tests/storefront_read_state_contract.rs";
const contractPath = "crates/rustok-forum/contracts/forum-visibility-scoped-bulk-read.json";
const upstreamPath = "crates/rustok-forum/contracts/forum-projection-invalidation.json";
const rebuildPath = "crates/rustok-forum/contracts/forum-search-rebuild-scope-preservation.json";
const approvedReplyPath = "crates/rustok-forum/contracts/forum-approved-reply-search.json";
const notePath = "crates/rustok-forum/docs/forum-20bl-visibility-scoped-bulk-read.md";

const owner = read(ownerPath);
const storefrontOwner = read(storefrontOwnerPath);
const services = read(servicesPath);
const lib = read(libPath);
const context = read(contextPath);
const rest = read(restPath);
const graphql = read(graphqlPath);
const runtime = read(runtimePath);
const storefrontGraphql = read(storefrontGraphqlPath);
const storefrontSelector = read(storefrontSelectorPath);
const storefrontGraphqlAdapter = read(storefrontGraphqlAdapterPath);
const storefrontNativeAdapter = read(storefrontNativeAdapterPath);
const sqliteTest = read(sqliteTestPath);
const transportTest = read(transportTestPath);
const storefrontTest = read(storefrontTestPath);
const note = read(notePath);

let contract = null;
let upstream = null;
let rebuild = null;
let approvedReply = null;
for (const [label, source, assign] of [
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [upstreamPath, read(upstreamPath), (value) => { upstream = value; }],
  [rebuildPath, read(rebuildPath), (value) => { rebuild = value; }],
  [approvedReplyPath, read(approvedReplyPath), (value) => { approvedReply = value; }],
]) {
  try {
    assign(JSON.parse(source));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

for (const marker of [
  'VISIBILITY_BULK_READ_CURSOR_VERSION: &str = "brv1"',
  "pub struct ForumVisibilityScopedReadStateService",
  "mark_category_read_with_audience_context",
  "mark_all_read_with_audience_context",
  "ForumCategoryAudienceVisibilityService::new",
  "ForumTopicAudienceVisibilityService::new",
  "ForumTopicVisibilityScope::storefront_for_viewer",
  ".limit(limit + 1)",
  "visible_topic_ids.len() as u64",
  "visibility_channel_scope_token",
  "expected_channel_scope_token",
  "CategoryNotFound(category_id)",
  "latest_public_positions_in_tx",
  "latest_topic_revisions_in_tx",
  "upsert_topic_read_high_water_in_tx",
]) {
  requireMarker(owner, marker, ownerPath);
}
for (const forbidden of [
  "forum_category_audience_policy::",
  "forum_topic_audience_policy::",
  "forum_topic_channel_access::",
  "SELECT ",
  "total_visible",
]) {
  rejectMarker(owner, forbidden, ownerPath);
}

for (const marker of [
  "mark_category_read_audience_visible",
  "mark_all_read_audience_visible",
  "ForumVisibilityScopedReadStateService",
]) {
  requireMarker(storefrontOwner, marker, storefrontOwnerPath);
}
for (const marker of [
  'include!("read_tracking_audience.rs")',
  'include!("storefront_read_state_bulk.rs")',
  "ForumVisibilityScopedReadStateService",
]) {
  requireMarker(services, marker, servicesPath);
}
requireMarker(lib, "ForumVisibilityScopedReadStateService", libPath);

for (const marker of [
  "Rest",
  "MarkCategoryRead",
  "MarkAllRead",
  "forum-rest-mark-category-read",
  "forum-graphql-mark-all-read",
]) {
  requireMarker(context, marker, contextPath);
}
for (const marker of [
  "ForumVisibilityScopedReadStateService",
  "ForumTopicReadTransport::Rest",
  "ForumTopicReadOperation::MarkCategoryRead",
  "ForumTopicReadOperation::MarkAllRead",
  "mark_category_read_with_audience_context",
  "mark_all_read_with_audience_context",
]) {
  requireMarker(rest, marker, restPath);
}
for (const forbidden of [".mark_category_read(\n", ".mark_all_read(\n"]) rejectMarker(rest, forbidden, restPath);

for (const marker of [
  "visibility_scoped_read_state_service",
  "ForumTopicReadOperation::MarkCategoryRead",
  "ForumTopicReadOperation::MarkAllRead",
  "mark_category_read_with_audience_context",
  "mark_all_read_with_audience_context",
]) {
  requireMarker(graphql, marker, graphqlPath);
}
for (const forbidden of [".mark_category_read(\n", ".mark_all_read(\n"]) rejectMarker(graphql, forbidden, graphqlPath);
requireMarker(runtime, "visibility_scoped_read_state_service", runtimePath);

for (const marker of [
  "mark_forum_storefront_category_read",
  "mark_all_forum_storefront_topics_read",
  "mark_category_read_audience_visible",
  "mark_all_read_audience_visible",
  "ForumTopicReadOperation::MarkCategoryRead",
  "ForumTopicReadOperation::MarkAllRead",
]) {
  requireMarker(storefrontGraphql, marker, storefrontGraphqlPath);
}
for (const marker of [
  "mark_storefront_category_read",
  "mark_all_storefront_topics_read",
  "mark_storefront_category_read_server",
  "mark_storefront_category_read_graphql",
  "mark_all_storefront_topics_read_server",
  "mark_all_storefront_topics_read_graphql",
]) {
  requireMarker(storefrontSelector, marker, storefrontSelectorPath);
}
rejectMarker(storefrontSelector, "or_else", storefrontSelectorPath);

for (const marker of [
  "markForumStorefrontCategoryRead",
  "markAllForumStorefrontTopicsRead",
  "MarkForumTopicsReadBatchGraphqlInput",
  "nextCursor",
  "hasMore",
  "snapshotAt",
]) {
  requireMarker(storefrontGraphqlAdapter, marker, storefrontGraphqlAdapterPath);
}
for (const marker of [
  'endpoint = "forum/storefront-category-read"',
  'endpoint = "forum/storefront-all-read"',
  "mark_category_read_audience_visible",
  "mark_all_read_audience_visible",
  "MarkCategoryRead",
  "MarkAllRead",
]) {
  requireMarker(storefrontNativeAdapter, marker, storefrontNativeAdapterPath);
}

for (const marker of [
  "first.processed, 0",
  "first.has_more",
  "cross_channel",
  "CategoryNotFound",
  "states.len(), 1",
]) {
  requireMarker(sqliteTest, marker, sqliteTestPath);
}
requireMarker(transportTest, "category_and_all_read_transports_use_exact_visibility_owner", transportTestPath);
for (const marker of [
  "markForumStorefrontCategoryRead",
  "markAllForumStorefrontTopicsRead",
  "NATIVE_BULK_ADAPTER",
]) {
  requireMarker(storefrontTest, marker, storefrontTestPath);
}
for (const marker of [
  "processed = 0",
  "has_more = true",
  "exact cursor version is `brv1`",
  "no native-to-GraphQL or GraphQL-to-native fallback",
  "FORUM-20BL",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BL") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BK") failures.push(`${contractPath}: unexpected upstream task`);
  if (contract.downstream_task !== "FORUM-20BP") failures.push(`${contractPath}: unexpected downstream task`);
  if (contract.rebuild_scope_preservation_contract !== rebuildPath) failures.push(`${contractPath}: rebuild handoff drift`);
  if (contract.approved_reply_search_contract !== approvedReplyPath) failures.push(`${contractPath}: approved reply handoff drift`);
  if (contract.downstream_handoff?.rebuild_completion_contract !== rebuildPath) failures.push(`${contractPath}: historical rebuild completion drift`);
  if (contract.downstream_handoff?.completion_contract !== approvedReplyPath) failures.push(`${contractPath}: reply completion drift`);
  if (contract.downstream_handoff?.approved_public_reply_documents_completed !== true) failures.push(`${contractPath}: reply completion not recorded`);
  for (const key of [
    "category_root_uses_exact_category_audience_owner",
    "each_raw_topic_uses_exact_topic_audience_owner",
    "route_channel_applied",
    "missing_required_owner_facts_fail_closed",
  ]) {
    if (contract.owner_boundary?.[key] !== true) failures.push(`${contractPath}: owner ${key} drift`);
  }
  if (contract.owner_boundary?.transport_local_policy_copy_added !== false) failures.push(`${contractPath}: transport-local policy copy must remain absent`);
  for (const key of [
    "raw_candidate_query_uses_limit_plus_one",
    "cursor_advances_over_raw_candidates",
    "processed_counts_only_exact_visible_writes",
    "zero_processed_page_may_have_more",
    "cursor_bound_to_normalized_route_channel_digest",
    "cross_channel_resume_rejected",
    "visibility_reauthorized_on_each_resumed_page",
  ]) {
    if (contract.bounded_cursor_boundary?.[key] !== true) failures.push(`${contractPath}: cursor ${key} drift`);
  }
  if (contract.bounded_cursor_boundary?.maximum_raw_candidates_per_page !== 100) failures.push(`${contractPath}: raw candidate maximum drift`);
  for (const key of [
    "rest_routes_use_exact_owner",
    "graphql_fields_use_exact_owner",
    "storefront_graphql_category_command_added",
    "storefront_graphql_all_command_added",
    "storefront_native_category_command_added",
    "storefront_native_all_command_added",
    "storefront_compile_profile_selects_one_transport",
  ]) {
    if (contract.transport_boundary?.[key] !== true) failures.push(`${contractPath}: transport ${key} drift`);
  }
  if (contract.transport_boundary?.cross_transport_fallback_added !== false) failures.push(`${contractPath}: cross-transport fallback must remain absent`);
  if (contract.compatibility?.legacy_br1_used_by_public_bulk_transports !== false) failures.push(`${contractPath}: public transports must not use legacy br1`);
  for (const [key, expected] of Object.entries({
    workspace_dependency_changed: false,
    cargo_lock_changed: false,
    migration_added: false,
    ffa_status_changed: false,
    fba_status_changed: false,
  })) {
    if (contract.compatibility?.[key] !== expected) failures.push(`${contractPath}: compatibility ${key} drift`);
  }
  if (contract.remaining_scope?.includes("decide whether approved public replies become separate Search documents under FORUM-23")) {
    failures.push(`${contractPath}: completed reply decision remains open`);
  }
}

if (upstream) {
  if (upstream.visibility_scoped_bulk_read_contract !== contractPath) failures.push(`${upstreamPath}: visible bulk handoff drift`);
  if (upstream.rebuild_scope_preservation_contract !== rebuildPath) failures.push(`${upstreamPath}: rebuild handoff drift`);
  if (upstream.approved_reply_search_contract !== approvedReplyPath) failures.push(`${upstreamPath}: approved reply handoff drift`);
  if (upstream.downstream_task !== "FORUM-20BP") failures.push(`${upstreamPath}: downstream task drift`);
}
if (rebuild) {
  if (rebuild.task !== "FORUM-20BM") failures.push(`${rebuildPath}: unexpected task`);
  if (rebuild.upstream_task !== "FORUM-20BL") failures.push(`${rebuildPath}: unexpected upstream task`);
  if (rebuild.downstream_task !== "FORUM-20BP") failures.push(`${rebuildPath}: downstream task drift`);
  if (rebuild.approved_reply_search_contract !== approvedReplyPath) failures.push(`${rebuildPath}: approved reply handoff drift`);
  if (rebuild.replacement_boundary?.failed_forum_rebuild_keeps_previous_forum_scope !== true) failures.push(`${rebuildPath}: failed Forum rebuild must preserve scope`);
  if (rebuild.replacement_boundary?.global_cross_source_atomicity_claimed !== false) failures.push(`${rebuildPath}: global atomicity must remain unclaimed`);
}
if (approvedReply) {
  if (approvedReply.task !== "FORUM-20BO") failures.push(`${approvedReplyPath}: unexpected task`);
  if (approvedReply.compatibility?.ffa_status_changed !== false || approvedReply.compatibility?.fba_status_changed !== false) {
    failures.push(`${approvedReplyPath}: reply Search must not promote UI readiness`);
  }
}

if (failures.length > 0) {
  console.error("forum visibility-scoped bulk read verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum visibility-scoped bulk read ownership verified");
