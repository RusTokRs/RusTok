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
  "crates/rustok-forum/contracts/forum-notification-inbox-grouped-storefront-ui.json";
const navigationPath =
  "crates/rustok-forum/contracts/forum-notification-navigation-badge.json";
const groupedGraphqlPath =
  "crates/rustok-forum/contracts/forum-notification-inbox-grouped-graphql.json";
const contract = JSON.parse(read(contractPath) || "{}");
const navigationAbsolute = path.join(repoRoot, navigationPath);
const navigation = existsSync(navigationAbsolute)
  ? JSON.parse(readFileSync(navigationAbsolute, "utf8") || "{}")
  : null;
const groupedGraphqlAbsolute = path.join(repoRoot, groupedGraphqlPath);
const groupedGraphql = existsSync(groupedGraphqlAbsolute)
  ? JSON.parse(readFileSync(groupedGraphqlAbsolute, "utf8") || "{}")
  : null;
const navigationDelivered =
  navigation?.schema_version === 1 &&
  navigation?.task === "FORUM-20AJ" &&
  navigation?.upstream_task === "FORUM-20AI" &&
  navigation?.composition?.exact_navigation_badge === true;
const groupedGraphqlDelivered =
  groupedGraphql?.schema_version === 1 &&
  groupedGraphql?.task === "FORUM-20AK" &&
  groupedGraphql?.upstream_task === "FORUM-20AJ" &&
  groupedGraphql?.composition?.dual_path_group_summary_read === true &&
  groupedGraphql?.composition?.dual_path_group_items_read === true;

const core = read(contract.storefront_core_file ?? "");
const transport = read(contract.storefront_transport_file ?? "");
const ui = read(contract.storefront_ui_file ?? "");
const library = read(contract.storefront_lib_file ?? "");
const readme = read(contract.storefront_readme ?? "");
const proof = read(contract.storefront_test ?? "");
const nativeAdapter = read(contract.native_adapter_file ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const canonicalPlan = read(contract.canonical_plan ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("grouped storefront UI contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AI" || contract.upstream_task !== "FORUM-20AH") {
  failures.push("grouped storefront UI contract must connect FORUM-20AH/20AI");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("grouped storefront UI contract must not claim unexecuted evidence");
}

for (const key of [
  "grouped_leptos_ui",
  "hydrated_inbox_interactions",
  "ssr_blocking_bootstrap",
  "exact_unread_badge",
  "reusable_unread_badge_component",
  "explicit_loading_state",
  "explicit_empty_state",
  "explicit_unavailable_state",
  "bounded_group_summary_paging",
  "bounded_group_item_paging",
  "page_identity_deduplication",
  "single_expanded_group",
  "stale_item_response_guard",
  "fresh_open_authorization",
  "authorized_browser_navigation",
  "bounded_group_mark_read",
  "bounded_group_mark_unread",
  "bounded_group_archive",
  "fresh_write_idempotency_key",
  "authoritative_post_write_refresh",
  "no_optimistic_unread_mutation",
  "bounded_action_continuation_message",
  "safe_template_text_fallbacks",
  "in_memory_only_ui_state",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`grouped storefront UI contract must record ${key}`);
  }
}
for (const key of [
  "global_navigation_badge_composition",
  "graphql_adapter",
  "local_storage",
  "shadow_inbox",
  "channel_delivery",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`historical grouped UI contract must keep ${key} false`);
  }
}

for (const sync of [contract.canonical_plan_sync, contract.notifications_local_plan_sync]) {
  if (sync?.status !== "pending" || sync.required_ledger_through !== "FORUM-20AI") {
    failures.push("historical Forum and Notifications ledgers must remain pending through FORUM-20AI");
  }
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");

for (const marker of [
  "pub fn display_title(&self)",
  "pub fn display_body(&self)",
  "pub struct NotificationStorefrontInboxSnapshot",
  "pub fn append_page(&mut self, page: NotificationStorefrontGroupSummaryPage)",
  "collect::<BTreeSet<_>>()",
  "known.insert(group.group_key.clone())",
  "pub struct NotificationStorefrontGroupItemsSnapshot",
  "known.insert(item.id.clone())",
]) {
  requireText(core, marker, `grouped storefront core is missing ${marker}`);
}
for (const forbidden of ["serde_json::Value", "HashMap<String", "unsafe {"]) {
  rejectText(core, forbidden, `grouped storefront core must not use ${forbidden}`);
}

for (const marker of [
  "Resource::new_blocking",
  "load_inbox_snapshot().await",
  "load_notification_unread_count().await?",
  "load_notification_group_summaries",
  "load_notification_group_items",
  "authorize_notification_open",
  "apply_notification_group_state",
  "const SUMMARY_PAGE_SIZE: u16 = 20",
  "const ITEM_PAGE_SIZE: u16 = 20",
  "const GROUP_ACTION_PAGE_SIZE: u16 = 64",
  "pub fn NotificationUnreadBadge",
  "data-notification-unread-count",
  "Your inbox is clear",
  "Notification inbox unavailable",
  "Loading notifications...",
  "Load more groups",
  "Load more notifications",
  "let (expanded_group, set_expanded_group)",
  "let (items_request_nonce, set_items_request_nonce)",
  "items_request_nonce.get() == request_nonce",
  "Uuid::new_v4()",
  "NotificationStorefrontGroupStateAction::MarkRead",
  "NotificationStorefrontGroupStateAction::MarkUnread",
  "NotificationStorefrontGroupStateAction::Archive",
  "More matching items remain; repeat the action after refresh.",
  "on_refresh.run(feedback)",
  "NotificationStorefrontOpenDecision::Allowed { route }",
  "navigate_to_route(route.as_str())",
]) {
  requireText(ui, marker, `grouped storefront UI is missing ${marker}`);
}
for (const forbidden of [
  "localStorage",
  "gloo_storage",
  "dangerously_set_inner_html",
  "inner_html",
  "async_graphql",
  "NotificationSourceRegistry::default",
  "sea_orm::",
]) {
  rejectText(ui, forbidden, `grouped storefront UI must not use ${forbidden}`);
}

for (const marker of [
  "mod native_server_adapter;",
  "load_notification_group_summaries",
  "load_notification_group_items",
  "authorize_notification_open",
  "apply_notification_group_state",
  "legacy explicit degraded sentinel",
  "NotificationsView` no longer uses this sentinel",
]) {
  requireText(transport, marker, `storefront transport is missing ${marker}`);
}
requireText(library, "pub use ui::leptos::{NotificationUnreadBadge, NotificationsView};", "storefront library must export grouped UI");

for (const marker of [
  "summary_pages_append_without_duplicate_group_state",
  "item_pages_append_without_duplicate_notification_identity",
  "presentation_uses_bounded_template_fields_then_semantic_fallbacks",
  "group_action_labels_match_transport_contract",
]) {
  requireText(proof, marker, `grouped storefront state proof is missing ${marker}`);
}
for (const marker of [
  "notification_storefront_unread_count_native",
  "notification_storefront_group_summaries_native",
  "notification_storefront_group_items_native",
  "notification_storefront_open_native",
  "notification_storefront_group_state_native",
  "if !auth.is_human_user_principal()",
  "let actor = auth.port_actor();",
]) {
  requireText(nativeAdapter, marker, `native adapter is missing ${marker}`);
}

for (const marker of [
  "NotificationsView` now renders the owner-backed grouped inbox",
  "exact unread-count badge",
  "authoritative refresh after every mutation",
  "in-memory page deduplication",
  "NotificationUnreadBadge",
]) {
  requireText(readme, marker, `storefront README is missing ${marker}`);
}
if (navigationDelivered) {
  requireText(readme, "`NotificationNavigation` is a module-owned no-prop header action", "FORUM-20AJ navigation README state is missing");
}
if (groupedGraphqlDelivered) {
  for (const marker of [
    "unread count, grouped summaries, and exact-group item pages use one selected read",
    "notificationInboxGroupSummaries",
    "notificationInboxGroupItems",
    "Fresh notification open authorization and group-state",
  ]) {
    requireText(readme, marker, `FORUM-20AK README state is missing ${marker}`);
  }
  if (
    !groupedGraphql.not_delivered?.includes("GraphQL notification open authorization") ||
    !groupedGraphql.not_delivered?.includes("GraphQL group-state commands")
  ) {
    failures.push("FORUM-20AK must keep open and command GraphQL parity pending");
  }
}

for (const marker of [
  "# FORUM-20AI grouped notification storefront UI",
  "exact unread count from the owner count endpoint",
  "request nonce",
  "maximum of 64 eligible rows",
  "does not optimistically",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `FORUM-20AI owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AH" ||
  upstream.composition?.native_server_adapter !== true ||
  upstream.composition?.grouped_leptos_ui !== false ||
  !upstream.not_delivered?.includes("grouped Leptos inbox rendering and hydrated paging state")
) {
  failures.push("FORUM-20AI must close the FORUM-20AH grouped UI residual");
}

if (failures.length > 0) {
  console.error("Forum notification grouped storefront UI verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification grouped storefront UI contract is source-ready.");
