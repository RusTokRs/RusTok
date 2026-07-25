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

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-notification-recipient-target-open.json") || "{}");
const source = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const recipientOwner = read(contract.recipient_context_owner_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const visibility = JSON.parse(read(contract.visibility_contract ?? "") || "{}");
const mention = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const subscriptions = JSON.parse(read(contract.downstream_chain_contract ?? "") || "{}");
const targetTest = read(contract.test_file ?? "");
const mentionTest = read(contract.downstream_test_file ?? "");
const subscriptionTest = read(contract.downstream_chain_test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 3) failures.push("forum notification recipient contract must use schema_version=3");
if (contract.task !== "FORUM-20N" || contract.upstream_task !== "FORUM-20M" || contract.downstream_task !== "FORUM-20O" || contract.downstream_chain_task !== "FORUM-20P") failures.push("recipient contract must connect FORUM-20M/N/O/P");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("recipient contract must not claim unexecuted evidence");

for (const field of [
  "factory_recipient_capability_lookup", "factory_facts_capability_lookup", "bounded_target_open_context",
  "exact_recipient_resolution", "authenticated_topic_viewer", "recipient_specific_topic_open",
  "recipient_specific_reply_open", "public_only_fallback", "inactive_or_missing_recipient_fail_closed",
  "retryability_preserved", "topic_created_public_description_unchanged",
  "recipient_specific_topic_subscription_audience", "recipient_specific_mention_description",
  "recipient_specific_mention_audience", "shared_recipient_resolution_helper", "sqlite_contract_test",
  "downstream_sqlite_contract_test", "downstream_chain_sqlite_contract_test",
]) if (contract.composition?.[field] !== true) failures.push(`recipient contract must record ${field}=true`);
for (const residual of [
  "initially non-public topic-created descriptor materialization", "profile privacy and blocking policy",
  "host trust channel and group facts adapters", "final notification creation and delivery authorization",
  "search index SEO and deep-link migration", "PostgreSQL and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`recipient contract must keep ${residual} open`);

const slices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20P" || JSON.stringify(sync.required_delivered_sections) !== JSON.stringify(slices)) failures.push("recipient contract must require FORUM-20H through FORUM-20P");
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") failures.push("pending plan boundary must remain FORUM-20G");
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded in FORUM-20A-G");
  for (const slice of slices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan contains ${slice}; update plan sync metadata`);
} else if (sync.status !== "synchronized") failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()", "host.shared_get::<SharedForumAudienceFactsPort>()",
  "async fn load_topic_for_viewer(", "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)", "async fn load_target_for_viewer(",
  "reply.status == ReplyStatus::Pending", "reply.status != ReplyStatus::Approved",
  "const RECIPIENT_CONTEXT_DEADLINE: Duration = Duration::from_secs(2)", "async fn resolve_recipient_viewer(",
  "ForumNotificationRecipientContextResolver::new(Some(port))", "recipient.into_topic_viewer()",
  "target_open_context(&request)", "async fn load_mention_target_for_recipient(",
  "async fn topic_subscription_recipient_visible(", "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
  "ForumError::CapabilityUnavailable { .. }", "NotificationProviderError::CapabilityUnavailable { retryable: true }",
]) requireText(source, marker, `recipient source is missing ${marker}`);
for (const forbidden of ["forum_category_audience_policy", "forum_topic_audience_policy", "Rbac::permissions_for_role", "SecurityContext::new("]) rejectText(source, forbidden, `recipient paths must reuse owners instead of ${forbidden}`);

const describe = between(source, "async fn describe_event(", "async fn resolve_audience(", "describe_event");
const audience = between(source, "async fn resolve_audience(", "async fn authorize_target_open(", "resolve_audience");
const targetOpen = between(source, "async fn authorize_target_open(", "fn recipient_operation_context(", "authorize_target_open");
for (const [block, marker, label] of [
  [describe, "load_public_topic(event.tenant_id, event.aggregate_id)", "public topic descriptor"],
  [audience, "topic_subscription_recipient_visible(", "exact topic subscription audience"],
  [describe, "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)", "exact mention description"],
  [audience, "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)", "exact mention audience"],
  [targetOpen, "resolve_recipient_viewer(", "exact target-open"],
]) requireText(block, marker, `${label} is missing ${marker}`);

for (const marker of ["pub fn authenticated(", "ForumTopicVisibilityScope::storefront_for_viewer(", "resolve_for_constraints("]) requireText(visibilityOwner, marker, `visibility owner is missing ${marker}`);
for (const marker of ["pub struct ForumNotificationRecipientContextResolver", "validate_caller_context(&caller_context, tenant_id)?", "validate_recipient_context(&recipient_context, tenant_id, recipient_id)?", "pub fn into_topic_viewer(self)"]) requireText(recipientOwner, marker, `recipient owner is missing ${marker}`);
for (const marker of ["notification_target_open_uses_exact_recipient_role_for_topics_and_replies", "NotificationOpenAuthorization::Allowed", "NotificationOpenAuthorization::Unavailable"]) requireText(targetTest, marker, `target-open test is missing ${marker}`);
for (const marker of ["mention_description_and_audience_use_the_exact_recipient_for_topics_and_replies", "stale customer mention descriptor should be rechecked"]) requireText(mentionTest, marker, `mention test is missing ${marker}`);
for (const marker of ["topic_subscription_audience_filters_exact_recipients_before_cursor_progress", "first_page.recipients().is_empty()", "BTreeSet::from([allowed_third, allowed_fifth])"]) requireText(subscriptionTest, marker, `subscription test is missing ${marker}`);

if (upstream.schema_version !== 4 || upstream.task !== "FORUM-20M" || upstream.topic_subscription_consumer_task !== "FORUM-20P") failures.push("FORUM-20N must remain synchronized with FORUM-20M/P");
if (visibility.schema_version !== 7 || visibility.task !== "FORUM-20K" || visibility.downstream_task !== "FORUM-20P" || visibility.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20N must remain synchronized with FORUM-20K/P");
if (mention.schema_version !== 2 || mention.task !== "FORUM-20O" || mention.downstream_task !== "FORUM-20P") failures.push("FORUM-20N must remain synchronized with FORUM-20O/P");
if (subscriptions.schema_version !== 1 || subscriptions.task !== "FORUM-20P" || subscriptions.composition?.bounded_raw_keyset_scan !== true) failures.push("FORUM-20N must remain synchronized with FORUM-20P");

if (failures.length > 0) {
  console.error("Forum notification recipient authorization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification recipient authorization contract is source-ready.");
