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
function requireText(source, marker, message) { if (!source.includes(marker)) failures.push(message); }
function rejectText(source, marker, message) { if (source.includes(marker)) failures.push(message); }
function between(source, start, end, label) {
  const startIndex = source.indexOf(start);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (startIndex < 0 || endIndex < 0) { failures.push(`${label}: unable to isolate source block`); return ""; }
  return source.slice(startIndex, endIndex);
}

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-notification-recipient-target-open.json") || "{}");
const source = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const recipientOwner = read(contract.recipient_context_owner_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const visibilityContract = JSON.parse(read(contract.visibility_contract ?? "") || "{}");
const downstream = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const downstreamChain = JSON.parse(read(contract.downstream_chain_contract ?? "") || "{}");
const targetOpenTest = read(contract.test_file ?? "");
const mentionTest = read(contract.downstream_test_file ?? "");
const subscriptionTest = read(contract.downstream_chain_test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 3) failures.push("forum notification recipient contract must use schema_version=3");
if (contract.task !== "FORUM-20N" || contract.upstream_task !== "FORUM-20M" || contract.downstream_task !== "FORUM-20O" || contract.downstream_chain_task !== "FORUM-20P") failures.push("forum notification recipient contract must connect FORUM-20M/N/O/P");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("recipient authorization publication must not claim unexecuted evidence");

for (const delivered of [
  "factory_recipient_capability_lookup", "factory_facts_capability_lookup", "bounded_target_open_context",
  "exact_recipient_resolution", "authenticated_topic_viewer", "recipient_specific_topic_open",
  "recipient_specific_reply_open", "public_only_fallback", "inactive_or_missing_recipient_fail_closed",
  "retryability_preserved", "topic_created_public_description_unchanged",
  "recipient_specific_topic_subscription_audience", "recipient_specific_mention_description",
  "recipient_specific_mention_audience", "shared_recipient_resolution_helper", "sqlite_contract_test",
  "downstream_sqlite_contract_test", "downstream_chain_sqlite_contract_test",
]) if (contract.composition?.[delivered] !== true) failures.push(`forum notification recipient contract must record ${delivered} as delivered`);
for (const residual of [
  "initially non-public topic-created descriptor materialization", "profile privacy and blocking policy",
  "host trust channel and group facts adapters", "final notification creation and delivery authorization",
  "search index SEO and deep-link migration", "PostgreSQL and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`forum notification recipient contract must keep ${residual} explicitly open`);

const deliveredSlices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20P" || JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) failures.push("forum notification recipient contract must require FORUM-20H through FORUM-20P");
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") failures.push("pending canonical plan synchronization must identify FORUM-20G as the current plan boundary");
  requireText(plan, "FORUM-20A-G provide", "pending canonical plan synchronization must remain grounded in the current FORUM-20A-G ledger row");
  for (const slice of deliveredSlices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`);
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-P provide", "synchronized canonical plan must advance through P");
  for (const slice of deliveredSlices) requireText(plan, `### Delivered in \`${slice}\``, `synchronized canonical plan is missing ${slice}`);
} else failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()", "host.shared_get::<SharedForumAudienceFactsPort>()",
  "recipient_context_port: Option<SharedForumNotificationRecipientContextPort>", "facts_port: Option<SharedForumAudienceFactsPort>",
  "async fn load_active_topic(", "async fn load_topic_for_viewer(", "async fn load_topic_for_subscription_audience(",
  "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())", ".is_topic_visible(tenant_id, topic_id, None, viewer)",
  "async fn load_target_for_viewer(", "reply.status == ReplyStatus::Pending", "reply.status != ReplyStatus::Approved",
  "const RECIPIENT_CONTEXT_DEADLINE: Duration = Duration::from_secs(2)", "async fn resolve_recipient_viewer(",
  "ForumNotificationRecipientContextResolver::new(Some(port))", "recipient.into_topic_viewer()", "recipient_operation_context(",
  "target_open_context(&request)", "Err(ForumError::CapabilityFailure {", "retryable: false, ..",
  "self.load_public_target(request.tenant_id, source_kind, request.target.id)", "ForumError::CapabilityUnavailable { .. }",
  "NotificationProviderError::CapabilityUnavailable { retryable: true }", "async fn load_mention_target_for_recipient(",
  "MENTION_DESCRIBE_ACTOR", "MENTION_AUDIENCE_ACTOR", "async fn topic_subscription_recipient_visible(",
  "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
]) requireText(source, marker, `forum notification recipient source is missing ${marker}`);
for (const forbidden of [
  "forum_category_audience_policy", "forum_category_audience_role", "forum_category_audience_channel",
  "forum_category_audience_group", "forum_category_audience_user", "forum_topic_audience_policy",
  "forum_topic_audience_role", "forum_topic_audience_channel", "forum_topic_audience_group",
  "forum_topic_audience_user", "Rbac::permissions_for_role", "SecurityContext::new(",
]) rejectText(source, forbidden, `forum notification recipient paths must reuse owners instead of ${forbidden}`);

const describeBlock = between(source, "async fn describe_event(", "async fn resolve_audience(", "describe_event");
const audienceBlock = between(source, "async fn resolve_audience(", "async fn authorize_target_open(", "resolve_audience");
const targetOpenBlock = between(source, "async fn authorize_target_open(", "fn recipient_operation_context(", "authorize_target_open");
for (const [block, marker, label] of [
  [describeBlock, "load_public_topic(event.tenant_id, event.aggregate_id)", "topic-created public description"],
  [audienceBlock, "load_topic_for_subscription_audience(event.tenant_id, event.aggregate_id)", "active topic subscription source recheck"],
  [audienceBlock, "topic_subscription_recipient_visible(", "exact topic subscription audience"],
  [describeBlock, "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)", "exact mention description"],
  [audienceBlock, "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)", "exact mention audience"],
  [targetOpenBlock, "resolve_recipient_viewer(", "exact target-open authorization"],
]) requireText(block, marker, `${label} is missing ${marker}`);

for (const marker of ["pub fn authenticated(", "ForumTopicVisibilityScope::storefront_for_viewer(", "resolve_for_constraints(", "Local allow/deny/role resolution intentionally skips owner-port calls"]) requireText(visibilityOwner, marker, `authenticated visibility owner is missing ${marker}`);
for (const marker of ["pub struct ForumNotificationRecipientContextResolver", "pub async fn resolve(", "validate_caller_context(&caller_context, tenant_id)?", "validate_recipient_context(&recipient_context, tenant_id, recipient_id)?", "SecurityContext::try_from_port_context(&recipient_context)", "pub fn into_topic_viewer(self)"]) requireText(recipientOwner, marker, `recipient context owner is missing ${marker}`);

for (const marker of ["notification_target_open_uses_exact_recipient_role_for_topics_and_replies", "impl ForumNotificationRecipientContextPort for StaticRecipientContextPort", "roles_any: vec![UserRole::Customer]", "customer target-open authorization should complete", "manager target-open authorization should fail closed", "unavailable recipient should fail closed without an existence oracle", "NotificationOpenAuthorization::Allowed", "NotificationOpenAuthorization::Unavailable"]) requireText(targetOpenTest, marker, `recipient target-open SQLite contract is missing ${marker}`);
for (const marker of ["mention_description_and_audience_use_the_exact_recipient_for_topics_and_replies", "customer topic mention audience should resolve", "customer reply mention audience should resolve", "manager topic mention description should fail closed", "stale customer mention descriptor should be rechecked"]) requireText(mentionTest, marker, `recipient mention SQLite contract is missing ${marker}`);
for (const marker of ["topic_subscription_audience_filters_exact_recipients_before_cursor_progress", "roles_any: vec![UserRole::Customer]", "topic audience should become non-public and deny exact recipients", "first_page.recipients().is_empty()", "recorded_calls(&calls), vec![denied_first, unavailable_second]", "BTreeSet::from([allowed_third, allowed_fifth])"]) requireText(subscriptionTest, marker, `recipient topic subscription SQLite contract is missing ${marker}`);

if (upstream.schema_version !== 4 || upstream.task !== "FORUM-20M" || upstream.downstream_task !== "FORUM-20N" || upstream.topic_subscription_consumer_task !== "FORUM-20P" || upstream.composition?.notification_source_factory_consumption !== true || upstream.composition?.recipient_target_open_authorization !== true || upstream.composition?.recipient_topic_subscription_audience_authorization !== true) failures.push("FORUM-20N must remain synchronized with FORUM-20M/P");
if (visibilityContract.schema_version !== 7 || visibilityContract.task !== "FORUM-20K" || visibilityContract.downstream_task !== "FORUM-20P" || visibilityContract.composition?.topic_subscription_active_source_recheck !== true || visibilityContract.composition?.recipient_specific_target_open !== true || visibilityContract.composition?.recipient_specific_mention_description !== true || visibilityContract.composition?.recipient_specific_mention_audience !== true || visibilityContract.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20N must remain synchronized with FORUM-20K/P");
if (downstream.schema_version !== 2 || downstream.task !== "FORUM-20O" || downstream.upstream_task !== "FORUM-20N" || downstream.downstream_task !== "FORUM-20P" || downstream.composition?.recipient_specific_mention_description !== true || downstream.composition?.recipient_specific_mention_audience !== true || downstream.composition?.topic_created_subscription_audience_downstream !== true) failures.push("FORUM-20N must remain synchronized with FORUM-20O/P");
if (downstreamChain.schema_version !== 1 || downstreamChain.task !== "FORUM-20P" || downstreamChain.upstream_task !== "FORUM-20O" || downstreamChain.composition?.active_topic_source_recheck !== true || downstreamChain.composition?.stale_public_descriptor_to_non_public_recipient_filtering !== true || downstreamChain.composition?.bounded_raw_keyset_scan !== true || downstreamChain.composition?.recipient_specific_topic_visibility !== true) failures.push("FORUM-20N must remain synchronized with FORUM-20P");

for (const marker of ["## `FORUM-20` — ACL and visibility inheritance", "notifications, search, SEO and deep links must call the same"]) requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
if (failures.length > 0) {
  console.error("Forum notification recipient authorization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification recipient authorization contract is source-ready.");
