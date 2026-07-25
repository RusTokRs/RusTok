#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT) : path.resolve(scriptDir, "../..");
const failures = [];
function read(relativePath) {
  const absolute = path.join(repoRoot, relativePath);
  if (!existsSync(absolute)) { failures.push(`${relativePath}: required file is missing`); return ""; }
  return readFileSync(absolute, "utf8");
}
function requireText(source, marker, message) { if (!source.includes(marker)) failures.push(message); }
function rejectText(source, marker, message) { if (source.includes(marker)) failures.push(message); }
function between(source, start, end, label) {
  const a = source.indexOf(start); const b = source.indexOf(end, a + start.length);
  if (a < 0 || b < 0) { failures.push(`${label}: unable to isolate source block`); return ""; }
  return source.slice(a, b);
}

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-notification-visibility-composition.json") || "{}");
const source = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const baseOwner = read(contract.base_visibility_owner_file ?? "");
const baselineTest = read(contract.baseline_test_file ?? "");
const richerTest = read(contract.test_file ?? "");
const targetTest = read(contract.recipient_test_file ?? "");
const mentionTest = read(contract.recipient_mention_test_file ?? "");
const subscriptionTest = read(contract.recipient_topic_subscription_test_file ?? "");
const targetContract = JSON.parse(read(contract.recipient_target_open_contract ?? "") || "{}");
const mentionContract = JSON.parse(read(contract.recipient_mention_contract ?? "") || "{}");
const subscriptionContract = JSON.parse(read(contract.recipient_topic_subscription_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 7) failures.push("forum notification visibility contract must use schema_version=7");
if (contract.task !== "FORUM-20K" || contract.supersedes_task !== "FORUM-20I" || contract.downstream_task !== "FORUM-20P") failures.push("visibility contract must remain FORUM-20K and advance through FORUM-20P");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("visibility contract must not claim unexecuted evidence");
for (const field of [
  "exact_richer_public_owner", "normalized_category_layers", "normalized_topic_layer",
  "dynamic_policy_recheck", "recipient_context_factory_consumption", "recipient_specific_target_open",
  "topic_created_public_description", "recipient_specific_topic_subscription_audience",
  "topic_subscription_sparse_cursor", "recipient_specific_mention_description",
  "recipient_specific_mention_audience", "shared_recipient_resolution_helper",
]) if (contract.composition?.[field] !== true) failures.push(`visibility contract must record ${field}=true`);
for (const residual of [
  "initially non-public topic-created descriptor materialization", "profile privacy and blocking policy",
  "host trust channel and group facts adapters for authenticated consumers", "search index SEO and deep-link migration",
  "final notification creation and delivery authorization", "PostgreSQL and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`visibility contract must keep ${residual} open`);

const slices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20P" || JSON.stringify(sync.required_delivered_sections) !== JSON.stringify(slices)) failures.push("visibility contract must require FORUM-20H through FORUM-20P");
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") failures.push("pending plan boundary must remain FORUM-20G");
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded in FORUM-20A-G");
  for (const slice of slices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan contains ${slice}; update plan sync metadata`);
} else if (sync.status !== "synchronized") failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "pub struct ForumTopicAudienceViewer", "pub fn public() -> Self", "pub fn authenticated(",
  "pub struct ForumTopicAudienceVisibilityService", "pub async fn is_topic_visible(",
  "ForumTopicVisibilityScope::storefront_for_viewer(", "load_policy_for_topic(&self.db, tenant_id, &topic)",
]) requireText(visibilityOwner, marker, `exact visibility owner is missing ${marker}`);
for (const marker of ["pub struct ForumTopicVisibilityScope", "pub struct ForumTopicVisibilityService", "forum_topic::Column::Status.eq(TopicStatus::Open)", "all_topic_channel_access_subquery(tenant_id)"]) requireText(baseOwner, marker, `base visibility owner is missing ${marker}`);
for (const marker of [
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()", "host.shared_get::<SharedForumAudienceFactsPort>()",
  "async fn load_topic_for_viewer(", "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)", "async fn load_public_topic(",
  "async fn load_target_for_viewer(", "async fn load_mention_target_for_recipient(",
  "async fn resolve_recipient_viewer(", "async fn topic_subscription_recipient_visible(",
  "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR", ".limit((limit + 1) as u64)",
  "subscriptions.truncate(limit);", "for subscription in subscriptions",
  "fn forum_owner_error(error: ForumError) -> NotificationProviderError",
]) requireText(source, marker, `notification source is missing ${marker}`);
for (const forbidden of ["ForumTopicVisibilityScope::storefront(None)", "ForumTopicVisibilityService::new(", "forum_category_audience_policy", "forum_topic_audience_policy"]) rejectText(source, forbidden, `notification source must reuse exact owner instead of ${forbidden}`);

const describe = between(source, "async fn describe_event(", "async fn resolve_audience(", "describe_event");
const audience = between(source, "async fn resolve_audience(", "async fn authorize_target_open(", "resolve_audience");
const targetOpen = between(source, "async fn authorize_target_open(", "fn recipient_operation_context(", "authorize_target_open");
for (const [block, marker, label] of [
  [describe, "load_public_topic(event.tenant_id, event.aggregate_id)", "public topic description"],
  [audience, "topic_subscription_recipient_visible(", "recipient topic subscription audience"],
  [describe, "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)", "recipient mention description"],
  [audience, "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)", "recipient mention audience"],
  [targetOpen, "resolve_recipient_viewer(", "recipient target-open"],
]) requireText(block, marker, `${label} is missing ${marker}`);

for (const marker of ["forum_topic_and_user_mention_sources_support_notifications_profiles", "restricted topic description should fail closed", "NotificationProviderError::Internal { retryable: true }"]) requireText(baselineTest, marker, `baseline test is missing ${marker}`);
for (const marker of ["notification_source_rechecks_category_and_topic_richer_visibility", "stale public descriptor should be rechecked"]) requireText(richerTest, marker, `richer visibility test is missing ${marker}`);
for (const marker of ["notification_target_open_uses_exact_recipient_role_for_topics_and_replies", "NotificationOpenAuthorization::Allowed", "NotificationOpenAuthorization::Unavailable"]) requireText(targetTest, marker, `target-open test is missing ${marker}`);
for (const marker of ["mention_description_and_audience_use_the_exact_recipient_for_topics_and_replies", "stale customer mention descriptor should be rechecked"]) requireText(mentionTest, marker, `mention test is missing ${marker}`);
for (const marker of ["topic_subscription_audience_filters_exact_recipients_before_cursor_progress", "first_page.recipients().is_empty()", "BTreeSet::from([allowed_third, allowed_fifth])"]) requireText(subscriptionTest, marker, `subscription test is missing ${marker}`);

if (targetContract.schema_version !== 3 || targetContract.task !== "FORUM-20N" || targetContract.downstream_chain_task !== "FORUM-20P" || targetContract.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20K must remain synchronized with FORUM-20N/P");
if (mentionContract.schema_version !== 2 || mentionContract.task !== "FORUM-20O" || mentionContract.downstream_task !== "FORUM-20P") failures.push("FORUM-20K must remain synchronized with FORUM-20O/P");
if (subscriptionContract.schema_version !== 1 || subscriptionContract.task !== "FORUM-20P" || subscriptionContract.composition?.bounded_raw_keyset_scan !== true || subscriptionContract.composition?.sparse_all_denied_page !== true) failures.push("FORUM-20K must remain synchronized with FORUM-20P");

if (failures.length > 0) {
  console.error("Forum notification visibility composition verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification richer visibility composition contract is source-ready.");
