#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

const paths = {
  eventTypes: "crates/rustok-events/src/types.rs",
  auth: "apps/server/src/services/auth_admin_mutation_provider/user_admin.rs",
  profilesRedaction: "crates/rustok-profiles/src/account_redaction.rs",
  profilesRedactionTest: "crates/rustok-profiles/tests/account_redaction_test.rs",
  profilesLib: "crates/rustok-profiles/src/lib.rs",
  inbox: "crates/rustok-search/src/forum_inbox.rs",
  ingestion: "crates/rustok-search/src/ingestion.rs",
  contract: "crates/rustok-forum/contracts/forum-search-account-deletion-redaction.json",
  umbrellaContract: "crates/rustok-forum/contracts/forum-search-public-author-summary.json",
  note: "crates/rustok-forum/docs/forum-23a11-account-deletion-redaction.md",
  umbrellaNote: "crates/rustok-forum/docs/forum-23a-search-public-author-summary.md",
  verifier: "scripts/verify/verify-forum-search-account-deletion-redaction.mjs",
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

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

function requireAll(source, markers, label) {
  for (const marker of markers) requireMarker(source, marker, label);
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

const eventTypes = read(paths.eventTypes);
const auth = read(paths.auth);
const profilesRedaction = read(paths.profilesRedaction);
const profilesRedactionTest = read(paths.profilesRedactionTest);
const profilesLib = read(paths.profilesLib);
const inbox = read(paths.inbox);
const ingestion = read(paths.ingestion);
const note = read(paths.note);
const umbrellaNote = read(paths.umbrellaNote);
const contract = parseJson(paths.contract);
const umbrellaContract = parseJson(paths.umbrellaContract);

requireAll(eventTypes, ["UserDeleted {", "user_id: Uuid"], paths.eventTypes);

requireAll(
  profilesRedaction,
  [
    "pub async fn redact_profile_for_account_deactivation_in_tx(",
    "transaction: &DatabaseTransaction",
    "Column::TenantId.eq(tenant_id)",
    "let Some(profile) = profile else",
    "return Ok(false);",
    "active.status = Set(ProfileStatus::Hidden)",
    "active.updated_at = Set(Utc::now().into())",
    "active.update(transaction).await?",
    "Ok(true)",
  ],
  paths.profilesRedaction,
);
requireOrder(
  profilesRedaction,
  [
    "Column::TenantId.eq(tenant_id)",
    "let Some(profile) = profile else",
    "active.status = Set(ProfileStatus::Hidden)",
    "active.update(transaction).await?",
  ],
  paths.profilesRedaction,
);
requireAll(
  profilesLib,
  [
    "mod account_redaction;",
    "pub use account_redaction::redact_profile_for_account_deactivation_in_tx;",
  ],
  paths.profilesLib,
);
requireAll(
  profilesRedactionTest,
  [
    "account_redaction_hides_existing_tenant_profile",
    "assert_eq!(profile.status, ProfileStatus::Hidden)",
    "account_redaction_accepts_missing_profile_as_redacted_state",
    "assert!(!changed)",
    "account_redaction_does_not_cross_tenant_scope",
    "other_tenant_id",
    "assert_eq!(profile.status, ProfileStatus::Active)",
  ],
  paths.profilesRedactionTest,
);

requireAll(
  auth,
  [
    "rustok_core::events::EventTransport",
    "rustok_events::DomainEvent",
    "rustok_outbox::{OutboxTransport, TransactionalEventBus}",
    "async fn delete_user(",
    "TransactionalEventBus::new(",
    "OutboxTransport::new(self.db.clone())",
    "AuthLifecycleService::deactivate_user_in_tx",
    "rustok_profiles::redact_profile_for_account_deactivation_in_tx(",
    "revoke_active_sessions(&tx",
    "reserve_rbac_invalidation_generation(&tx)",
    ".publish_in_tx(",
    "Some(context.actor_id)",
    "DomainEvent::UserDeleted { user_id: user.id }",
    "tx.rollback().await.err()",
    "Durable UserDeleted publication failed; account deactivation rolled back",
    "durable user deletion invalidation is unavailable",
    "publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation).await",
  ],
  paths.auth,
);
requireOrder(
  auth,
  [
    "async fn delete_user(",
    "let event_bus = TransactionalEventBus::new(",
    "let tx = self",
    "AuthLifecycleService::deactivate_user_in_tx",
    "rustok_profiles::redact_profile_for_account_deactivation_in_tx(",
    "revoke_active_sessions(&tx",
    "reserve_rbac_invalidation_generation(&tx)",
    ".publish_in_tx(",
    "DomainEvent::UserDeleted { user_id: user.id }",
    "tx.commit()",
    "publish_committed_user_invalidation(context.tenant_id, user.id, durable_generation).await",
  ],
  paths.auth,
);
rejectMarker(auth, "event_bus.publish(", paths.auth);

requireAll(
  inbox,
  [
    "DomainEvent::ProfileUpdated { user_id, .. }",
    "DomainEvent::UserDeleted { user_id }",
    "Some(Self::Author(*user_id))",
    "AUTHOR_SCOPE_PREFIX",
    "scope_key.starts_with(AUTHOR_SCOPE_PREFIX)",
    "profile_and_account_changes_have_redaction_barrier_scope",
  ],
  paths.inbox,
);
requireAll(
  ingestion,
  [
    "DomainEvent::UserDeleted { .. }",
    "self.forum_projector.is_some()",
    "projector.rebuild_tenant(envelope.tenant_id).await",
    '"rebuild_forum_author_projection"',
    "assert!(!handler.handles(&DomainEvent::UserDeleted",
  ],
  paths.ingestion,
);

requireAll(
  note,
  [
    "FORUM-23A11",
    "deactivation, not hard erasure",
    "redact_profile_for_account_deactivation_in_tx",
    "TransactionalEventBus::publish_in_tx",
    "authenticated administrator",
    "explicitly rolled back",
    "forum_author:<user_id>",
    "update_user(status = inactive|banned)",
    "Not run by the implementation agent",
  ],
  paths.note,
);
requireAll(
  umbrellaNote,
  [
    "Latest slice: `FORUM-23A11`",
    "FORUM-23A11: canonical account deletion redaction",
    "UserDeleted` is sufficient deletion-redaction evidence for this canonical path",
    "arbitrary `update_user` status changes",
  ],
  paths.umbrellaNote,
);

if (contract) {
  if (contract.task !== "FORUM-23A11") failures.push(`${paths.contract}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${paths.contract}: unexpected status`);
  }
  const expectedPaths = {
    canonical_event_owner: paths.eventTypes,
    auth_deactivation_owner: paths.auth,
    profiles_redaction_owner: paths.profilesRedaction,
    profiles_redaction_test: paths.profilesRedactionTest,
    profiles_public_api: paths.profilesLib,
    search_inbox: paths.inbox,
    search_ingestion: paths.ingestion,
    umbrella_contract: paths.umbrellaContract,
    owner_note: paths.note,
    verifier: paths.verifier,
  };
  for (const [key, expected] of Object.entries(expectedPaths)) {
    if (contract[key] !== expected) failures.push(`${paths.contract}: ${key} drift`);
  }
  for (const key of [
    "auth_delete_is_deactivation_not_hard_erasure",
    "auth_user_is_locked_in_tenant_scope",
    "auth_user_is_deactivated_in_transaction",
    "profiles_redaction_helper_accepts_caller_transaction",
    "profiles_redaction_is_tenant_scoped",
    "existing_profile_is_marked_hidden",
    "missing_profile_is_valid_redacted_state",
    "profiles_redaction_test_covers_existing_missing_and_cross_tenant_states",
    "sessions_are_revoked_in_transaction",
    "rbac_generation_is_reserved_in_transaction",
    "user_deleted_is_published_in_transaction",
    "user_deleted_uses_authenticated_admin_actor",
    "event_insertion_precedes_commit",
    "event_failure_explicitly_rolls_back",
    "post_commit_rbac_fast_fanout_is_preserved",
  ]) {
    if (contract.owner_transaction_boundary?.[key] !== true) {
      failures.push(`${paths.contract}: owner transaction ${key} drift`);
    }
  }
  for (const key of [
    "user_deleted_is_existing_canonical_event",
    "user_deleted_maps_to_forum_author_scope",
    "user_deleted_is_handled_only_when_forum_source_is_composed",
    "user_deleted_rebuilds_current_forum_owner_state",
    "author_scope_remains_redaction_barrier",
    "hidden_or_missing_profile_projects_null_author",
    "projection_failure_remains_retryable",
  ]) {
    if (contract.search_redaction_boundary?.[key] !== true) {
      failures.push(`${paths.contract}: Search redaction ${key} drift`);
    }
  }
  for (const key of [
    "hard_account_erasure_added",
    "auth_admin_port_signature_changed",
    "event_schema_changed",
    "database_migration_added",
    "dependency_added",
    "cargo_lock_changed",
    "forum_graphql_changed",
    "forum_rest_changed",
    "search_query_api_changed",
    "search_document_schema_changed",
  ]) {
    if (contract.compatibility?.[key] !== false) {
      failures.push(`${paths.contract}: compatibility ${key} must remain false`);
    }
  }
  for (const key of [
    "all_user_status_disabling_paths_are_redacted",
    "account_rows_are_physically_deleted",
    "profiles_rows_are_physically_deleted",
    "user_deleted_alone_without_owner_redaction_is_sufficient",
    "general_cross_producer_owner_revision_ordering_is_complete",
    "runtime_verification_was_executed",
  ]) {
    if (contract.non_claims?.[key] !== true) {
      failures.push(`${paths.contract}: non-claim ${key} drift`);
    }
  }
  if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
    failures.push(`${paths.contract}: execution status drift`);
  }
  if (contract.downstream_task !== "FORUM-23B") {
    failures.push(`${paths.contract}: unexpected downstream task`);
  }
}

if (umbrellaContract) {
  if (umbrellaContract.task !== "FORUM-23A") {
    failures.push(`${paths.umbrellaContract}: unexpected task`);
  }
  if (umbrellaContract.latest_slice !== "FORUM-23A11") {
    failures.push(`${paths.umbrellaContract}: unexpected latest slice`);
  }
  for (const key of [
    "canonical_delete_user_deactivates_auth_and_hides_profile_in_one_transaction",
    "missing_profile_is_valid_but_still_emits_user_deleted",
    "user_deleted_is_persisted_in_the_deactivation_transaction",
    "user_deleted_event_failure_rolls_back_auth_profile_session_and_rbac_writes",
    "user_deleted_uses_authenticated_admin_actor",
    "user_deleted_uses_author_redaction_scope",
    "user_deleted_rebuilds_current_forum_owner_state",
  ]) {
    if (umbrellaContract.redaction_boundary?.[key] !== true) {
      failures.push(`${paths.umbrellaContract}: redaction boundary ${key} drift`);
    }
  }
  for (const nonClaim of [
    "arbitrary user status updates outside canonical delete_user redact Profiles owner state",
    "canonical delete_user performs hard account erasure",
    "general cross-producer owner revision ordering is complete",
  ]) {
    if (!umbrellaContract.non_claims?.includes(nonClaim)) {
      failures.push(`${paths.umbrellaContract}: missing non-claim ${nonClaim}`);
    }
  }
}

if (failures.length > 0) {
  console.error("Forum account deletion author redaction verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum account deletion author redaction verification passed.");
