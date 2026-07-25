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
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification recipient context contract must use schema_version=1");
}
if (contract.task !== "FORUM-20L" || contract.upstream_task !== "FORUM-20K") {
  failures.push("forum notification recipient context contract must belong to FORUM-20L after FORUM-20K");
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
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification recipient context contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "host recipient context adapter implementation",
  "host runtime publication of the recipient context capability",
  "notification source factory consumption of the recipient context capability",
  "recipient-specific audience filtering for non-public topics",
  "recipient-specific target-open authorization for non-public topics and replies",
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

const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20L") {
  failures.push("forum recipient context contract must require the canonical ledger through FORUM-20L");
}
if (
  JSON.stringify(planSync.required_delivered_sections) !==
  JSON.stringify(["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L"])
) {
  failures.push("forum recipient context contract must require FORUM-20H/I/J/K/L delivered sections");
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
  for (const slice of ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L"]) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(
    plan,
    "FORUM-20A-L provide",
    "synchronized canonical plan must advance the FORUM-20 ledger through L",
  );
  for (const slice of ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L"]) {
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
  "context.require_policy(PortCallPolicy::read())",
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
  rejectText(
    owner,
    forbidden,
    `forum notification recipient context owner must remain storage and host neutral instead of ${forbidden}`,
  );
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
  upstream.schema_version !== 4 ||
  upstream.task !== "FORUM-20K" ||
  upstream.composition?.exact_richer_public_owner !== true
) {
  failures.push("FORUM-20L recipient context capability must remain grounded in delivered FORUM-20K public visibility composition");
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
