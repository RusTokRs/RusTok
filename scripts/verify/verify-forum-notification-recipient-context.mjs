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

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-notification-recipient-context.json") || "{}");
const owner = read(contract.owner_file ?? "");
const crate = read(contract.crate_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const downstream = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const targetOpen = JSON.parse(read(contract.target_open_consumer_contract ?? "") || "{}");
const mention = JSON.parse(read(contract.mention_consumer_contract ?? "") || "{}");
const subscriptions = JSON.parse(read(contract.topic_subscription_consumer_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 5) failures.push("forum notification recipient context contract must use schema_version=5");
if (
  contract.task !== "FORUM-20L" ||
  contract.upstream_task !== "FORUM-20K" ||
  contract.downstream_task !== "FORUM-20M" ||
  contract.target_open_consumer_task !== "FORUM-20N" ||
  contract.mention_consumer_task !== "FORUM-20O" ||
  contract.topic_subscription_consumer_task !== "FORUM-20P"
) failures.push("forum recipient context contract must connect FORUM-20K/L/M/N/O/P");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("recipient context contract must not claim unexecuted evidence");

for (const field of [
  "bounded_request", "caller_read_semantics", "caller_tenant_validation", "system_or_service_caller",
  "recipient_read_semantics", "recipient_tenant_validation", "exact_user_actor_validation",
  "role_and_permission_snapshot_validation", "authenticated_topic_viewer_conversion",
  "typed_missing_capability", "retryable_failure_mapping", "inline_contract_tests",
  "host_adapter_implementation", "host_runtime_publication", "notification_source_factory_consumption",
  "recipient_target_open_authorization", "recipient_mention_description_authorization",
  "recipient_mention_audience_authorization", "recipient_topic_subscription_audience_authorization",
  "shared_consumer_resolution_helper",
]) if (contract.composition?.[field] !== true) failures.push(`recipient context contract must record ${field}=true`);

for (const residual of [
  "initially non-public topic-created descriptor materialization", "profile privacy and blocking policy",
  "trust channel and group facts host adapters", "final notification creation and delivery authorization",
  "search index SEO and deep-link migration", "PostgreSQL and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`recipient context contract must keep ${residual} open`);

const slices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20P" || JSON.stringify(sync.required_delivered_sections) !== JSON.stringify(slices)) failures.push("recipient context contract must require FORUM-20H through FORUM-20P");
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") failures.push("pending plan boundary must remain FORUM-20G");
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded in FORUM-20A-G");
  for (const slice of slices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan contains ${slice}; update plan sync metadata`);
} else if (sync.status === "synchronized") {
  requireText(plan, "FORUM-20A-P provide", "synchronized plan must advance through P");
} else failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "pub const FORUM_NOTIFICATION_RECIPIENT_CONTEXT_CAPABILITY",
  "pub struct ForumNotificationRecipientContextRequest",
  "pub trait ForumNotificationRecipientContextPort: Send + Sync",
  "pub type SharedForumNotificationRecipientContextPort",
  "pub struct ForumNotificationRecipientContextResolver",
  "ForumNotificationRecipientContextRequest::new(tenant_id, recipient_id)",
  ".require_policy(PortCallPolicy::read())",
  "PortActorKind::System | PortActorKind::Service",
  "SecurityContext::try_from_port_context(&recipient_context)",
  "ForumTopicAudienceViewer::authenticated(self.security, self.port_context)",
  "ForumError::capability_unavailable(",
  "ForumError::capability_failure(",
]) requireText(owner, marker, `recipient context owner is missing ${marker}`);
for (const forbidden of ["sea_orm", "DatabaseConnection", "crate::entities", "HostRuntimeContext"]) rejectText(owner, forbidden, `recipient context owner must remain neutral instead of ${forbidden}`);
for (const marker of ["pub mod notification_recipient;", "ForumNotificationRecipientContextResolver", "SharedForumNotificationRecipientContextPort"]) requireText(crate, marker, `forum crate surface is missing ${marker}`);

if (upstream.schema_version !== 7 || upstream.task !== "FORUM-20K" || upstream.downstream_task !== "FORUM-20P" || upstream.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20L must remain grounded in FORUM-20K/P visibility composition");
if (downstream.schema_version !== 4 || downstream.task !== "FORUM-20M" || downstream.topic_subscription_consumer_task !== "FORUM-20P" || downstream.composition?.recipient_topic_subscription_audience_authorization !== true) failures.push("FORUM-20L must remain synchronized with FORUM-20M/P host composition");
if (targetOpen.schema_version !== 3 || targetOpen.task !== "FORUM-20N" || targetOpen.downstream_chain_task !== "FORUM-20P" || targetOpen.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20L must remain synchronized with FORUM-20N/P consumer chain");
if (mention.schema_version !== 2 || mention.task !== "FORUM-20O" || mention.downstream_task !== "FORUM-20P" || mention.composition?.topic_created_subscription_audience_downstream !== true) failures.push("FORUM-20L must remain synchronized with FORUM-20O/P consumer chain");
if (subscriptions.schema_version !== 1 || subscriptions.task !== "FORUM-20P" || subscriptions.composition?.exact_recipient_context_per_scanned_subscription !== true) failures.push("FORUM-20L must remain synchronized with FORUM-20P subscription consumer");

if (failures.length > 0) {
  console.error("Forum notification recipient context verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification recipient context contract is source-ready.");
