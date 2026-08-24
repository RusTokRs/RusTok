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

const capabilityPath = "crates/rustok-core/src/search_projection.rs";
const searchFacadePath = "crates/rustok-search/src/projection_source.rs";
const projectorPath = "crates/rustok-search/src/forum_projector.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const searchLibPath = "crates/rustok-search/src/lib.rs";
const replyAudiencePath = "crates/rustok-forum/src/services/reply_audience_read.rs";
const publicDiscoveryPath = "crates/rustok-forum/src/services/public_discovery.rs";
const providerPath = "crates/rustok-forum/src/search_projection.rs";
const replyUpdatePath = "crates/rustok-forum/src/services/reply_inline.rs";
const forumLibPath = "crates/rustok-forum/src/lib.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-projection.json";
const invalidationPath = "crates/rustok-forum/contracts/forum-projection-invalidation.json";
const visibilityBulkPath = "crates/rustok-forum/contracts/forum-visibility-scoped-bulk-read.json";
const rebuildPreservationPath = "crates/rustok-forum/contracts/forum-search-rebuild-scope-preservation.json";
const approvedReplyPath = "crates/rustok-forum/contracts/forum-approved-reply-search.json";
const upstreamPath = "crates/rustok-forum/contracts/forum-public-discovery-seo.json";
const notePath = "crates/rustok-forum/docs/forum-20bj-search-projection.md";

const capability = read(capabilityPath);
const searchFacade = read(searchFacadePath);
const projector = read(projectorPath);
const ingestion = read(ingestionPath);
const searchLib = read(searchLibPath);
const replyAudience = read(replyAudiencePath);
const publicDiscovery = read(publicDiscoveryPath);
const provider = read(providerPath);
const replyUpdate = read(replyUpdatePath);
const forumLib = read(forumLibPath);
const note = read(notePath);
let contract = null;
let invalidation = null;
let visibilityBulk = null;
let rebuildPreservation = null;
let approvedReply = null;
let upstream = null;
for (const [label, source, assign] of [
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [invalidationPath, read(invalidationPath), (value) => { invalidation = value; }],
  [visibilityBulkPath, read(visibilityBulkPath), (value) => { visibilityBulk = value; }],
  [rebuildPreservationPath, read(rebuildPreservationPath), (value) => { rebuildPreservation = value; }],
  [approvedReplyPath, read(approvedReplyPath), (value) => { approvedReply = value; }],
  [upstreamPath, read(upstreamPath), (value) => { upstream = value; }],
]) {
  try {
    assign(JSON.parse(source));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

for (const marker of [
  "pub struct SearchProjectionDocument",
  "pub trait SearchProjectionSource",
  "pub trait SearchProjectionSourceFactory",
  "pub struct SearchProjectionSourceRegistry",
  "MAX_SEARCH_PROJECTION_PAGE_SIZE",
  "already registered",
  "register_search_projection_source",
  "search_projection_source_registry_from_extensions",
]) {
  requireMarker(capability, marker, capabilityPath);
}
for (const forbidden of ["search_documents", "ForumPublicDiscoveryService", "ForumSearchProjector"]) {
  rejectMarker(capability, forbidden, capabilityPath);
}
requireMarker(searchFacade, "pub use rustok_core::search_projection::*;", searchFacadePath);

for (const marker of [
  "pub async fn get_public_storefront_visible_with_locale_fallback",
  "statuses.is_some_and(|allowed| !allowed.contains(&reply.status))",
  ".is_topic_visible(tenant_id, reply.topic_id, channel_slug, &viewer)",
]) {
  requireMarker(replyAudience, marker, replyAudiencePath);
}
for (const marker of [
  "replies: ForumReplyAudienceReadService",
  "pub async fn get_public_reply_with_locale_fallback",
]) {
  requireMarker(publicDiscovery, marker, publicDiscoveryPath);
}

for (const marker of [
  "ForumPublicDiscoveryService",
  "forum_category_taxonomy_binding::Entity::find()",
  "TaxonomyOwnerCategoryReader",
  "projection.available_locales",
  "forum_topic_translation::Entity::find()",
  "forum_reply_body::Entity::find()",
  "get_public_category_with_locale_fallback",
  "get_public_topic_with_locale_fallback",
  "get_public_reply_with_locale_fallback",
  "ProjectionCursor::Reply",
  "ProjectionCandidate::Reply",
  "Some(&[ReplyStatus::Approved])",
  "if reply.effective_locale != locale",
  'const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category"',
  'const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic"',
  'const FORUM_REPLY_ENTITY_TYPE: &str = "forum_reply"',
  'document_key: format!("forum_reply:{reply_id}:{locale}")',
  '"kind": "forum_reply"',
  "rustok_core::search_projection",
]) {
  requireMarker(provider, marker, providerPath);
}
for (const forbidden of [
  "forum_category_translation::Entity::find()",
  "forum_reply::Entity::find()",
  "ForumAudienceEvaluator",
  "forum_category_audience_policies",
  "forum_topic_audience_policies",
  "SecurityContext::system()",
  "rustok_search::",
]) {
  rejectMarker(provider, forbidden, providerPath);
}

for (const marker of [
  "CREATE TEMP TABLE forum_search_projection_stage",
  "ON COMMIT DROP",
  "delete_forum_scope(&tx, tenant_id)",
  "FROM forum_search_projection_stage",
  "refresh_entity",
  "delete_forum_entity",
  "Forum Search projection cursor did not advance",
  "foreign or non-public document",
  "FORUM_TOPIC_ENTITY_TYPE | FORUM_REPLY_ENTITY_TYPE",
  "if entity_type == FORUM_TOPIC_ENTITY_TYPE",
  "return self.rebuild_tenant(tenant_id).await",
  "'forum_category', 'forum_topic', 'forum_reply'",
]) {
  requireMarker(projector, marker, projectorPath);
}

for (const marker of [
  "ForumTopicCreated",
  "ForumTopicReplied",
  "ForumTopicStatusChanged",
  "ForumTopicPinned",
  "ForumReplyStatusChanged",
  '"forum_category"',
  '"forum_topic"',
  "handle_forum_module_toggle",
  "rebuild_forum_scope",
  "delete_forum_scope",
]) {
  requireMarker(ingestion, marker, ingestionPath);
}
for (const marker of [
  'target_type: "forum_topic".to_string()',
  "target_id: Some(topic_id)",
  ".publish_in_tx(",
]) {
  requireMarker(replyUpdate, marker, replyUpdatePath);
}

for (const marker of [
  "mod forum_projector;",
  "pub mod projection_source;",
  "search_projection_source_registry_from_extensions",
  "SearchIngestionHandler::with_forum_source",
]) {
  requireMarker(searchLib, marker, searchLibPath);
}
for (const marker of [
  "mod search_projection;",
  "rustok_core::search_projection::register_search_projection_source",
  "ForumSearchProjectionSourceFactory",
  '&["content", "taxonomy"]',
]) {
  requireMarker(forumLib, marker, forumLibPath);
}
for (const forbidden of [
  "rustok_search::register_search_projection_source",
  '&["content", "taxonomy", "search"]',
]) {
  rejectMarker(forumLib, forbidden, forumLibPath);
}
for (const marker of [
  "rustok-core",
  "temporary staging table",
  "Explicit reindex requests support",
  "FORUM-20BK",
  "projection invalidation events",
  "Cargo.lock is unchanged",
  "does not gain a Search crate or hard",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BJ") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BI") failures.push(`${contractPath}: unexpected upstream task`);
  if (contract.downstream_task !== "FORUM-20BP") failures.push(`${contractPath}: unexpected downstream task`);
  if (contract.invalidation_contract !== invalidationPath) failures.push(`${contractPath}: invalidation handoff drift`);
  if (contract.visibility_scoped_bulk_read_contract !== visibilityBulkPath) failures.push(`${contractPath}: visible bulk handoff drift`);
  if (contract.rebuild_scope_preservation_contract !== rebuildPreservationPath) failures.push(`${contractPath}: rebuild handoff drift`);
  if (contract.approved_reply_search_contract !== approvedReplyPath) failures.push(`${contractPath}: approved reply handoff drift`);
  for (const type of ["forum_category", "forum_topic", "forum_reply"]) {
    if (!contract.entity_types?.includes(type)) failures.push(`${contractPath}: missing entity type ${type}`);
  }
  for (const key of [
    "neutral_capability_has_no_forum_dependency",
    "neutral_capability_has_no_search_storage_or_query_logic",
    "search_facade_reexports_neutral_capability",
    "category_candidates_use_exact_public_discovery",
    "topic_candidates_use_exact_public_discovery",
    "reply_candidates_require_approved_status_and_exact_parent_discovery",
    "reply_topic_and_category_are_reauthorized_for_context",
    "per_entity_locale_fanout_bounded",
  ]) {
    if (contract.source_boundary?.[key] !== true) failures.push(`${contractPath}: source boundary ${key} drift`);
  }
  if (contract.source_boundary?.cross_consumer_audience_policy_copy_added !== false) {
    failures.push(`${contractPath}: audience policy copy must remain absent`);
  }
  for (const key of [
    "search_owns_projection_storage",
    "explicit_forum_rebuild_uses_postgresql_temporary_stage",
    "explicit_forum_rebuild_replaces_scope_after_successful_scan",
    "explicit_forum_rebuild_source_failure_keeps_previous_scope",
    "full_search_rebuild_source_failure_keeps_previous_forum_scope",
    "earlier_successful_scope_may_commit_before_later_failure",
    "target_refresh_deletes_and_reinserts_in_one_transaction",
    "topic_refresh_rebuilds_atomic_forum_scope_with_child_replies",
    "denied_closed_missing_or_deleted_target_removes_stale_documents",
  ]) {
    if (contract.persistence_boundary?.[key] !== true) failures.push(`${contractPath}: persistence boundary ${key} drift`);
  }
  for (const key of ["direct_search_rebuild_deletes_external_scopes", "global_cross_source_atomicity_added"]) {
    if (contract.persistence_boundary?.[key] !== false) failures.push(`${contractPath}: persistence ${key} must remain false`);
  }
  for (const key of [
    "forum_topic_created_refreshes_topic",
    "forum_topic_replied_refreshes_topic_and_reply_scope",
    "forum_topic_status_changed_refreshes_topic_and_reply_scope",
    "forum_topic_pinned_refreshes_topic_and_reply_scope",
    "forum_reply_status_changed_refreshes_topic_and_reply_scope",
    "forum_reply_edit_publishes_topic_reindex",
    "forum_module_enable_rebuilds_scope",
    "forum_module_disable_deletes_scope",
    "explicit_forum_scope_reindex_supported",
    "explicit_forum_category_reindex_supported",
    "explicit_forum_topic_reindex_supported",
    "automatic_category_policy_change_reindex_added",
    "automatic_topic_policy_change_reindex_added",
    "automatic_topic_content_translation_tag_solution_change_reindex_added",
    "automatic_category_content_translation_tree_change_reindex_added",
    "automatic_category_and_reply_count_reindex_added",
    "owner_transactional_outbox_delivery_added",
    "reply_documents_projected",
  ]) {
    if (contract.ingestion_boundary?.[key] !== true) failures.push(`${contractPath}: ingestion ${key} drift`);
  }
  for (const key of ["root_reindex_event_schema_changed", "new_reindex_target_string_added"]) {
    if (contract.ingestion_boundary?.[key] !== false) failures.push(`${contractPath}: ingestion ${key} must remain false`);
  }
  if (contract.ingestion_boundary?.completion_contract !== invalidationPath) failures.push(`${contractPath}: invalidation completion drift`);
  if (contract.downstream_handoff?.completion_contract !== rebuildPreservationPath) failures.push(`${contractPath}: historical rebuild completion drift`);
  if (contract.downstream_handoff?.reply_completion_contract !== approvedReplyPath) failures.push(`${contractPath}: reply completion drift`);
  if (contract.remaining_scope?.includes("decide whether approved public replies become separate Search documents under FORUM-23")) {
    failures.push(`${contractPath}: completed reply decision remains open`);
  }
  for (const [key, expected] of Object.entries({
    forum_to_search_workspace_dependency_added: false,
    forum_module_declares_core_search_dependency: false,
    forum_runtime_works_without_search_listener: true,
    cargo_lock_changed: false,
    cargo_lock_regeneration_required: false,
    migration_added: false,
  })) {
    if (contract.compatibility?.[key] !== expected) failures.push(`${contractPath}: compatibility ${key} drift`);
  }
}

for (const [label, value] of [
  [invalidationPath, invalidation],
  [visibilityBulkPath, visibilityBulk],
  [rebuildPreservationPath, rebuildPreservation],
]) {
  if (!value) continue;
  if (value.downstream_task !== "FORUM-20BP") failures.push(`${label}: downstream task must advance to FORUM-20BP`);
  if (value.approved_reply_search_contract !== approvedReplyPath) failures.push(`${label}: approved reply handoff drift`);
}
if (approvedReply) {
  if (approvedReply.task !== "FORUM-20BO") failures.push(`${approvedReplyPath}: unexpected task`);
  if (approvedReply.upstream_task !== "FORUM-20BN") failures.push(`${approvedReplyPath}: unexpected upstream task`);
  if (approvedReply.downstream_task !== "FORUM-20BP") failures.push(`${approvedReplyPath}: unexpected downstream task`);
  if (approvedReply.reply_selected_read_owner !== replyAudiencePath) failures.push(`${approvedReplyPath}: selected reply owner drift`);
  if (approvedReply.public_discovery_owner !== publicDiscoveryPath) failures.push(`${approvedReplyPath}: public discovery owner drift`);
}
if (upstream) {
  if (upstream.search_boundary?.forum_projection_consumer_wired !== true) failures.push(`${upstreamPath}: projection consumer handoff not advanced`);
  if (upstream.search_boundary?.forum_search_documents_written !== true) failures.push(`${upstreamPath}: Search document handoff not advanced`);
  if (upstream.search_boundary?.completion_contract !== contractPath) failures.push(`${upstreamPath}: completion contract drift`);
  if (upstream.search_boundary?.downstream_workspace_dependency_added !== false) failures.push(`${upstreamPath}: workspace dependency drift`);
  if (upstream.downstream_task !== "FORUM-20BK") failures.push(`${upstreamPath}: historical downstream task drift`);
}

if (failures.length > 0) {
  console.error("forum Search projection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Search projection composition verified");
