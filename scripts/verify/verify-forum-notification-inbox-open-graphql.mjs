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
  read("crates/rustok-forum/contracts/forum-notification-inbox-open-graphql.json") || "{}",
);
const ownerGraphql = read(contract.notifications_graphql_file ?? "");
const ownerPort = read(contract.notifications_storefront_port ?? "");
const ownerOpen = read(contract.notifications_open_service ?? "");
const transport = read(contract.storefront_transport_file ?? "");
const graphqlAdapter = read(contract.storefront_graphql_adapter_file ?? "");
const nativeAdapter = read(contract.storefront_native_adapter_file ?? "");
const ui = read(contract.storefront_ui_file ?? "");
const readme = read(contract.storefront_readme ?? "");
const proof = read(contract.storefront_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const canonicalPlan = read(contract.canonical_plan ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AL" ||
  contract.upstream_task !== "FORUM-20AK"
) {
  failures.push("open GraphQL contract must connect FORUM-20AK/20AL with schema_version=1");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("open GraphQL contract must not claim unexecuted evidence");
}

for (const key of [
  "owner_open_graphql_query",
  "typed_open_decision",
  "optional_route_wire",
  "bounded_notification_id",
  "non_nil_notification_id",
  "human_user_required",
  "service_principal_rejected",
  "auth_tenant_match_required",
  "module_enabled_required",
  "admission_before_identifier_validation",
  "context_derived_tenant_scope",
  "context_derived_recipient_scope",
  "graphql_request_excludes_owner_identity",
  "five_second_read_deadline",
  "storefront_channel_context",
  "permission_claim_forwarding",
  "storefront_port_reused",
  "notification_owner_filter",
  "current_policy_reauthorization",
  "current_source_reauthorization",
  "non_oracular_unavailable",
  "owner_route_only",
  "allowed_route_required",
  "dual_path_open_authorization",
  "native_open_path",
  "graphql_open_path",
  "no_transport_fallback",
  "compatibility_open_wrapper",
  "explicit_context_open_function",
  "raw_native_open_alias",
  "ui_navigation_allowed_only",
  "safe_graphql_error_mapping",
  "source_contract_proof",
  "storefront_readme_updated",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`open GraphQL contract must record ${key}`);
  }
}
for (const key of [
  "parallel_inbox_service",
  "direct_storefront_database_query",
  "group_state_graphql_parity",
  "local_storage",
  "shadow_inbox",
  "channel_delivery",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`open GraphQL contract must keep ${key} false`);
  }
}

for (const sync of [contract.canonical_plan_sync, contract.notifications_local_plan_sync]) {
  if (sync?.status !== "pending" || sync.required_ledger_through !== "FORUM-20AL") {
    failures.push("Forum and Notifications ledgers must remain pending through FORUM-20AL");
  }
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");

for (const marker of [
  "pub enum GqlNotificationInboxOpenDecision",
  "pub struct GqlNotificationInboxOpenAuthorization",
  "async fn notification_inbox_authorize_open",
  "let scope = authenticated_scope(ctx)?;",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "parse_notification_id(notification_id.as_str())?",
  "MAX_NOTIFICATION_ID_BYTES",
  ".filter(|notification_id| !notification_id.is_nil())",
  ".authorize_open(",
  "scope.port_context(\"open\")",
  "NotificationInboxStorefrontOpenRequest { notification_id }",
  "NotificationInboxStorefrontOpenDecision::Allowed { route }",
  "route: Some(route.as_str().to_string())",
  "NotificationInboxStorefrontOpenDecision::Unavailable",
  "route: None",
  "if !auth.is_human_user_principal()",
  "if auth.tenant_id != tenant.id",
  "actor: auth.port_actor()",
  ".with_deadline(GRAPHQL_READ_DEADLINE)",
  ".with_channel(\"storefront\")",
  "context = context.with_claim(claim.clone())",
  "PUBLIC_UNAVAILABLE_MESSAGE",
]) {
  requireText(ownerGraphql, marker, `owner GraphQL is missing ${marker}`);
}

const openResolver = between(
  ownerGraphql,
  "async fn notification_inbox_authorize_open",
  "#[derive(Clone)]",
  "open GraphQL resolver",
);
const authIndex = openResolver.indexOf("let scope = authenticated_scope(ctx)?;");
const moduleIndex = openResolver.indexOf("require_module_enabled(ctx, MODULE_SLUG).await?");
const parseIndex = openResolver.indexOf("parse_notification_id(notification_id.as_str())?");
const portIndex = openResolver.indexOf(".authorize_open(");
if (!(authIndex >= 0 && moduleIndex > authIndex && parseIndex > moduleIndex && portIndex > parseIndex)) {
  failures.push("open GraphQL resolver must admit auth/module before UUID validation and owner access");
}
const signature = between(
  ownerGraphql,
  "async fn notification_inbox_authorize_open",
  ") -> Result<GqlNotificationInboxOpenAuthorization>",
  "open GraphQL signature",
);
for (const forbidden of ["tenant_id", "recipient_id", "user_id"]) {
  rejectText(signature, forbidden, `open GraphQL request must not accept ${forbidden}`);
}
rejectText(ownerGraphql, "async_graphql::Error::new(error.to_string())", "owner GraphQL must not expose raw errors");

for (const marker of [
  "PortCallPolicy::read()",
  "NotificationInboxOpenService",
  "NotificationInboxStorefrontOpenRequest",
]) {
  requireText(ownerPort, marker, `owner storefront port is missing ${marker}`);
}
for (const marker of [
  "find_by_id(request.notification_id)",
  "TenantId.eq(request.tenant_id)",
  "RecipientId.eq(request.recipient_id)",
  "NotificationRecipientPolicyDecision::Suppress",
  "authorize_target_open",
  "NotificationOpenAuthorization::Unavailable",
]) {
  requireText(ownerOpen, marker, `owner open service is missing ${marker}`);
}

for (const marker of [
  "query NotificationStorefrontAuthorizeOpen",
  "$notificationId: String!",
  "notificationInboxAuthorizeOpen",
  "decision",
  "route",
  "OpenAuthorizationVariables",
  "OpenDecisionWire",
  "pub async fn authorize_open",
  "OpenDecisionWire::Allowed",
  "OpenDecisionWire::Unavailable",
  "notification inbox open response is invalid",
]) {
  requireText(graphqlAdapter, marker, `storefront GraphQL adapter is missing ${marker}`);
}
const adapterProduction = graphqlAdapter.split("#[cfg(test)]")[0];
for (const forbidden of ["tenantId", "recipientId", "userId", "serde_json::Value"]) {
  rejectText(adapterProduction, forbidden, `open GraphQL adapter must not expose ${forbidden}`);
}

for (const marker of [
  "authorize_notification_open as authorize_notification_open_native",
  "pub async fn authorize_notification_open_selected",
  "notifications.storefront.open_authorization",
  "selected_storefront_read_transport_path()",
  "authorize_notification_open_native(native_request)",
  "graphql_adapter::authorize_open",
  "pub async fn authorize_notification_open(",
  "current_storefront_transport_context()",
]) {
  requireText(transport, marker, `selected open transport is missing ${marker}`);
}
rejectText(transport, "fallback_failed", "selected open authorization must not add fallback");
requireText(transport, "apply_notification_group_state,", "native group-state command must remain exported");
rejectText(transport, "apply_notification_group_state_selected", "group-state GraphQL parity must remain pending");

for (const marker of [
  "notification_storefront_open_native",
  "authenticated_context(\"open\", None).await?",
  "Uuid::parse_str(request.notification_id.as_str())",
  ".authorize_open(",
]) {
  requireText(nativeAdapter, marker, `native open adapter is missing ${marker}`);
}
const nativeAuth = nativeAdapter.indexOf('authenticated_context("open", None).await?');
const nativeParse = nativeAdapter.indexOf("Uuid::parse_str(request.notification_id.as_str())");
if (!(nativeAuth >= 0 && nativeParse > nativeAuth)) {
  failures.push("native open adapter must continue authenticating before UUID validation");
}

for (const marker of [
  "authorize_notification_open(NotificationStorefrontOpenRequest",
  "Ok(NotificationStorefrontOpenDecision::Allowed { route })",
  "navigate_to_route(route.as_str())",
  "Ok(NotificationStorefrontOpenDecision::Unavailable)",
]) {
  requireText(ui, marker, `grouped UI is missing ${marker}`);
}

for (const marker of [
  "owner_open_query_authenticates_before_bounded_identifier_validation",
  "open_decision_is_non_oracular_and_route_is_allowed_only",
  "storefront_open_graphql_request_exposes_only_notification_identity",
  "open_transport_is_selected_without_fallback_and_ui_navigates_only_after_allowed",
]) {
  requireText(proof, marker, `open GraphQL source proof is missing ${marker}`);
}
for (const marker of [
  "notificationInboxAuthorizeOpen",
  "Fresh GraphQL open authorization accepts only one bounded non-nil notification UUID",
  "Missing, foreign, suppressed, or no-longer-openable notifications",
  "open authorization is now dual-path",
  "group-state commands remain native-only",
]) {
  requireText(readme, marker, `storefront README is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AL notification open GraphQL authorization",
  "notificationInboxAuthorizeOpen",
  "NotificationInboxStorefrontPort::authorize_open",
  "ALLOWED",
  "UNAVAILABLE",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AK" ||
  upstream.composition?.open_authorization_graphql_parity !== false ||
  !upstream.not_delivered?.includes("GraphQL notification open authorization")
) {
  failures.push("FORUM-20AL must close the FORUM-20AK open authorization residual");
}
if (
  contract.composition?.group_state_graphql_parity !== false ||
  !contract.not_delivered?.includes("GraphQL group-state commands")
) {
  failures.push("FORUM-20AL must keep GraphQL group-state commands pending");
}

if (failures.length > 0) {
  console.error("Forum notification open GraphQL verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification open GraphQL contract is source-ready.");
