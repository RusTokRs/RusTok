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

const outboxBusPath = "crates/rustok-outbox/src/transactional.rs";
const outboxTransportPath = "crates/rustok-outbox/src/transport.rs";
const helperPath = "crates/rustok-forum/src/services/projection_invalidation.rs";
const servicesModPath = "crates/rustok-forum/src/services/mod.rs";
const categoryProjectionPath = "crates/rustok-forum/src/services/category_projection_owner.rs";
const categoryCommandPath = "crates/rustok-forum/src/services/category_command_owner.rs";
const categoryLifecyclePath = "crates/rustok-forum/src/services/category_lifecycle_owner.rs";
const categoryVisibilityPath = "crates/rustok-forum/src/services/category_visibility.rs";
const categoryAudiencePath = "crates/rustok-forum/src/services/category_audience_owner.rs";
const topicAudiencePath = "crates/rustok-forum/src/services/topic_audience_owner.rs";
const topicInlinePath = "crates/rustok-forum/src/services/topic_inline.rs";
const topicOwnerPath = "crates/rustok-forum/src/services/topic_owner.rs";
const replyOwnerPath = "crates/rustok-forum/src/services/reply_owner.rs";
const moderationOwnerPath = "crates/rustok-forum/src/services/moderation_owner.rs";
const moderationPublicPath = "crates/rustok-forum/src/services/moderation_public_owner.rs";
const searchContractPath = "crates/rustok-forum/contracts/forum-search-projection.json";
const contractPath = "crates/rustok-forum/contracts/forum-projection-invalidation.json";
const notePath = "crates/rustok-forum/docs/forum-20bk-projection-invalidation.md";

const outboxBus = read(outboxBusPath);
const outboxTransport = read(outboxTransportPath);
const helper = read(helperPath);
const servicesMod = read(servicesModPath);
const categoryProjection = read(categoryProjectionPath);
const categoryCommand = read(categoryCommandPath);
const categoryLifecycle = read(categoryLifecyclePath);
const categoryVisibility = read(categoryVisibilityPath);
const categoryAudience = read(categoryAudiencePath);
const topicAudience = read(topicAudiencePath);
const topicInline = read(topicInlinePath);
const topicOwner = read(topicOwnerPath);
const replyOwner = read(replyOwnerPath);
const moderationOwner = read(moderationOwnerPath);
const moderationPublic = read(moderationPublicPath);
const note = read(notePath);
let searchContract = null;
let contract = null;
try {
  searchContract = JSON.parse(read(searchContractPath));
} catch (error) {
  failures.push(`${searchContractPath}: invalid JSON: ${error.message}`);
}
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "pub async fn publish_root_in_tx<C>",
  "validate_event(&event)?",
  "OutboxTransport::write_envelope_in_tx",
  "EventEnvelope::new(tenant_id, actor_id, event)",
]) {
  requireMarker(outboxBus, marker, outboxBusPath);
}
for (const marker of [
  "pub async fn write_envelope_in_tx<C>",
  "Self::model_from_envelope(envelope)?",
  "validate_registered_schema()",
  "pub async fn write_contract_envelope_in_tx<C>",
]) {
  requireMarker(outboxTransport, marker, outboxTransportPath);
}

for (const marker of [
  'FORUM_PROJECTION_SCOPE: &str = "forum"',
  'FORUM_CATEGORY_PROJECTION_TARGET: &str = "forum_category"',
  'FORUM_TOPIC_PROJECTION_TARGET: &str = "forum_topic"',
  "TransactionalEventBus::publish_root_in_tx",
  "DomainEvent::ReindexRequested",
  "DatabaseBackend::Postgres",
  "event.validate()",
  "publish_forum_projection_scope_direct_in_tx",
  "publish_forum_category_projection_in_tx",
  "publish_forum_topic_projection_in_tx",
]) {
  requireMarker(helper, marker, helperPath);
}
for (const forbidden of [
  "ForumProjectionInvalidated",
  "INSERT INTO sys_events",
  "EventEnvelope::new",
]) {
  rejectMarker(helper, forbidden, helperPath);
}

for (const [source, label, markers] of [
  [categoryProjection, categoryProjectionPath, [
    "pub(super) async fn create",
    "pub(super) async fn update",
    "publish_forum_projection_scope_direct_in_tx",
  ]],
  [categoryCommand, categoryCommandPath, [
    "move_category",
    "reorder_siblings",
    "publish_forum_projection_scope_direct_in_tx",
  ]],
  [categoryLifecycle, categoryLifecyclePath, [
    "archive_subtree_for_delete",
    "restore_subtree",
    "if !changed.is_empty()",
    "publish_forum_projection_scope_direct_in_tx",
  ]],
  [categoryVisibility, categoryVisibilityPath, [
    "SetForumCategoryVisibilityPolicyInput",
    "publish_forum_projection_scope_direct_in_tx",
  ]],
  [categoryAudience, categoryAudiencePath, [
    "ForumCategoryAudiencePolicyOwnerService",
    "load_category_audience_policy",
    "publish_forum_projection_scope_direct_in_tx",
  ]],
]) {
  for (const marker of markers) requireMarker(source, marker, label);
}

for (const marker of [
  "ForumTopicAudiencePolicyOwnerService",
  "load_policy_for_topic",
  "publish_forum_topic_projection_direct_in_tx",
]) {
  requireMarker(topicAudience, marker, topicAudiencePath);
}
for (const marker of [
  "ForumTopicCreated",
  "publish_forum_category_projection_in_tx",
  "publish_forum_topic_projection_in_tx",
]) {
  requireMarker(topicInline, marker, topicInlinePath);
}
for (const marker of [
  "publish_forum_topic_projection_in_tx",
  "publish_forum_category_projection_in_tx",
  "topic.status != TopicStatus::Archived",
]) {
  requireMarker(topicOwner, marker, topicOwnerPath);
}
for (const marker of [
  "status == ReplyStatus::Approved",
  "publish_forum_category_projection_in_tx",
  "DomainEvent::ForumReplyStatusChanged",
]) {
  requireMarker(replyOwner, marker, replyOwnerPath);
}
for (const marker of [
  "set_topic_locked",
  "mark_solution_with_optional_audience_context",
  "clear_solution_with_optional_audience_context",
  "publish_forum_topic_projection_in_tx",
  "changed_category_id",
  "publish_forum_category_projection_in_tx",
]) {
  requireMarker(moderationOwner, marker, moderationOwnerPath);
}

for (const marker of [
  "mod moderation_legacy;",
  "mod moderation_owner;",
  "mod moderation_public_owner;",
  "pub use super::moderation_public_owner::ModerationService;",
  "ForumCategoryAudiencePolicyOwnerService as ForumCategoryAudiencePolicyService",
  "ForumTopicAudiencePolicyOwnerService as ForumTopicAudiencePolicyService",
  'include!("category_projection_owner.rs")',
  'include!("category_command_owner.rs")',
  'include!("category_lifecycle_owner.rs")',
]) {
  requireMarker(servicesMod, marker, servicesModPath);
}
for (const forbidden of [
  "__ModerationServiceLegacyTarget",
  "pub use super::moderation_owner::ModerationService",
]) {
  rejectMarker(servicesMod, forbidden, servicesModPath);
}
for (const marker of [
  "pub struct ModerationService",
  "inner: super::moderation_owner::ModerationService",
  "pub async fn pin_topic",
  "pub async fn lock_topic",
  "pub async fn mark_solution",
]) {
  requireMarker(moderationPublic, marker, moderationPublicPath);
}
rejectMarker(moderationPublic, "impl Deref", moderationPublicPath);

for (const marker of [
  "search.reindex_requested",
  "owner transaction",
  "PostgreSQL-only",
  "validation only",
  "at-least-once",
  "Full Forum scope",
  "FORUM-20BL",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BK") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BJ") {
    failures.push(`${contractPath}: unexpected upstream task`);
  }
  if (contract.downstream_task !== "FORUM-20BL") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
  if (contract.root_event !== "rustok_events::DomainEvent::ReindexRequested") {
    failures.push(`${contractPath}: root event drift`);
  }
  for (const key of [
    "owner_transaction_required_on_postgresql",
    "postgresql_direct_owner_invalidations_persisted",
    "non_postgresql_direct_owner_invalidations_validation_only",
    "domain_event_validation_preserved",
    "registered_envelope_schema_validation_preserved_for_persisted_events",
    "canonical_sys_events_outbox_reused",
    "duplicate_invalidations_allowed",
    "consumer_operations_are_idempotent",
  ]) {
    if (contract.outbox_boundary?.[key] !== true) {
      failures.push(`${contractPath}: outbox boundary ${key} drift`);
    }
  }
  for (const key of [
    "non_postgresql_forum_search_projector_supported",
    "new_root_domain_event_added",
    "new_event_schema_added",
    "sql_trigger_event_envelope_copy_added",
    "second_database_connection_required",
  ]) {
    if (contract.outbox_boundary?.[key] !== false) {
      failures.push(`${contractPath}: outbox boundary ${key} must remain false`);
    }
  }
  for (const section of [
    "forum_scope_invalidation",
    "category_target_invalidation",
    "topic_target_invalidation",
    "owner_sealing",
  ]) {
    if (!contract[section]) failures.push(`${contractPath}: missing ${section}`);
  }
  for (const [key, expected] of Object.entries({
    existing_reindex_target_strings_changed: false,
    search_ingestion_contract_changed: false,
    workspace_dependency_changed: false,
    cargo_lock_changed: false,
    migration_added: false,
    sqlite_forum_domain_fixtures_require_outbox_migration: false,
  })) {
    if (contract.compatibility?.[key] !== expected) {
      failures.push(`${contractPath}: compatibility ${key} drift`);
    }
  }
}

if (searchContract) {
  if (searchContract.invalidation_contract !== contractPath) {
    failures.push(`${searchContractPath}: invalidation contract handoff drift`);
  }
  if (searchContract.ingestion_boundary?.completion_contract !== contractPath) {
    failures.push(`${searchContractPath}: completion contract drift`);
  }
  for (const key of [
    "automatic_category_policy_change_reindex_added",
    "automatic_topic_policy_change_reindex_added",
    "automatic_topic_content_translation_tag_solution_change_reindex_added",
    "automatic_category_content_translation_tree_change_reindex_added",
    "automatic_category_and_reply_count_reindex_added",
    "owner_transactional_outbox_delivery_added",
  ]) {
    if (searchContract.ingestion_boundary?.[key] !== true) {
      failures.push(`${searchContractPath}: ${key} handoff drift`);
    }
  }
  if (searchContract.downstream_task !== "FORUM-20BL") {
    failures.push(`${searchContractPath}: downstream task drift`);
  }
}

if (failures.length > 0) {
  console.error("forum projection invalidation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum projection invalidation ownership verified");
