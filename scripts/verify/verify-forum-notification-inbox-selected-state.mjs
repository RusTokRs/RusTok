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
  "crates/rustok-forum/contracts/forum-notification-inbox-selected-state.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const stateOwner = read(contract.notifications_state_owner_file ?? "");
const library = read(contract.notifications_lib_file ?? "");
const proof = read(contract.sqlite_proof ?? "");
const note = read(contract.owner_note ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("selected-state contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AB" || contract.upstream_task !== "FORUM-20AA") {
  failures.push("selected-state contract must connect FORUM-20AA/20AB");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("selected-state contract must not claim unexecuted evidence");
}

for (const key of [
  "bounded_selected_state_owner",
  "nonnil_owner_identity_validation",
  "nonempty_selection_validation",
  "shared_hard_bound_64",
  "nonnil_notification_identity_validation",
  "duplicate_identity_rejection",
  "input_order_preserved",
  "exact_state_owner_reuse",
  "mark_seen_supported",
  "mark_read_supported",
  "mark_unread_supported",
  "archive_supported",
  "unavailable_and_unchanged_conflated",
  "identity_free_count_response",
  "no_foreign_owner_calls",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`selected-state contract must record ${key}`);
  }
}
if (contract.not_delivered?.some((item) => item.includes("selected-id"))) {
  failures.push("FORUM-20AB must close selected-ID owner mutations");
}
if (!contract.not_delivered?.includes("grouped inbox views")) {
  failures.push("grouped inbox views must remain open");
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.status !== "pending" || sync.required_ledger_through !== "FORUM-20AB") {
  failures.push("canonical plan synchronization must remain explicit through FORUM-20AB");
}
if (sync.current_plan_through !== "FORUM-20G") {
  failures.push("pending plan synchronization must identify FORUM-20G");
}
requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded through G");
rejectText(plan, "### Delivered in `FORUM-20AB`", "canonical plan sync status is stale");

for (const marker of [
  "pub const MAX_NOTIFICATION_INBOX_SELECTED_IDS",
  "MAX_NOTIFICATION_INBOX_PAGE_SIZE as usize",
  "pub enum NotificationInboxSelectedAction",
  "MarkSeen",
  "MarkRead",
  "MarkUnread",
  "Archive",
  "pub struct NotificationInboxSelectedStateRequest",
  "pub notification_ids: Vec<Uuid>",
  "pub struct NotificationInboxSelectedStateResult",
  "pub requested: u16",
  "pub changed: u16",
  "pub not_changed: u16",
  "pub struct NotificationInboxSelectedStateService",
  "validate_request(&request)?",
  "for notification_id in request.notification_ids",
  "NotificationInboxStateRequest",
  "self.state.mark_seen(state_request).await?",
  "self.state.mark_read(state_request).await?",
  "self.state.mark_unread(state_request).await?",
  "self.state.archive(state_request).await?",
  "NotificationInboxStateDecision::Available { changed: true, .. }",
  "not_changed: requested - changed",
  "request.notification_ids.is_empty()",
  "request.notification_ids.len() > MAX_NOTIFICATION_INBOX_SELECTED_IDS",
  "notification_id.is_nil()",
  "HashSet::with_capacity",
  "selection must not contain duplicates",
]) {
  requireText(owner, marker, `selected-state owner is missing ${marker}`);
}
for (const forbidden of [
  "notification::Entity",
  "NotificationInboxOpenService",
  "NotificationRecipientPolicy",
  "NotificationSourceRegistry",
  "authorize_target_open",
  "delivery_attempt",
  "ActiveModel",
  "Set(",
]) {
  rejectText(owner, forbidden, `selected-state owner must not use ${forbidden}`);
}

const validate = owner.indexOf("validate_request(&request)?");
const iterate = owner.indexOf("for notification_id in request.notification_ids");
const mutate = owner.indexOf("self.state.mark_seen(state_request).await?");
if (validate < 0 || iterate < 0 || mutate < 0 || !(validate < iterate && iterate < mutate)) {
  failures.push("selected-state owner must validate the full selection before mutation");
}

for (const marker of [
  "pub async fn mark_seen(",
  "pub async fn mark_read(",
  "pub async fn mark_unread(",
  "pub async fn archive(",
  "NotificationInboxStateDecision::Unavailable",
]) {
  requireText(stateOwner, marker, `state owner is missing ${marker}`);
}
for (const marker of [
  "mod inbox_selected;",
  "MAX_NOTIFICATION_INBOX_SELECTED_IDS",
  "NotificationInboxSelectedAction",
  "NotificationInboxSelectedStateRequest",
  "NotificationInboxSelectedStateResult",
  "NotificationInboxSelectedStateService",
]) {
  requireText(library, marker, `public library is missing ${marker}`);
}
for (const marker of [
  "selected_actions_delegate_to_exact_state_owner_without_oracles",
  "invalid_selected_state_requests_fail_before_mutation",
  "selected_state_bound_matches_the_shared_inbox_hard_limit",
  "NotificationInboxSelectedAction::MarkSeen",
  "NotificationInboxSelectedAction::MarkRead",
  "NotificationInboxSelectedAction::MarkUnread",
  "NotificationInboxSelectedAction::Archive",
  "requested: 5",
  "changed: 3",
  "not_changed: 2",
  "MAX_NOTIFICATION_INBOX_SELECTED_IDS + 1",
  "vec![notification_id, notification_id]",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `SQLite proof is missing ${marker}`);
}
for (const marker of [
  "# FORUM-20AB selected inbox state owner",
  "1 through 64 unique, non-nil notification IDs",
  "NotificationInboxStateService",
  "`requested`, `changed`, and `not_changed`",
  "tests/inbox_selected_state_sqlite.rs",
  "not run by the implementation agent",
]) {
  requireText(note, marker, `owner contract note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AA" ||
  upstream.composition?.bounded_owner_mark_all_archive_service !== true ||
  !upstream.not_delivered?.includes("arbitrary selected-id bulk mutations")
) {
  failures.push("FORUM-20AB must remain linked to the FORUM-20AA residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox selected-state verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox selected-state contract is source-ready.");
