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
  "crates/rustok-forum/contracts/forum-notification-inbox-mark-all-read.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const stateOwner = read(contract.notifications_state_owner_file ?? "");
const countOwner = read(contract.notifications_count_owner_file ?? "");
const entities = read(contract.notifications_entity_file ?? "");
const migration = read(contract.notifications_migration_file ?? "");
const library = read(contract.notifications_lib_file ?? "");
const rootReadme = read(contract.notifications_readme ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const ownerPlan = read(contract.notifications_implementation_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification mark-all-read contract must use schema_version=1");
}
if (contract.task !== "FORUM-20Y" || contract.upstream_task !== "FORUM-20X") {
  failures.push("forum notification mark-all-read contract must connect FORUM-20X/Y");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("mark-all-read contract must not claim unexecuted evidence");
}

for (const delivered of [
  "bounded_owner_mark_all_read_service",
  "nonnil_identity_validation",
  "shared_page_bounds",
  "shared_versioned_cursor",
  "stable_descending_selection",
  "exact_tenant_recipient_filters",
  "unread_and_seen_selection_only",
  "raw_selection_before_mutation",
  "exact_state_owner_reuse",
  "unread_to_read_timestamp_invariants",
  "seen_to_read_timestamp_invariants",
  "read_and_archived_unchanged",
  "resumable_cursor_progress",
  "empty_foreign_no_oracle",
  "no_foreign_owner_calls",
  "semantic_target_not_exposed",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "root_notifications_docs",
  "live_notifications_docs",
  "owner_implementation_ledger",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification mark-all-read contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "mark-all-unread mark-all-archive and arbitrary selected-id bulk mutations",
  "grouped inbox views",
  "tenant-wide scheduled reconciliation and payload redaction",
  "external inbox transport and UI adapters",
  "channel delivery enqueue and transports",
  "delivery-time target authorization",
  "host trust facts adapter",
  "host channel membership facts adapter",
  "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification mark-all-read contract must keep ${residual} explicitly open`);
  }
}
if (contract.not_delivered?.some((item) => item === "bulk and mark-all inbox mutations")) {
  failures.push("FORUM-20Y must narrow the historical bulk/mark-all residual");
}

const deliveredSlices = [
  "FORUM-20H",
  "FORUM-20I",
  "FORUM-20J",
  "FORUM-20K",
  "FORUM-20L",
  "FORUM-20M",
  "FORUM-20N",
  "FORUM-20O",
  "FORUM-20P",
  "FORUM-20Q",
  "FORUM-20R",
  "FORUM-20S",
  "FORUM-20T",
  "FORUM-20U",
  "FORUM-20V",
  "FORUM-20W",
  "FORUM-20X",
  "FORUM-20Y",
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20Y") {
  failures.push("mark-all-read contract must require the canonical ledger through FORUM-20Y");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("mark-all-read contract must require FORUM-20H through FORUM-20Y delivered sections");
}
if (planSync.status === "pending") {
  if (planSync.current_plan_through !== "FORUM-20G") {
    failures.push("pending canonical plan synchronization must identify FORUM-20G as current");
  }
  requireText(
    plan,
    "FORUM-20A-G provide",
    "pending canonical plan synchronization must remain grounded in FORUM-20A-G",
  );
  for (const slice of deliveredSlices) {
    rejectText(
      plan,
      `### Delivered in \`${slice}\``,
      `canonical plan now contains ${slice}; update canonical_plan_sync before claiming pending through G`,
    );
  }
} else if (planSync.status === "synchronized") {
  requireText(plan, "FORUM-20A-Y provide", "synchronized canonical plan must advance through Y");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct NotificationInboxMarkAllReadRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub cursor: Option<String>",
  "pub limit: u16",
  "pub fn bounded_limit(&self) -> u64",
  "DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE",
  "MAX_NOTIFICATION_INBOX_PAGE_SIZE",
  "pub struct NotificationInboxMarkAllReadPage",
  "pub scanned: u16",
  "pub marked_read: u16",
  "pub next_cursor: Option<String>",
  "pub has_more: bool",
  "pub struct NotificationInboxMarkAllReadService",
  "pub async fn mark_page(",
  "validate_request(&request)?",
  ".map(decode_inbox_cursor)",
  "notification::Entity::find()",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  ".add(notification::Column::State.eq(NotificationState::Unread))",
  ".add(notification::Column::State.eq(NotificationState::Seen))",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "rows.truncate(limit as usize)",
  "rows.last().map(encode_inbox_cursor)",
  ".mark_read(NotificationInboxStateRequest",
  "NotificationInboxStateDecision::Available { changed: true, .. }",
  "notification inbox mark-all-read identity must not be nil",
]) {
  requireText(owner, marker, `notification mark-all-read owner is missing ${marker}`);
}

for (const forbidden of [
  "NotificationInboxOpenService",
  "NotificationInboxListService",
  "NotificationRecipientPolicy",
  "NotificationSourceRegistry",
  "authorize_target_open",
  "target_owner",
  "target_kind",
  "target_id",
  "delivery_attempt",
  "update_many",
  "ActiveModel",
  "Set(",
]) {
  rejectText(owner, forbidden, `mark-all-read owner must preserve its narrow boundary against ${forbidden}`);
}

const markPageIndex = owner.indexOf("pub async fn mark_page(");
const tenantIndex = owner.indexOf(
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  markPageIndex,
);
const recipientIndex = owner.indexOf(
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  markPageIndex,
);
const unreadIndex = owner.indexOf(
  ".add(notification::Column::State.eq(NotificationState::Unread))",
  markPageIndex,
);
const seenIndex = owner.indexOf(
  ".add(notification::Column::State.eq(NotificationState::Seen))",
  markPageIndex,
);
const createdOrderIndex = owner.indexOf(
  ".order_by_desc(notification::Column::CreatedAt)",
  markPageIndex,
);
const idOrderIndex = owner.indexOf(
  ".order_by_desc(notification::Column::Id)",
  markPageIndex,
);
const loadIndex = owner.indexOf(".all(&self.db)", markPageIndex);
const mutationIndex = owner.indexOf(".mark_read(NotificationInboxStateRequest", markPageIndex);
if (
  markPageIndex < 0 ||
  tenantIndex < 0 ||
  recipientIndex < 0 ||
  unreadIndex < 0 ||
  seenIndex < 0 ||
  createdOrderIndex < 0 ||
  idOrderIndex < 0 ||
  loadIndex < 0 ||
  mutationIndex < 0 ||
  !(markPageIndex < tenantIndex &&
    tenantIndex < recipientIndex &&
    recipientIndex < unreadIndex &&
    unreadIndex < seenIndex &&
    seenIndex < createdOrderIndex &&
    createdOrderIndex < idOrderIndex &&
    idOrderIndex < loadIndex &&
    loadIndex < mutationIndex)
) {
  failures.push("mark-all-read must complete bounded exact-recipient eligible selection before state mutation");
}

for (const marker of [
  "pub async fn mark_read(",
  "state: Set(NotificationState::Read)",
  "seen_at: Set(Some(timestamp.to_owned()))",
  "read_at: Set(Some(timestamp.to_owned()))",
  ".filter(notification::Column::State.eq(NotificationState::Unread))",
  ".filter(notification::Column::State.eq(NotificationState::Seen))",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
]) {
  requireText(stateOwner, marker, `exact inbox state owner is missing ${marker}`);
}

for (const marker of [
  "pub struct NotificationInboxUnreadCountService",
  ".filter(notification::Column::State.eq(NotificationState::Unread))",
  ".count(&self.db)",
]) {
  requireText(countOwner, marker, `unread count owner is missing ${marker}`);
}
for (const marker of [
  "pub state: NotificationState",
  "pub seen_at: Option<DateTimeWithTimeZone>",
  "pub read_at: Option<DateTimeWithTimeZone>",
  "pub archived_at: Option<DateTimeWithTimeZone>",
]) {
  requireText(entities, marker, `notification persistence state is missing ${marker}`);
}
for (const marker of [
  "CREATE INDEX IF NOT EXISTS idx_notifications_inbox",
  "ON notifications (tenant_id, recipient_id, state, created_at DESC, id DESC)",
]) {
  requireText(migration, marker, `mark-all-read index support is missing ${marker}`);
}
for (const marker of [
  "mod inbox_bulk;",
  "NotificationInboxMarkAllReadPage",
  "NotificationInboxMarkAllReadRequest",
  "NotificationInboxMarkAllReadService",
]) {
  requireText(library, marker, `notifications public library surface is missing ${marker}`);
}

for (const marker of [
  "bounded_page_marks_unread_and_seen_without_touching_terminal_states",
  "mark_all_read_pages_are_bounded_and_resumable",
  "empty_foreign_and_invalid_requests_fail_closed",
  "mark_all_read_limits_use_shared_inbox_bounds",
  "NotificationInboxMarkAllReadPage {",
  "scanned: 2",
  "marked_read: 2",
  "assert_eq!(unread_after.seen_at, unread_after.read_at)",
  "assert_eq!(seen_after.seen_at, seen_before.seen_at)",
  "assert_eq!(read_after.updated_at, read_before.updated_at)",
  "assert_eq!(archived_after.updated_at, archived_before.updated_at)",
  "delivery_attempt::Entity::find()",
  "invalid-cursor",
]) {
  requireText(proof, marker, `mark-all-read SQLite proof is missing ${marker}`);
}

for (const marker of [
  "bounded mark-all-read",
  "NotificationInboxMarkAllReadService",
  "### 7. Bounded mark-all-read",
  "tests/inbox_mark_all_read_sqlite.rs",
  "mark-all-unread/archive",
]) {
  requireText(rootReadme, marker, `notifications root README is missing ${marker}`);
}
for (const marker of [
  "### Bounded mark-all-read",
  "NotificationInboxMarkAllReadService",
  "unread or seen",
  "tests/inbox_mark_all_read_sqlite.rs",
  "verify-forum-notification-inbox-mark-all-read.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20Y`",
  "NotificationInboxMarkAllReadService",
  "created_at DESC, id DESC",
  "tests/inbox_mark_all_read_sqlite.rs",
  "mark-all-unread",
]) {
  requireText(ownerPlan, marker, `notifications owner implementation plan is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20X" ||
  upstream.upstream_task !== "FORUM-20W" ||
  upstream.composition?.exact_owner_unread_count_service !== true ||
  !upstream.not_delivered?.includes("bulk and mark-all inbox mutations")
) {
  failures.push("FORUM-20Y must remain linked to the historical FORUM-20X bulk/mark-all residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox mark-all-read verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox mark-all-read contract is source-ready.");
