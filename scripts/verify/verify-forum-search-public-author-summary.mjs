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

function rejectAll(source, markers, label) {
  for (const marker of markers) rejectMarker(source, marker, label);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing ordered marker ${marker}`);
      return;
    }
    previous = index;
  }
}

const paths = {
  source: "crates/rustok-forum/src/search_projection.rs",
  author: "crates/rustok-forum/src/search_projection_author.rs",
  forumLib: "crates/rustok-forum/src/lib.rs",
  presentation: "crates/rustok-profiles/src/presentation.rs",
  profilesLib: "crates/rustok-profiles/src/lib.rs",
  mutationFacade: "crates/rustok-profiles/src/mutations.rs",
  graphqlMutation: "crates/rustok-profiles/src/graphql/mutation.rs",
  profileEvent: "crates/rustok-profiles/src/profile_updated_event.rs",
  contentWrite: "crates/rustok-profiles/src/content_write.rs",
  handleWrite: "crates/rustok-profiles/src/handle_write.rs",
  localeWrite: "crates/rustok-profiles/src/locale_write.rs",
  mediaWrite: "crates/rustok-profiles/src/media_write.rs",
  visibilityWrite: "crates/rustok-profiles/src/visibility_write.rs",
  upsertWrite: "crates/rustok-profiles/src/upsert_write.rs",
  cli: "crates/rustok-profiles/cli/src/lib.rs",
  accountRedaction: "crates/rustok-profiles/src/account_redaction.rs",
  authDelete: "apps/server/src/services/auth_admin_mutation_provider/user_admin.rs",
  inbox: "crates/rustok-search/src/forum_inbox.rs",
  ingestion: "crates/rustok-search/src/ingestion.rs",
  serviceBoundaryContract:
    "crates/rustok-forum/contracts/forum-search-profile-service-mutation-boundary.json",
  facadeContract:
    "crates/rustok-forum/contracts/forum-search-profile-event-aware-mutation-api.json",
  deprecationContract:
    "crates/rustok-forum/contracts/forum-search-profile-legacy-mutation-deprecation.json",
  deletionContract:
    "crates/rustok-forum/contracts/forum-search-account-deletion-redaction.json",
  contract: "crates/rustok-forum/contracts/forum-search-public-author-summary.json",
  note: "crates/rustok-forum/docs/forum-23a-search-public-author-summary.md",
};

const source = read(paths.source);
const author = read(paths.author);
const forumLib = read(paths.forumLib);
const presentation = read(paths.presentation);
const profilesLib = read(paths.profilesLib);
const mutationFacade = read(paths.mutationFacade);
const graphqlMutation = read(paths.graphqlMutation);
const profileEvent = read(paths.profileEvent);
const contentWrite = read(paths.contentWrite);
const handleWrite = read(paths.handleWrite);
const localeWrite = read(paths.localeWrite);
const mediaWrite = read(paths.mediaWrite);
const visibilityWrite = read(paths.visibilityWrite);
const upsertWrite = read(paths.upsertWrite);
const cli = read(paths.cli);
const accountRedaction = read(paths.accountRedaction);
const authDelete = read(paths.authDelete);
const inbox = read(paths.inbox);
const ingestion = read(paths.ingestion);
const note = read(paths.note);
const serviceBoundaryContract = parseJson(paths.serviceBoundaryContract);
const facadeContract = parseJson(paths.facadeContract);
const deprecationContract = parseJson(paths.deprecationContract);
const deletionContract = parseJson(paths.deletionContract);
const contract = parseJson(paths.contract);

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
  paths.author,
);
rejectAll(
  author,
  [
    '"tags": summary.tags',
    '"preferred_locale": summary.preferred_locale',
    '"visibility": summary.visibility',
    '"bio": summary.bio',
    '"banner_media_id": summary.banner_media_id',
  ],
  paths.author,
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
  paths.source,
);
rejectAll(source, ['"author_id": topic.author_id', '"author_id": reply.author_id'], paths.source);
requireMarker(forumLib, "mod search_projection_author;", paths.forumLib);

requireAll(
  presentation,
  [
    "ProfilePrivacyService::new(self.db.clone())",
    "ProfileAccessAudience::Anonymous",
    "ProfilePrivacyDecision::Allow",
  ],
  paths.presentation,
);

requireAll(
  profilesLib,
  [
    "mod account_redaction;",
    "mod content_write;",
    "mod handle_write;",
    "mod locale_write;",
    "mod media_write;",
    "mod profile_updated_event;",
    "mod upsert_write;",
    "mod visibility_write;",
    "pub use account_redaction::redact_profile_for_account_deactivation_in_tx;",
    "pub use mutations::ProfileMutationService;",
  ],
  paths.profilesLib,
);

const facadeMethods = [
  "upsert_profile_with_event",
  "update_profile_handle_with_event",
  "update_profile_content_with_event",
  "update_profile_locale_with_event",
  "update_profile_visibility_with_event",
  "update_profile_media_with_event",
  "backfill_profile_with_event",
];
for (const method of facadeMethods) {
  requireMarker(mutationFacade, `pub async fn ${method}(`, paths.mutationFacade);
}
requireAll(
  mutationFacade,
  ["db: &'a DatabaseConnection", "event_bus: &'a TransactionalEventBus", "self.event_bus"],
  paths.mutationFacade,
);
requireAll(
  graphqlMutation,
  [
    "ProfileMutationService::new(db, event_bus)",
    "mutations.upsert_profile_with_event(",
    "mutations.update_profile_handle_with_event(",
    "mutations.update_profile_content_with_event(",
    "mutations.update_profile_locale_with_event(",
    "mutations.update_profile_visibility_with_event(",
    "mutations.update_profile_media_with_event(",
    "validate_profile_media_references(",
  ],
  paths.graphqlMutation,
);
rejectAll(
  graphqlMutation,
  ["DomainEvent::ProfileUpdated", "service.upsert_profile(", "service.update_profile_"],
  paths.graphqlMutation,
);

requireAll(
  profileEvent,
  [
    "publish_profile_updated_in_tx",
    "publish_profile_updated_with_actor_in_tx",
    "actor_id: Option<Uuid>",
    ".publish_in_tx(",
    "DomainEvent::ProfileUpdated",
    "ProfileError::EventPublishUnavailable",
  ],
  paths.profileEvent,
);

for (const [helperPath, helper] of [
  [paths.contentWrite, contentWrite],
  [paths.handleWrite, handleWrite],
  [paths.localeWrite, localeWrite],
  [paths.mediaWrite, mediaWrite],
  [paths.visibilityWrite, visibilityWrite],
]) {
  requireAll(
    helper,
    ["db.begin().await?", "publish_profile_updated_in_tx", "txn.rollback().await?", "txn.commit().await?"],
    helperPath,
  );
  requireOrder(helper, ["db.begin().await?", "publish_profile_updated_in_tx", "txn.commit().await?"], helperPath);
}
rejectMarker(localeWrite, "profile_translation", paths.localeWrite);
requireAll(
  upsertWrite,
  [
    "pub async fn backfill_profile_with_event(",
    "ExistingProfilePolicy::Update",
    "ExistingProfilePolicy::Skip",
    "db.begin().await?",
    "entities::profile::ActiveModel",
    "entities::profile_translation::Entity::find()",
    "entities::profile_tag::Entity::delete_many()",
    "publish_profile_updated_in_tx",
    "publish_profile_updated_with_actor_in_tx",
    "txn.rollback().await?",
    "txn.commit().await?",
  ],
  paths.upsertWrite,
);
requireAll(
  cli,
  [
    "let emit_events = !dry_run;",
    "TransactionalEventBus::new(",
    "ProfileMutationService::new(&db, &event_bus)",
    ".backfill_profile_with_event(",
    "let event_published = result.created;",
  ],
  paths.cli,
);
rejectAll(cli, ["DomainEvent::ProfileUpdated", "bus.publish(", ".backfill_profile("], paths.cli);

requireAll(
  accountRedaction,
  [
    "pub async fn redact_profile_for_account_deactivation_in_tx(",
    "transaction: &DatabaseTransaction",
    "Column::TenantId.eq(tenant_id)",
    "return Ok(false);",
    "active.status = Set(ProfileStatus::Hidden)",
    "active.update(transaction).await?",
  ],
  paths.accountRedaction,
);
requireAll(
  authDelete,
  [
    "TransactionalEventBus::new(",
    "OutboxTransport::new(self.db.clone())",
    "AuthLifecycleService::deactivate_user_in_tx",
    "redact_profile_for_account_deactivation_in_tx(",
    "revoke_active_sessions(&tx",
    "reserve_rbac_invalidation_generation(&tx)",
    ".publish_in_tx(",
    "Some(context.actor_id)",
    "DomainEvent::UserDeleted { user_id: user.id }",
    "tx.rollback().await.err()",
    "tx.commit()",
  ],
  paths.authDelete,
);
requireOrder(
  authDelete,
  [
    "async fn delete_user(",
    "AuthLifecycleService::deactivate_user_in_tx",
    "redact_profile_for_account_deactivation_in_tx(",
    "revoke_active_sessions(&tx",
    "reserve_rbac_invalidation_generation(&tx)",
    ".publish_in_tx(",
    "DomainEvent::UserDeleted { user_id: user.id }",
    "tx.commit()",
    "publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation).await",
  ],
  paths.authDelete,
);

requireAll(
  inbox,
  [
    "Author(Uuid)",
    "DomainEvent::ProfileUpdated { user_id, .. }",
    "DomainEvent::UserDeleted { user_id }",
    "Some(Self::Author(*user_id))",
    "scope_key.starts_with(AUTHOR_SCOPE_PREFIX)",
    "profile_and_account_changes_have_redaction_barrier_scope",
  ],
  paths.inbox,
);
requireAll(
  ingestion,
  [
    "DomainEvent::ProfileUpdated { .. }",
    "DomainEvent::UserDeleted { .. }",
    '"rebuild_forum_author_projection"',
    "projector.rebuild_tenant(envelope.tenant_id).await",
  ],
  paths.ingestion,
);

if (serviceBoundaryContract?.task !== "FORUM-23A8") {
  failures.push(`${paths.serviceBoundaryContract}: expected A8 source gate`);
}
if (serviceBoundaryContract?.source_boundary?.repository_production_call_sites_are_forbidden !== true) {
  failures.push(`${paths.serviceBoundaryContract}: production source gate drift`);
}
if (facadeContract?.task !== "FORUM-23A9") {
  failures.push(`${paths.facadeContract}: expected A9 facade contract`);
}
if (facadeContract?.mutation_api_boundary?.facade_is_publicly_exported !== true) {
  failures.push(`${paths.facadeContract}: public facade drift`);
}
if (deprecationContract?.task !== "FORUM-23A10") {
  failures.push(`${paths.deprecationContract}: expected A10 deprecation contract`);
}
if (deprecationContract?.deprecation_boundary?.legacy_methods_emit_rust_deprecation_diagnostics !== true) {
  failures.push(`${paths.deprecationContract}: legacy deprecation drift`);
}
if (deletionContract?.task !== "FORUM-23A11") {
  failures.push(`${paths.deletionContract}: expected A11 deletion contract`);
}

requireAll(
  note,
  [
    "Latest slice: `FORUM-23A11`",
    "ProfilePresentationService",
    "raw `payload.author_id`",
    "FORUM-23A11: canonical account deletion redaction",
    "redact_profile_for_account_deactivation_in_tx",
    "DomainEvent::UserDeleted",
    "authenticated administrator",
    "forum_author:<user_id>",
    "arbitrary `update_user` status changes",
    "owner-issued monotonic",
    "Not run by the implementation agent",
  ],
  paths.note,
);

if (contract) {
  if (contract.task !== "FORUM-23A") failures.push(`${paths.contract}: unexpected task`);
  if (contract.latest_slice !== "FORUM-23A11") {
    failures.push(`${paths.contract}: unexpected latest slice`);
  }
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  const expectedPaths = {
    forum_projection_source: paths.source,
    forum_author_adapter: paths.author,
    profiles_owner: paths.presentation,
    profiles_account_redaction_owner: paths.accountRedaction,
    auth_account_deactivation_owner: paths.authDelete,
    search_inbox: paths.inbox,
    search_ingestion: paths.ingestion,
    owner_note: paths.note,
    verifier: "scripts/verify/verify-forum-search-public-author-summary.mjs",
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${paths.contract}: ${key} drift`);
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
      failures.push(`${paths.contract}: projection boundary ${key} drift`);
    }
  }
  for (const key of [
    "profile_updated_is_durable",
    "profile_visibility_write_and_event_are_atomic",
    "profile_handle_write_and_event_are_atomic",
    "profile_content_write_and_event_are_atomic",
    "profile_locale_write_and_event_are_atomic",
    "profile_media_write_and_event_are_atomic",
    "profile_upsert_write_and_event_are_atomic",
    "profiles_cli_backfill_creation_and_event_are_atomic",
    "canonical_delete_user_deactivates_auth_and_hides_profile_in_one_transaction",
    "missing_profile_is_valid_but_still_emits_user_deleted",
    "user_deleted_is_persisted_in_the_deactivation_transaction",
    "user_deleted_event_failure_rolls_back_auth_profile_session_and_rbac_writes",
    "user_deleted_uses_authenticated_admin_actor",
    "user_deleted_uses_author_redaction_scope",
    "author_scope_is_tenant_and_user_scoped",
    "author_scope_is_not_suppressed_by_forum_wall_clock_watermark",
    "author_change_rebuilds_current_forum_owner_state",
    "user_deleted_rebuilds_current_forum_owner_state",
    "projection_failure_remains_retryable",
    "tenant_advisory_lock_is_preserved",
  ]) {
    if (contract.redaction_boundary?.[key] !== true) {
      failures.push(`${paths.contract}: redaction boundary ${key} drift`);
    }
  }
  if (contract.redaction_boundary?.schema_migration_added !== false) {
    failures.push(`${paths.contract}: schema migration must remain absent`);
  }
  for (const key of [
    "forum_graphql_changed",
    "forum_rest_changed",
    "search_query_api_changed",
    "search_document_schema_changed",
    "profiles_owner_storage_schema_changed",
    "auth_owner_storage_schema_changed",
    "event_schema_changed",
    "dependency_added",
    "cargo_lock_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${paths.contract}: compatibility ${key} must remain false`);
    }
  }
  for (const nonClaim of [
    "arbitrary user status updates outside canonical delete_user redact Profiles owner state",
    "canonical delete_user performs hard account erasure",
    "deprecated ProfileService mutation methods are compile-time private",
    "general cross-producer owner revision ordering is complete",
  ]) {
    if (!contract.non_claims?.includes(nonClaim)) {
      failures.push(`${paths.contract}: missing non-claim ${nonClaim}`);
    }
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${paths.contract}: execution status drift`);
  }
  if (contract.downstream_task !== "FORUM-23B") {
    failures.push(`${paths.contract}: unexpected downstream task`);
  }
}

if (failures.length > 0) {
  console.error("Forum public author Search projection verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum public author Search projection verification passed.");
