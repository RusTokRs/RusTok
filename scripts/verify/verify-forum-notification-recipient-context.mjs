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
  "crates/rustok-forum/contracts/forum-notification-recipient-context.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.owner_file ?? "");
const crate = read(contract.crate_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const downstream = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const consumer = JSON.parse(read(contract.consumer_contract ?? "") || "{}");
const downstreamConsumer = JSON.parse(read(contract.downstream_consumer_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 4) {
  failures.push("forum notification recipient context contract must use schema_version=4");
}
if (
  contract.task !== "FORUM-20L" ||
  contract.upstream_task !== "FORUM-20K" ||
  contract.downstream_task !== "FORUM-20M" ||
  contract.consumer_task !== "FORUM-20N" ||
  contract.downstream_consumer_task !== "FORUM-20O"
) {
  failures.push("forum notification recipient context contract must connect FORUM-20K/L/M/N/O");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("recipient context source publication must not claim unexecuted evidence");
}

for (const delivered of [
  "bounded_request",
  "caller_read_semantics",
  "caller_tenant_validation",
  "system_or_service_caller",
  "recipient_read_semantics",
  "recipient_tenant_validation",
  "exact_user_actor_validation",
  "role_and_permission_snapshot_validation",
  "authenticated_topic_viewer_conversion",
  "typed_missing_capability",
  "retryable_failure_mapping",
  "inline_contract_tests",
  "host_adapter_implementation",
  "host_runtime_publication",
  "notification_source_factory_consumption",
  "recipient_target_open_authorization",
  "recipient_mention_description_authorization",
  "recipient_mention_audience_authorization",
  "shared_consumer_resolution_helper",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification recipient context contract must record ${delivered} as delivered`);
  }
}
for (const residual of [
  "recipient-specific topic-created subscription filtering before pagination",
  "initially non-public topic-created descriptor materialization",
  "profile privacy and blocking policy",
  "trust channel and group facts host adapters",
  "final notification creation and delivery authorization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification recipient context contract must keep ${residual} explicitly open`);
  }
}
for (const staleResidual of [
  "host recipient context adapter implementation",
  "host runtime publication of the recipient context capability",
  "notification source factory consumption of the recipient context capability",
  "recipient-specific target-open authorization for non-public topics and replies",
]) {
  if (contract.not_delivered?.includes(staleResidual)) {
    failures.push(`forum notification recipient context contract must remove delivered residual ${staleResidual}`);
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
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20O") {
  failures.push("forum recipient context contract must require the canonical ledger through FORUM-20O");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum recipient context contract must require FORUM-20H through FORUM-20O delivered sections");
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
  requireText(plan, "FORUM-20A-O provide", "synchronized canonical plan must advance the FORUM-20 ledger through O");
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
  "pub const FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY",
  "pub const FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY_UNAVAILABLE",
  "pub struct ForumNotificationRecipientContextRequest",
  "pub trait ForumNotificationRecipientContextPort: Send + Sync",
  "pub type SharedForumNotificationRecipientContextPort",
  "pub struct ForumNotificationRecipientContext",
  "pub struct ForumNotificationRecipientContextResolver",
  "ForumNotificationRecipientContextRequest::new(tenant_id, recipient_id)",
  ".require_policy(PortCallPolicy::read())",
  "PortActorKind::System | PortActorKind::Service",
  "context.actor.kind != PortActorKind::User",
  "SecurityContext::try_from_port_context(&recipient_context)",
  "ForumTopicAudienceViewer::authenticated(self.security, self.port_context)",
  "ForumError::capability_unavailable(",
  "ForumError::capability_failure(",
  "recipient_context_resolver_builds_exact_topic_viewer",
  "recipient_context_resolver_rejects_foreign_actor",
  "recipient_context_resolver_reports_missing_capability",
]) {
  requireText(owner, marker, `forum notification recipient context owner is missing ${marker}`);
}
for (const forbidden of [
  "sea_orm",
  "DatabaseConnection",
  "crate::entities",
  "forum_user",
  "forum_profile",
  "forum_channel",
  "forum_group",
  "HostRuntimeContext",
]) {
  rejectText(owner, forbidden, `forum notification recipient context owner must remain storage and host neutral instead of ${forbidden}`);
}
for (const marker of [
  "pub mod notification_recipient;",
  "FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY",
  "ForumNotificationRecipientContextPort",
  "ForumNotificationRecipientContextResolver",
  "SharedForumNotificationRecipientContextPort",
]) {
  requireText(crate, marker, `forum crate surface is missing ${marker}`);
}

if (
  upstream.schema_version !== 6 ||
  upstream.task !== "FORUM-20K" ||
  upstream.downstream_task !== "FORUM-20O" ||
  upstream.composition?.exact_richer_public_owner !== true ||
  upstream.composition?.recipient_specific_target_open !== true ||
  upstream.composition?.recipient_specific_mention_description !== true ||
  upstream.composition?.recipient_specific_mention_audience !== true
) {
  failures.push("FORUM-20L recipient context capability must remain grounded in FORUM-20K visibility composition through FORUM-20O");
}
if (
  downstream.schema_version !== 3 ||
  downstream.task !== "FORUM-20M" ||
  downstream.upstream_task !== "FORUM-20L" ||
  downstream.downstream_task !== "FORUM-20N" ||
  downstream.consumer_task !== "FORUM-20O" ||
  downstream.composition?.server_adapter !== true ||
  downstream.composition?.runtime_extension_publication !== true ||
  downstream.composition?.notification_source_factory_consumption !== true ||
  downstream.composition?.recipient_mention_description_authorization !== true ||
  downstream.composition?.recipient_mention_audience_authorization !== true
) {
  failures.push("FORUM-20L recipient context capability must remain synchronized with FORUM-20M host composition through FORUM-20O");
}
if (
  consumer.schema_version !== 2 ||
  consumer.task !== "FORUM-20N" ||
  consumer.upstream_task !== "FORUM-20M" ||
  consumer.downstream_task !== "FORUM-20O" ||
  consumer.composition?.exact_recipient_resolution !== true ||
  consumer.composition?.recipient_specific_topic_open !== true ||
  consumer.composition?.recipient_specific_reply_open !== true ||
  consumer.composition?.recipient_specific_mention_description !== true ||
  consumer.composition?.recipient_specific_mention_audience !== true
) {
  failures.push("FORUM-20L recipient context capability must remain synchronized with the FORUM-20N recipient consumer");
}
if (
  downstreamConsumer.schema_version !== 1 ||
  downstreamConsumer.task !== "FORUM-20O" ||
  downstreamConsumer.upstream_task !== "FORUM-20N" ||
  downstreamConsumer.composition?.exact_mention_recipient_resolution !== true ||
  downstreamConsumer.composition?.recipient_specific_mention_description !== true ||
  downstreamConsumer.composition?.recipient_specific_mention_audience !== true
) {
  failures.push("FORUM-20L recipient context capability must remain synchronized with the FORUM-20O mention consumer");
}

for (const marker of [
  "## `FORUM-20` — ACL and visibility inheritance",
  "notifications, search, SEO and deep links must call the same",
]) {
  requireText(plan, marker, `canonical Forum plan is missing the visibility boundary ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum notification recipient context verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification recipient context contract is source-ready.");
