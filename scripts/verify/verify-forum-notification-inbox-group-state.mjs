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
  "crates/rustok-forum/contracts/forum-notification-inbox-group-state.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const stateOwner = read(contract.notifications_exact_state_owner_file ?? "");
const groupOwner = read(contract.notifications_group_owner_file ?? "");
const entity = read(contract.notifications_entity_file ?? "");
const library = read(contract.notifications_lib_file ?? "");
const readme = read(contract.notifications_readme ?? "");
const liveContract = read(contract.notifications_live_contract ?? "");
const localPlan = read(contract.notifications_local_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("group-state contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AF" || contract.upstream_task !== "FORUM-20AE") {
  failures.push("group-state contract must connect FORUM-20AE/20AF");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("group-state contract must not claim unexecuted evidence");
}

for (const key of [
  "bounded_group_state_owner",
  "typed_group_state_action",
  "group_mark_read",
  "group_mark_unread",
  "group_archive",
  "nonnil_owner_identity_validation",
  "shared_group_key_validation",
  "exact_tenant_recipient_group_selection",
  "eligible_state_selection_before_mutation",
  "shared_page_bounds",
  "shared_versioned_cursor",
  "stable_created_id_order",
  "raw_eligible_progress_cursor",
  "exact_state_owner_delegation",
  "direct_unread_to_read_invariant",
  "seen_to_read_history_preserved",
  "mark_unread_timestamps_cleared",
  "archive_history_preserved",
  "archive_terminal",
  "missing_foreign_satisfied_empty",
  "no_notification_identity_response",
  "no_privacy_source_target_calls",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`group-state contract must record ${key}`);
  }
}
for (const key of [
  "external_transport_adapter",
  "grouped_ui",
  "tenant_wide_reconciliation",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`group-state contract must keep ${key} false`);
  }
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20AF") {
  failures.push("canonical ledger must be required through FORUM-20AF");
}
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan sync must identify FORUM-20G");
  }
  requireText(plan, "FORUM-20A-G provide", "pending canonical sync must remain grounded through G");
  rejectText(plan, "### Delivered in `FORUM-20AF`", "canonical plan sync status is stale");
}
const localSync = contract.notifications_local_plan_sync ?? {};
if (localSync.required_ledger_through !== "FORUM-20AF") {
  failures.push("Notifications local ledger must be required through FORUM-20AF");
}
if (localSync.status === "pending") {
  if (localSync.current_plan_through !== "FORUM-20AA") {
    failures.push("pending Notifications ledger sync must identify FORUM-20AA");
  }
  requireText(localPlan, "### `FORUM-20AA`", "pending local sync must remain grounded through AA");
  rejectText(localPlan, "### `FORUM-20AF`", "Notifications local plan sync status is stale");
}

for (const marker of [
  "pub enum NotificationInboxGroupStateAction",
  "MarkRead",
  "MarkUnread",
  "Archive",
  "pub struct NotificationInboxGroupStateRequest",
  "pub struct NotificationInboxGroupStatePage",
  "pub struct NotificationInboxGroupStateService",
  "pub async fn apply_page",
  "validate_inbox_group_key(&request.group_key)",
  "notification::Column::TenantId.eq(request.tenant_id)",
  "notification::Column::RecipientId.eq(request.recipient_id)",
  "notification::Column::GroupKey.eq(request.group_key.as_str())",
  "notification::Column::State.eq(NotificationState::Unread)",
  "notification::Column::State.eq(NotificationState::Seen)",
  "notification::Column::State.eq(NotificationState::Read)",
  "notification::Column::State.ne(NotificationState::Archived)",
  "decode_inbox_cursor",
  "encode_inbox_cursor",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "NotificationInboxStateService::new(db)",
  "self.state.mark_read(state_request).await?",
  "self.state.mark_unread(state_request).await?",
  "self.state.archive(state_request).await?",
  "pub scanned: u16",
  "pub changed: u16",
  "pub next_cursor: Option<String>",
  "pub has_more: bool",
]) {
  requireText(owner, marker, `group-state owner is missing ${marker}`);
}
for (const forbidden of [
  "MarkSeen",
  "NotificationRecipientPolicy",
  "NotificationSourceRegistry",
  "authorize_open",
  "authorize_target_open",
  "delivery_attempt",
  "NotificationTargetRoute",
]) {
  rejectText(owner, forbidden, `group-state owner must not use ${forbidden}`);
}

for (const marker of [
  "pub(crate) fn validate_inbox_group_key",
  "MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES",
  "group_key != group_key.trim()",
  "group_key.chars().any(char::is_control)",
]) {
  requireText(groupOwner, marker, `shared group-key owner is missing ${marker}`);
}
for (const marker of [
  "pub async fn mark_read",
  "pub async fn mark_unread",
  "pub async fn archive",
  "seen_at: Set(Some(timestamp.to_owned()))",
  "read_at: Set(Some(timestamp.to_owned()))",
  "seen_at: Set(None)",
  "read_at: Set(None)",
  "archived_at: Set(Some(timestamp.to_owned()))",
]) {
  requireText(stateOwner, marker, `exact state owner is missing ${marker}`);
}
requireText(entity, "pub group_key: Option<String>", "notification entity must retain group_key");

for (const marker of [
  "mod inbox_group_state;",
  "NotificationInboxGroupStateAction",
  "NotificationInboxGroupStatePage",
  "NotificationInboxGroupStateRequest",
  "NotificationInboxGroupStateService",
]) {
  requireText(library, marker, `notifications library is missing ${marker}`);
}

for (const marker of [
  "bounded_group_mark_read_is_exact_and_cursor_stable",
  "group_mark_unread_and_archive_preserve_exact_state_invariants",
  "missing_foreign_and_invalid_group_state_requests_fail_closed",
  "group_state_limits_reuse_shared_inbox_bounds",
  "NotificationInboxGroupStateAction::MarkRead",
  "NotificationInboxGroupStateAction::MarkUnread",
  "NotificationInboxGroupStateAction::Archive",
  "assert_eq!(direct.seen_at, direct.read_at)",
  "seen-to-read must preserve the existing seen timestamp",
  "assert!(row.seen_at.is_none())",
  "assert!(row.read_at.is_none())",
  "another group must remain unchanged",
  "another recipient must remain unchanged",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `SQLite group-state proof is missing ${marker}`);
}

for (const marker of [
  "NotificationInboxGroupStateService",
  "bounded exact-group state commands",
  "tests/inbox_group_state_sqlite.rs",
  "External transport adapters and grouped UI remain closed",
]) {
  requireText(readme, marker, `notifications README is missing ${marker}`);
}
for (const marker of [
  "NotificationInboxGroupStateService",
  "Bounded group state commands",
  "inbox_group_state_sqlite",
  "grouped UI",
]) {
  requireText(liveContract, marker, `notifications live contract is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AF bounded notification group state commands",
  "mark_read",
  "mark_unread",
  "archive",
  "NotificationInboxStateService",
  "tests/inbox_group_state_sqlite.rs",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AE" ||
  upstream.composition?.bounded_group_summary_owner !== true ||
  upstream.composition?.group_level_state_commands !== false ||
  !upstream.not_delivered?.includes(
    "group-level mark-read mark-unread and archive commands",
  )
) {
  failures.push("FORUM-20AF must close the FORUM-20AE group-state residual");
}

if (failures.length > 0) {
  console.error("Forum notification group-state verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification bounded group-state contract is source-ready.");
