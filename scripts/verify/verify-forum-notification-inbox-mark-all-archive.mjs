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
  "crates/rustok-forum/contracts/forum-notification-inbox-mark-all-archive.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const stateOwner = read(contract.notifications_state_owner_file ?? "");
const library = read(contract.notifications_lib_file ?? "");
const rootReadme = read(contract.notifications_readme ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const ownerPlan = read(contract.notifications_implementation_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("mark-all-archive contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AA" || contract.upstream_task !== "FORUM-20Z") {
  failures.push("mark-all-archive contract must connect FORUM-20Z/20AA");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("mark-all-archive contract must not claim unexecuted evidence");
}

for (const key of [
  "bounded_owner_mark_all_archive_service",
  "nonnil_identity_validation",
  "shared_page_bounds",
  "shared_versioned_cursor",
  "stable_descending_selection",
  "exact_tenant_recipient_filters",
  "non_archived_selection_only",
  "raw_selection_before_mutation",
  "exact_state_owner_reuse",
  "unread_to_archive_transition",
  "seen_to_archive_history_preserved",
  "read_to_archive_history_preserved",
  "archived_unchanged",
  "resumable_cursor_progress",
  "empty_foreign_no_oracle",
  "no_foreign_owner_calls",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`mark-all-archive contract must record ${key}`);
  }
}
if (!contract.not_delivered?.includes("arbitrary selected-id bulk mutations")) {
  failures.push("selected-id bulk mutations must remain open");
}
if (contract.not_delivered?.some((item) => item.includes("mark-all-archive"))) {
  failures.push("FORUM-20AA must close mark-all-archive");
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20AA") {
  failures.push("canonical ledger must be required through FORUM-20AA");
}
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") {
    failures.push("pending plan sync must identify FORUM-20G");
  }
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded through G");
  rejectText(plan, "### Delivered in `FORUM-20AA`", "canonical plan sync status is stale");
}

for (const marker of [
  "pub struct NotificationInboxMarkAllArchiveRequest",
  "pub struct NotificationInboxMarkAllArchivePage",
  "pub marked_archived: u16",
  "pub struct NotificationInboxMarkAllArchiveService",
  "validate_mark_all_archive_request(&request)?",
  ".map(decode_inbox_cursor)",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  ".add(notification::Column::State.eq(NotificationState::Unread))",
  ".add(notification::Column::State.eq(NotificationState::Seen))",
  ".add(notification::Column::State.eq(NotificationState::Read))",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "rows.truncate(limit as usize)",
  "rows.last().map(encode_inbox_cursor)",
  ".archive(NotificationInboxStateRequest",
  "notification inbox mark-all-archive identity must not be nil",
]) {
  requireText(owner, marker, `mark-all-archive owner is missing ${marker}`);
}
for (const forbidden of [
  "NotificationInboxOpenService",
  "NotificationRecipientPolicy",
  "NotificationSourceRegistry",
  "authorize_target_open",
  "delivery_attempt",
  "ActiveModel",
  "Set(",
]) {
  rejectText(owner, forbidden, `mark-all-archive owner must not use ${forbidden}`);
}

const start = owner.indexOf("request: NotificationInboxMarkAllArchiveRequest");
const load = owner.indexOf(".all(&self.db)", start);
const mutate = owner.indexOf(".archive(NotificationInboxStateRequest", start);
if (start < 0 || load < 0 || mutate < 0 || !(start < load && load < mutate)) {
  failures.push("mark-all-archive must load one bounded page before mutation");
}

for (const marker of [
  "pub async fn archive(",
  "state: Set(NotificationState::Archived)",
  "archived_at: Set(Some(timestamp.to_owned()))",
  ".filter(notification::Column::State.ne(NotificationState::Archived))",
]) {
  requireText(stateOwner, marker, `state owner is missing ${marker}`);
}
for (const marker of [
  "NotificationInboxMarkAllArchivePage",
  "NotificationInboxMarkAllArchiveRequest",
  "NotificationInboxMarkAllArchiveService",
]) {
  requireText(library, marker, `public library is missing ${marker}`);
}
for (const marker of [
  "bounded_page_archives_non_archived_and_preserves_state_history",
  "mark_all_archive_pages_are_bounded_and_resumable",
  "empty_foreign_and_invalid_mark_all_archive_requests_fail_closed",
  "mark_all_archive_limits_use_shared_inbox_bounds",
  "marked_archived: 3",
  "assert_eq!(seen_after.seen_at, seen_before.seen_at)",
  "assert_eq!(read_after.read_at, read_before.read_at)",
  "assert_eq!(archived_after.updated_at, archived_before.updated_at)",
  "delivery_attempt::Entity::find()",
  "invalid-cursor",
]) {
  requireText(proof, marker, `SQLite proof is missing ${marker}`);
}
for (const marker of [
  "bounded mark-all-archive",
  "NotificationInboxMarkAllArchiveService",
  "### 9. Bounded mark-all-archive",
  "tests/inbox_mark_all_archive_sqlite.rs",
  "arbitrary selected-ID bulk",
]) {
  requireText(rootReadme, marker, `root README is missing ${marker}`);
}
for (const marker of [
  "### Bounded mark-all-archive",
  "NotificationInboxMarkAllArchiveService",
  "`unread`, `seen`, or `read`",
  "tests/inbox_mark_all_archive_sqlite.rs",
  "verify-forum-notification-inbox-mark-all-archive.mjs",
]) {
  requireText(docs, marker, `live contract is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20AA`",
  "NotificationInboxMarkAllArchiveService",
  "created_at DESC, id DESC",
  "tests/inbox_mark_all_archive_sqlite.rs",
  "arbitrary selected-ID bulk",
]) {
  requireText(ownerPlan, marker, `owner ledger is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20Z" ||
  upstream.composition?.bounded_owner_mark_all_unread_service !== true ||
  !upstream.not_delivered?.includes(
    "mark-all-archive and arbitrary selected-id bulk mutations",
  )
) {
  failures.push("FORUM-20AA must remain linked to the FORUM-20Z residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox mark-all-archive verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox mark-all-archive contract is source-ready.");
