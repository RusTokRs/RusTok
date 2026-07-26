#!/usr/bin/env node
// Profiles storefront owner-boundary, transport selection, operation telemetry, optimistic recovery, Media presentation, and accessibility guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath) {
  if (!existsSync(repoPath(relativePath))) fail(`${relativePath}: expected file`);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const paths = {
  followRead: "crates/rustok-social-graph/src/follow_read.rs",
  socialLib: "crates/rustok-social-graph/src/lib.rs",
  socialGraphql: "crates/rustok-social-graph/src/graphql.rs",
  socialPorts: "crates/rustok-social-graph/src/ports.rs",
  socialObservability: "crates/rustok-social-graph/src/observability.rs",
  native: "crates/rustok-profiles/storefront/src/transport/native_server_adapter.rs",
  graphql: "crates/rustok-profiles/storefront/src/transport/graphql_adapter.rs",
  profileGraphql: "crates/rustok-profiles/src/graphql/types.rs",
  profileMutation: "crates/rustok-profiles/src/graphql/mutation.rs",
  profileQuery: "crates/rustok-profiles/src/graphql/query.rs",
  profileCli: "crates/rustok-profiles/cli/src/lib.rs",
  profileError: "crates/rustok-profiles/src/error.rs",
  profileLib: "crates/rustok-profiles/src/lib.rs",
  observability: "crates/rustok-profiles/src/observability.rs",
  mediaPublic: "crates/rustok-media/src/public_image.rs",
  core: "crates/rustok-profiles/storefront/src/core.rs",
  ui: "crates/rustok-profiles/storefront/src/ui/leptos.rs",
  en: "crates/rustok-profiles/storefront/locales/en.json",
  ru: "crates/rustok-profiles/storefront/locales/ru.json",
  test: "crates/rustok-social-graph/tests/follow_state_sqlite.rs",
};

for (const value of Object.values(paths)) assertExists(value);

const followRead = readRepo(paths.followRead);
const socialLib = readRepo(paths.socialLib);
const socialGraphql = readRepo(paths.socialGraphql);
const socialPorts = readRepo(paths.socialPorts);
const socialObservability = readRepo(paths.socialObservability);
const native = readRepo(paths.native);
const graphql = readRepo(paths.graphql);
const profileGraphql = readRepo(paths.profileGraphql);
const profileMutation = readRepo(paths.profileMutation);
const profileQuery = readRepo(paths.profileQuery);
const profileCli = readRepo(paths.profileCli);
const profileError = readRepo(paths.profileError);
const profileLib = readRepo(paths.profileLib);
const observability = readRepo(paths.observability);
const mediaPublic = readRepo(paths.mediaPublic);
const core = readRepo(paths.core);
const ui = readRepo(paths.ui);
const en = readRepo(paths.en);
const ru = readRepo(paths.ru);
const test = readRepo(paths.test);

assertContains(followRead, "pub trait SocialGraphFollowReadPort", `${paths.followRead}: owner read port missing`);
assertContains(followRead, "revision: Option<i64>", `${paths.followRead}: revision-bearing state missing`);
assertContains(followRead, ".relation_state(", `${paths.followRead}: state read must use owner service`);
assertContains(socialLib, "pub mod follow_read;", `${paths.socialLib}: follow read module not wired`);
assertContains(socialLib, "SocialGraphFollowReadPort", `${paths.socialLib}: follow read port not exported`);

assertContains(socialGraphql, "async fn follow_state", `${paths.socialGraphql}: followState query missing`);
assertContains(socialGraphql, "revision: state.revision.map", `${paths.socialGraphql}: query must expose optional revision string`);
assertContains(graphql, "followState(userId: $userId)", `${paths.graphql}: storefront must request revision-bearing state`);
assertContains(graphql, "revision: Option<String>", `${paths.graphql}: missing relation must keep null revision`);
assertNotContains(graphql, "isFollowing(userId", `${paths.graphql}: storefront must not downgrade to bool-only follow reads`);

assertContains(socialLib, "pub mod observability;", `${paths.socialLib}: Social Graph operation telemetry module not wired`);
assertContains(socialLib, "SocialGraphCommandTimer", `${paths.socialLib}: Social Graph command telemetry contract not exported`);
assertContains(
  socialObservability,
  'SOCIAL_GRAPH_OPERATION_TARGET: &str = "rustok_social_graph::operations"',
  `${paths.socialObservability}: stable Social Graph telemetry target missing`,
);
for (const operation of [
  "social_graph.block",
  "social_graph.unblock",
  "social_graph.mute",
  "social_graph.unmute",
  "social_graph.follow",
  "social_graph.unfollow",
]) {
  assertContains(socialObservability, `"${operation}"`, `${paths.socialObservability}: stable command operation missing: ${operation}`);
}
for (const field of [
  "operation =",
  "tenant_id =",
  "source_user_id =",
  "target_user_id =",
  "outcome =",
  "duration_ms =",
  "error_code",
  "retryable",
]) {
  assertContains(socialObservability, field, `${paths.socialObservability}: command telemetry field missing: ${field}`);
}
for (const sensitiveField of [
  "idempotency_key =",
  "expected_revision =",
  "correlation_id =",
  "locale =",
  "channel =",
  "claims =",
  "roles =",
]) {
  assertNotContains(socialObservability, sensitiveField, `${paths.socialObservability}: command telemetry must not record field: ${sensitiveField}`);
}
assertContains(socialPorts, "SocialGraphCommandTimer::start", `${paths.socialPorts}: owner command port telemetry must start after tenant parsing`);
assertContains(socialPorts, "SocialGraphCommandOperation::from_relation_state", `${paths.socialPorts}: owner command operation classifier missing`);
assertContains(socialPorts, "timer.finish_failure(&error.code, error.retryable)", `${paths.socialPorts}: policy/actor failure telemetry missing`);
assertContains(socialPorts, "timer.finish_port_result(&result)", `${paths.socialPorts}: owner command result telemetry missing`);
assertNotContains(socialGraphql, "tracing::", `${paths.socialGraphql}: GraphQL adapter must not own Social Graph command telemetry`);

assertContains(native, "ProfilePresentationService::for_audience", `${paths.native}: native profile read must use owner presentation service`);
assertNotContains(native, "ProfilePrivacyService::new", `${paths.native}: native adapter must not duplicate privacy composition`);
assertNotContains(native, "ProfileService::new", `${paths.native}: native presentation must not use raw ProfileService`);
assertContains(native, "SocialGraphFollowReadPort::source_follow_state", `${paths.native}: native follow state must retain revision`);

for (const [source, sourcePath] of [
  [native, paths.native],
  [profileGraphql, paths.profileGraphql],
]) {
  assertContains(source, "MediaPublicImageReadPort", `${sourcePath}: must use Media public image owner port`);
  assertContains(source, "get_public_image_asset", `${sourcePath}: must request Media-owned presentation descriptor`);
  assertContains(source, "validate_profile_media_asset", `${sourcePath}: must revalidate profile tenant/uploader/MIME`);
  assertNotContains(source, "public_image_path(", `${sourcePath}: must not construct Media capability paths`);
  assertNotContains(source, '"/api/media/public/images', `${sourcePath}: must not own Media route strings`);
}
assertContains(mediaPublic, "MediaImagePublicUrlPolicy::ProxyRequired", `${paths.mediaPublic}: storage-relative proxy policy missing`);

assertContains(profileLib, "pub mod observability;", `${paths.profileLib}: operation telemetry module not wired`);
assertContains(profileLib, "ProfileOperationTimer", `${paths.profileLib}: operation telemetry contract not exported`);
assertContains(profileLib, "ProfileBackfillTimer", `${paths.profileLib}: backfill telemetry contract not exported`);
assertContains(observability, 'PROFILE_OPERATION_TARGET: &str = "rustok_profiles::operations"', `${paths.observability}: stable telemetry target missing`);
assertContains(observability, 'PROFILE_BACKFILL_OPERATION: &str = "profile.backfill"', `${paths.observability}: stable backfill operation missing`);
for (const operation of [
  "profile.upsert",
  "profile.update_handle",
  "profile.update_content",
  "profile.update_locale",
  "profile.update_visibility",
  "profile.update_media",
  "profile.publish_updated_event",
  "profile.backfill",
]) {
  assertContains(observability, `"${operation}"`, `${paths.observability}: stable operation missing: ${operation}`);
}
for (const field of [
  "operation =",
  "tenant_id =",
  "user_id =",
  "outcome =",
  "duration_ms =",
  "error_code",
  "retryable",
  "dry_run =",
  "emit_events =",
  "scanned_users",
  "skipped_existing",
  "planned_creates",
  "created_profiles",
  "published_events",
]) {
  assertContains(observability, field, `${paths.observability}: telemetry field missing: ${field}`);
}
for (const sensitiveField of [
  "handle =",
  "display_name =",
  "bio =",
  "locale =",
  "preferred_locale =",
  "email =",
  "media_id =",
  "avatar_media_id =",
  "banner_media_id =",
  "url =",
  "endpoint =",
]) {
  assertNotContains(observability, sensitiveField, `${paths.observability}: telemetry must not record sensitive field: ${sensitiveField}`);
}
for (const operationVariant of [
  "ProfileOperation::Upsert",
  "ProfileOperation::UpdateHandle",
  "ProfileOperation::UpdateContent",
  "ProfileOperation::UpdateLocale",
  "ProfileOperation::UpdateVisibility",
  "ProfileOperation::UpdateMedia",
  "ProfileOperation::PublishUpdatedEvent",
]) {
  assertContains(profileMutation, operationVariant, `${paths.profileMutation}: write path is missing telemetry: ${operationVariant}`);
}
assertContains(profileMutation, "timer.finish_profile_result(&result)", `${paths.profileMutation}: owner result telemetry missing`);
assertContains(profileMutation, "timer.finish_failure(PROFILE_EVENT_PUBLISH_ERROR, true)", `${paths.profileMutation}: event failure telemetry missing`);
for (const [source, sourcePath] of [
  [profileMutation, paths.profileMutation],
  [profileQuery, paths.profileQuery],
]) {
  assertContains(source, "ProfileError::PresentationUnavailable", `${sourcePath}: presentation-unavailable error mapping missing`);
}
assertContains(profileError, '"profiles.presentation_unavailable"', `${paths.profileError}: presentation error code missing`);
assertContains(profileError, '"profiles.storage_unavailable"', `${paths.profileError}: storage error code missing`);
assertContains(profileError, "pub const fn is_retryable", `${paths.profileError}: retryability classification missing`);

assertContains(profileCli, "ProfileBackfillTimer::start", `${paths.profileCli}: backfill telemetry must start after normalized options`);
assertContains(profileCli, "telemetry.finish_success(", `${paths.profileCli}: successful backfill telemetry missing`);
assertContains(profileCli, "backfill_port_failed", `${paths.profileCli}: port failure telemetry missing`);
assertContains(profileCli, "backfill_profile_failed", `${paths.profileCli}: profile failure telemetry missing`);
assertContains(profileCli, "backfill_failed", `${paths.profileCli}: external failure telemetry missing`);
for (const marker of [
  "host_resolution",
  "tenant_read",
  "user_read",
  "enrichment_read",
  "existing_profile_read",
  "profile_plan",
  "profile_create",
  "event_publish",
  "BACKFILL_HOST_ERROR",
  "BACKFILL_TENANT_READ_ERROR",
  "BACKFILL_USER_READ_ERROR",
  "BACKFILL_ENRICHMENT_READ_ERROR",
  "BACKFILL_EVENT_PUBLISH_ERROR",
]) {
  assertContains(profileCli, marker, `${paths.profileCli}: stable backfill telemetry marker missing: ${marker}`);
}
assertNotContains(profileCli, "tracing::", `${paths.profileCli}: CLI must emit telemetry through the Profiles owner contract`);

assertContains(core, '"native" => ProfilesStorefrontTransportProfile::Native', `${paths.core}: explicit native selector missing`);
assertContains(core, '"graphql" => ProfilesStorefrontTransportProfile::Graphql', `${paths.core}: explicit GraphQL selector missing`);
assertContains(core, 'panic!("unsupported profiles storefront transport profile', `${paths.core}: invalid transport configuration must fail closed`);
assertNotContains(core, "_ => ProfilesStorefrontTransportProfile::Native", `${paths.core}: unknown transport values must not silently fall back to native`);
assertContains(core, "fn invalid_transport_profile_fails_closed()", `${paths.core}: invalid transport regression test missing`);

assertContains(core, "pub fn recovered_follow_state", `${paths.core}: pure recovery selector missing`);
assertContains(ui, "recovered_follow_state", `${paths.ui}: UI must use validated recovery state`);
assertContains(ui, "load_profiles_storefront_page(", `${paths.ui}: mutation failure must re-read current state`);
assertContains(ui, "set_profiles_storefront_follow(transport, command).await", `${paths.ui}: owner mutation call missing`);
assertNotContains(ui, /loop\s*\{|while\s*\(|for\s+.*set_profiles_storefront_follow/, `${paths.ui}: recovery must not automatically retry writes`);

for (const marker of [
  "aria-pressed=",
  "aria-busy=",
  'aria-live="polite"',
  'role="alert"',
  'role="img"',
  'alt=""',
]) {
  assertContains(ui, marker, `${paths.ui}: accessibility marker ${marker} missing`);
}
assertContains(en, '"followRecovered"', `${paths.en}: recovery message missing`);
assertContains(ru, '"followRecovered"', `${paths.ru}: recovery message missing`);

for (const marker of ["initial.revision, None", "active.revision, Some(1)", "inactive.revision, Some(2)"]) {
  assertContains(test, marker, `${paths.test}: scenario ${marker} missing`);
}
assertContains(test, "PortErrorKind::Forbidden", `${paths.test}: actor mismatch evidence missing`);

if (failures.length > 0) {
  console.error("Profiles storefront boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Profiles storefront boundary verification passed");
