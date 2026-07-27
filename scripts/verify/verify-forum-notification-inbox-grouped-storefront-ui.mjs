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
  "crates/rustok-forum/contracts/forum-notification-inbox-grouped-storefront-ui.json";
const contract = JSON.parse(read(contractPath) || "{}");
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
  "state_contract_test",
  "owner_contract_note",
  "storefront_readme_updated",
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
    failures.push(`grouped storefront UI contract must keep ${key} false`);
  }
}

for (const sync of [contract.canonical_plan_sync, contract.notifications_local_plan_sync]) {
  if (sync?.status !== "pending" || sync.required_ledger_through !== "FORUM-20AI") {
    failures.push("Forum and Notifications ledgers must remain pending through FORUM-20AI");
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
  contract.notifications_owner_docs_sync?.required_contract_through !== "FORUM-20AI"
) {
  failures.push("large Notifications owner docs must record pending sync through FORUM-20AI");
}
requireText(canonicalPlan, "FORUM-20A-G provide", "canonical plan must remain grounded through G");
requireText(localPlan, "### `FORUM-20AA`", "Notifications local plan must remain grounded through AA");

for (const marker of [
  "pub const fn as_str(self)",
  "pub fn display_title(&self)",
  "pub fn display_body(&self)",
  "pub struct NotificationStorefrontInboxSnapshot",
  "pub fn append_page(&mut self, page: NotificationStorefrontGroupSummaryPage)",
  "collect::<BTreeSet<_>>()",
  "known.insert(group.group_key.clone())",
  "pub struct NotificationStorefrontGroupItemsSnapshot",
  "pub fn from_page(group_key: String, page: NotificationStorefrontGroupItemsPage)",
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
  "set_items_request_nonce.set(request_nonce)",
  "NotificationStorefrontGroupItemsSnapshot::from_page",
  "state.append_page(page)",
  "Uuid::new_v4()",
  "NotificationStorefrontGroupStateAction::MarkRead",
  "NotificationStorefrontGroupStateAction::MarkUnread",
  "NotificationStorefrontGroupStateAction::Archive",
  "More matching items remain; repeat the action after refresh.",
  "on_refresh.run(feedback)",
  "NotificationStorefrontOpenDecision::Allowed { route }",
  "navigate_to_route(route.as_str())",
  "web_sys::window()",
  "This notification target is no longer available.",
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
  "Entity::",
]) {
  rejectText(ui, forbidden, `grouped storefront UI must not use ${forbidden}`);
}

const groupAction = between(
  ui,
  "let apply_group_action = Callback::new(",
  "let open_notification = Callback::new",
  "group action callback",
);
requireText(groupAction, "apply_notification_group_state", "group action must call the native command");
requireText(groupAction, "on_refresh.run(feedback)", "group action must trigger authoritative refresh with preserved feedback");
rejectText(groupAction, "set_snapshot", "group action must not optimistically change summary or unread state");
rejectText(groupAction, "loop {", "group action must not start an unbounded continuation loop");
rejectText(groupAction, "while ", "group action must not start an unbounded continuation loop");

const openAction = between(
  ui,
  "let open_notification = Callback::new",
  "view! {",
  "notification open callback",
);
requireText(openAction, "authorize_notification_open", "open callback must authorize before navigation");
const authorizeIndex = openAction.indexOf("authorize_notification_open");
const navigateIndex = openAction.indexOf("navigate_to_route");
if (!(authorizeIndex >= 0 && navigateIndex > authorizeIndex)) {
  failures.push("notification navigation must occur after fresh open authorization");
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
for (const marker of [
  "pub use ui::leptos::{NotificationUnreadBadge, NotificationsView};",
  "pub use core::*;",
  "pub use transport::*;",
]) {
  requireText(library, marker, `storefront library is missing ${marker}`);
}

for (const marker of [
  "summary_pages_append_without_duplicate_group_state",
  "item_pages_append_without_duplicate_notification_identity",
  "presentation_uses_bounded_template_fields_then_semantic_fallbacks",
  "group_action_labels_match_transport_contract",
  "assert_eq!(appended, 1)",
  "NotificationStorefrontGroupStateAction::MarkUnread.as_str()",
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
  requireText(nativeAdapter, marker, `upstream native adapter is missing ${marker}`);
}

for (const marker of [
  "NotificationsView` now renders the owner-backed grouped inbox",
  "exact unread-count badge",
  "authoritative refresh after every mutation",
  "in-memory page deduplication",
  "NotificationUnreadBadge",
  "global navigation/header composition is not part",
]) {
  requireText(readme, marker, `storefront README is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AI grouped notification storefront UI",
  "exact unread count from the owner count endpoint",
  "request nonce",
  "maximum of 64 eligible rows",
  "does not optimistically",
  "source-ready / unvalidated",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AH" ||
  upstream.composition?.native_server_adapter !== true ||
  upstream.composition?.grouped_leptos_ui !== false ||
  !upstream.not_delivered?.includes("grouped Leptos inbox rendering and hydrated paging state")
) {
  failures.push("FORUM-20AI must close the FORUM-20AH grouped-UI residual");
}

if (failures.length > 0) {
  console.error("Forum notification grouped storefront UI verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification grouped storefront UI contract is source-ready.");
