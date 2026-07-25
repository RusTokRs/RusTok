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
function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
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
  "crates/rustok-forum/contracts/forum-notification-topic-subscription-audience.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const recipientOwner = read(contract.recipient_context_owner_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const visibilityContract = JSON.parse(read(contract.visibility_contract ?? "") || "{}");
const fanoutContract = JSON.parse(read(contract.fanout_contract ?? "") || "{}");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) failures.push("forum topic subscription audience contract must use schema_version=1");
if (contract.task !== "FORUM-20P" || contract.upstream_task !== "FORUM-20O" || contract.fanout_prerequisite_task !== "NOTIFY-03I") failures.push("forum topic subscription audience contract must connect FORUM-20O/P to NOTIFY-03I");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("topic subscription audience publication must not claim unexecuted evidence");

for (const delivered of [
  "notification_sparse_page_prerequisite",
  "active_topic_source_recheck",
  "stale_public_descriptor_to_non_public_recipient_filtering",
  "bounded_raw_keyset_scan",
  "limit_plus_one_lookahead",
  "last_scanned_raw_cursor",
  "actor_subscription_excluded",
  "exact_recipient_context_per_scanned_subscription",
  "authenticated_topic_viewer",
  "recipient_specific_topic_visibility",
  "sparse_all_denied_page",
  "mixed_allowed_denied_page",
  "terminal_allowed_page",
  "public_only_fallback",
  "unavailable_recipient_skipped",
  "retryability_preserved",
  "initial_descriptor_behavior_unchanged",
  "sqlite_contract_test",
]) {
  if (contract.composition?.[delivered] !== true) failures.push(`forum topic subscription audience contract must record ${delivered} as delivered`);
}
for (const residual of [
  "initially non-public topic-created descriptor materialization",
  "profile privacy and blocking policy",
  "host trust channel and group facts adapters",
  "final notification creation and delivery authorization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) failures.push(`forum topic subscription audience contract must keep ${residual} explicitly open`);
}

const deliveredSlices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20P") failures.push("forum topic subscription audience contract must require the canonical ledger through FORUM-20P");
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) failures.push("forum topic subscription audience contract must require FORUM-20H through FORUM-20P delivered sections");
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") failures.push("pending canonical plan synchronization must identify FORUM-20G as the current plan boundary");
  requireText(plan, "FORUM-20A-G provide", "pending canonical plan synchronization must remain grounded in the current FORUM-20A-G ledger row");
  for (const slice of deliveredSlices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`);
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-P provide", "synchronized canonical plan must advance the FORUM-20 ledger through P");
  for (const slice of deliveredSlices) requireText(plan, `### Delivered in \`${slice}\``, `synchronized canonical plan is missing the delivered ${slice} section`);
} else failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "const TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR: &str = \"forum-notification-topic-subscription-audience\"",
  "async fn load_active_topic(",
  "async fn load_topic_for_subscription_audience(",
  "if self.recipient_context_port.is_none()",
  "self.load_public_topic(tenant_id, topic_id).await",
  "self.load_active_topic(tenant_id, topic_id).await",
  "async fn topic_subscription_recipient_visible(",
  "return Ok(true);",
  "resolve_recipient_viewer(",
  "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
  "load_topic_for_viewer(tenant_id, topic_id, &viewer)",
  "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)",
  ".limit((limit + 1) as u64)",
  "let has_more = subscriptions.len() > limit;",
  "subscriptions.truncate(limit);",
  "NotificationAudienceCursor::new(subscription.user_id.to_string())",
  "for subscription in subscriptions",
  ".topic_subscription_recipient_visible(",
]) requireText(source, marker, `forum topic subscription source is missing ${marker}`);

const audienceBlock = between(source, "async fn resolve_audience(", "async fn authorize_target_open(", "forum notification resolve_audience");
requireOrder(
  audienceBlock,
  [
    "load_topic_for_subscription_audience(event.tenant_id, event.aggregate_id)",
    ".limit((limit + 1) as u64)",
    "let has_more = subscriptions.len() > limit;",
    "subscriptions.truncate(limit);",
    "let next_cursor = if has_more",
    "let mut recipients = Vec::with_capacity(subscriptions.len());",
    "for subscription in subscriptions",
    ".topic_subscription_recipient_visible(",
    "NotificationAudiencePage::try_new(recipients, next_cursor)",
  ],
  "topic subscription audience must load the active source and compute raw keyset progress before exact recipient filtering",
);
for (const forbidden of [
  "forum_category_audience_policy", "forum_category_audience_role", "forum_category_audience_channel",
  "forum_category_audience_group", "forum_category_audience_user", "forum_topic_audience_policy",
  "forum_topic_audience_role", "forum_topic_audience_channel", "forum_topic_audience_group",
  "forum_topic_audience_user", "Rbac::permissions_for_role", "SecurityContext::new(",
]) rejectText(source, forbidden, `forum topic subscription audience must reuse owners instead of ${forbidden}`);

for (const marker of ["pub fn authenticated(", "ForumTopicVisibilityScope::storefront_for_viewer(", "resolve_for_constraints(", "Local allow/deny/role resolution intentionally skips owner-port calls"]) requireText(visibilityOwner, marker, `authenticated visibility owner is missing ${marker}`);
for (const marker of ["pub struct ForumNotificationRecipientContextResolver", "pub async fn resolve(", "validate_caller_context(&caller_context, tenant_id)?", "validate_recipient_context(&recipient_context, tenant_id, recipient_id)?", "SecurityContext::try_from_port_context(&recipient_context)", "pub fn into_topic_viewer(self)"]) requireText(recipientOwner, marker, `recipient context owner is missing ${marker}`);

for (const marker of [
  "topic_subscription_audience_filters_exact_recipients_before_cursor_progress",
  "impl ForumNotificationRecipientContextPort for RecordingRecipientContextPort",
  "let denied_first = Uuid::from_u128(1)",
  "let unavailable_second = Uuid::from_u128(2)",
  "let allowed_third = Uuid::from_u128(3)",
  "let denied_fourth = Uuid::from_u128(4)",
  "let allowed_fifth = Uuid::from_u128(5)",
  "roles_any: vec![UserRole::Customer]",
  "deny_user_ids: vec![denied_first, denied_fourth]",
  "topic audience should become non-public and deny exact recipients",
  "first_page.recipients().is_empty()",
  "recorded_calls(&calls), vec![denied_first, unavailable_second]",
  "second_page.recipients()[0].recipient_id, allowed_third",
  "third_page.recipients()[0].recipient_id, allowed_fifth",
  "BTreeSet::from([allowed_third, allowed_fifth])",
]) requireText(testSource, marker, `topic subscription SQLite contract is missing ${marker}`);

if (fanoutContract.schema_version !== 5 || !fanoutContract.promoted_by_slices?.includes("NOTIFY-03I") || fanoutContract.fanout_job?.sparse_page_may_continue !== true || fanoutContract.fanout_job?.sparse_page_creates_no_candidates !== true || fanoutContract.fanout_job?.cursor_must_advance !== true) failures.push("FORUM-20P requires the NOTIFY-03I sparse fanout contract");
if (upstream.schema_version !== 2 || upstream.task !== "FORUM-20O" || upstream.downstream_task !== "FORUM-20P" || upstream.composition?.topic_created_subscription_audience_downstream !== true) failures.push("FORUM-20P must remain synchronized with the FORUM-20O recipient contract");
if (visibilityContract.schema_version !== 7 || visibilityContract.task !== "FORUM-20K" || visibilityContract.downstream_task !== "FORUM-20P" || visibilityContract.composition?.topic_subscription_active_source_recheck !== true || visibilityContract.composition?.recipient_specific_topic_subscription_audience !== true || visibilityContract.composition?.topic_subscription_sparse_cursor !== true) failures.push("FORUM-20P must remain synchronized with the FORUM-20K visibility contract");

for (const marker of ["## `FORUM-20` — ACL and visibility inheritance", "notifications, search, SEO and deep links must call the same"]) requireText(plan, marker, `canonical Forum plan is missing the visibility boundary ${marker}`);

if (failures.length > 0) {
  console.error("Forum notification topic subscription audience verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification topic subscription audience contract is source-ready.");
