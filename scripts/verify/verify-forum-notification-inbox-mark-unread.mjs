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

function section(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start + startMarker.length);
  if (start < 0 || end < 0 || end <= start) return "";
  return source.slice(start, end);
}

const contractPath =
  "crates/rustok-forum/contracts/forum-notification-inbox-mark-unread.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const entities = read(contract.notifications_entity_file ?? "");
const migration = read(contract.notifications_migration_file ?? "");
const service = read(contract.notifications_service_file ?? "");
const rootReadme = read(contract.notifications_readme ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const ownerPlan = read(contract.notifications_implementation_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification mark unread contract must use schema_version=1");
}
if (contract.task !== "FORUM-20W" || contract.upstream_task !== "FORUM-20V") {
  failures.push("forum notification mark unread contract must connect FORUM-20V/W");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("mark unread contract must not claim unexecuted evidence");
}

for (const delivered of [
  "exact_owner_mark_unread_command",
  "nonnil_identity_validation",
  "exact_tenant_recipient_notification_filters",
  "seen_to_unread_transition",
  "read_to_unread_transition",
  "seen_timestamp_clear",
  "read_timestamp_clear",
  "archived_terminal",
  "already_unread_idempotent",
  "archived_idempotent",
  "updated_at_only_on_transition",
  "missing_foreign_no_oracle",
  "no_foreign_owner_calls",
  "typed_state_snapshot_reuse",
  "semantic_target_not_exposed",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "root_notifications_docs",
  "live_notifications_docs",
  "owner_implementation_ledger",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification mark unread contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "bulk and mark-all inbox mutations",
  "canonical unread counts and grouped inbox views",
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
    failures.push(`forum notification mark unread contract must keep ${residual} explicitly open`);
  }
}
if (contract.not_delivered?.includes("mark unread mutation")) {
  failures.push("FORUM-20W must remove exact mark unread from current residuals");
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
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20W") {
  failures.push("mark unread contract must require the canonical ledger through FORUM-20W");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("mark unread contract must require FORUM-20H through FORUM-20W delivered sections");
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
  requireText(plan, "FORUM-20A-W provide", "synchronized canonical plan must advance through W");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct NotificationInboxStateRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub notification_id: Uuid",
  "pub enum NotificationInboxStateDecision",
  "pub struct NotificationInboxStateService",
  "pub async fn mark_unread(",
  "validate_request(&request)?",
  "notification::Entity::update_many()",
  ".filter(notification::Column::Id.eq(request.notification_id))",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  "notification::Entity::find_by_id(request.notification_id)",
  "return Ok(NotificationInboxStateDecision::Unavailable)",
  "notification inbox state identity must not be nil",
]) {
  requireText(owner, marker, `notification mark unread owner is missing ${marker}`);
}

const markUnread = section(owner, "pub async fn mark_unread(", "pub async fn archive(");
for (const marker of [
  "state: Set(NotificationState::Unread)",
  "seen_at: Set(None)",
  "read_at: Set(None)",
  "updated_at: Set(timestamp)",
  "Condition::any()",
  ".add(notification::Column::State.eq(NotificationState::Seen))",
  ".add(notification::Column::State.eq(NotificationState::Read))",
]) {
  requireText(markUnread, marker, `mark unread transition is missing ${marker}`);
}
for (const forbidden of [
  "archived_at: Set(",
  "NotificationState::Archived",
  "NotificationSourceRegistry",
  "NotificationRecipientPolicy",
  "authorize_target_open",
  "target_owner",
  "target_kind",
  "target_id",
  "delivery_attempt",
]) {
  rejectText(markUnread, forbidden, `mark unread must preserve its narrow owner boundary against ${forbidden}`);
}

const markUnreadIndex = owner.indexOf("pub async fn mark_unread(");
const exactIdIndex = owner.indexOf(
  ".filter(notification::Column::Id.eq(request.notification_id))",
  markUnreadIndex,
);
const tenantIndex = owner.indexOf(
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  markUnreadIndex,
);
const recipientIndex = owner.indexOf(
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  markUnreadIndex,
);
const stateIndex = owner.indexOf("Condition::any()", markUnreadIndex);
if (
  markUnreadIndex < 0 ||
  exactIdIndex < 0 ||
  tenantIndex < 0 ||
  recipientIndex < 0 ||
  stateIndex < 0 ||
  !(markUnreadIndex < exactIdIndex && exactIdIndex < tenantIndex && tenantIndex < recipientIndex && recipientIndex < stateIndex)
) {
  failures.push("mark unread must apply exact notification tenant and recipient filters before eligible-state filtering");
}

for (const forbidden of [
  "NotificationSourceRegistry",
  "NotificationRecipientPolicy",
  "authorize_target_open",
  "target_owner",
  "target_kind",
  "target_id",
  "delivery_attempt",
]) {
  rejectText(owner, forbidden, `inbox state owner must not depend on foreign policy or target data through ${forbidden}`);
}

for (const marker of [
  "pub state: NotificationState",
  "pub seen_at: Option<DateTimeWithTimeZone>",
  "pub read_at: Option<DateTimeWithTimeZone>",
  "pub archived_at: Option<DateTimeWithTimeZone>",
  "pub updated_at: DateTimeWithTimeZone",
]) {
  requireText(entities, marker, `notification persistence state identity is missing ${marker}`);
}
for (const marker of [
  "CONSTRAINT ck_notifications_read_seen",
  "read_at IS NULL OR seen_at IS NOT NULL",
  "state = 'unread' AND seen_at IS NULL AND read_at IS NULL AND archived_at IS NULL",
  "state = 'seen' AND seen_at IS NOT NULL AND read_at IS NULL AND archived_at IS NULL",
  "state = 'read' AND seen_at IS NOT NULL AND read_at IS NOT NULL AND archived_at IS NULL",
  "state = 'archived' AND archived_at IS NOT NULL",
]) {
  requireText(migration, marker, `notification persistence state constraint is missing ${marker}`);
}

for (const marker of [
  "NotificationInboxStateService",
  "exact-item read-state",
  "bulk inbox",
]) {
  requireText(service, marker, `notifications service boundary is missing ${marker}`);
}

for (const marker of [
  "mark_unread_reopens_seen_and_read_without_unarchiving",
  "seen notification should become unread",
  "read notification should become unread",
  "archived notification should stay archived",
  "archived notification should not reopen as unread",
  "foreign or missing mark unread should fail closed",
  "assert!(seen_unread.seen_at.is_none())",
  "assert!(seen_unread.read_at.is_none())",
  "assert!(read_unread.seen_at.is_none())",
  "assert!(read_unread.read_at.is_none())",
  "assert_eq!(archived_after, archived_before)",
  "NotificationInboxStateDecision::Unavailable",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `mark unread SQLite proof is missing ${marker}`);
}

for (const marker of [
  "seen/read/mark-unread/archive state APIs",
  "mark_unread",
  "former `mark-unread, bulk/mark-all` residual",
  "bulk/mark-all mutations",
  "tests/inbox_state_sqlite.rs",
]) {
  requireText(rootReadme, marker, `notifications root README is missing ${marker}`);
}
for (const marker of [
  "Exact seen/read/mark-unread/archive state APIs",
  "explicit reopen command",
  "No command reopens an archived row",
  "mark-unread, bulk/mark-all",
  "verify-forum-notification-inbox-mark-unread.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20W`",
  "NotificationInboxStateService::mark_unread",
  "seen or read rows back to unread",
  "bulk/mark-all mutations",
  "tests/inbox_state_sqlite.rs",
]) {
  requireText(ownerPlan, marker, `notifications owner implementation plan is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20V" ||
  upstream.upstream_task !== "FORUM-20U" ||
  upstream.downstream_task !== "FORUM-20W" ||
  upstream.composition?.bounded_owner_reconciliation_service !== true ||
  !upstream.not_delivered?.includes("mark unread mutation")
) {
  failures.push("FORUM-20W must remain linked to the historical FORUM-20V mark-unread residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox mark unread verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox mark unread contract is source-ready.");
