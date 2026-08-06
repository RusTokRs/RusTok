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

const replyAudiencePath = "crates/rustok-forum/src/services/reply_audience_read.rs";
const publicDiscoveryPath = "crates/rustok-forum/src/services/public_discovery.rs";
const sourcePath = "crates/rustok-forum/src/search_projection.rs";
const replyUpdatePath = "crates/rustok-forum/src/services/reply_inline.rs";
const projectorPath = "crates/rustok-search/src/forum_projector.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const enginePath = "crates/rustok-search/src/engine.rs";
const pgEnginePath = "crates/rustok-search/src/pg_engine.rs";
const storefrontNativePath = "crates/rustok-search/storefront/src/transport/native_server_adapter.rs";
const adminPreviewPath = "crates/rustok-search/admin/src/transport/native_server_adapter/mapping.rs";
const adminGlobalPath = "apps/admin/src/widgets/app_shell/native_server_adapter.rs";
const rustTestPath = "crates/rustok-search/tests/forum_approved_reply_projection_contract.rs";
const canonicalEvidencePath = "crates/rustok-search/contracts/evidence/search-canonical-url-contract.json";
const contractPath = "crates/rustok-forum/contracts/forum-approved-reply-search.json";
const routeCutoverPath = "crates/rustok-forum/contracts/forum-search-canonical-route-cutover.json";
const notePath = "crates/rustok-forum/docs/forum-20bo-approved-reply-search.md";
const snapshotPath = "crates/rustok-forum/contracts/forum-graphql-query-snapshot-cleanup.json";
const searchPath = "crates/rustok-forum/contracts/forum-search-projection.json";
const invalidationPath = "crates/rustok-forum/contracts/forum-projection-invalidation.json";
const rebuildPath = "crates/rustok-forum/contracts/forum-search-rebuild-scope-preservation.json";
const visibilityPath = "crates/rustok-forum/contracts/forum-visibility-scoped-bulk-read.json";

const replyAudience = read(replyAudiencePath);
const publicDiscovery = read(publicDiscoveryPath);
const source = read(sourcePath);
const replyUpdate = read(replyUpdatePath);
const projector = read(projectorPath);
const ingestion = read(ingestionPath);
const engine = read(enginePath);
const pgEngine = read(pgEnginePath);
const storefrontNative = read(storefrontNativePath);
const adminPreview = read(adminPreviewPath);
const adminGlobal = read(adminGlobalPath);
const rustTest = read(rustTestPath);
const note = read(notePath);
let canonicalEvidence = null;
let contract = null;
let routeCutover = null;
const upstreams = [];
for (const [label, body, assign] of [
  [canonicalEvidencePath, read(canonicalEvidencePath), (value) => { canonicalEvidence = value; }],
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [routeCutoverPath, read(routeCutoverPath), (value) => { routeCutover = value; }],
  [snapshotPath, read(snapshotPath), (value) => upstreams.push([snapshotPath, value])],
  [searchPath, read(searchPath), (value) => upstreams.push([searchPath, value])],
  [invalidationPath, read(invalidationPath), (value) => upstreams.push([invalidationPath, value])],
  [rebuildPath, read(rebuildPath), (value) => upstreams.push([rebuildPath, value])],
  [visibilityPath, read(visibilityPath), (value) => upstreams.push([visibilityPath, value])],
]) {
  try {
    assign(JSON.parse(body));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

for (const marker of [
  "pub async fn get_public_storefront_visible_with_locale_fallback",
  "statuses.is_some_and(|allowed| !allowed.contains(&reply.status))",
  ".is_topic_visible(tenant_id, reply.topic_id, channel_slug, &viewer)",
  ".get_with_locale_fallback(",
]) {
  requireMarker(replyAudience, marker, replyAudiencePath);
}
for (const marker of [
  "replies: ForumReplyAudienceReadService",
  "pub async fn get_public_reply_with_locale_fallback",
  ".get_public_storefront_visible_with_locale_fallback(",
]) {
  requireMarker(publicDiscovery, marker, publicDiscoveryPath);
}

for (const marker of [
  'const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply"',
  "forum_reply_body::Entity::find()",
  "ProjectionCandidate::Reply",
  "ProjectionCursor::Reply",
  'format!("reply:{entity_id}:{locale}")',
  '"reply" => Ok(Self::Reply',
  ".get_public_reply_with_locale_fallback(",
  "Some(&[ReplyStatus::Approved])",
  "if reply.effective_locale != locale",
  ".get_public_topic_with_locale_fallback(",
  "if topic.effective_locale != locale",
  ".get_public_category_with_locale_fallback(",
  "exact_topic_route(&self.db, tenant_id, topic.id, locale)",
  'document_key: format!("forum_reply:{reply_id}:{locale}")',
  "entity_type: FORUM_REPLY_ENTITY_TYPE.to_string()",
  '"kind": "forum_reply"',
  '"reply_id": reply_id',
  '"topic_id": topic.id',
  '"is_solution": is_solution',
  'format!("{topic_route}?reply={reply_id}")',
  "FORUM_REPLY_ENTITY_TYPE =>",
]) {
  requireMarker(source, marker, sourcePath);
}
for (const forbidden of [
  "forum_reply::Entity::find()",
  "ForumAudienceEvaluator",
  "forum_category_audience_policy::",
  "forum_topic_audience_policy::",
  "rustok_search::",
  '"/modules/forum?topic=',
]) {
  rejectMarker(source, forbidden, sourcePath);
}

for (const marker of [
  'target_type: "forum_topic".to_string()',
  "target_id: Some(topic_id)",
  ".publish_in_tx(",
  "txn.commit().await?",
]) {
  requireMarker(replyUpdate, marker, replyUpdatePath);
}
rejectMarker(replyUpdate, 'target_type: "forum_reply"', replyUpdatePath);

for (const marker of [
  'const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply"',
  "FORUM_TOPIC_ENTITY_TYPE | FORUM_REPLY_ENTITY_TYPE",
  "if entity_type == FORUM_TOPIC_ENTITY_TYPE",
  "return self.rebuild_tenant(tenant_id).await",
  "'forum_category', 'forum_topic', 'forum_reply'",
  "CREATE TEMP TABLE forum_search_projection_stage",
  "ON COMMIT DROP",
]) {
  requireMarker(projector, marker, projectorPath);
}
for (const marker of [
  "ForumTopicReplied",
  "ForumReplyStatusChanged",
  '"forum_topic"',
  "refresh_entity",
]) {
  requireMarker(ingestion, marker, ingestionPath);
}

for (const marker of [
  'const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply"',
  "canonical_forum_projected_result_url(value)",
  'value.payload.get("route")',
  'parse_payload_uuid(&value.payload, "reply_id")',
  "if reply_id != value.id",
  'parse_payload_uuid(&value.payload, "topic_id")',
  "canonical_forum_topic_route(route, locale.as_str(), topic_id, Some(reply_id))",
  "forum_topic_short_identity",
  "canonical_url_accepts_owner_projected_forum_category_topic_and_reply_routes",
  "canonical_url_rejects_stale_or_malformed_forum_route_projections",
]) {
  requireMarker(engine, marker, enginePath);
}
for (const forbidden of [
  "canonical_forum_reply_result_url",
  "{FORUM_STOREFRONT_ROUTE}?topic=",
]) {
  rejectMarker(engine, forbidden, enginePath);
}

requireMarker(pgEngine, 'clauses.push("is_public = TRUE".to_string())', pgEnginePath);
rejectMarker(pgEngine, "status = 'approved'", pgEnginePath);
for (const [consumer, label, marker] of [
  [storefrontNative, storefrontNativePath, "rustok_search::canonical_search_result_url(&value)"],
  [adminPreview, adminPreviewPath, "rustok_search::canonical_search_result_url(&item)"],
  [adminGlobal, adminGlobalPath, "rustok_search::canonical_search_result_url(&item)"],
]) {
  requireMarker(consumer, marker, label);
  rejectMarker(consumer, "canonical_forum_reply_result_url", label);
}
for (const marker of [
  '("forum_category", "forum" | "rustok-forum")',
  "Permission::FORUM_CATEGORIES_READ",
  '("forum_topic", "forum" | "rustok-forum")',
  "Permission::FORUM_TOPICS_READ",
  '("forum_reply", "forum" | "rustok-forum")',
  "Permission::FORUM_REPLIES_READ",
  'required_admin_search_permission("forum_reply", "content")',
]) {
  requireMarker(adminGlobal, marker, adminGlobalPath);
}

for (const marker of [
  "forum_source_publishes_only_exact_public_approved_reply_documents",
  "reply_edits_reuse_topic_invalidation_and_topic_refresh_rebuilds_child_scope",
  "canonical_reply_route_is_bound_to_owner_topic_route_and_result_identity",
  "admin_global_search_maps_forum_results_to_domain_permissions",
]) {
  requireMarker(rustTest, marker, rustTestPath);
}
for (const marker of [
  "FORUM-20BO",
  "independent `forum_reply` Search documents",
  "selected anonymous-public reply read",
  "pending, rejected, deleted",
  "three bounded phases",
  "topic and all child replies together",
  "No new root event or reindex target string",
  "FORUM-24Q supersedes the original UUID query navigation",
  "/{locale}/forum/t/{short_id}/{slug}?reply={reply_id}",
  "It does not rebuild the topic path or retain a UUID compatibility fallback",
  "Published storefront searches filter on `is_public = TRUE`",
  "`FORUM_REPLIES_READ`",
  "FORUM-20BP",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BO") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BN") failures.push(`${contractPath}: unexpected upstream task`);
  if (contract.downstream_task !== "FORUM-20BP") failures.push(`${contractPath}: unexpected downstream task`);
  if (contract.reply_selected_read_owner !== replyAudiencePath) failures.push(`${contractPath}: reply owner path drift`);
  if (contract.public_discovery_owner !== publicDiscoveryPath) failures.push(`${contractPath}: public discovery path drift`);
  if (contract.projection_boundary?.entity_type !== "forum_reply") failures.push(`${contractPath}: entity type drift`);
  for (const key of [
    "raw_candidates_are_reply_locale_bodies",
    "cursor_advances_over_hidden_or_unapproved_replies",
    "selected_public_reply_owner_added",
    "approved_status_required_from_typed_forum_owner",
    "parent_topic_exact_public_discovery_required",
    "parent_category_exact_public_discovery_required",
    "route_channel_restricted_parent_absent_without_route_channel",
    "exact_raw_candidate_locale_required",
    "reply_body_is_search_document_body",
    "topic_title_is_search_document_title",
    "category_name_is_search_document_subtitle",
  ]) {
    if (contract.projection_boundary?.[key] !== true) failures.push(`${contractPath}: projection ${key} drift`);
  }
  for (const key of [
    "missing_exact_locale_body_emits_document",
    "pending_rejected_deleted_or_hidden_reply_emits_document",
    "projection_source_loads_reply_status_or_body_directly",
    "safe_author_summary_added",
    "vote_score_projected",
    "reply_position_projected",
    "reply_search_storage_added_to_forum",
    "audience_policy_copied_into_search",
  ]) {
    if (contract.projection_boundary?.[key] !== false) failures.push(`${contractPath}: projection ${key} must remain false`);
  }
  for (const key of [
    "search_owns_reply_document_storage",
    "existing_search_documents_table_reused",
    "forum_scope_stage_includes_reply_documents",
    "forum_scope_delete_includes_reply_documents",
    "targeted_reply_document_replace_supported",
    "topic_refresh_rebuilds_atomic_forum_scope",
    "topic_refresh_and_child_reply_refresh_are_one_search_transaction",
    "topic_visibility_change_removes_child_reply_documents",
    "topic_copy_or_category_context_change_refreshes_child_reply_documents",
    "reply_create_and_status_events_reuse_existing_topic_refresh",
    "reply_edit_publishes_owner_transactional_topic_invalidation",
  ]) {
    if (contract.persistence_boundary?.[key] !== true) failures.push(`${contractPath}: persistence ${key} drift`);
  }
  for (const key of [
    "new_root_domain_event_added",
    "new_reindex_target_string_added",
    "out_of_order_owner_revision_guard_added",
    "search_schema_migration_added",
  ]) {
    if (contract.persistence_boundary?.[key] !== false) failures.push(`${contractPath}: persistence ${key} must remain false`);
  }
  for (const key of [
    "canonical_pair_required",
    "reply_id_payload_must_match_result_id",
    "topic_id_payload_must_be_non_nil_uuid",
    "reply_query_is_additive_topic_open_hint",
    "spoofed_or_malformed_payload_is_non_navigable",
  ]) {
    if (contract.canonical_url_boundary?.[key] !== true) failures.push(`${contractPath}: historical URL ${key} drift`);
  }
  for (const key of [
    "standalone_reply_storefront_page_added",
    "storefront_reply_anchor_behavior_added",
    "transport_local_url_fallback_added",
  ]) {
    if (contract.canonical_url_boundary?.[key] !== false) failures.push(`${contractPath}: historical URL ${key} must remain false`);
  }
  for (const key of [
    "published_only_search_filters_by_is_public",
    "graphql_result_mapping_is_entity_generic",
    "storefront_native_result_mapping_is_entity_generic",
    "search_admin_preview_mapping_is_entity_generic",
    "admin_global_category_requires_forum_categories_read",
    "admin_global_topic_requires_forum_topics_read",
    "admin_global_reply_requires_forum_replies_read",
    "admin_global_wrong_source_fails_closed",
  ]) {
    if (contract.consumer_boundary?.[key] !== true) failures.push(`${contractPath}: consumer ${key} drift`);
  }
  for (const key of [
    "published_only_search_filters_by_fixed_status_allowlist",
    "consumer_local_reply_url_added",
  ]) {
    if (contract.consumer_boundary?.[key] !== false) failures.push(`${contractPath}: consumer ${key} must remain false`);
  }
  for (const key of [
    "workspace_dependency_changed",
    "cargo_lock_changed",
    "migration_added",
    "ffa_status_changed",
    "fba_status_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) failures.push(`${contractPath}: compatibility ${key} must remain false`);
  }
}

if (routeCutover) {
  if (routeCutover.task !== "FORUM-24Q") failures.push(`${routeCutoverPath}: unexpected task`);
  if (!routeCutover.projection_owner?.reply_uses_canonical_topic_route) {
    failures.push(`${routeCutoverPath}: reply must reuse canonical topic route`);
  }
  if (!routeCutover.search_boundary?.owner_projected_route_required) {
    failures.push(`${routeCutoverPath}: owner-projected route must be required`);
  }
  if (!routeCutover.reindex?.legacy_documents_fail_closed_until_reindexed) {
    failures.push(`${routeCutoverPath}: stale reply documents must fail closed`);
  }
  if (routeCutover.reindex?.compatibility_fallback_added) {
    failures.push(`${routeCutoverPath}: compatibility fallback must remain absent`);
  }
}

for (const [label, upstream] of upstreams) {
  if (upstream.approved_reply_search_contract !== contractPath) {
    failures.push(`${label}: approved reply contract handoff drift`);
  }
  if (upstream.remaining_scope?.includes("decide whether approved public replies become separate Search documents under FORUM-23")) {
    failures.push(`${label}: completed reply decision remains open`);
  }
  if ([searchPath, invalidationPath, rebuildPath, visibilityPath].includes(label) && upstream.downstream_task !== "FORUM-20BP") {
    failures.push(`${label}: downstream task must advance to FORUM-20BP`);
  }
}
if (canonicalEvidence) {
  const cases = new Set((canonicalEvidence.cases ?? []).map((entry) => entry.name));
  for (const required of [
    "forum_projection_owner_routes",
    "forum_reply_canonical_route",
    "forum_reply_fail_closed",
    "forum_stale_projection_fail_closed",
    "admin_forum_permission_gate",
  ]) {
    if (!cases.has(required)) failures.push(`${canonicalEvidencePath}: missing case ${required}`);
  }
}

if (failures.length > 0) {
  console.error("forum approved reply Search verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum approved reply Search verified");
