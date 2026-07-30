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

function requireAll(source, markers, label) {
  for (const marker of markers) requireMarker(source, marker, label);
}

function rejectAll(source, markers, label) {
  for (const marker of markers) rejectMarker(source, marker, label);
}

function requireOrder(source, first, second, label) {
  const firstIndex = source.indexOf(first);
  const secondIndex = source.indexOf(second);
  if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
    failures.push(`${label}: expected ${first} before ${second}`);
  }
}

const sourcePath = "crates/rustok-forum/src/search_projection.rs";
const authorPath = "crates/rustok-forum/src/search_projection_author.rs";
const forumLibPath = "crates/rustok-forum/src/lib.rs";
const inboxPath = "crates/rustok-search/src/forum_inbox.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const profilesPath = "crates/rustok-profiles/src/presentation.rs";
const profilesLibPath = "crates/rustok-profiles/src/lib.rs";
const profilesMutationPath = "crates/rustok-profiles/src/graphql/mutation.rs";
const profilesUpdatedEventPath = "crates/rustok-profiles/src/profile_updated_event.rs";
const profilesContentWritePath = "crates/rustok-profiles/src/content_write.rs";
const profilesHandleWritePath = "crates/rustok-profiles/src/handle_write.rs";
const profilesLocaleWritePath = "crates/rustok-profiles/src/locale_write.rs";
const profilesMediaWritePath = "crates/rustok-profiles/src/media_write.rs";
const profilesUpsertWritePath = "crates/rustok-profiles/src/upsert_write.rs";
const profilesVisibilityWritePath = "crates/rustok-profiles/src/visibility_write.rs";
const profilesErrorPath = "crates/rustok-profiles/src/error.rs";
const profilesCliBackfillPath = "crates/rustok-profiles/cli/src/lib.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-public-author-summary.json";
const notePath = "crates/rustok-forum/docs/forum-23a-search-public-author-summary.md";

const source = read(sourcePath);
const author = read(authorPath);
const forumLib = read(forumLibPath);
const inbox = read(inboxPath);
const ingestion = read(ingestionPath);
const profiles = read(profilesPath);
const profilesLib = read(profilesLibPath);
const profilesMutation = read(profilesMutationPath);
const profilesUpdatedEvent = read(profilesUpdatedEventPath);
const profilesContentWrite = read(profilesContentWritePath);
const profilesHandleWrite = read(profilesHandleWritePath);
const profilesLocaleWrite = read(profilesLocaleWritePath);
const profilesMediaWrite = read(profilesMediaWritePath);
const profilesUpsertWrite = read(profilesUpsertWritePath);
const profilesVisibilityWrite = read(profilesVisibilityWritePath);
const profilesError = read(profilesErrorPath);
const profilesCliBackfill = read(profilesCliBackfillPath);
const note = read(notePath);
let contract = null;
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}

requireAll(
  author,
  [
    "ProfilePresentationService::new(db.clone())",
    ".find_profile_summary(tenant_id, author_id, Some(locale), None)",
    "public_author_payload",
    '"user_id": summary.user_id',
    '"handle": summary.handle',
    '"display_name": summary.display_name',
    '"avatar_media_id": summary.avatar_media_id',
    "public_author_payload_exposes_only_the_safe_summary",
    "absent_or_denied_author_is_not_serialized",
  ],
  authorPath,
);
rejectAll(
  author,
  [
    '"tags": summary.tags',
    '"preferred_locale": summary.preferred_locale',
    '"visibility": summary.visibility',
  ],
  authorPath,
);

requireAll(
  source,
  [
    "load_public_author_summary",
    "handle: author_handle",
    "public_author_keywords",
    '"author": author_payload',
    '"author_id": author_id',
    '"has_public_author": author_id.is_some()',
  ],
  sourcePath,
);
rejectAll(
  source,
  ['"author_id": topic.author_id', '"author_id": reply.author_id'],
  sourcePath,
);
requireMarker(forumLib, "mod search_projection_author;", forumLibPath);

requireAll(
  profiles,
  [
    "ProfilePrivacyService::new(self.db.clone())",
    "ProfileAccessAudience::Anonymous",
    "ProfilePrivacyDecision::Allow",
  ],
  profilesPath,
);

requireAll(
  profilesLib,
  [
    "mod content_write;",
    "mod handle_write;",
    "mod locale_write;",
    "mod media_write;",
    "mod profile_updated_event;",
    "mod upsert_write;",
    "mod visibility_write;",
    "pub use upsert_write::backfill_profile_with_event;",
  ],
  profilesLibPath,
);

requireAll(
  profilesMutation,
  [
    "upsert_profile_with_event(",
    "update_profile_content_with_event(",
    "update_profile_handle_with_event(",
    "update_profile_locale_with_event(",
    "update_profile_media_with_event(",
    "update_profile_visibility_with_event(",
    "validate_profile_media_references(",
    "ProfileError::EventPublishUnavailable",
  ],
  profilesMutationPath,
);
rejectAll(
  profilesMutation,
  [
    "service.upsert_profile(",
    "service.update_profile_content(",
    "service.update_profile_handle(",
    "service.update_profile_locale(",
    "service.update_profile_media(",
    "service.update_profile_visibility(",
    "async fn publish_profile_updated(",
    "DomainEvent::ProfileUpdated",
    "use rustok_events::DomainEvent;",
  ],
  profilesMutationPath,
);
requireOrder(
  profilesMutation,
  "validate_profile_media_references(",
  "upsert_profile_with_event(",
  profilesMutationPath,
);

requireAll(
  profilesUpdatedEvent,
  [
    "publish_profile_updated_in_tx",
    "publish_profile_updated_with_actor_in_tx",
    "actor_id: Option<Uuid>",
    "profile: &entities::profile::Model",
    ".publish_in_tx(",
    "DomainEvent::ProfileUpdated",
    "ProfileOperation::PublishUpdatedEvent",
    "ProfileError::EventPublishUnavailable",
    "Profile update event publication failed",
  ],
  profilesUpdatedEventPath,
);
rejectMarker(profilesUpdatedEvent, "profile: &ProfileRecord", profilesUpdatedEventPath);
requireOrder(
  profilesUpdatedEvent,
  "publish_profile_updated_in_tx",
  "publish_profile_updated_with_actor_in_tx",
  profilesUpdatedEventPath,
);

function verifyTransactionalHelper(sourceText, helperPath, markers, writeMarker, logMarker) {
  requireAll(
    sourceText,
    [
      "db.begin().await?",
      ...markers,
      writeMarker,
      "publish_profile_updated_in_tx",
      "txn.rollback().await?",
      "txn.commit().await?",
      logMarker,
    ],
    helperPath,
  );
  requireOrder(sourceText, writeMarker, "publish_profile_updated_in_tx", helperPath);
  requireOrder(sourceText, "publish_profile_updated_in_tx", "txn.commit().await?", helperPath);
}

verifyTransactionalHelper(
  profilesContentWrite,
  profilesContentWritePath,
  [
    "ProfileService::normalize_display_name(display_name)?",
    "Column::ProfileUserId.eq(user_id)",
    "active.display_name = Set(display_name)",
    "active.bio = Set(bio.map(str::to_string))",
    ".insert(&txn)",
  ],
  ".update(&txn).await?",
  "Profile content event publication failed; rolling back owner write",
);

verifyTransactionalHelper(
  profilesHandleWrite,
  profilesHandleWritePath,
  [
    "ProfileService::normalize_handle(handle)?",
    "Column::Handle.eq(handle.clone())",
    "ProfileError::DuplicateHandle(handle)",
  ],
  ".update(&txn).await?",
  "Profile handle event publication failed; rolling back owner write",
);

verifyTransactionalHelper(
  profilesLocaleWrite,
  profilesLocaleWritePath,
  [
    "ProfileService::normalize_locale(preferred_locale)?",
    "ProfileService::normalize_locale(tenant_default_locale)?",
    "active.preferred_locale = Set(preferred_locale)",
    "selection policy only",
  ],
  ".update(&txn).await?",
  "Profile locale event publication failed; rolling back owner write",
);
rejectMarker(profilesLocaleWrite, "profile_translation", profilesLocaleWritePath);

verifyTransactionalHelper(
  profilesMediaWrite,
  profilesMediaWritePath,
  [
    "active.avatar_media_id = Set(avatar_media_id)",
    "active.banner_media_id = Set(banner_media_id)",
  ],
  ".update(&txn).await?",
  "Profile media event publication failed; rolling back owner write",
);

verifyTransactionalHelper(
  profilesVisibilityWrite,
  profilesVisibilityWritePath,
  ["active.visibility = Set(visibility)"],
  ".update(&txn).await?",
  "Profile visibility event publication failed; rolling back owner write",
);

requireAll(
  profilesUpsertWrite,
  [
    "pub async fn backfill_profile_with_event(",
    "ExistingProfilePolicy::Update",
    "ExistingProfilePolicy::Skip",
    "existing_policy == ExistingProfilePolicy::Skip && existing.is_some()",
    "ProfileService::normalize_handle(&handle)?",
    "ProfileService::normalize_display_name(&display_name)?",
    "ProfileService::normalize_locale(preferred_locale.as_deref())?",
    "db.begin().await?",
    "Column::TenantId.eq(tenant_id)",
    "Column::Handle.eq(handle.clone())",
    "ProfileError::DuplicateHandle(handle)",
    "entities::profile::ActiveModel",
    "entities::profile_translation::Entity::find()",
    "entities::profile_tag::Entity::delete_many()",
    "ensure_terms_for_module_in_tx",
    "TaxonomyTermKind::Tag",
    ".insert(&txn)",
    "publish_profile_updated_in_tx",
    "publish_profile_updated_with_actor_in_tx",
    "txn.rollback().await?",
    "txn.commit().await?",
    "Profile upsert event publication failed; rolling back owner write",
  ],
  profilesUpsertWritePath,
);
requireOrder(
  profilesUpsertWrite,
  "existing_policy == ExistingProfilePolicy::Skip && existing.is_some()",
  "let handle_owner = entities::profile::Entity::find()",
  profilesUpsertWritePath,
);
requireOrder(
  profilesUpsertWrite,
  "entities::profile::ActiveModel",
  "publish_profile_updated_in_tx",
  profilesUpsertWritePath,
);
requireOrder(
  profilesUpsertWrite,
  "entities::profile_translation::Entity::find()",
  "publish_profile_updated_in_tx",
  profilesUpsertWritePath,
);
requireOrder(
  profilesUpsertWrite,
  "entities::profile_tag::Entity::delete_many()",
  "publish_profile_updated_in_tx",
  profilesUpsertWritePath,
);
requireOrder(
  profilesUpsertWrite,
  "publish_profile_updated_with_actor_in_tx",
  "txn.commit().await?",
  profilesUpsertWritePath,
);

requireAll(
  profilesCliBackfill,
  [
    "let emit_events = !dry_run;",
    "TransactionalEventBus::new(",
    "OutboxTransport::new(db.clone())",
    "backfill_profile_with_event(",
    "let event_published = result.created;",
    "published_events += 1;",
    '"event_published": false',
    '"emit_events": emit_events',
  ],
  profilesCliBackfillPath,
);
rejectAll(
  profilesCliBackfill,
  [
    'flag(options, "emit_events")',
    "DomainEvent::ProfileUpdated",
    "use rustok_events::DomainEvent;",
    "BACKFILL_EVENT_PUBLISH_ERROR",
    "if let Some(bus)",
    "bus.publish(",
    ".backfill_profile(",
  ],
  profilesCliBackfillPath,
);
requireOrder(
  profilesCliBackfill,
  "if dry_run",
  "backfill_profile_with_event(",
  profilesCliBackfillPath,
);

requireAll(
  profilesError,
  [
    "EventPublishUnavailable",
    '"profiles.event_publish_unavailable"',
    "Self::PresentationUnavailable | Self::EventPublishUnavailable | Self::Database(_)",
  ],
  profilesErrorPath,
);

requireAll(
  inbox,
  [
    "Author(Uuid)",
    "DomainEvent::ProfileUpdated { user_id, .. }",
    "AUTHOR_SCOPE_PREFIX",
    "scope_key.starts_with(AUTHOR_SCOPE_PREFIX)",
    "profile_changes_have_redaction_barrier_scope",
  ],
  inboxPath,
);
rejectMarker(inbox, "DomainEvent::UserDeleted", inboxPath);
requireAll(
  ingestion,
  [
    "DomainEvent::ProfileUpdated { .. }",
    '"rebuild_forum_author_projection"',
    "projector.rebuild_tenant(envelope.tenant_id).await",
  ],
  ingestionPath,
);
rejectMarker(ingestion, "DomainEvent::UserDeleted", ingestionPath);

requireAll(
  note,
  [
    "FORUM-23A7",
    "ProfilePresentationService",
    "forum_author:<user_id>",
    "raw `payload.author_id`",
    "owner-issued monotonic",
    "upsert_my_profile",
    "CLI backfill",
    "system actor",
    "--emit-events",
    "direct non-event",
    "does not treat `UserDeleted`",
    "Not run by the implementation agent",
  ],
  notePath,
);

if (contract) {
  if (contract.task !== "FORUM-23A") failures.push(`${contractPath}: unexpected task`);
  if (contract.latest_slice !== "FORUM-23A7") {
    failures.push(`${contractPath}: unexpected latest slice`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
  }
  if (contract.profiles_upsert_event_owner !== profilesUpsertWritePath) {
    failures.push(`${contractPath}: unexpected upsert event owner`);
  }
  if (contract.profiles_cli_backfill_owner !== profilesCliBackfillPath) {
    failures.push(`${contractPath}: unexpected CLI backfill owner`);
  }

  for (const key of [
    "anonymous_profiles_presentation_is_authoritative",
    "forum_does_not_copy_profile_privacy_policy",
    "missing_or_denied_profile_projects_null_author",
    "profile_owner_failure_is_retryable_not_empty",
    "raw_topic_author_id_is_not_serialized",
    "raw_reply_author_id_is_not_serialized",
    "profile_tags_are_not_serialized",
    "preferred_locale_is_not_serialized",
    "profile_visibility_is_not_serialized",
    "public_handle_populates_search_handle",
    "public_handle_and_display_name_extend_keywords",
    "author_filter_value_is_populated_only_for_public_author",
  ]) {
    if (contract.projection_boundary?.[key] !== true) {
      failures.push(`${contractPath}: projection boundary ${key} drift`);
    }
  }

  for (const key of [
    "profile_updated_is_durable",
    "profile_updated_is_emitted_after_owner_write",
    "transactional_profile_event_publisher_is_shared",
    "transactional_profile_event_publisher_accepts_owner_model",
    "transactional_profile_event_publisher_accepts_system_actor",
    "profile_visibility_write_and_event_are_atomic",
    "visibility_event_failure_rolls_back_owner_write",
    "profile_handle_write_and_event_are_atomic",
    "handle_event_failure_rolls_back_owner_write",
    "profile_content_write_and_event_are_atomic",
    "content_event_failure_rolls_back_owner_write",
    "profile_locale_write_and_event_are_atomic",
    "locale_event_failure_rolls_back_owner_write",
    "locale_write_does_not_create_translations",
    "profile_media_write_and_event_are_atomic",
    "media_event_failure_rolls_back_owner_write",
    "profile_upsert_write_and_event_are_atomic",
    "upsert_event_failure_rolls_back_owner_write",
    "upsert_couples_profile_translation_tags_and_event",
    "upsert_media_validation_precedes_owner_transaction",
    "profiles_cli_backfill_creation_and_event_are_atomic",
    "profiles_cli_backfill_event_failure_rolls_back_owner_write",
    "profiles_cli_backfill_requires_event_for_non_dry_run",
    "profiles_cli_backfill_dry_run_emits_no_event",
    "profiles_cli_backfill_skips_concurrent_existing_profile",
    "remaining_profile_summary_write_and_event_pairs_are_atomic",
    "author_scope_is_tenant_and_user_scoped",
    "author_scope_is_not_suppressed_by_forum_wall_clock_watermark",
    "author_change_rebuilds_current_forum_owner_state",
    "projection_failure_remains_retryable",
    "tenant_advisory_lock_is_preserved",
  ]) {
    if (contract.redaction_boundary?.[key] !== true) {
      failures.push(`${contractPath}: redaction boundary ${key} drift`);
    }
  }
  if (contract.redaction_boundary?.schema_migration_added !== false) {
    failures.push(`${contractPath}: schema migration must remain absent`);
  }

  for (const key of [
    "forum_graphql_changed",
    "forum_rest_changed",
    "search_query_api_changed",
    "search_document_schema_changed",
    "forum_owner_storage_changed",
    "profiles_owner_storage_changed",
    "cargo_lock_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${contractPath}: compatibility ${key} must remain false`);
    }
  }

  for (const nonClaim of [
    "account deletion redaction is complete",
    "all Profiles owner writes trigger durable Forum Search invalidation",
    "direct non-event ProfileService mutation APIs are production-inaccessible",
  ]) {
    if (!contract.non_claims?.includes(nonClaim)) {
      failures.push(`${contractPath}: missing non-claim ${nonClaim}`);
    }
  }
  if (
    !contract.remaining_scope?.includes(
      "consolidate or restrict direct non-event ProfileService mutation APIs",
    )
  ) {
    failures.push(`${contractPath}: missing direct service mutation debt`);
  }
  if (contract.downstream_task !== "FORUM-23B") {
    failures.push(`${contractPath}: unexpected downstream task`);
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${contractPath}: execution status drift`);
  }
}

if (failures.length > 0) {
  console.error("Forum public author Search projection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum public author Search projection verification passed.");
