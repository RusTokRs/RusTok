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

const registryPath = "crates/rustok-search/src/projection_source.rs";
const projectorPath = "crates/rustok-search/src/forum_projector.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const searchLibPath = "crates/rustok-search/src/lib.rs";
const providerPath = "crates/rustok-forum/src/search_projection.rs";
const forumLibPath = "crates/rustok-forum/src/lib.rs";
const forumCargoPath = "crates/rustok-forum/Cargo.toml";
const contractPath = "crates/rustok-forum/contracts/forum-search-projection.json";
const upstreamPath = "crates/rustok-forum/contracts/forum-public-discovery-seo.json";
const notePath = "crates/rustok-forum/docs/forum-20bj-search-projection.md";

const registry = read(registryPath);
const projector = read(projectorPath);
const ingestion = read(ingestionPath);
const searchLib = read(searchLibPath);
const provider = read(providerPath);
const forumLib = read(forumLibPath);
const forumCargo = read(forumCargoPath);
const note = read(notePath);
let contract = null;
let upstream = null;
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}
try {
  upstream = JSON.parse(read(upstreamPath));
} catch (error) {
  failures.push(`${upstreamPath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "pub struct SearchProjectionDocument",
  "pub trait SearchProjectionSource",
  "pub trait SearchProjectionSourceFactory",
  "pub struct SearchProjectionSourceRegistry",
  "MAX_SEARCH_PROJECTION_PAGE_SIZE",
  "already registered",
  "register_search_projection_source",
]) {
  requireMarker(registry, marker, registryPath);
}

for (const marker of [
  "ForumPublicDiscoveryService",
  "forum_category_translation::Entity::find()",
  "forum_topic_translation::Entity::find()",
  "get_public_category_with_locale_fallback",
  "get_public_topic_with_locale_fallback",
  "ProjectionCursor",
  "MAX_ENTITY_LOCALES",
  'const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category"',
  'const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic"',
]) {
  requireMarker(provider, marker, providerPath);
}
for (const forbidden of [
  "ForumAudienceEvaluator",
  "forum_category_audience_policies",
  "forum_topic_audience_policies",
  "SecurityContext::system()",
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
  "mod forum_projector;",
  "pub mod projection_source;",
  "search_projection_source_registry_from_extensions",
  "SearchIngestionHandler::with_forum_source",
]) {
  requireMarker(searchLib, marker, searchLibPath);
}

for (const marker of [
  "mod search_projection;",
  "register_search_projection_source",
  "ForumSearchProjectionSourceFactory",
  '&["content", "taxonomy"]',
]) {
  requireMarker(forumLib, marker, forumLibPath);
}
requireMarker(forumCargo, "rustok-search.workspace = true", forumCargoPath);

for (const marker of [
  "temporary staging table",
  "explicit Forum reindex",
  "FORUM-20BK",
  "projection invalidation events",
  "Cargo.lock",
  "does not declare a hard module runtime dependency",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BJ") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BI") {
    failures.push(`${contractPath}: unexpected upstream task`);
  }
  if (contract.downstream_task !== "FORUM-20BK") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
  for (const key of [
    "category_candidates_use_exact_public_discovery",
    "topic_candidates_use_exact_public_discovery",
    "cross_consumer_audience_policy_copy_added",
    "per_entity_locale_fanout_bounded",
  ]) {
    const expected = key === "cross_consumer_audience_policy_copy_added" ? false : true;
    if (contract.source_boundary?.[key] !== expected) {
      failures.push(`${contractPath}: source boundary ${key} drift`);
    }
  }
  for (const key of [
    "search_owns_projection_storage",
    "explicit_forum_rebuild_uses_postgresql_temporary_stage",
    "explicit_forum_rebuild_replaces_scope_after_successful_scan",
    "explicit_forum_rebuild_source_failure_keeps_previous_scope",
    "target_refresh_deletes_and_reinserts_in_one_transaction",
    "denied_closed_missing_or_deleted_target_removes_stale_documents",
  ]) {
    if (!contract.persistence_boundary?.[key]) {
      failures.push(`${contractPath}: persistence boundary must lock ${key}`);
    }
  }
  if (contract.persistence_boundary?.full_search_rebuild_source_failure_keeps_previous_forum_scope !== false) {
    failures.push(`${contractPath}: cross-source rebuild limitation must remain explicit`);
  }
  for (const key of [
    "forum_topic_created_refreshes_topic",
    "forum_topic_replied_refreshes_topic",
    "forum_topic_status_changed_refreshes_topic",
    "forum_topic_pinned_refreshes_topic",
    "forum_reply_status_changed_refreshes_topic",
    "forum_module_enable_rebuilds_scope",
    "forum_module_disable_deletes_scope",
    "explicit_forum_scope_reindex_supported",
    "explicit_forum_category_reindex_supported",
    "explicit_forum_topic_reindex_supported",
  ]) {
    if (!contract.ingestion_boundary?.[key]) {
      failures.push(`${contractPath}: ingestion boundary must lock ${key}`);
    }
  }
  for (const key of [
    "automatic_category_policy_change_reindex_added",
    "automatic_topic_policy_change_reindex_added",
    "automatic_topic_content_translation_tag_solution_change_reindex_added",
    "automatic_category_content_translation_tree_change_reindex_added",
  ]) {
    if (contract.ingestion_boundary?.[key] !== false) {
      failures.push(`${contractPath}: ${key} must remain explicit downstream scope`);
    }
  }
  if (contract.compatibility?.forum_module_declares_core_search_dependency !== false) {
    failures.push(`${contractPath}: Search must remain an optional runtime consumer`);
  }
  if (contract.compatibility?.forum_runtime_works_without_search_listener !== true) {
    failures.push(`${contractPath}: Forum runtime independence must be locked`);
  }
  if (contract.compatibility?.cargo_lock_regenerated !== false) {
    failures.push(`${contractPath}: lockfile handoff must remain explicit`);
  }
  if (contract.compatibility?.migration_added !== false) {
    failures.push(`${contractPath}: migration must remain absent`);
  }
}

if (upstream) {
  if (upstream.search_boundary?.forum_projection_consumer_wired !== true) {
    failures.push(`${upstreamPath}: projection consumer handoff not advanced`);
  }
  if (upstream.search_boundary?.forum_search_documents_written !== true) {
    failures.push(`${upstreamPath}: Search document handoff not advanced`);
  }
  if (upstream.search_boundary?.completion_contract !== contractPath) {
    failures.push(`${upstreamPath}: completion contract drift`);
  }
  if (upstream.downstream_task !== "FORUM-20BK") {
    failures.push(`${upstreamPath}: downstream task drift`);
  }
}

if (failures.length > 0) {
  console.error("forum Search projection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Search projection composition verified");
