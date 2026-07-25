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

const contract = JSON.parse(read("crates/rustok-forum/contracts/forum-notification-recipient-host-runtime.json") || "{}");
const adapter = read(contract.adapter_file ?? "");
const services = read(contract.services_file ?? "");
const runtime = read(contract.runtime_composition_file ?? "");
const source = read(contract.notification_source_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const downstream = JSON.parse(read(contract.downstream_contract ?? "") || "{}");
const mention = JSON.parse(read(contract.mention_consumer_contract ?? "") || "{}");
const subscriptions = JSON.parse(read(contract.topic_subscription_consumer_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 4) failures.push("forum notification recipient host runtime contract must use schema_version=4");
if (contract.task !== "FORUM-20M" || contract.upstream_task !== "FORUM-20L" || contract.downstream_task !== "FORUM-20N" || contract.mention_consumer_task !== "FORUM-20O" || contract.topic_subscription_consumer_task !== "FORUM-20P") failures.push("host runtime contract must connect FORUM-20L/M/N/O/P");
if (contract.policy?.permission_bound !== 512) failures.push("recipient permission claims must remain bounded at 512");
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") failures.push("host runtime contract must not claim unexecuted evidence");

for (const field of [
  "server_adapter", "active_user_lookup", "tenant_user_predicates", "rbac_role_snapshot",
  "rbac_permission_snapshot", "exact_user_port_actor", "bounded_permission_claims",
  "read_metadata_preserved", "runtime_extension_publication",
  "publication_before_notification_source_materialization", "feature_guarded_server_module",
  "inline_contract_tests", "notification_source_factory_consumption",
  "recipient_target_open_authorization", "recipient_mention_description_authorization",
  "recipient_mention_audience_authorization", "recipient_topic_subscription_audience_authorization",
]) if (contract.composition?.[field] !== true) failures.push(`host runtime contract must record ${field}=true`);

for (const residual of [
  "initially non-public topic-created descriptor materialization", "profile privacy and blocking policy",
  "trust channel and group facts host adapters", "final notification creation and delivery authorization",
  "search index SEO and deep-link migration", "PostgreSQL and cross-consumer runtime evidence",
]) if (!contract.not_delivered?.includes(residual)) failures.push(`host runtime contract must keep ${residual} open`);

const slices = ["FORUM-20H", "FORUM-20I", "FORUM-20J", "FORUM-20K", "FORUM-20L", "FORUM-20M", "FORUM-20N", "FORUM-20O", "FORUM-20P"];
const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20P" || JSON.stringify(sync.required_delivered_sections) !== JSON.stringify(slices)) failures.push("host runtime contract must require FORUM-20H through FORUM-20P");
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") failures.push("pending plan boundary must remain FORUM-20G");
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded in FORUM-20A-G");
  for (const slice of slices) rejectText(plan, `### Delivered in \`${slice}\``, `canonical plan contains ${slice}; update plan sync metadata`);
} else if (sync.status !== "synchronized") failures.push("canonical_plan_sync.status must be pending or synchronized");

for (const marker of [
  "pub(crate) struct ServerForumNotificationRecipientContextPort",
  "impl ForumNotificationRecipientContextPort for ServerForumNotificationRecipientContextPort",
  "caller_context.require_policy(PortCallPolicy::read())",
  "PortActorKind::System | PortActorKind::Service",
  "users::Column::TenantId.eq(request.tenant_id)",
  "users::Column::Id.eq(request.recipient_id)",
  "users::Column::Status.eq(UserStatus::Active)",
  "RbacService::get_user_permissions(", "RbacService::get_user_role(",
  "PortActor::user(request.recipient_id.to_string())", "MAX_RECIPIENT_PERMISSION_CLAIMS: usize = 512",
  "recipient_context.deadline_ms = caller_context.deadline_ms",
]) requireText(adapter, marker, `host recipient adapter is missing ${marker}`);
for (const forbidden of ["rustok_forum::entities", "forum_category_audience_", "forum_topic_audience_", "SecurityContext::new(", "Rbac::permissions_for_role"]) rejectText(adapter, forbidden, `host adapter must not bypass owner boundaries with ${forbidden}`);
for (const marker of ["#[cfg(feature = \"mod-forum\")]", "pub mod forum_notification_recipient_context;"]) requireText(services, marker, `server services surface is missing ${marker}`);
for (const marker of ["ServerForumNotificationRecipientContextPort::shared(", "extensions.insert(recipient_context)", "materialize_notification_source_registry(&mut extensions, &host)"]) requireText(runtime, marker, `server runtime composition is missing ${marker}`);
if (runtime.indexOf("extensions.insert(recipient_context)") > runtime.indexOf("materialize_notification_source_registry(&mut extensions, &host)")) failures.push("recipient context must be published before source materialization");
for (const marker of [
  "host.shared_get::<SharedForumNotificationRecipientContextPort>()",
  "async fn resolve_recipient_viewer(", "target_open_context(&request)",
  "load_mention_target_for_recipient(&event, &payload, MENTION_DESCRIBE_ACTOR)",
  "load_mention_target_for_recipient(&event, &payload, MENTION_AUDIENCE_ACTOR)",
  "async fn topic_subscription_recipient_visible(", "TOPIC_SUBSCRIPTION_AUDIENCE_ACTOR",
]) requireText(source, marker, `notification source consumer is missing ${marker}`);

if (upstream.schema_version !== 5 || upstream.task !== "FORUM-20L" || upstream.topic_subscription_consumer_task !== "FORUM-20P" || upstream.composition?.recipient_topic_subscription_audience_authorization !== true) failures.push("FORUM-20M must remain synchronized with FORUM-20L/P");
if (downstream.schema_version !== 3 || downstream.task !== "FORUM-20N" || downstream.downstream_chain_task !== "FORUM-20P" || downstream.composition?.recipient_specific_topic_subscription_audience !== true) failures.push("FORUM-20M must remain synchronized with FORUM-20N/P");
if (mention.schema_version !== 2 || mention.task !== "FORUM-20O" || mention.downstream_task !== "FORUM-20P") failures.push("FORUM-20M must remain synchronized with FORUM-20O/P");
if (subscriptions.schema_version !== 1 || subscriptions.task !== "FORUM-20P" || subscriptions.composition?.exact_recipient_context_per_scanned_subscription !== true) failures.push("FORUM-20M must remain synchronized with FORUM-20P");

if (failures.length > 0) {
  console.error("Forum notification recipient host runtime verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Forum notification recipient host runtime contract is source-ready.");
