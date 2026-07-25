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
  "crates/rustok-forum/contracts/forum-notification-recipient-host-runtime.json";
const contract = JSON.parse(read(contractPath) || "{}");
const adapter = read(contract.adapter_file ?? "");
const services = read(contract.services_file ?? "");
const runtime = read(contract.runtime_composition_file ?? "");
const notificationSource = read(contract.notification_source_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const downstream = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const mentionConsumer = JSON.parse(read(contract.mention_consumer_contract ?? "") || "{}");
const subscriptionConsumer = JSON.parse(read(contract.topic_subscription_consumer_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 4) {
  failures.push("forum notification recipient host runtime contract must use schema_version=4");
}
if (
  contract.task !== "FORUM-20M" ||
  contract.upstream_task !== "FORUM-20L" ||
  contract.downstream_task !== "FORUM-20N" ||
  contract.mention_consumer_task !== "FORUM-20O" ||
  contract.topic_subscription_consumer_task !== "FORUM-20P"
) {
  failures.push("forum notification recipient host runtime contract must connect FORUM-20L/M/N/O/P");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("recipient host runtime publication must not claim unexecuted evidence");
}
if (contract.policy?.permission_bound !== 512) {
  failures.push("forum notification recipient permission claims must remain bounded at 512");
}

for (const delivered of [
  "server_adapter",
  "active_user_lookup",
  "tenant_user_predicates",
  "rbac_role_snapshot",
  "rbac_permission_snapshot",
  "exact_user_port_actor",
  "bounded_permission_claims",
  "read_metadata_preserved",
  "runtime_extension_publication",
  "publication_before_notification_source_materialization",
  "feature_guarded_server_module",
  "inline_contract_tests",
  "notification_source_factory_consumption",
  "recipient_target_open_authorization",
  "recipient_mention_description_authorization",
  "recipient_mention_audience_authorization",
  "recipient_topic_subscription_audience_authorization",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification recipient host runtime contract must record ${delivered} as delivered`);
  }
}
for (const residual of [
  "initially non-public topic-created descriptor materialization",
  "profile privacy and blocking policy",
  "trust channel and group facts host adapters",
  "final notification creation and delivery authorization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification recipient host runtime contract must keep ${residual} explicitly open`);
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
  failures.push("forum recipient host runtime contract must require the canonical ledger through FORUM-20P");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("forum recipient host runtime contract must require FORUM-20H through FORUM-20P delivered sections");
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
  "pub(crate) struct ServerForumNotificationRecipientContextPort",
  "impl ForumNotificationRecipientContextPort for ServerForumNotificationRecipientContextPort",
  "caller_context.require_policy(PortCallPolicy::read())",
  "PortActorKind::System | PortActorKind::Service",
  "users::Column::TenantId.eq(request.tenant_id)",
  "users::Column::Id.eq(request.recipient_id)",
  "users::Column::Status.eq(UserStatus::Active)",
  "RbacService::get_user_permissions(",
  "RbacService::get_user_role(",
  "PortActor::user(request.recipient_id.to_string())",
  "MAX_RECIPIENT_PERMISSION_CLAIMS: usize = 512",
  "permissions.is_empty()",
  "permissions.len() > MAX_RECIPIENT_PERMISSION_CLAIMS",
  "recipient_context.causation_id = caller_context.causation_id.clone()",
  "recipient_context.traceparent = caller_context.traceparent.clone()",
  "recipient_context.deadline_ms = caller_context.deadline_ms",
  "recipient_context_preserves_bounded_read_metadata",
  "recipient_context_rejects_empty_authority",
  "recipient_context_rejects_user_caller",
]) {
  requireText(adapter, marker, `forum notification recipient host adapter is missing ${marker}`);
}
for (const forbidden of [
  "rustok_forum::entities",
  "forum_category_audience_",
  "forum_topic_audience_",
  "forum_user_",
  "forum_profile",
  "forum_channel",
  "forum_group",
  "SecurityContext::new(",
  "Rbac::permissions_for_role",
]) {
  rejectText(adapter, forbidden, `forum notification recipient host adapter must not bypass owner boundaries with ${forbidden}`);
}
for (const marker of [
  "#[cfg(feature = \"mod-forum\")]",
  "pub mod forum_notification_recipient_context;",
]) {
  requireText(services, marker, `server services surface is missing ${marker}`);
}
for (const marker of [
  "ServerForumNotificationRecipientContextPort::shared(",
  "extensions.insert(recipient_context)",
  "extensions.contains::<rustok_forum::SharedForumNotificationRecipientContextPort>()",
]) {
  requireText(runtime, marker, `server runtime composition is missing ${marker}`);
}
const publicationIndex = runtime.indexOf("extensions.insert(recipient_context)");
const materializationIndex = runtime.indexOf("materialize_notification_source_registry(&mut extensions, &host)");
if (publicationIndex < 0 || materializationIndex < 0 || publicationIndex > materializationIndex) {
  failures.push("recipient context capability must be published before notification source materialization");
}

for (const marker of [
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()",
  "recipient_context_port: Option<SharedForumNotificationRecipientContextPort>",
  "async fn resolve_recipient_viewer(",
  "ForumNotificationRecipientContextResolver::new(Some(port))",
  "target_open_context(&request)",
  "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)",
  "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)",
  "async fn topic_subscription_recipient_visible(",
  "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
  "recipient.into_topic_viewer()",
]) {
  requireText(notificationSource, marker, `notification source consumer is missing ${marker}`);
}

if (
  upstream.schema_version !== 5 ||
  upstream.task !== "FORUM-20L" ||
  upstream.downstream_task !== "FORUM-20M" ||
  upstream.target_open_consumer_task !== "FORUM-20N" ||
  upstream.mention_consumer_task !== "FORUM-20O" ||
  upstream.topic_subscription_consumer_task !== "FORUM-20P" ||
  upstream.composition?.host_adapter_implementation !== true ||
  upstream.composition?.host_runtime_publication !== true ||
  upstream.composition?.notification_source_factory_consumption !== true ||
  upstream.composition?.recipient_mention_description_authorization !== true ||
  upstream.composition?.recipient_mention_audience_authorization !== true ||
  upstream.composition?.recipient_topic_subscription_audience_authorization !== true
) {
  failures.push("FORUM-20M host runtime must remain synchronized with the FORUM-20L capability contract through FORUM-20P");
}
if (
  downstream.schema_version !== 3 ||
  downstream.task !== "FORUM-20N" ||
  downstream.upstream_task !== "FORUM-20M" ||
  downstream.downstream_task !== "FORUM-20O" ||
  downstream.downstream_chain_task !== "FORUM-20P" ||
  downstream.composition?.factory_recipient_capability_lookup !== true ||
  downstream.composition?.recipient_specific_topic_open !== true ||
  downstream.composition?.recipient_specific_reply_open !== true ||
  downstream.composition?.recipient_specific_mention_description !== true ||
  downstream.composition?.recipient_specific_mention_audience !== true ||
  downstream.composition?.recipient_specific_topic_subscription_audience !== true
) {
  failures.push("FORUM-20M host runtime must remain synchronized with the FORUM-20N recipient consumer through P");
}
if (
  mentionConsumer.schema_version !== 2 ||
  mentionConsumer.task !== "FORUM-20O" ||
  mentionConsumer.upstream_task !== "FORUM-20N" ||
  mentionConsumer.downstream_task !== "FORUM-20P" ||
  mentionConsumer.composition?.recipient_specific_mention_description !== true ||
  mentionConsumer.composition?.recipient_specific_mention_audience !== true ||
  mentionConsumer.composition?.topic_created_subscription_audience_downstream !== true
) {
  failures.push("FORUM-20M host runtime must remain synchronized with the FORUM-20O mention consumer through P");
}
if (
  subscriptionConsumer.schema_version !== 1 ||
  subscriptionConsumer.task !== "FORUM-20P" ||
  subscriptionConsumer.composition?.exact_recipient_context_per_scanned_subscription !== true ||
  subscriptionConsumer.composition?.recipient_specific_topic_visibility !== true
) {
  failures.push("FORUM-20M host runtime must remain synchronized with the FORUM-20P subscription consumer");
}

if (failures.length > 0) {
  console.error("Forum notification recipient host runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification recipient host runtime contract is source-ready.");
