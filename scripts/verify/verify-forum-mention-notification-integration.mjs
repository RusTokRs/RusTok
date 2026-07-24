#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "../..");
const contractPath = "crates/rustok-forum/contracts/forum-mention-notification-integration.json";
const failures = [];

function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolute, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function rejectText(source, marker, message) {
  if (source.includes(marker)) failures.push(message);
}

const contract = JSON.parse(read(contractPath));
const forumSource = read(contract.forum_source_provider);
const candidateService = read(contract.notifications_candidate_service);
const candidateContract = JSON.parse(read(contract.notifications_candidate_contract));
const recipientPolicy = read(contract.recipient_policy_runtime);
const socialGraphContract = JSON.parse(read(contract.social_graph_policy_contract));

if (contract.schema_version !== 1) {
  failures.push(`${contractPath}: expected schema_version 1`);
}
if (contract.canonical_plan !== "crates/rustok-forum/docs/implementation-plan.md") {
  failures.push(`${contractPath}: canonical Forum plan link drifted`);
}
if (contract.execution_status !== "source_locked_pending_maintainer_execution") {
  failures.push(`${contractPath}: source evidence must not claim maintainer execution`);
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push(`${contractPath}: verifier execution status must remain explicit`);
}
for (const requiredDeferred of [
  "successful PostgreSQL mention/quote runtime proof",
  "bounded moderator audience expansion for forum.mention.audience_added",
  "recipient privacy and source authorization recheck on inbox open",
  "recipient privacy and source authorization recheck before delayed channel delivery",
  "retention purge and reconciliation evidence",
]) {
  if (!contract.deferred?.includes(requiredDeferred)) {
    failures.push(`${contractPath}: missing deferred boundary: ${requiredDeferred}`);
  }
}

for (const marker of [
  'const USER_MENTION_ADDED_TYPE: &str = "forum.mention.user_added";',
  "user_mention_relation_exists",
  "load_public_target",
  "ForumTargetAvailability::Deferred",
  "event.actor_id == Some(payload.mentioned_user_id)",
  "NotificationAudiencePage::try_new(",
  "recipient_id: payload.mentioned_user_id",
  "authorize_target_open",
  '"/modules/forum?category={}&topic={}&reply={}"',
  '"/modules/forum?category={}&topic={}"',
]) {
  requireText(
    forumSource,
    marker,
    `${contract.forum_source_provider}: missing Forum mention-source invariant: ${marker}`,
  );
}
rejectText(
  forumSource,
  '"forum.mention.audience_added"',
  `${contract.forum_source_provider}: moderator audience delivery must remain deferred until bounded expansion exists`,
);
for (const forbiddenOwnerImport of [
  "use rustok_profiles",
  "use rustok_social_graph",
  "use rustok_notifications::",
]) {
  rejectText(
    forumSource,
    forbiddenOwnerImport,
    `${contract.forum_source_provider}: Forum source provider must not own profile, social-graph or notification persistence policy`,
  );
}

for (const marker of [
  "pub trait NotificationRecipientPolicy",
  "self.policy.evaluate(policy_request).await",
  "NotificationRecipientPolicyDecision::Suppress",
  "provider",
  ".authorize_target_open(AuthorizeNotificationTargetRequest {",
  "recipient_id: item.recipient_id",
  "persist_final_notification",
  "OnConflict::columns([",
  "notification::Column::TenantId",
  "notification::Column::RecipientId",
  "notification::Column::SourceSlug",
  "notification::Column::SourceEventId",
  "notification::Column::NotificationType",
  "ensure_notification_identity",
  "txn.commit().await?",
]) {
  requireText(
    candidateService,
    marker,
    `${contract.notifications_candidate_service}: missing candidate privacy/dedupe invariant: ${marker}`,
  );
}

if (candidateContract.recipient_privacy_policy?.allow_all_default_forbidden !== true) {
  failures.push(`${contract.notifications_candidate_contract}: allow-all privacy default must remain forbidden`);
}
if (candidateContract.recipient_privacy_policy?.production_runtime_composition_delivered !== true) {
  failures.push(`${contract.notifications_candidate_contract}: production privacy runtime is not recorded as delivered`);
}
if (candidateContract.source_authorization?.authorize_target_open_before_creation !== true) {
  failures.push(`${contract.notifications_candidate_contract}: source authorization must precede final creation`);
}
if (candidateContract.final_notification?.semantic_replay_equality_required !== true) {
  failures.push(`${contract.notifications_candidate_contract}: semantic replay equality must remain required`);
}
if (candidateContract.final_notification?.candidate_completion_same_transaction !== true) {
  failures.push(`${contract.notifications_candidate_contract}: candidate completion must remain transactional`);
}
if (candidateContract.upstream_runtime?.default_enabled !== false) {
  failures.push(`${contract.notifications_candidate_contract}: candidate processing must remain disabled by default`);
}

for (const marker of [
  "ProfilePrivacyDecision::RecipientUnavailable",
  "ProfilePrivacyDecision::Restricted",
  "NotificationRecipientSuppression::ProfileRestricted",
  "blocks_between",
  "NotificationRecipientSuppression::Blocked",
  "source_mutes_target",
  "NotificationRecipientSuppression::Muted",
  "NotificationRecipientPolicyError::retryable",
  "with_candidate_worker_enabled(candidate_worker_enabled_from_environment())",
]) {
  requireText(
    recipientPolicy,
    marker,
    `${contract.recipient_policy_runtime}: missing recipient privacy composition marker: ${marker}`,
  );
}

if (socialGraphContract.privacy_semantics?.block_either_direction_suppresses !== true) {
  failures.push(`${contract.social_graph_policy_contract}: block must suppress in either direction`);
}
if (socialGraphContract.privacy_semantics?.mute_source_to_target_only !== true) {
  failures.push(`${contract.social_graph_policy_contract}: mute direction must remain explicit`);
}
if (socialGraphContract.server_composition?.profile_then_block_then_mute !== true) {
  failures.push(`${contract.social_graph_policy_contract}: privacy evaluation order drifted`);
}
if (socialGraphContract.server_composition?.candidate_worker_default_enabled !== false) {
  failures.push(`${contract.social_graph_policy_contract}: candidate worker must remain opt-in`);
}

if (failures.length > 0) {
  console.error("forum mention notification integration verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum mention notification integration verification passed");
