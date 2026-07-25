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

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-notification-recipient-mention-audience.json") || "{}");
const source = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const recipientOwner = read(contract.recipient_context_owner_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const visibility = JSON.parse(read(contract.visibility_contract ?? "") || "{}");
const downstream = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const mentionTest = read(contract.test_file ?? "");
const subscriptionTest = read(contract.downstream_test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 2) failures.push("forum notification recipient mention contract must use schema_version=2");
if (contract.task !== "FORUM-20O" || contract.upstream_task !== "FORUM-20N" || contract.downstream_task !== "FORUM-20P") failures.push("mention contract must connect FORUM-20N/O/P");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("mention contract must not claim unexecuted evidence");

for (const field of [
  "shared_recipient_resolution_helper", "bounded_mention_description_context", "bounded_mention_audience_context",
  "exact_mention_recipient_resolution", "authenticated_topic_viewer", "recipient_specific_mention_description",
  "recipient_specific_mention_audience", "topic_mention_target", "reply_mention_target", "public_only_fallback",
  "unavailable_recipient_fail_closed", "retryability_preserved", "stale_descriptor_recheck",
  "topic_created_description_unchanged", "topic_created_subscription_audience_downstream",
  "sqlite_contract_test", "downstream_sqlite_contract_test",
]) if (contract.composition?.[field] !== true) failures.push(`mention contract must record ${field}=true`);
for (const residual of [
  "initially non-public topic-created descriptor materialization", "profile privacy and blocking policy",
  "host trust channel and group facts adapters", "final notification creation and delivery authorization",
  "search index SEO and deep-link migration", "PostgreSQL and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`mention contract must keep ${residual} open`);

const slices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20P" || JSON.stringify(sync.required_delivered_sections) !== JSON.stringify(slices)) failures.push("mention contract must require FORUM-20H through FORUM-20P");
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") failures.push("pending plan boundary must remain FORUM-20G");
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded in FORUM-20A-G");
  for (const slice of slices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan contains ${slice}; update plan sync metadata`);
} else if (sync.status !== "synchronized") failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()", "host.shared_get::<SharedForumAudienceFactsPort>()",
  "const MENTION_DESCRIBE_ACTOR: &str = \"forum-notification-mention-describe\"",
  "const MENTION_AUDIENCE_ACTOR: &str = \"forum-notification-mention-audience\"",
  "async fn load_mention_target_for_recipient(", "async fn resolve_recipient_viewer(",
  "ForumNotificationRecipientContextResolver::new(Some(port))", "recipient.into_topic_viewer()",
  "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)", "async fn topic_subscription_recipient_visible(",
]) requireText(source, marker, `mention source is missing ${marker}`);
for (const forbidden of ["forum_category_audience_policy", "forum_topic_audience_policy", "Rbac::permissions_for_role", "SecurityContext::new("]) rejectText(source, forbidden, `notification recipient paths must reuse owners instead of ${forbidden}`);

const describe = between(source, "async fn describe_event(", "async fn resolve_audience(", "describe_event");
const audience = between(source, "async fn resolve_audience(", "async fn authorize_target_open(", "resolve_audience");
for (const [block, marker, label] of [
  [describe, "load_public_topic(event.tenant_id, event.aggregate_id)", "public topic descriptor"],
  [describe, "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)", "exact mention description"],
  [audience, "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)", "exact mention audience"],
  [audience, "topic_subscription_recipient_visible(", "exact topic subscription audience"],
]) requireText(block, marker, `${label} is missing ${marker}`);

for (const marker of ["pub fn authenticated(", "ForumTopicVisibilityScope::storefront_for_viewer(", "resolve_for_constraints("]) requireText(visibilityOwner, marker, `visibility owner is missing ${marker}`);
for (const marker of ["pub struct ForumNotificationRecipientContextResolver", "validate_caller_context(&caller_context, tenant_id)?", "validate_recipient_context(&recipient_context, tenant_id, recipient_id)?", "pub fn into_topic_viewer(self)"]) requireText(recipientOwner, marker, `recipient owner is missing ${marker}`);
for (const marker of ["mention_description_and_audience_use_the_exact_recipient_for_topics_and_replies", "customer topic mention audience should resolve", "customer reply mention audience should resolve", "stale customer mention descriptor should be rechecked"]) requireText(mentionTest, marker, `mention test is missing ${marker}`);
for (const marker of ["topic_subscription_audience_filters_exact_recipients_before_cursor_progress", "first_page.recipients().is_empty()", "BTreeSet::from([allowed_third, allowed_fifth])"]) requireText(subscriptionTest, marker, `subscription test is missing ${marker}`);

if (upstream.schema_version !== 3 || upstream.task !== "FORUM-20N" || upstream.downstream_chain_task !== "FORUM-20P" || upstream.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20O must remain synchronized with FORUM-20N/P");
if (visibility.schema_version !== 7 || visibility.task !== "FORUM-20K" || visibility.downstream_task !== "FORUM-20P" || visibility.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20O must remain synchronized with FORUM-20K/P");
if (downstream.schema_version !== 1 || downstream.task !== "FORUM-20P" || downstream.upstream_task !== "FORUM-20O" || downstream.composition?.bounded_raw_keyset_scan !== true) failures.push("FORUM-20O must remain synchronized with FORUM-20P");

if (failures.length > 0) {
  console.error("Forum notification recipient mention audience verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification recipient mention audience contract is source-ready.");
