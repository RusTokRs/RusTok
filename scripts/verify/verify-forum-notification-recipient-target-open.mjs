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
  "crates/rustok-forum/contracts/forum-notification-recipient-target-open.json";
const contract = JSON.parse(read(contractPath) || "{}");
const source = read(contract.notification_source_file ?? "");
const visibilityOwner = read(contract.visibility_owner_file ?? "");
const recipientOwner = read(contract.recipient_context_owner_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const visibilityContract = JSON.parse(read(contract.visibility_contract ?? "") || "{}");
const testSource = read(contract.test_file ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification recipient target-open contract must use schema_version=1");
}
if (contract.task !== "FORUM-20N" || contract.upstream_task !== "FORUM-20M") {
  failures.push("forum notification recipient target-open contract must belong to FORUM-20N after FORUM-20M");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("recipient target-open publication must not claim unexecuted evidence");
}

for (const delivered of [
  "factory_recipient_capability_lookup",
  "factory_facts_capability_lookup",
  "bounded_target_open_context",
  "exact_recipient_resolution",
  "authenticated_topic_viewer",
  "recipient_specific_topic_open",
  "recipient_specific_reply_open",
  "public_only_fallback",
  "inactive_or_missing_recipient_fail_closed",
  "retryability_preserved",
  "public_description_unchanged",
  "public_audience_unchanged",
  "sqlite_contract_test",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification recipient target-open contract must record ${delivered} as delivered`);
  }
}
for (const residual of [
  "recipient-specific audience filtering for non-public topics before pagination",
  "profile privacy and blocking policy",
  "host trust channel and group facts adapters",
  "final notification creation and delivery authorization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification recipient target-open contract must keep ${residual} explicitly open`);
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
  failures.push("forum notification recipient target-open contract must require the canonical ledger through FORUM-20N");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum notification recipient target-open contract must require FORUM-20H through FORUM-20N delivered sections");
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
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()",
  "host.shared_get::<SharedForumAudienceFactsPort>()",
  "recipient_context_port: Option<SharedForumNotificationRecipientContextPort>",
  "facts_port: Option<SharedForumAudienceFactsPort>",
  "async fn load_topic_for_viewer(",
  "ForumTopicAudienceVisibilityService::new(self.db.clone(), self.facts_port.clone())",
  ".is_topic_visible(tenant_id, topic_id, None, viewer)",
  "async fn load_target_for_viewer(",
  "reply.status == ReplyStatus::Pending",
  "reply.status != ReplyStatus::Approved",
  "const TARGET_OPEN_DEADLINE: Duration = Duration::from_secs(2)",
  "PortActor::service(TARGET_OPEN_ACTOR)",
  ".with_deadline(TARGET_OPEN_DEADLINE)",
  "ForumNotificationRecipientContextResolver::new(Some(port))",
  "target_open_context(&request)",
  "recipient.into_topic_viewer()",
  "Err(ForumError::CapabilityFailure {",
  "retryable: false, ..",
  "self.load_public_target(request.tenant_id, source_kind, request.target.id)",
  "ForumError::CapabilityUnavailable { .. }",
  "NotificationProviderError::CapabilityUnavailable { retryable: true }",
]) {
  requireText(source, marker, `forum notification recipient target-open source is missing ${marker}`);
}
for (const forbidden of [
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
  "Rbac::permissions_for_role",
  "SecurityContext::new(",
]) {
  rejectText(
    source,
    forbidden,
    `forum notification target-open must reuse owners instead of ${forbidden}`,
  );
}

const describeIndex = source.indexOf("async fn describe_event(");
const audienceIndex = source.indexOf("async fn resolve_audience(");
const targetOpenIndex = source.indexOf("async fn authorize_target_open(");
const recipientResolutionIndex = source.indexOf("ForumNotificationRecipientContextResolver::new(Some(port))");
if (
  describeIndex < 0 ||
  audienceIndex < 0 ||
  targetOpenIndex < 0 ||
  recipientResolutionIndex < targetOpenIndex ||
  recipientResolutionIndex < audienceIndex
) {
  failures.push("exact recipient resolution must remain scoped to authorize_target_open after public description and audience methods");
}

for (const marker of [
  "pub fn authenticated(",
  "ForumTopicVisibilityScope::storefront_for_viewer(",
  "resolve_for_constraints(",
  "Local allow/deny/role resolution intentionally skips owner-port calls",
]) {
  requireText(visibilityOwner, marker, `authenticated visibility owner is missing ${marker}`);
}
for (const marker of [
  "pub struct ForumNotificationRecipientContextResolver",
  "pub async fn resolve(",
  "validate_caller_context(&caller_context, tenant_id)?",
  "validate_recipient_context(&recipient_context, tenant_id, recipient_id)?",
  "SecurityContext::try_from_port_context(&recipient_context)",
  "pub fn into_topic_viewer(self)",
]) {
  requireText(recipientOwner, marker, `recipient context owner is missing ${marker}`);
}

for (const marker of [
  "notification_target_open_uses_exact_recipient_role_for_topics_and_replies",
  "impl ForumNotificationRecipientContextPort for StaticRecipientContextPort",
  "roles_any: vec![UserRole::Customer]",
  "SharedForumNotificationRecipientContextPort",
  "Forum source factory should consume the recipient capability",
  "customer target-open authorization should complete",
  "manager target-open authorization should fail closed",
  "unavailable recipient should fail closed without an existence oracle",
  "NotificationOpenAuthorization::Allowed",
  "NotificationOpenAuthorization::Unavailable",
]) {
  requireText(testSource, marker, `recipient target-open SQLite contract is missing ${marker}`);
}

if (
  upstream.schema_version !== 2 ||
  upstream.task !== "FORUM-20M" ||
  upstream.downstream_task !== "FORUM-20N" ||
  upstream.composition?.notification_source_factory_consumption !== true ||
  upstream.composition?.recipient_target_open_authorization !== true
) {
  failures.push("FORUM-20N target-open must remain synchronized with the FORUM-20M host runtime contract");
}
if (
  visibilityContract.schema_version !== 5 ||
  visibilityContract.task !== "FORUM-20K" ||
  visibilityContract.downstream_task !== "FORUM-20N" ||
  visibilityContract.composition?.recipient_specific_target_open !== true ||
  visibilityContract.composition?.public_only_description_and_audience !== true
) {
  failures.push("FORUM-20N target-open must remain synchronized with the FORUM-20K visibility composition contract");
}

for (const marker of [
  "## `FORUM-20` — ACL and visibility inheritance",
  "notifications, search, SEO and deep links must call the same",
]) {
  requireText(plan, marker, `canonical Forum plan is missing the visibility boundary ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum notification recipient target-open verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification recipient target-open contract is source-ready.");
