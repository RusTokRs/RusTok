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

function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return "";
  }
  return source.slice(startIndex, endIndex);
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
const mentionTestSource = read(contract.recipient_mention_test_file ?? "");
const subscriptionTestSource = read(contract.recipient_topic_subscription_test_file ?? "");
const descriptorContract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-topic-descriptor-materialization.json") ||
    "{}",
);
const descriptorTestSource = read(descriptorContract.runtime_test_file ?? "");
const recipientContract = JSON.parse(read(contract.recipient_target_open_contract ?? "") || "{}");
const mentionContract = JSON.parse(read(contract.recipient_mention_contract ?? "") || "{}");
const subscriptionContract = JSON.parse(read(contract.recipient_topic_subscription_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 7) {
  failures.push("forum notification visibility contract must use schema_version=7");
}
if (
  contract.task !== "FORUM-20K" ||
  contract.supersedes_task !== "FORUM-20I" ||
  contract.downstream_task !== "FORUM-20P"
) {
  failures.push("forum notification visibility contract must remain FORUM-20K and advance through FORUM-20P");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("source publication must not claim unexecuted notification evidence");
}

for (const residual of [
  "initially non-public topic-created descriptor materialization",
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
  "topic_created_public_description",
  "recipient_specific_topic_subscription_audience",
  "topic_subscription_sparse_cursor",
  "recipient_specific_mention_description",
  "recipient_specific_mention_audience",
  "shared_recipient_resolution_helper",
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
  "FORUM-20O",
  "FORUM-20P",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20P") {
  failures.push("forum notification visibility contract must require the canonical ledger through FORUM-20P");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum notification visibility contract must require FORUM-20H through FORUM-20P delivered sections");
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
  requireText(plan, "FORUM-20A-P provide", "synchronized canonical plan must advance the FORUM-20 ledger through P");
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
  "async fn load_topic_for_description(",
  "ForumTopicAudienceViewer::public()",
  "async fn load_target_for_viewer(",
  "async fn load_public_target(",
  "async fn load_mention_target_for_recipient(",
  "async fn topic_subscription_recipient_visible(",
  "async fn resolve_recipient_viewer(",
  "ForumNotificationRecipientContextResolver::new(Some(port))",
  "recipient.into_topic_viewer()",
  "recipient_operation_context(",
  "MENTION_DESCRIBE_ACTOR",
  "MENTION_AUDIENCE_ACTOR",
  "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
  "target_open_context(&request)",
  ".limit((limit + 1) as u64)",
  "subscriptions.truncate(limit);",
  "for subscription in subscriptions",
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

const visibilityIndex = notificationSource.indexOf(".is_topic_visible(tenant_id, topic_id, None, viewer)");
const materializeIndex = notificationSource.indexOf("let topic = forum_topic::Entity::find()");
if (visibilityIndex < 0 || materializeIndex < 0 || visibilityIndex > materializeIndex) {
  failures.push("notification richer visibility must be evaluated before topic materialization");
}

const describeBlock = between(
  notificationSource,
  "async fn describe_event(",
  "async fn resolve_audience(",
  "forum notification describe_event",
);
const audienceBlock = between(
  notificationSource,
  "async fn resolve_audience(",
  "async fn authorize_target_open(",
  "forum notification resolve_audience",
);
const targetOpenBlock = between(
  notificationSource,
  "async fn authorize_target_open(",
  "fn recipient_operation_context(",
  "forum notification authorize_target_open",
);
for (const [block, marker, label] of [
  [describeBlock, "load_topic_for_description(event.tenant_id, event.aggregate_id)", "topic-created capability-gated description"],
  [audienceBlock, "load_public_topic(event.tenant_id, event.aggregate_id)", "topic-created public source recheck"],
  [audienceBlock, "topic_subscription_recipient_visible(", "recipient topic subscription audience"],
  [describeBlock, "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)", "recipient mention description"],
  [audienceBlock, "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)", "recipient mention audience"],
  [targetOpenBlock, "resolve_recipient_viewer(", "recipient target-open authorization"],
]) {
  requireText(block, marker, `${label} is missing ${marker}`);
}

for (const marker of [
  "forum_topic_and_user_mention_sources_support_notifications_profiles",
  "restricted topic description should fail closed",
  "restricted topic authorization should fail closed",
  "restricted mention description should fail closed",
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
for (const marker of [
  "mention_description_and_audience_use_the_exact_recipient_for_topics_and_replies",
  "customer topic mention audience should resolve",
  "customer reply mention audience should resolve",
  "manager topic mention description should fail closed",
  "stale customer mention descriptor should be rechecked",
]) {
  requireText(mentionTestSource, marker, `recipient mention SQLite scenario is missing ${marker}`);
}
for (const marker of [
  "initially_non_public_topic_descriptor_requires_recipient_capability_and_reauthorizes",
  "without recipient capability an initially non-public topic must remain absent",
  "active initially non-public topic should materialize a descriptor",
  "page.recipients()[0].recipient_id, allowed_recipient",
]) {
  requireText(descriptorTestSource, marker, `topic descriptor SQLite scenario is missing ${marker}`);
}
for (const marker of [
  "topic_subscription_audience_filters_exact_recipients_before_cursor_progress",
  "first_page.recipients().is_empty()",
  "recorded_calls(&calls), vec![denied_first, unavailable_second]",
  "second_page.recipients()[0].recipient_id, allowed_third",
  "third_page.recipients()[0].recipient_id, allowed_fifth",
  "BTreeSet::from([allowed_third, allowed_fifth])",
]) {
  requireText(subscriptionTestSource, marker, `recipient topic subscription SQLite scenario is missing ${marker}`);
}

if (
  recipientContract.schema_version !== 3 ||
  recipientContract.task !== "FORUM-20N" ||
  recipientContract.downstream_task !== "FORUM-20O" ||
  recipientContract.downstream_chain_task !== "FORUM-20P" ||
  recipientContract.composition?.recipient_specific_topic_open !== true ||
  recipientContract.composition?.recipient_specific_reply_open !== true ||
  recipientContract.composition?.recipient_specific_mention_description !== true ||
  recipientContract.composition?.recipient_specific_mention_audience !== true ||
  recipientContract.composition?.recipient_specific_topic_subscription_audience !== true
) {
  failures.push("FORUM-20K visibility composition must remain synchronized with the FORUM-20N recipient contract through P");
}
if (
  mentionContract.schema_version !== 2 ||
  mentionContract.task !== "FORUM-20O" ||
  mentionContract.upstream_task !== "FORUM-20N" ||
  mentionContract.downstream_task !== "FORUM-20P" ||
  mentionContract.composition?.recipient_specific_mention_description !== true ||
  mentionContract.composition?.recipient_specific_mention_audience !== true ||
  mentionContract.composition?.topic_created_subscription_audience_downstream !== true
) {
  failures.push("FORUM-20K visibility composition must remain synchronized with the FORUM-20O mention audience contract through P");
}
if (
  descriptorContract.schema_version !== 1 ||
  descriptorContract.task !== "FORUM-20AP" ||
  descriptorContract.upstream_task !== "FORUM-20AO" ||
  descriptorContract.composition?.topic_created_descriptor_materialization !== true ||
  descriptorContract.composition?.exact_recipient_subscription_reauthorization !== true
) {
  failures.push("FORUM-20K visibility composition must recognize the downstream FORUM-20AP descriptor closure");
}
if (
  subscriptionContract.schema_version !== 1 ||
  subscriptionContract.task !== "FORUM-20P" ||
  subscriptionContract.upstream_task !== "FORUM-20O" ||
  subscriptionContract.composition?.bounded_raw_keyset_scan !== true ||
  subscriptionContract.composition?.recipient_specific_topic_visibility !== true ||
  subscriptionContract.composition?.sparse_all_denied_page !== true
) {
  failures.push("FORUM-20K visibility composition must remain synchronized with the FORUM-20P subscription audience contract");
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
