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
    failures.push(`${label}: bounded section is missing`);
    return "";
  }
  return source.slice(from, to);
}

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-inbox-group-state-graphql.json") ||
    "{}",
);
const owner = read(contract.notifications_graphql_file ?? "");
const manifest = read(contract.notifications_manifest ?? "");
const port = read(contract.notifications_storefront_port ?? "");
const adapter = read(contract.storefront_graphql_adapter_file ?? "");
const transport = read(contract.storefront_transport_file ?? "");
const ui = read(contract.storefront_ui_file ?? "");
const proof = read(contract.storefront_proof ?? "");
const note = read(contract.owner_note ?? "");
const canonical = read(contract.canonical_plan ?? "");
const local = read(contract.notifications_local_plan ?? "");
const ownerReadme = read(contract.notifications_owner_readme ?? "");
const live = read(contract.notifications_live_contract ?? "");
const residual = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-inbox-open-graphql.json") || "{}",
);

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AN" ||
  contract.upstream_task !== "FORUM-20AM" ||
  contract.residual_task !== "FORUM-20AL"
) {
  failures.push("group-state GraphQL contract must connect FORUM-20AL/20AM/20AN");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("group-state GraphQL contract must not claim unexecuted evidence");
}
for (const key of [
  "owner_mutation_root",
  "typed_group_action",
  "bounded_progress_page",
  "human_user_required",
  "service_principal_rejected",
  "auth_tenant_match_required",
  "module_enabled_required",
  "admission_before_command_validation",
  "context_derived_tenant_scope",
  "context_derived_recipient_scope",
  "graphql_request_excludes_owner_identity",
  "five_second_write_deadline",
  "bounded_idempotency_key",
  "idempotency_forwarded_to_port_context",
  "storefront_channel_context",
  "permission_claim_forwarding",
  "storefront_port_reused",
  "owner_group_state_service_reused",
  "native_write_path",
  "graphql_write_path",
  "no_transport_fallback",
  "compatibility_write_wrapper",
  "ui_call_site_preserved",
  "authoritative_refresh_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`contract must record ${key}`);
  }
}
for (const key of [
  "direct_storefront_database_query",
  "parallel_state_service",
  "selected_id_bulk_graphql",
  "local_storage",
  "shadow_inbox",
  "channel_delivery",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`contract must keep ${key} false`);
  }
}

for (const marker of [
  "pub struct NotificationsMutation",
  "async fn notification_inbox_apply_group_state",
  "GqlNotificationInboxGroupStateAction",
  "GqlNotificationInboxGroupStatePage",
  "let scope = authenticated_scope(ctx)?;",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "parse_idempotency_key(idempotency_key)?",
  "MAX_IDEMPOTENCY_KEY_BYTES",
  'scope.write_port_context("group-state", idempotency_key)',
  ".with_deadline(deadline)",
  ".with_idempotency_key(idempotency_key)",
  '.with_channel("storefront")',
  ".apply_group_state(",
]) {
  requireText(owner, marker, `owner GraphQL is missing ${marker}`);
}
requireText(
  manifest,
  'mutation = "graphql::NotificationsMutation"',
  "manifest must publish mutation root",
);
requireText(port, "PortCallPolicy::write()", "owner port must enforce write policy");
requireText(
  port,
  "NotificationInboxGroupStateService",
  "owner port must reuse group state service",
);
rejectText(
  owner,
  "let value = value.trim();",
  "GraphQL idempotency keys must remain exact opaque values",
);

const mutation = between(
  owner,
  "async fn notification_inbox_apply_group_state",
  ") -> Result<GqlNotificationInboxGroupStatePage>",
  "group-state mutation signature",
);
for (const forbidden of ["tenant_id", "recipient_id", "user_id"]) {
  rejectText(mutation, forbidden, `mutation must not accept ${forbidden}`);
}
const mutationBody = between(
  owner,
  "async fn notification_inbox_apply_group_state",
  "#[derive(Clone)]",
  "group-state mutation body",
);
const authIndex = mutationBody.indexOf("let scope = authenticated_scope(ctx)?;");
const moduleIndex = mutationBody.indexOf(
  "require_module_enabled(ctx, MODULE_SLUG).await?",
);
const idempotencyIndex = mutationBody.indexOf("parse_idempotency_key(idempotency_key)?");
const portIndex = mutationBody.indexOf(".apply_group_state(");
if (
  !(
    authIndex >= 0 &&
    moduleIndex > authIndex &&
    idempotencyIndex > moduleIndex &&
    portIndex > idempotencyIndex
  )
) {
  failures.push("mutation must admit auth/module before command validation and owner access");
}

for (const marker of [
  "mutation NotificationStorefrontApplyGroupState",
  "$groupKey: String!",
  "$action: NotificationInboxGroupStateAction!",
  "$idempotencyKey: String!",
  "notificationInboxApplyGroupState",
  "GroupStateActionWire",
  "GroupStateVariables",
  "pub async fn apply_group_state",
  "scanned",
  "changed",
  "nextCursor",
  "hasMore",
]) {
  requireText(adapter, marker, `GraphQL adapter is missing ${marker}`);
}
const adapterProduction = adapter.split("#[cfg(test)]")[0];
for (const forbidden of ["tenantId", "recipientId", "userId", "serde_json::Value"]) {
  rejectText(adapterProduction, forbidden, `GraphQL adapter must not expose ${forbidden}`);
}

for (const marker of [
  "apply_notification_group_state as apply_notification_group_state_native",
  "selected_storefront_write_transport_path",
  "apply_notification_group_state_selected",
  "notifications.storefront.group_state",
  "apply_notification_group_state_native(native_command)",
  "graphql_adapter::apply_group_state",
  "pub async fn apply_notification_group_state(",
  "current_storefront_transport_context()",
]) {
  requireText(transport, marker, `selected write transport is missing ${marker}`);
}
rejectText(transport, "fallback_failed", "group-state transport must not add fallback");
requireText(ui, "apply_notification_group_state(", "UI must keep compatibility command wrapper");
requireText(ui, "on_refresh.run(feedback)", "UI must preserve authoritative refresh");

for (const marker of [
  "owner_mutation_admits_before_bounded_command_and_reuses_port",
  "graphql_wire_carries_typed_action_idempotency_and_progress_only",
  "selected_write_path_preserves_native_and_ui_without_fallback",
]) {
  requireText(proof, marker, `source proof is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AN notification group-state GraphQL commands",
  "source-ready / unvalidated",
  "notificationInboxApplyGroupState",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}
for (const marker of [
  "FORUM-20A-AN provide",
  "### Delivered in `FORUM-20AN`",
  "auth-reactive automatic grouped-inbox bootstrap refresh",
]) {
  requireText(canonical, marker, `canonical plan is missing ${marker}`);
}
for (const marker of ["### `FORUM-20AN`", "GraphQL CSR/headless path without fallback"]) {
  requireText(local, marker, `local plan is missing ${marker}`);
}
for (const marker of [
  "notificationInboxApplyGroupState",
  "typed actions and explicit",
  "auth-reactive automatic grouped bootstrap refresh",
]) {
  requireText(ownerReadme, marker, `owner README is missing ${marker}`);
}
for (const marker of [
  "GraphQL group-state mutations now delegate",
  "auth-reactive automatic grouped bootstrap refresh",
]) {
  requireText(live, marker, `live contract is missing ${marker}`);
}
if (
  residual.composition?.group_state_graphql_parity !== false ||
  !residual.not_delivered?.includes("GraphQL group-state commands")
) {
  failures.push("FORUM-20AN must close the historical FORUM-20AL group-state residual");
}

if (failures.length > 0) {
  console.error("Forum notification group-state GraphQL verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification group-state GraphQL contract is source-ready.");
