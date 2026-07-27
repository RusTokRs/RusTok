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
  "crates/rustok-forum/contracts/forum-notification-inbox-native-storefront-adapter.json";
const contract = JSON.parse(read(contractPath) || "{}");
const core = read(contract.storefront_core_file ?? "");
const transport = read(contract.storefront_transport_file ?? "");
const adapter = read(contract.storefront_native_adapter_file ?? "");
const library = read(contract.storefront_lib_file ?? "");
const cargo = read(contract.storefront_cargo_file ?? "");
const storefrontReadme = read(contract.storefront_readme ?? "");
const proof = read(contract.storefront_test ?? "");
const ownerPort = read(contract.owner_port_file ?? "");
const hostRuntime = read(contract.host_runtime_file ?? "");
const runtimeExtensions = read(contract.runtime_extension_file ?? "");
const serverComposition = read(contract.server_composition_file ?? "");
const appCargo = read(contract.application_storefront_cargo_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const canonicalPlan = read(contract.canonical_plan ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");
const ownerReadme = read(contract.notifications_owner_readme ?? "");
const liveContract = read(contract.notifications_live_contract ?? "");
const ui = read("crates/rustok-notifications/storefront/src/ui/leptos.rs");

if (contract.schema_version !== 1) {
  failures.push("native storefront adapter contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AH" || contract.upstream_task !== "FORUM-20AG") {
  failures.push("native storefront adapter contract must connect FORUM-20AG/20AH");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("native storefront adapter contract must not claim unexecuted evidence");
}

for (const key of [
  "native_server_adapter",
  "existing_storefront_ssr_composition",
  "unread_count_server_function",
  "group_summary_server_function",
  "group_items_server_function",
  "open_authorization_server_function",
  "group_state_server_function",
  "client_transport_wrappers",
  "request_dtos_exclude_owner_identity",
  "auth_context_extraction",
  "service_principal_rejected",
  "canonical_auth_port_actor_mapping",
  "tenant_context_extraction",
  "request_context_extraction",
  "auth_tenant_match_required",
  "authenticated_user_port_actor",
  "read_deadline_context",
  "write_idempotency_context",
  "permission_claim_context",
  "storefront_channel_context",
  "authentication_before_open_uuid_validation",
  "host_source_registry_reuse",
  "host_recipient_policy_reuse",
  "owner_storefront_service_delegation",
  "generic_runtime_unavailable_error",
  "safe_owner_port_error_mapping",
  "wasm_safe_transport_models",
  "non_ssr_explicit_failure",
  "transport_contract_test",
  "owner_contract_note",
  "storefront_readme_updated",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`native storefront adapter contract must record ${key}`);
  }
}
for (const key of [
  "grouped_leptos_ui",
  "hydrated_inbox_state",
  "graphql_adapter",
  "shadow_inbox",
  "channel_delivery",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`native storefront adapter contract must keep ${key} false`);
  }
}

if (
  contract.canonical_plan_sync?.status !== "pending" ||
  contract.canonical_plan_sync?.required_ledger_through !== "FORUM-20AH" ||
  contract.canonical_plan_sync?.current_plan_through !== "FORUM-20G"
) {
  failures.push("canonical Forum ledger must remain pending from FORUM-20G through FORUM-20AH");
}
if (
  contract.notifications_local_plan_sync?.status !== "pending" ||
  contract.notifications_local_plan_sync?.required_ledger_through !== "FORUM-20AH" ||
  contract.notifications_local_plan_sync?.current_plan_through !== "FORUM-20AA"
) {
  failures.push("Notifications ledger must remain pending from FORUM-20AA through FORUM-20AH");
}
if (
  contract.notifications_owner_docs_sync?.status !== "pending" ||
  contract.notifications_owner_docs_sync?.required_contract_through !== "FORUM-20AH"
) {
  failures.push("large Notifications owner docs must record pending sync through FORUM-20AH");
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");
requireText(
  ownerReadme,
  "External transport adapters and grouped UI remain closed",
  "pending owner README sync must remain explicit",
);
requireText(liveContract, "grouped UI", "pending live-contract sync must remain explicit");

for (const marker of [
  "pub struct NotificationStorefrontUnreadCount",
  "pub struct NotificationStorefrontGroupSummaryRequest",
  "pub struct NotificationStorefrontGroupItemsRequest",
  "pub struct NotificationStorefrontOpenRequest",
  "pub struct NotificationStorefrontGroupStateCommand",
  "pub struct NotificationStorefrontItem",
  "pub struct NotificationStorefrontGroupSummaryPage",
  "pub struct NotificationStorefrontGroupItemsPage",
  "pub enum NotificationStorefrontOpenDecision",
  "pub struct NotificationStorefrontGroupStatePage",
  "pub idempotency_key: String",
  "pub template_data: BTreeMap<String, String>",
  "pub created_at: String",
]) {
  requireText(core, marker, `storefront core is missing ${marker}`);
}
for (const forbidden of [
  "pub tenant_id:",
  "pub recipient_id:",
  "pub user_id:",
  "Uuid",
  "DateTime",
  "NotificationTargetRoute",
]) {
  rejectText(core, forbidden, `wasm-safe storefront DTOs must not expose ${forbidden}`);
}

for (const marker of [
  "mod native_server_adapter;",
  "load_notification_unread_count",
  "load_notification_group_summaries",
  "load_notification_group_items",
  "authorize_notification_open",
  "apply_notification_group_state",
  "NotificationStorefrontState::foundation()",
]) {
  requireText(transport, marker, `storefront transport is missing ${marker}`);
}
requireText(library, "pub use core::*;", "storefront library must export transport models");
requireText(library, "pub use transport::*;", "storefront library must export native functions");

for (const endpoint of [
  "notifications/storefront/unread-count",
  "notifications/storefront/group-summaries",
  "notifications/storefront/group-items",
  "notifications/storefront/open",
  "notifications/storefront/group-state",
]) {
  requireText(adapter, endpoint, `native adapter is missing endpoint ${endpoint}`);
}
for (const marker of [
  "leptos_axum::extract::<AuthContext>()",
  "leptos_axum::extract::<TenantContext>()",
  "leptos_axum::extract::<RequestContext>()",
  "if auth.tenant_id != tenant.id",
  "if !auth.is_human_user_principal()",
  "let actor = auth.port_actor();",
  ".with_deadline(Duration::from_secs(5))",
  ".with_channel(\"storefront\")",
  "context = context.with_claim(permission.to_string())",
  "context = context.with_idempotency_key(idempotency_key)",
  "shared_get::<Arc<NotificationSourceRegistry>>()",
  "shared_get::<NotificationRecipientPolicyRuntime>()",
  "NotificationInboxStorefrontService::new(",
  "policy.policy_arc()",
  "const PUBLIC_CAPABILITY_UNAVAILABLE: &str = \"notification inbox capability is unavailable\";",
  ".ok_or_else(capability_unavailable)?",
  "ServerFnError::new(error.message)",
  ".unread_count(context)",
  ".list_group_summaries(",
  ".list_group_items(",
  ".authorize_open(",
  ".apply_group_state(",
  "notification storefront native transport requires the `ssr` feature",
]) {
  requireText(adapter, marker, `native adapter is missing ${marker}`);
}
rejectText(
  adapter,
  "PortActor::user(auth.user_id.to_string())",
  "native adapter must not relabel service principals as users",
);

const authIndex = adapter.indexOf('authenticated_context("open", None).await?');
const parseIndex = adapter.indexOf("Uuid::parse_str(request.notification_id.as_str())");
if (!(authIndex >= 0 && parseIndex > authIndex)) {
  failures.push("open endpoint must authenticate before UUID validation");
}
for (const forbidden of [
  "NotificationSourceRegistry::default",
  "NotificationSourceRegistry::new",
  "NotificationRecipientPolicyRuntime::new",
  "sea_orm::",
  "Entity::",
  "async_graphql",
  "view!",
]) {
  rejectText(adapter, forbidden, `native adapter must not use ${forbidden}`);
}

for (const marker of [
  '"rustok-api/server"',
  '"dep:leptos_axum"',
  '"dep:rustok-api"',
  '"dep:rustok-notifications"',
  'leptos_axum = { workspace = true, optional = true }',
  'rustok-api = { workspace = true, default-features = false, optional = true }',
  'rustok-notifications = { path = "..", optional = true }',
]) {
  requireText(cargo, marker, `storefront Cargo contract is missing ${marker}`);
}
requireText(appCargo, '"dep:rustok-notifications-storefront"', "application storefront must enable notifications package");
requireText(appCargo, '"rustok-notifications-storefront/ssr"', "application storefront must enable notifications SSR adapter");

for (const marker of [
  "native_storefront_requests_do_not_expose_owner_identity_fields",
  "group_state_command_retains_write_admission_input",
  "grouped_ui_remains_explicitly_unavailable_until_composed",
  "assert!(!object.contains_key(\"tenant_id\"))",
  "assert!(!object.contains_key(\"recipient_id\"))",
  "assert_eq!(encoded[\"action\"], \"mark_unread\")",
  "NotificationInboxAvailability::Unavailable",
]) {
  requireText(proof, marker, `storefront transport proof is missing ${marker}`);
}

for (const marker of [
  "pub trait NotificationInboxStorefrontPort",
  "PortCallPolicy::read()",
  "PortCallPolicy::write()",
  "PortActorKind::User",
  "NotificationInboxStorefrontService",
]) {
  requireText(ownerPort, marker, `upstream owner port is missing ${marker}`);
}
for (const marker of ["pub fn shared_get<T>", "pub fn db_clone", "with_extension_values"]) {
  requireText(hostRuntime, marker, `host runtime is missing ${marker}`);
}
requireText(runtimeExtensions, "pub fn apply_to_host_runtime", "runtime extensions must transfer values to HostRuntimeContext");
requireText(serverComposition, "materialize_notification_source_registry", "server must materialize source providers");
requireText(serverComposition, "extensions.insert(policy)", "server must compose recipient policy runtime");

for (const marker of [
  "native Leptos server-function adapter",
  "OAuth service principals",
  "AuthContext::port_actor",
  "HostRuntimeContext",
  "five-second deadline",
  "idempotency key",
  "grouped Leptos inbox view has not been delivered",
]) {
  requireText(storefrontReadme, marker, `storefront README is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AH native notification storefront adapter",
  "OAuth service principals are rejected",
  "authentication and tenant admission occur before notification",
  "Arc<NotificationSourceRegistry>",
  "NotificationRecipientPolicyRuntime",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

requireText(ui, "NotificationInboxAvailability::Unavailable", "grouped UI must remain unavailable");
rejectText(ui, "load_notification_group_summaries", "grouped UI must not be claimed in FORUM-20AH");

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AG" ||
  upstream.composition?.native_server_adapter !== false ||
  !upstream.not_delivered?.includes(
    "Notifications storefront native server function adapter and host composition",
  )
) {
  failures.push("FORUM-20AH must close the FORUM-20AG native-adapter residual");
}

if (failures.length > 0) {
  console.error("Forum notification native storefront adapter verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification native storefront adapter contract is source-ready.");
