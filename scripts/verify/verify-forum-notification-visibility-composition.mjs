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
const richerTestSource = read(contract.test_file ?? "");
const recipientTestSource = read(contract.recipient_test_file ?? "");
const recipientContract = JSON.parse(read(contract.recipient_target_open_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 5) {
  failures.push("forum notification visibility contract must use schema_version=5");
}
if (
  contract.task !== "FORUM-20K" ||
  contract.supersedes_task !== "FORUM-20I" ||
  contract.downstream_task !== "FORUM-20N"
) {
  failures.push("forum notification visibility contract must remain FORUM-20K and advance through FORUM-20N");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted notification evidence");
}

for (const residual of [
  "recipient-specific authenticated audience filtering before notification pagination",
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
  "recipient_context_factory_consumption",
  "recipient_specific_target_open",
  "public_only_description_and_audience",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification visibility contract must record ${delivered} as delivered`);
  }
}

const deliveredSlices = [
  "FORUM-20H",
  "FORUM-20I",
  "FORUM-20J",
  "FORUM-20K",
  "FORUM-20L",
  "FORUM-20M",
  "FORUM-20N",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20N") {
  failures.push("forum notification visibility contract must require the canonical ledger through FORUM-20N");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum notification visibility contract must require FORUM-20H through FORUM-20N delivered sections");
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
  for (const slice of deliveredSlices) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(
    plan,
    "FORUM-20A-N provide",
    "synchronized canonical plan must advance the FORUM-20 ledger through N",
  );
  for (const slice of deliveredSlices) {
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
  "pub fn authenticated(",
  "pub struct ForumTopicAudienceVisibilityService",
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
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()",
  "host.shared_get::<SharedForumAudienceFactsPort>()",
  "async fn load_topic_for_viewer(",
  "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)",
  "async fn load_public_topic(",
  "ForumTopicAudienceViewer::public()",
  "async fn load_target_for_viewer(",
  "async fn load_public_target(",
  "ForumNotificationRecipientContextResolver::new(Some(port))",
  "recipient.into_topic_viewer()",
  "target_open_context(&request)",
  "fn forum_owner_error(error: ForumError) -> NotificationProviderError",
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
]) {
  rejectText(
    notificationSource,
    forbidden,
    `forum notification source must reuse the richer owner instead of ${forbidden}`,
  );
}

const describeIndex = notificationSource.indexOf("async fn describe_event(");
const audienceIndex = notificationSource.indexOf("async fn resolve_audience(");
const targetOpenIndex = notificationSource.indexOf("async fn authorize_target_open(");
const resolverIndex = notificationSource.indexOf("ForumNotificationRecipientContextResolver::new(Some(port))");
if (
  describeIndex < 0 ||
  audienceIndex < 0 ||
  targetOpenIndex < 0 ||
  resolverIndex < targetOpenIndex ||
  resolverIndex < audienceIndex
) {
  failures.push("recipient context resolution must remain scoped to target-open authorization after public description and audience paths");
}

for (const marker of [
  "forum_topic_and_user_mention_sources_support_notifications_profiles",
  "restricted topic description should fail closed",
  "restricted topic authorization should fail closed",
  "cross-tenant authorization should fail closed",
  "closed target authorization should fail closed",
  "NotificationProviderError::Internal { retryable: true }",
]) {
  requireText(baselineTestSource, marker, `notification baseline SQLite scenario is missing ${marker}`);
}
for (const marker of [
  "notification_source_rechecks_category_and_topic_richer_visibility",
  "topic-level richer visibility should fail closed",
  "stale public descriptor should be rechecked",
  "category policy change should invalidate the old public descriptor",
]) {
  requireText(richerTestSource, marker, `notification richer visibility SQLite scenario is missing ${marker}`);
}
for (const marker of [
  "notification_target_open_uses_exact_recipient_role_for_topics_and_replies",
  "SharedForumNotificationRecipientContextPort",
  "roles_any: vec![UserRole::Customer]",
  "NotificationOpenAuthorization::Allowed",
  "NotificationOpenAuthorization::Unavailable",
]) {
  requireText(recipientTestSource, marker, `recipient target-open SQLite scenario is missing ${marker}`);
}

if (
  recipientContract.schema_version !== 1 ||
  recipientContract.task !== "FORUM-20N" ||
  recipientContract.composition?.recipient_specific_topic_open !== true ||
  recipientContract.composition?.recipient_specific_reply_open !== true
) {
  failures.push("FORUM-20K visibility composition must remain synchronized with the FORUM-20N recipient target-open contract");
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
