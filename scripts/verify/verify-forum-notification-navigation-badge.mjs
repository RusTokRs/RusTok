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
  "crates/rustok-forum/contracts/forum-notification-navigation-badge.json";
const contract = JSON.parse(read(contractPath) || "{}");
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
const centralManifestDocs = read(contract.central_manifest_docs ?? "");

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
  "deterministic_header_action_order",
  "host_has_no_notifications_import",
  "primary_navigation_preserved",
  "source_contract_proof",
  "host_slot_contract_proof",
  "storefront_readme_updated",
  "owner_contract_note",
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
    failures.push(`navigation badge contract must keep ${key} false`);
  }
}

for (const sync of [contract.canonical_plan_sync, contract.notifications_local_plan_sync]) {
  if (sync?.status !== "pending" || sync.required_ledger_through !== "FORUM-20AJ") {
    failures.push("Forum and Notifications ledgers must remain pending through FORUM-20AJ");
  }
}
if (contract.canonical_plan_sync?.current_plan_through !== "FORUM-20G") {
  failures.push("pending canonical plan sync must identify FORUM-20G");
}
if (contract.notifications_local_plan_sync?.current_plan_through !== "FORUM-20AA") {
  failures.push("pending Notifications plan sync must identify FORUM-20AA");
}
if (
  contract.notifications_owner_docs_sync?.status !== "pending" ||
  contract.notifications_owner_docs_sync?.required_contract_through !== "FORUM-20AJ"
) {
  failures.push("large Notifications owner docs must remain pending through FORUM-20AJ");
}
if (
  contract.central_manifest_docs_sync?.status !== "pending" ||
  contract.central_manifest_docs_sync?.required_slot !== "header_actions"
) {
  failures.push("central manifest docs must record pending header_actions synchronization");
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");
rejectText(
  centralManifestDocs,
  "`header_navigation`, `header_actions`, `home_after_hero`",
  "central manifest docs unexpectedly claim synchronized header_actions state",
);

for (const marker of [
  "pub struct NotificationsQuery",
  "pub struct GqlNotificationInboxUnreadCount",
  "async fn notification_inbox_unread_count",
  "ctx.data_opt::<AuthContext>()",
  "if !auth.is_human_user_principal()",
  "ctx.data_opt::<TenantContext>()",
  "if auth.tenant_id != tenant.id",
  "require_module_enabled(ctx, MODULE_SLUG).await?",
  "NotificationInboxUnreadCountService::new(db)",
  "tenant_id: tenant.id",
  "recipient_id: auth.user_id",
  "NOTIFICATION_INBOX_USER_REQUIRED",
  "NOTIFICATION_INBOX_TENANT_MISMATCH",
  "NOTIFICATION_INBOX_UNAVAILABLE",
  "PUBLIC_UNAVAILABLE_MESSAGE",
  "other.is_retryable()",
  "extensions.set(\"retryable\", retryable)",
]) {
  requireText(ownerGraphql, marker, `owner GraphQL is missing ${marker}`);
}
const ownerQuery = between(
  ownerGraphql,
  "async fn notification_inbox_unread_count",
  "fn map_notification_error",
  "owner unread-count GraphQL query",
);
for (const forbidden of [
  "tenant_id:",
  "recipient_id:",
  "user_id:",
  "PortActor::service",
  "NotificationSourceRegistry",
  "NotificationRecipientPolicyRuntime",
]) {
  if (forbidden.endsWith(":")) {
    const signature = between(
      ownerGraphql,
      "async fn notification_inbox_unread_count",
      ") -> Result<GqlNotificationInboxUnreadCount>",
      "owner GraphQL signature",
    );
    rejectText(signature, forbidden, `owner GraphQL request must not accept ${forbidden}`);
  } else {
    rejectText(ownerQuery, forbidden, `owner unread count must not require ${forbidden}`);
  }
}
const authIndex = ownerQuery.indexOf("ctx.data_opt::<AuthContext>()");
const humanIndex = ownerQuery.indexOf("if !auth.is_human_user_principal()");
const tenantIndex = ownerQuery.indexOf("ctx.data_opt::<TenantContext>()");
const moduleIndex = ownerQuery.indexOf("require_module_enabled(ctx, MODULE_SLUG).await?");
const databaseIndex = ownerQuery.indexOf("ctx\n            .data_opt::<DatabaseConnection>()");
if (!(authIndex >= 0 && humanIndex > authIndex && tenantIndex > humanIndex && moduleIndex > tenantIndex && databaseIndex > moduleIndex)) {
  failures.push("owner GraphQL must admit human auth and tenant before module/database access");
}
rejectText(
  ownerGraphql,
  "async_graphql::Error::new(error.to_string())",
  "owner GraphQL must not expose raw owner errors",
);
rejectText(ownerGraphql, "format!(\"{error}\")", "owner GraphQL must not format raw owner errors");
requireText(ownerCargo, "async-graphql.workspace = true", "Notifications owner must declare async-graphql");
requireText(ownerLib, "pub mod graphql;", "Notifications library must expose its GraphQL module");
requireText(ownerLib, "NotificationsQuery", "Notifications library must export NotificationsQuery");

for (const marker of [
  "[provides.graphql]",
  "query = \"graphql::NotificationsQuery\"",
  "id = \"notifications-header-action\"",
  "component = \"NotificationNavigation\"",
  "slot = \"header_actions\"",
  "order = 100",
  "[provides.storefront_ui.i18n]",
  "leptos_locales_path = \"storefront/locales\"",
]) {
  requireText(manifest, marker, `Notifications manifest is missing ${marker}`);
}

for (const marker of [
  "mod graphql_adapter;",
  "pub struct NotificationNavigationTransportContext",
  "fn selected_navigation_transport_path()",
  "UiTransportPath::NativeServer",
  "UiTransportPath::Graphql",
  "execute_selected_transport",
  "load_notification_unread_count",
  "graphql_adapter::load_navigation_unread_count",
  "notifications.storefront.navigation.unread_count",
]) {
  requireText(transport, marker, `navigation transport is missing ${marker}`);
}
rejectText(transport, "fallback_failed", "navigation transport must not add cross-path fallback");
for (const marker of [
  "query NotificationStorefrontNavigationUnreadCount",
  "notificationInboxUnreadCount",
  "unreadCount",
  "execute_graphql",
  "access_token",
  "tenant_slug",
]) {
  requireText(graphqlAdapter, marker, `navigation GraphQL adapter is missing ${marker}`);
}
for (const forbidden of ["tenantId", "recipientId", "userId"] ) {
  rejectText(graphqlAdapter, forbidden, `navigation GraphQL request must not expose ${forbidden}`);
}

for (const marker of [
  "pub fn NotificationNavigation()",
  "use_context::<UiRouteContext>()",
  "module_route_base(\"notifications\")",
  "use_context::<AuthContext>()",
  "AuthContext::get_token",
  "AuthContext::get_tenant",
  "Resource::new_blocking",
  "load_notification_navigation_unread_count",
  "NotificationUnreadBadge",
  "count.unread_count > 0",
  "data-notification-navigation=\"true\"",
  "data-notification-navigation=\"unavailable\"",
]) {
  requireText(navigation, marker, `navigation component is missing ${marker}`);
}
for (const forbidden of [
  "/modules/notifications",
  "localStorage",
  "gloo_storage",
  "window.location",
  "NotificationInboxGroupSummary",
  "set_unread",
]) {
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
  "pub fn with_count",
]) {
  requireText(i18n, marker, `storefront i18n adapter is missing ${marker}`);
}
for (const marker of ["notifications", "navigation", "label", "unread"]) {
  requireText(localeEn, marker, `English locale is missing ${marker}`);
  requireText(localeRu, marker, `Russian locale is missing ${marker}`);
}

for (const marker of [
  "HeaderActions",
  "header_actions",
  "StorefrontSlot::HeaderActions",
]) {
  requireText(hostRegistry + hostBuild + xtaskUi, marker, `host slot contracts are missing ${marker}`);
}
for (const marker of [
  "components_for_slot(StorefrontSlot::HeaderActions",
  "action_views=header_action_views",
]) {
  requireText(hostApp, marker, `storefront layout is missing ${marker}`);
}
for (const marker of [
  "action_views: Vec<AnyView>",
  "data-storefront-header-actions=\"true\"",
  "{action_views}",
]) {
  requireText(hostHeader, marker, `storefront header is missing ${marker}`);
}
requireText(hostHeader, "{navigation}", "primary navigation render must remain present");
for (const forbidden of [
  "rustok_notifications_storefront",
  "NotificationNavigation",
  "load_notification_navigation_unread_count",
]) {
  rejectText(hostApp + hostHeader, forbidden, `host composition must not import ${forbidden}`);
}

for (const marker of [
  "manifest_registers_module_owned_header_action_without_host_imports",
  "navigation_uses_context_route_and_best_effort_exact_count",
  "unread_count_transport_is_dual_path_without_identity_payload",
  "owner_graphql_derives_scope_and_sanitizes_failures",
]) {
  requireText(proof, marker, `navigation badge source proof is missing ${marker}`);
}
for (const marker of [
  "HeaderActions",
  "components_for_slot(StorefrontSlot::HeaderActions",
  "action_views: Vec<AnyView>",
  "data-storefront-header-actions",
]) {
  requireText(hostProof, marker, `host slot proof is missing ${marker}`);
}

for (const marker of [
  "`NotificationNavigation` is a module-owned no-prop header action",
  "module_route_base(\"notifications\")",
  "The navigation unread-count read is dual-path",
  "zero count still leaves the localized Notifications link available",
  "full grouped inbox, open authorization, and group-state commands are still native-only",
]) {
  requireText(storefrontReadme, marker, `storefront README is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AJ notification storefront navigation badge",
  "notification_inbox_unread_count",
  "header_actions",
  "module_route_base(\"notifications\")",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AI" ||
  upstream.composition?.grouped_leptos_ui !== true ||
  upstream.composition?.global_navigation_badge_composition !== false ||
  !upstream.not_delivered?.includes("global storefront navigation or header unread-badge slot")
) {
  failures.push("FORUM-20AJ must close the FORUM-20AI navigation-badge residual");
}

if (failures.length > 0) {
  console.error("Forum notification navigation badge verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification navigation badge contract is source-ready.");
