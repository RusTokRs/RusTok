#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
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

const contractPath =
  "crates/rustok-forum/contracts/forum-notification-visibility-composition.json";
const contract = JSON.parse(read(contractPath) || "{}");
const notificationSource = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const baseVisibilityOwner = read(contract.base_visibility_owner_file ?? "");
const baselineTestSource = read(contract.baseline_test_file ?? "");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 4) {
  failures.push("forum notification visibility contract must use schema_version=4");
}
if (contract.task !== "FORUM-20K" || contract.supersedes_task !== "FORUM-20I") {
  failures.push("forum notification richer visibility contract must belong to FORUM-20K and supersede FORUM-20I");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted notification evidence");
}
for (const residual of [
  "recipient-specific authenticated role trust channel group and explicit-user evaluation",
  "recipient-specific target-open authorization for non-public audiences",
  "profile privacy and blocking policy",
  "host trust channel and group facts adapters for authenticated consumers",
  "search index SEO and deep-link migration",
  "final notification creation and delivery authorization",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification visibility contract must keep ${residual} explicitly open`);
  }
}

for (const delivered of [
  "exact_richer_public_owner",
  "normalized_category_layers",
  "normalized_topic_layer",
  "dynamic_policy_recheck",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification visibility contract must record ${delivered} as delivered`);
  }
}

const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20K") {
  failures.push("forum notification visibility contract must require the canonical ledger through FORUM-20K");
}
if (
  JSON.stringify(planSync.required_delivered_sections) !==
  JSON.stringify(["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K"])
) {
  failures.push("forum notification visibility contract must require FORUM-20H/I/J/K delivered sections");
}
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan synchronization must identify FORUM-20G as the current plan boundary");
  }
  requireText(
    plan,
    "FORUM-20A-G provide",
    "pending canonical plan synchronization must remain grounded in the current FORUM-20A-G ledger row",
  );
  for (const slice of ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K"]) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(
    plan,
    "FORUM-20A-K provide",
    "synchronized canonical plan must advance the FORUM-20 ledger through K",
  );
  for (const slice of ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K"]) {
    requireText(
      plan,
      `### Delivered in \`${slice}\``,
      `synchronized canonical plan is missing the delivered ${slice} section`,
    );
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct ForumTopicAudienceViewer",
  "pub fn public() -> Self",
  "pub struct ForumTopicAudienceVisibilityService",
  "pub fn without_facts_provider",
  "pub async fn is_topic_visible(",
  "ForumTopicVisibilityScope::storefront_for_viewer(",
  "ForumTopicVisibilityService::new(self.db.clone())",
  "load_policy_for_topic(&self.db, tenant_id, &topic)",
  "for layer in &policy.inherited_category_layers",
  "policy.configured_constraints",
]) {
  requireText(visibilityOwner, marker, `exact richer topic visibility owner is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumTopicVisibilityScope",
  "pub struct ForumTopicVisibilityService",
  "forum_topic::Column::Status.eq(TopicStatus::Open)",
  "all_topic_channel_access_subquery(tenant_id)",
]) {
  requireText(baseVisibilityOwner, marker, `base topic visibility owner is missing ${marker}`);
}

for (const marker of [
  "use crate::error::ForumError;",
  "use crate::services::{ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService};",
  "async fn load_public_topic(",
  "let viewer = ForumTopicAudienceViewer::public();",
  "ForumTopicAudienceVisibilityService::without_facts_provider(self.db.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, &viewer)",
  ".map_err(forum_owner_error)?",
  ".load_public_topic(tenant_id, source_id)",
  ".load_public_topic(tenant_id, reply.topic_id)",
  ".load_public_topic(event.tenant_id, event.aggregate_id)",
  "fn forum_owner_error(error: ForumError) -> NotificationProviderError",
  "retryable: error.is_retryable()",
]) {
  requireText(
    notificationSource,
    marker,
    `forum notification source is missing exact richer visibility composition ${marker}`,
  );
}
for (const forbidden of [
  "ForumTopicVisibilityScope::storefront(None)",
  "ForumTopicVisibilityService::new(",
  "async fn load_open_topic(",
  "async fn is_channel_restricted(",
  "forum_topic_channel_access",
  "forum_category_audience_policy",
  "forum_category_audience_role",
  "forum_category_audience_channel",
  "forum_category_audience_group",
  "forum_category_audience_user",
  "forum_topic_audience_policy",
  "forum_topic_audience_role",
  "forum_topic_audience_channel",
  "forum_topic_audience_group",
  "forum_topic_audience_user",
  "ForumAudienceFactsPort",
]) {
  rejectText(
    notificationSource,
    forbidden,
    `forum notification source must reuse the richer owner instead of ${forbidden}`,
  );
}

const visibilityIndex = notificationSource.indexOf(
  ".is_topic_visible(tenant_id, topic_id, None, &viewer)",
);
const materializeIndex = notificationSource.indexOf(
  "let topic = forum_topic::Entity::find()",
);
if (visibilityIndex < 0 || materializeIndex < 0 || visibilityIndex > materializeIndex) {
  failures.push("notification target richer visibility must be evaluated before target materialization");
}

for (const marker of [
  "forum_topic_and_user_mention_sources_support_notifications_profiles",
  "restricted topic description should fail closed",
  "restricted topic authorization should fail closed",
  "restricted mention description should fail closed",
  "cross-tenant authorization should fail closed",
  "closed target authorization should fail closed",
  "database failure should be classified",
  "NotificationProviderError::Internal { retryable: true }",
]) {
  requireText(baselineTestSource, marker, `notification baseline SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "notification_source_rechecks_category_and_topic_richer_visibility",
  "ForumTopicAudiencePolicyService::new(db.clone())",
  "ForumCategoryAudiencePolicyService::new(db.clone())",
  "topic-level richer visibility should fail closed",
  "stale public descriptor should be rechecked",
  "category-level richer visibility should fail closed",
  "category policy change should invalidate the old public descriptor",
  "NotificationOpenAuthorization::Unavailable",
]) {
  requireText(testSource, marker, `notification richer visibility SQLite scenario is missing ${marker}`);
}

for (const marker of [
  "## `FORUM-20` — ACL and visibility inheritance",
  "notifications, search, SEO and deep links must call the same",
]) {
  requireText(plan, marker, `canonical Forum plan is missing the visibility boundary ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum notification visibility composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification richer visibility composition contract is source-ready.");
