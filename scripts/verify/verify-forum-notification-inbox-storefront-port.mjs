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
function between(source, start, end, label) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  if (from < 0 || to < 0 || to <= from) {
    failures.push(`${label}: bounded source section is missing`);
    return "";
  }
  return source.slice(from, to);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-notification-inbox-storefront-port.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_port_file ?? "");
const library = read(contract.notifications_lib_file ?? "");
const cargo = read(contract.notifications_cargo_file ?? "");
const readme = read(contract.notifications_readme ?? "");
const liveContract = read(contract.notifications_live_contract ?? "");
const storefrontReadme = read(contract.notifications_storefront_readme ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("storefront-port contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AG" || contract.upstream_task !== "FORUM-20AF") {
  failures.push("storefront-port contract must connect FORUM-20AF/20AG");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("storefront-port contract must not claim unexecuted evidence");
}

for (const key of [
  "transport_neutral_storefront_port",
  "in_process_port_factory",
  "context_derived_tenant_scope",
  "context_derived_recipient_scope",
  "request_dtos_exclude_owner_identity",
  "user_actor_required",
  "read_policy_before_owner_access",
  "write_policy_before_owner_access",
  "write_idempotency_required",
  "unread_count_delegate",
  "group_summary_delegate",
  "group_items_delegate",
  "open_authorization_delegate",
  "group_state_delegate",
  "safe_port_error_mapping",
  "authorized_group_reads_preserved",
  "exact_group_state_invariants_preserved",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`storefront-port contract must record ${key}`);
  }
}
for (const key of [
  "native_server_adapter",
  "graphql_adapter",
  "grouped_leptos_ui",
  "host_route_registration",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`storefront-port contract must keep ${key} false`);
  }
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20AG") {
  failures.push("canonical ledger must be required through FORUM-20AG");
}
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan sync must identify FORUM-20G");
  }
  requireText(plan, "FORUM-20A-G provide", "pending canonical sync must remain grounded through G");
  rejectText(plan, "### Delivered in `FORUM-20AG`", "canonical plan sync status is stale");
}
const localSync = contract.notifications_local_plan_sync ?? {};
if (localSync.required_ledger_through !== "FORUM-20AG") {
  failures.push("Notifications local ledger must be required through FORUM-20AG");
}
if (localSync.status === "pending") {
  if (localSync.current_plan_through !== "FORUM-20AA") {
    failures.push("pending Notifications ledger sync must identify FORUM-20AA");
  }
  requireText(localPlan, "### `FORUM-20AA`", "pending local sync must remain grounded through AA");
  rejectText(localPlan, "### `FORUM-20AG`", "Notifications local plan sync status is stale");
}

for (const marker of [
  "pub trait NotificationInboxStorefrontPort: Send + Sync",
  "async fn unread_count",
  "async fn list_group_summaries",
  "async fn list_group_items",
  "async fn authorize_open",
  "async fn apply_group_state",
  "pub struct NotificationInboxStorefrontService",
  "pub fn in_process_notification_inbox_storefront_port",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "context.require_policy(policy)?",
  "context.actor.kind != PortActorKind::User",
  "context.tenant_id.as_str()",
  "context.actor.id.as_str()",
  "Uuid::parse_str(value)",
  ".filter(|value| !value.is_nil())",
  "NotificationInboxUnreadCountService::new",
  "NotificationInboxGroupSummaryService::new",
  "NotificationInboxGroupListService::new",
  "NotificationInboxOpenService::new",
  "NotificationInboxGroupStateService::new",
  "NotificationInboxUnreadCountRequest",
  "NotificationInboxGroupSummaryRequest",
  "NotificationInboxGroupListRequest",
  "NotificationInboxOpenRequest",
  "NotificationInboxGroupStateRequest",
  "fn notification_error_to_port_error",
  "PortErrorKind::Unavailable",
  "PortError::invariant_violation",
]) {
  requireText(owner, marker, `storefront port owner is missing ${marker}`);
}

for (const [start, end, label] of [
  [
    "pub struct NotificationInboxStorefrontGroupSummaryRequest",
    "pub struct NotificationInboxStorefrontGroupItemsRequest",
    "group-summary request",
  ],
  [
    "pub struct NotificationInboxStorefrontGroupItemsRequest",
    "pub struct NotificationInboxStorefrontOpenRequest",
    "group-items request",
  ],
  [
    "pub struct NotificationInboxStorefrontOpenRequest",
    "pub enum NotificationInboxStorefrontOpenDecision",
    "open request",
  ],
  [
    "pub struct NotificationInboxStorefrontGroupStateRequest",
    "/// Transport-neutral owner boundary",
    "group-state request",
  ],
]) {
  const request = between(owner, start, end, label);
  rejectText(request, "tenant_id", `${label} must not accept tenant identity`);
  rejectText(request, "recipient_id", `${label} must not accept recipient identity`);
}

const implementation = between(
  owner,
  "impl NotificationInboxStorefrontPort for NotificationInboxStorefrontService",
  "#[derive(Clone, Copy)]",
  "storefront port implementation",
);
if ((implementation.match(/PortCallPolicy::read\(\)/g) ?? []).length !== 4) {
  failures.push("storefront port must apply read policy to exactly four read operations");
}
if ((implementation.match(/PortCallPolicy::write\(\)/g) ?? []).length !== 1) {
  failures.push("storefront port must apply write policy to exactly one command operation");
}
for (const forbidden of [
  "#[server(",
  "leptos_axum",
  "async_graphql",
  "AuthContext",
  "TenantContext",
  "RequestContext",
  "delivery_attempt::",
  "notification::ActiveModel",
]) {
  rejectText(owner, forbidden, `transport-neutral storefront port must not use ${forbidden}`);
}

requireText(cargo, "rustok-api.workspace = true", "Notifications crate must depend on rustok-api");
for (const marker of [
  "mod inbox_storefront_port;",
  "NotificationInboxStorefrontPort",
  "NotificationInboxStorefrontService",
  "NotificationInboxStorefrontGroupSummaryRequest",
  "NotificationInboxStorefrontGroupItemsRequest",
  "NotificationInboxStorefrontOpenRequest",
  "NotificationInboxStorefrontGroupStateRequest",
  "in_process_notification_inbox_storefront_port",
]) {
  requireText(library, marker, `Notifications library is missing ${marker}`);
}

for (const marker of [
  "storefront_reads_derive_exact_scope_and_delegate_authorized_owners",
  "storefront_writes_require_idempotency_and_preserve_exact_state_invariants",
  "storefront_scope_policy_and_owner_errors_fail_closed_without_mutation",
  "PortActor::service(\"storefront-host\")",
  "notifications.storefront.user_required",
  "notifications.storefront.tenant_invalid",
  "notifications.storefront.user_invalid",
  "port.deadline_required",
  "port.idempotency_key_required",
  "NOTIFICATION_VALIDATION_ERROR",
  "another group must remain unchanged",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `SQLite storefront-port proof is missing ${marker}`);
}

requireText(
  readme,
  "External transport adapters and grouped UI remain closed",
  "Notifications README must keep native adapters and UI closed",
);
requireText(
  liveContract,
  "external inbox transport adapters",
  "Notifications live contract must keep external adapters pending",
);
for (const marker of [
  "owner transport-neutral inbox port exists",
  "explicit unavailable state",
  "native server adapter",
  "does not invent unread state",
]) {
  requireText(storefrontReadme, marker, `Notifications storefront README is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AG notification inbox storefront port",
  "PortContext.tenant_id",
  "PortActorKind::User",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "tests/inbox_storefront_port_sqlite.rs",
  "does not add a Leptos server function",
]) {
  requireText(note, marker, `storefront-port owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AF" ||
  upstream.composition?.bounded_group_state_owner !== true ||
  upstream.composition?.external_transport_adapter !== false ||
  !upstream.not_delivered?.includes("external inbox transport and grouped UI adapters")
) {
  failures.push("FORUM-20AG must narrow the FORUM-20AF external transport residual");
}

if (failures.length > 0) {
  console.error("Forum notification storefront-port verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification storefront port contract is source-ready.");
