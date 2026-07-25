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
  "crates/rustok-forum/contracts/forum-notification-inbox-state-mutations.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const surface = read(contract.notifications_surface_file ?? "");
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
  failures.push("forum notification inbox state contract must use schema_version=1");
}
if (contract.task !== "FORUM-20U" || contract.upstream_task !== "FORUM-20T") {
  failures.push("forum notification inbox state contract must connect FORUM-20T/U");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("inbox state mutations must not claim unexecuted evidence");
}

for (const delivered of [
  "exact_owner_state_service",
  "nonnil_identity_validation",
  "exact_tenant_recipient_notification_filters",
  "missing_foreign_no_oracle",
  "no_foreign_owner_calls",
  "monotonic_seen_transition",
  "monotonic_read_transition",
  "terminal_archive_transition",
  "direct_read_sets_seen_and_read",
  "timestamp_preservation",
  "idempotent_later_state_commands",
  "updated_at_only_on_transition",
  "typed_state_snapshot",
  "semantic_target_not_exposed",
  "delivery_attempts_unchanged",
  "public_crate_export",
  "sqlite_contract_proof",
  "root_notifications_docs",
  "live_notifications_docs",
  "owner_implementation_ledger",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification inbox state contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "mark unread mutation",
  "bulk and mark-all inbox mutations",
  "canonical unread counts and grouped inbox views",
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
    failures.push(`forum notification inbox state contract must keep ${residual} explicitly open`);
  }
}
if (contract.not_delivered?.includes("seen read and archive state mutations")) {
  failures.push("FORUM-20U must remove exact seen/read/archive mutations from current residuals");
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
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20U") {
  failures.push("inbox state contract must require the canonical ledger through FORUM-20U");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("inbox state contract must require FORUM-20H through FORUM-20U delivered sections");
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
  requireText(plan, "FORUM-20A-U provide", "synchronized canonical plan must advance through U");
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
  "pub struct NotificationInboxStateSnapshot",
  "pub state: NotificationState",
  "pub seen_at: Option<DateTime<FixedOffset>>",
  "pub read_at: Option<DateTime<FixedOffset>>",
  "pub archived_at: Option<DateTime<FixedOffset>>",
  "pub updated_at: DateTime<FixedOffset>",
  "pub enum NotificationInboxStateDecision",
  "Available {",
  "changed: bool",
  "snapshot: NotificationInboxStateSnapshot",
  "Unavailable",
  "pub struct NotificationInboxStateService",
  "db: DatabaseConnection",
  "pub async fn mark_seen(",
  "pub async fn mark_read(",
  "pub async fn archive(",
  "notification::Entity::update_many()",
  ".filter(notification::Column::Id.eq(request.notification_id))",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  "notification::Entity::find_by_id(request.notification_id)",
  "return Ok(NotificationInboxStateDecision::Unavailable)",
  "validate_request(&request)?",
  "notification inbox state identity must not be nil",
]) {
  requireText(owner, marker, `notification inbox state owner is missing ${marker}`);
}

const seen = section(owner, "pub async fn mark_seen(", "pub async fn mark_read(");
for (const marker of [
  "state: Set(NotificationState::Seen)",
  "seen_at: Set(Some(timestamp.to_owned()))",
  "updated_at: Set(timestamp)",
  ".filter(notification::Column::State.eq(NotificationState::Unread))",
]) {
  requireText(seen, marker, `mark_seen transition is missing ${marker}`);
}
for (const forbidden of ["read_at: Set(", "archived_at: Set(", "NotificationState::Read", "NotificationState::Archived"]) {
  rejectText(seen, forbidden, `mark_seen must remain a narrow unread-to-seen transition: ${forbidden}`);
}

const readState = section(owner, "pub async fn mark_read(", "pub async fn archive(");
for (const marker of [
  "state: Set(NotificationState::Read)",
  "seen_at: Set(Some(timestamp.to_owned()))",
  "read_at: Set(Some(timestamp.to_owned()))",
  ".filter(notification::Column::State.eq(NotificationState::Unread))",
  ".filter(notification::Column::State.eq(NotificationState::Seen))",
]) {
  requireText(readState, marker, `mark_read transition is missing ${marker}`);
}
rejectText(readState, "archived_at: Set(", "mark_read must not modify archive identity");

const archive = section(owner, "pub async fn archive(", "async fn load_decision(");
for (const marker of [
  "state: Set(NotificationState::Archived)",
  "archived_at: Set(Some(timestamp.to_owned()))",
  "updated_at: Set(timestamp)",
  ".filter(notification::Column::State.ne(NotificationState::Archived))",
]) {
  requireText(archive, marker, `archive transition is missing ${marker}`);
}
for (const forbidden of ["seen_at: Set(", "read_at: Set("]) {
  rejectText(archive, forbidden, `archive must preserve existing read timestamps: ${forbidden}`);
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
  "mod inbox_state;",
  "NotificationInboxStateDecision",
  "NotificationInboxStateRequest",
  "NotificationInboxStateService",
  "NotificationInboxStateSnapshot",
]) {
  requireText(surface, marker, `notifications public surface is missing ${marker}`);
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
  "Exact inbox target opens",
  "bounded authorized pages",
]) {
  requireText(service, marker, `notifications service boundary is missing ${marker}`);
}

for (const marker of [
  "exact_recipient_transitions_are_monotonic_and_idempotent",
  "direct_read_and_archive_preserve_timestamp_invariants",
  "foreign_missing_and_invalid_requests_fail_closed_without_mutation",
  "mark seen must not downgrade read state",
  "archived notification should not downgrade to seen",
  "archived notification should not downgrade to read",
  "archive should be idempotent",
  "assert_eq!(read.seen_at, read.read_at)",
  "assert_eq!(archived.seen_at, read.seen_at)",
  "assert_eq!(archived.read_at, read.read_at)",
  "NotificationInboxStateDecision::Unavailable",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `inbox state SQLite proof is missing ${marker}`);
}

for (const marker of [
  "NotificationInboxStateService",
  "exact seen/read/archive state APIs",
  "mark-unread, bulk/mark-all",
  "inbox_state_sqlite",
]) {
  requireText(rootReadme, marker, `notifications root README is missing ${marker}`);
}
for (const marker of [
  "### Exact inbox state mutations",
  "unread → seen → read → archived",
  "No command downgrades an archived row",
  "mark-unread, bulk/mark-all",
  "inbox_state_sqlite",
  "verify-forum-notification-inbox-state-mutations.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20U`",
  "NotificationInboxStateService",
  "mark-unread, bulk/mark-all",
  "tests/inbox_state_sqlite.rs",
]) {
  requireText(ownerPlan, marker, `notifications owner implementation plan is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20T" ||
  upstream.upstream_task !== "FORUM-20S" ||
  upstream.downstream_task !== "FORUM-20U" ||
  upstream.composition?.bounded_owner_listing_service !== true ||
  !upstream.not_delivered?.includes("seen read and archive state mutations")
) {
  failures.push("FORUM-20U must remain linked to the FORUM-20T listing contract and its state residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox state mutation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox state mutation contract is source-ready.");
