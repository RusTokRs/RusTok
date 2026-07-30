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

const sourcePath = "crates/rustok-forum/src/search_projection.rs";
const authorPath = "crates/rustok-forum/src/search_projection_author.rs";
const forumLibPath = "crates/rustok-forum/src/lib.rs";
const inboxPath = "crates/rustok-search/src/forum_inbox.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const profilesPath = "crates/rustok-profiles/src/presentation.rs";
const profilesMutationPath = "crates/rustok-profiles/src/graphql/mutation.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-public-author-summary.json";
const notePath = "crates/rustok-forum/docs/forum-23a-search-public-author-summary.md";

const source = read(sourcePath);
const author = read(authorPath);
const forumLib = read(forumLibPath);
const inbox = read(inboxPath);
const ingestion = read(ingestionPath);
const profiles = read(profilesPath);
const profilesMutation = read(profilesMutationPath);
const note = read(notePath);
let contract = null;
try {
  contract = JSON.parse(read(contractPath));
} catch (error) {
  failures.push(`${contractPath}: invalid JSON: ${error.message}`);
}

for (const marker of [
  "ProfilePresentationService::new(db.clone())",
  ".find_profile_summary(tenant_id, author_id, Some(locale), None)",
  "public_author_payload",
  '"user_id": summary.user_id',
  '"handle": summary.handle',
  '"display_name": summary.display_name',
  '"avatar_media_id": summary.avatar_media_id',
  "public_author_payload_exposes_only_the_safe_summary",
  "absent_or_denied_author_is_not_serialized",
]) {
  requireMarker(author, marker, authorPath);
}
for (const forbidden of [
  '"tags": summary.tags',
  '"preferred_locale": summary.preferred_locale',
  '"visibility": summary.visibility',
]) {
  rejectMarker(author, forbidden, authorPath);
}

for (const marker of [
  "load_public_author_summary",
  "handle: author_handle",
  "public_author_keywords",
  '"author": author_payload',
  '"author_id": author_id',
  '"has_public_author": author_id.is_some()',
]) {
  requireMarker(source, marker, sourcePath);
}
for (const forbidden of [
  '"author_id": topic.author_id',
  '"author_id": reply.author_id',
]) {
  rejectMarker(source, forbidden, sourcePath);
}
requireMarker(forumLib, "mod search_projection_author;", forumLibPath);

for (const marker of [
  "ProfilePrivacyService::new(self.db.clone())",
  "ProfileAccessAudience::Anonymous",
  "ProfilePrivacyDecision::Allow",
]) {
  requireMarker(profiles, marker, profilesPath);
}
for (const marker of [
  "service.update_profile_visibility",
  "service.update_profile_media",
  "publish_profile_updated(event_bus, tenant.id, auth.user_id, &profile).await?",
  "DomainEvent::ProfileUpdated",
]) {
  requireMarker(profilesMutation, marker, profilesMutationPath);
}

for (const marker of [
  "Author(Uuid)",
  "DomainEvent::ProfileUpdated { user_id, .. }",
  "AUTHOR_SCOPE_PREFIX",
  "scope_key.starts_with(AUTHOR_SCOPE_PREFIX)",
  "return Ok(None);",
  "profile_changes_have_redaction_barrier_scope",
]) {
  requireMarker(inbox, marker, inboxPath);
}
rejectMarker(inbox, "DomainEvent::UserDeleted", inboxPath);
for (const marker of [
  "DomainEvent::ProfileUpdated { .. }",
  '"rebuild_forum_author_projection"',
  "projector.rebuild_tenant(envelope.tenant_id).await",
]) {
  requireMarker(ingestion, marker, ingestionPath);
}
rejectMarker(ingestion, "DomainEvent::UserDeleted", ingestionPath);

for (const marker of [
  "FORUM-23A",
  "ProfilePresentationService",
  "forum_author:<user_id>",
  "raw `payload.author_id`",
  "owner-issued monotonic",
  "does not treat `UserDeleted`",
  "not run by the implementation agent",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-23A") failures.push(`${contractPath}: unexpected task`);
  if (contract.status !== "source_complete_execution_pending") {
    failures.push(`${contractPath}: unexpected status`);
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
    "author_filter_value_is_populated_only_for_public_author",
  ]) {
    if (contract.projection_boundary?.[key] !== true) {
      failures.push(`${contractPath}: projection boundary ${key} drift`);
    }
  }
  for (const key of [
    "profile_updated_is_durable",
    "profile_updated_is_emitted_after_owner_write",
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
  if (!contract.non_claims?.includes("account deletion redaction is complete")) {
    failures.push(`${contractPath}: account deletion non-claim is missing`);
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
