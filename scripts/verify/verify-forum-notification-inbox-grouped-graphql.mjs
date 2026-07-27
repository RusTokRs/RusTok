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

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-inbox-grouped-graphql.json") || "{}",
);
const ownerGraphql = read(contract.notifications_graphql_file ?? "");
const ownerLib = read(contract.notifications_lib_file ?? "");
const ownerCargo = read(contract.notifications_cargo_file ?? "");
const manifest = read(contract.notifications_manifest ?? "");
const ownerPort = read(contract.notifications_storefront_port ?? "");
const transport = read(contract.storefront_transport_file ?? "");
const graphqlAdapter = read(contract.storefront_graphql_adapter_file ?? "");
const ui = read(contract.storefront_ui_file ?? "");
const readme = read(contract.storefront_readme ?? "");
const proof = read(contract.storefront_proof ?? "");
const serverCargo = read(contract.server_cargo_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const canonicalPlan = read(contract.canonical_plan ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");

if (contract.schema_version !== 1 || contract.task !== "FORUM-20AK") {
  failures.push("grouped GraphQL contract must identify FORUM-20AK with schema_version=1");
}
if (contract.upstream_task !== "FORUM-20AJ") {
  failures.push("grouped GraphQL contract must follow FORUM-20AJ");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("grouped GraphQL contract must not claim unexecuted evidence");
}

for (const key of [
  "owner_group_summary_graphql_query",
  "owner_group_items_graphql_query",
  "owner_unread_graphql_query_preserved",
  "manifest_runtime_data_factory",
  "server_feature_wiring",
  "host_database_reused",
  "materialized_source_registry_reused",
  "recipient_policy_runtime_reused",
  "storefront_port_reused",
  "human_user_required",
  "service_principal_rejected",
  "auth_tenant_match_required",
  "module_enabled_required",
  "context_derived_tenant_scope",
  "context_derived_recipient_scope",
  "graphql_request_excludes_owner_identity",
  "five_second_read_deadline",
  "storefront_channel_context",
  "permission_claim_forwarding",
  "bounded_limit_conversion",
  "owner_cursor_validation",
  "owner_group_key_validation",
  "current_policy_reauthorization",
  "current_source_reauthorization",
  "non_oracular_suppression",
  "typed_state_enum",
  "typed_priority_enum",
  "bounded_template_key_value_wire",
  "dual_path_unread_read",
  "dual_path_group_summary_read",
  "dual_path_group_items_read",
  "native_read_path",
  "graphql_read_path",
  "no_transport_fallback",
  "compatibility_read_wrappers",
  "explicit_context_read_functions",
  "safe_graphql_error_mapping",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`grouped GraphQL contract must record ${key}`);
  }
}
for (const key of [
  "parallel_inbox_service",
  "direct_storefront_database_query",
  "arbitrary_json_wire",
  "open_authorization_graphql_parity",
  "group_state_graphql_parity",
  "local_storage",
  "shadow_inbox",
  "channel_delivery",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`grouped GraphQL contract must keep ${key} false`);
  }
}

for (const sync of [contract.canonical_plan_sync, contract.notifications_local_plan_sync]) {
  if (sync?.status !== "pending" || sync.required_ledger_through !== "FORUM-20AK") {
    failures.push("Forum and Notifications ledgers must remain pending through FORUM-20AK");
  }
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");

for (const marker of [
  "pub struct NotificationsGraphqlRuntimeData",
  "pub fn attach_schema_data(",
  "inputs.shared_get::<Arc<NotificationSourceRegistry>>()",
  "inputs.shared_get::<NotificationRecipientPolicyRuntime>()",
  "in_process_notification_inbox_storefront_port(",
  "inputs.db_clone()",
  "async fn notification_inbox_unread_count",
  "async fn notification_inbox_group_summaries",
  "async fn notification_inbox_group_items",
  "let scope = authenticated_scope(ctx)?;",
  "if !auth.is_human_user_principal()",
  "if auth.tenant_id != tenant.id",
  "actor: auth.port_actor()",
  ".with_deadline(GRAPHQL_READ_DEADLINE)",
  ".with_channel(\"storefront\")",
  "context = context.with_claim(claim.clone())",
  ".list_group_summaries(",
  ".list_group_items(",
  "u16::try_from(limit)",
  ".into_inner()",
  "GqlNotificationTemplateField { key, value }",
  "let PortError {",
  "PUBLIC_UNAVAILABLE_MESSAGE",
]) {
  requireText(ownerGraphql, marker, `owner GraphQL is missing ${marker}`);
}

for (const [start, end, label] of [
  ["async fn notification_inbox_group_summaries", ") -> Result<GqlNotificationInboxGroupSummaryPage>", "summary signature"],
  ["async fn notification_inbox_group_items", ") -> Result<GqlNotificationInboxGroupItemsPage>", "items signature"],
]) {
  const signature = between(ownerGraphql, start, end, label);
  for (const forbidden of ["tenant_id", "recipient_id", "user_id"]) {
    rejectText(signature, forbidden, `${label} must not accept ${forbidden}`);
  }
}

const runtimeFactory = between(
  ownerGraphql,
  "pub fn attach_schema_data(",
  "#[derive(Clone, Debug, Eq, PartialEq, SimpleObject)]",
  "runtime factory",
);
for (const forbidden of [
  "NotificationSourceRegistry::default",
  "NotificationRecipientPolicyRuntime::new",
  "NotificationInboxStorefrontService::new",
]) {
  rejectText(runtimeFactory, forbidden, `runtime factory must not create ${forbidden}`);
}

for (const marker of [
  "PortCallPolicy::read()",
  "PortActorKind::User",
  "NotificationInboxGroupSummaryService",
  "NotificationInboxGroupListService",
  "notification inbox capability is unavailable",
]) {
  requireText(ownerPort, marker, `owner storefront port is missing ${marker}`);
}

for (const marker of [
  "default = []",
  "server = [\"rustok-api/server\", \"dep:async-graphql\"]",
  "async-graphql = { workspace = true, optional = true }",
]) {
  requireText(ownerCargo, marker, `Notifications Cargo feature contract is missing ${marker}`);
}
requireText(ownerLib, "#[cfg(feature = \"server\")]\npub mod graphql;", "GraphQL module must be server-gated");
requireText(serverCargo, "rustok-notifications/server", "server feature must forward Notifications server support");
for (const marker of [
  "query = \"graphql::NotificationsQuery\"",
  "runtime_data_factory = \"graphql::attach_schema_data\"",
]) {
  requireText(manifest, marker, `Notifications manifest is missing ${marker}`);
}

for (const marker of [
  "query NotificationStorefrontGroupSummaries",
  "query NotificationStorefrontGroupItems",
  "$cursor: String",
  "$limit: Int",
  "$groupKey: String!",
  "$state: NotificationInboxItemState",
  "templateData { key value }",
  "GroupItemStateWire",
  "ItemStateWire",
  "PriorityWire",
]) {
  requireText(graphqlAdapter, marker, `storefront GraphQL adapter is missing ${marker}`);
}
const adapterProduction = graphqlAdapter.split("#[cfg(test)]")[0];
for (const forbidden of ["tenantId", "recipientId", "userId", "serde_json::Value"]) {
  rejectText(adapterProduction, forbidden, `storefront GraphQL adapter must not expose ${forbidden}`);
}

for (const marker of [
  "pub struct NotificationStorefrontTransportContext",
  "selected_storefront_read_transport_path",
  "UiTransportPath::NativeServer",
  "UiTransportPath::Graphql",
  "load_notification_unread_count_selected",
  "load_notification_group_summaries_selected",
  "load_notification_group_items_selected",
  "current_storefront_transport_context",
  "load_notification_group_summaries_native",
  "load_notification_group_items_native",
  "graphql_adapter::load_group_summaries",
  "graphql_adapter::load_group_items",
]) {
  requireText(transport, marker, `selected storefront transport is missing ${marker}`);
}
rejectText(transport, "fallback_failed", "selected storefront reads must not add fallback");

for (const marker of [
  "load_notification_unread_count_selected",
  "load_notification_group_summaries(",
  "load_notification_group_items(",
  "authorize_notification_open",
  "apply_notification_group_state",
  "on_refresh.run(feedback)",
]) {
  requireText(ui, marker, `grouped storefront UI is missing ${marker}`);
}
for (const forbidden of ["localStorage", "gloo_storage", "sea_orm::", "serde_json::Value"]) {
  rejectText(ui, forbidden, `grouped storefront UI must not use ${forbidden}`);
}

for (const marker of [
  "manifest_composes_owner_runtime_data_without_host_registry_code",
  "grouped_owner_queries_derive_identity_and_delegate_to_storefront_port",
  "grouped_graphql_wire_is_bounded_and_transport_neutral",
  "existing_grouped_ui_calls_selected_read_wrappers_only",
]) {
  requireText(proof, marker, `grouped GraphQL source proof is missing ${marker}`);
}
for (const marker of [
  "grouped summaries, exact-group item pages, fresh open authorization",
  "notificationInboxGroupSummaries",
  "notificationInboxGroupItems",
  "bounded template data as ordered key/value fields",
  "Fresh notification open authorization and group-state",
]) {
  requireText(readme, marker, `storefront README is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AK grouped notification inbox GraphQL reads",
  "graphql::attach_schema_data",
  "notificationInboxGroupSummaries",
  "NotificationInboxStorefrontPort",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AJ" ||
  upstream.composition?.grouped_inbox_graphql_parity !== false ||
  !upstream.not_delivered?.includes("GraphQL grouped inbox summary and item paging")
) {
  failures.push("FORUM-20AK must close the FORUM-20AJ grouped GraphQL read residual");
}
if (
  !contract.not_delivered?.includes("GraphQL notification open authorization") ||
  !contract.not_delivered?.includes("GraphQL group-state commands")
) {
  failures.push("FORUM-20AK must keep GraphQL open and command parity pending");
}

if (failures.length > 0) {
  console.error("Forum notification grouped GraphQL verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification grouped GraphQL contract is source-ready.");
