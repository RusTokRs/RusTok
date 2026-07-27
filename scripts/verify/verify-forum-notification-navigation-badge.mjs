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
  "crates/rustok-forum/contracts/forum-notification-navigation-badge.json";
const downstreamPath =
  "crates/rustok-forum/contracts/forum-notification-inbox-grouped-graphql.json";
const contract = JSON.parse(read(contractPath) || "{}");
const downstreamAbsolute = path.join(repoRoot, downstreamPath);
const downstream = existsSync(downstreamAbsolute)
  ? JSON.parse(readFileSync(downstreamAbsolute, "utf8") || "{}")
  : null;
const groupedGraphqlDelivered =
  downstream?.schema_version === 1 &&
  downstream?.task === "FORUM-20AK" &&
  downstream?.upstream_task === "FORUM-20AJ" &&
  downstream?.composition?.owner_group_summary_graphql_query === true &&
  downstream?.composition?.owner_group_items_graphql_query === true &&
  downstream?.composition?.dual_path_group_summary_read === true &&
  downstream?.composition?.dual_path_group_items_read === true;

const ownerGraphql = read(contract.notifications_graphql_file ?? "");
const ownerLib = read(contract.notifications_lib_file ?? "");
const ownerCargo = read(contract.notifications_cargo_file ?? "");
const manifest = read(contract.notifications_manifest ?? "");
const storefrontCargo = read(contract.storefront_cargo_file ?? "");
const transport = read(contract.storefront_transport_file ?? "");
const graphqlAdapter = read(contract.storefront_graphql_adapter_file ?? "");
const navigation = read(contract.storefront_navigation_file ?? "");
const storefrontLib = read(contract.storefront_library_file ?? "");
const i18n = read(contract.storefront_i18n_file ?? "");
const localeEn = read(contract.storefront_locale_en ?? "");
const localeRu = read(contract.storefront_locale_ru ?? "");
const storefrontReadme = read(contract.storefront_readme ?? "");
const proof = read(contract.storefront_proof ?? "");
const hostRegistry = read(contract.host_registry_file ?? "");
const hostApp = read(contract.host_app_file ?? "");
const hostHeader = read(contract.host_header_file ?? "");
const hostBuild = read(contract.host_build_file ?? "");
const hostProof = read(contract.host_slot_proof ?? "");
const xtaskUi = read(contract.xtask_ui_metadata_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const canonicalPlan = read(contract.canonical_plan ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("navigation badge contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AJ" || contract.upstream_task !== "FORUM-20AI") {
  failures.push("navigation badge contract must connect FORUM-20AI/20AJ");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("navigation badge contract must not claim unexecuted evidence");
}

for (const key of [
  "owner_graphql_query",
  "exact_unread_count_delegate",
  "human_user_required",
  "service_principal_rejected",
  "auth_tenant_match_required",
  "module_enabled_required",
  "context_derived_tenant_scope",
  "context_derived_recipient_scope",
  "safe_graphql_error_mapping",
  "graphql_request_excludes_owner_identity",
  "dual_path_navigation_transport",
  "native_navigation_path",
  "graphql_navigation_path",
  "no_transport_fallback",
  "module_owned_navigation_component",
  "localized_navigation_copy",
  "context_derived_module_route",
  "exact_navigation_badge",
  "zero_count_link",
  "best_effort_error_isolation",
  "generic_header_actions_slot",
  "manifest_component_registration",
  "host_has_no_notifications_import",
  "primary_navigation_preserved",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`navigation badge contract must record ${key}`);
  }
}
for (const key of [
  "grouped_inbox_graphql_parity",
  "open_authorization_graphql_parity",
  "group_state_graphql_parity",
  "local_storage",
  "shadow_inbox",
  "channel_delivery",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`historical navigation badge contract must keep ${key} false`);
  }
}

for (const sync of [contract.canonical_plan_sync, contract.notifications_local_plan_sync]) {
  if (sync?.status !== "pending" || sync.required_ledger_through !== "FORUM-20AJ") {
    failures.push("historical Forum and Notifications ledgers must remain pending through FORUM-20AJ");
  }
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");

for (const marker of [
  "pub struct NotificationsQuery",
  "pub struct GqlNotificationInboxUnreadCount",
  "async fn notification_inbox_unread_count",
  "authenticated_scope(ctx)?",
  "if !auth.is_human_user_principal()",
  "if auth.tenant_id != tenant.id",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "NotificationInboxUnreadCountService::new(db)",
  "tenant_id: scope.tenant_id",
  "recipient_id: scope.recipient_id",
  "NOTIFICATION_INBOX_USER_REQUIRED",
  "NOTIFICATION_INBOX_TENANT_MISMATCH",
  "NOTIFICATION_INBOX_UNAVAILABLE",
  "PUBLIC_UNAVAILABLE_MESSAGE",
]) {
  requireText(ownerGraphql, marker, `owner unread GraphQL is missing ${marker}`);
}
rejectText(ownerGraphql, "async_graphql::Error::new(error.to_string())", "owner GraphQL must not expose raw errors");
requireText(
  ownerCargo,
  "async-graphql = { workspace = true, optional = true }",
  "Notifications owner must declare optional async-graphql",
);
requireText(ownerLib, "pub mod graphql;", "Notifications library must expose GraphQL");

for (const marker of [
  "query = \"graphql::NotificationsQuery\"",
  "id = \"notifications-header-action\"",
  "component = \"NotificationNavigation\"",
  "slot = \"header_actions\"",
  "leptos_locales_path = \"storefront/locales\"",
]) {
  requireText(manifest, marker, `Notifications manifest is missing ${marker}`);
}

for (const marker of [
  "selected_storefront_read_transport_path",
  "UiTransportPath::NativeServer",
  "UiTransportPath::Graphql",
  "load_notification_navigation_unread_count",
  "load_notification_unread_count_selected",
  "graphql_adapter::load_navigation_unread_count",
]) {
  requireText(transport, marker, `navigation transport is missing ${marker}`);
}
rejectText(transport, "fallback_failed", "navigation transport must not add fallback");
for (const marker of [
  "query NotificationStorefrontNavigationUnreadCount",
  "notificationInboxUnreadCount",
  "unreadCount",
  "execute_graphql",
]) {
  requireText(graphqlAdapter, marker, `navigation GraphQL adapter is missing ${marker}`);
}
const adapterProduction = graphqlAdapter.split("#[cfg(test)]")[0];
for (const forbidden of ["tenantId", "recipientId", "userId"]) {
  rejectText(adapterProduction, forbidden, `navigation GraphQL request must not expose ${forbidden}`);
}

for (const marker of [
  "pub fn NotificationNavigation()",
  "use_context::<UiRouteContext>()",
  "module_route_base(\"notifications\")",
  "use_context::<AuthContext>()",
  "AuthContext::get_token",
  "AuthContext::get_tenant",
  "Resource::new_blocking",
  "NotificationUnreadBadge",
  "data-notification-navigation=\"true\"",
  "data-notification-navigation=\"unavailable\"",
]) {
  requireText(navigation, marker, `navigation component is missing ${marker}`);
}
for (const forbidden of ["/modules/notifications", "localStorage", "gloo_storage"]) {
  rejectText(navigation, forbidden, `navigation component must not use ${forbidden}`);
}
requireText(storefrontLib, "pub use ui::navigation::NotificationNavigation;", "storefront crate must export NotificationNavigation");
for (const marker of [
  "leptos-auth.workspace = true",
  "rustok-graphql.workspace = true",
  "rustok-ui-core.workspace = true",
  "rustok-ui-i18n-leptos.workspace = true",
  "rustok-ui-transport.workspace = true",
]) {
  requireText(storefrontCargo, marker, `storefront Cargo contract is missing ${marker}`);
}
for (const marker of [
  "LeptosUiMessages::new",
  "include_str!(\"../locales/en.json\")",
  "include_str!(\"../locales/ru.json\")",
]) {
  requireText(i18n, marker, `storefront i18n adapter is missing ${marker}`);
}
for (const marker of ["notifications", "navigation", "label", "unread"]) {
  requireText(localeEn, marker, `English locale is missing ${marker}`);
  requireText(localeRu, marker, `Russian locale is missing ${marker}`);
}

for (const marker of ["HeaderActions", "header_actions", "StorefrontSlot::HeaderActions"]) {
  requireText(hostRegistry + hostBuild + xtaskUi, marker, `host slot contracts are missing ${marker}`);
}
requireText(hostApp, "components_for_slot(StorefrontSlot::HeaderActions", "storefront layout must compose header actions");
requireText(hostHeader, "action_views: Vec<AnyView>", "storefront header must accept action views");
requireText(hostHeader, "{navigation}", "primary navigation must remain present");
for (const forbidden of ["rustok_notifications_storefront", "NotificationNavigation"]) {
  rejectText(hostApp + hostHeader, forbidden, `host must not import ${forbidden}`);
}

for (const marker of [
  "manifest_registers_module_owned_header_action_without_host_imports",
  "navigation_uses_context_route_and_best_effort_exact_count",
]) {
  requireText(proof, marker, `navigation badge proof is missing ${marker}`);
}
requireText(hostProof, "HeaderActions", "host slot proof must cover HeaderActions");

for (const marker of [
  "`NotificationNavigation` is a module-owned no-prop header action",
  "module_route_base(\"notifications\")",
  "zero count still leaves the localized Notifications link available",
]) {
  requireText(storefrontReadme, marker, `storefront README is missing ${marker}`);
}
if (groupedGraphqlDelivered) {
  for (const marker of [
    "unread count, grouped summaries, and exact-group item pages use one selected read",
    "notificationInboxGroupSummaries",
    "notificationInboxGroupItems",
    "Fresh notification open authorization and group-state",
  ]) {
    requireText(storefrontReadme, marker, `FORUM-20AK README state is missing ${marker}`);
  }
  if (
    !downstream.not_delivered?.includes("GraphQL notification open authorization") ||
    !downstream.not_delivered?.includes("GraphQL group-state commands")
  ) {
    failures.push("FORUM-20AK must keep open and command GraphQL parity pending");
  }
} else {
  requireText(
    storefrontReadme,
    "full grouped inbox, open authorization, and group-state commands are still native-only",
    "pre-FORUM-20AK README state is missing",
  );
}

for (const marker of [
  "# FORUM-20AJ notification storefront navigation badge",
  "notification_inbox_unread_count",
  "header_actions",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `FORUM-20AJ owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AI" ||
  upstream.composition?.global_navigation_badge_composition !== false ||
  !upstream.not_delivered?.includes("global storefront navigation or header unread-badge slot")
) {
  failures.push("FORUM-20AJ must close the FORUM-20AI navigation residual");
}

if (failures.length > 0) {
  console.error("Forum notification navigation badge verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification navigation badge contract is source-ready.");
