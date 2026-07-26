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
  "crates/rustok-forum/contracts/forum-notification-inbox-unread-count.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
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
  failures.push("forum notification unread count contract must use schema_version=1");
}
if (contract.task !== "FORUM-20X" || contract.upstream_task !== "FORUM-20W") {
  failures.push("forum notification unread count contract must connect FORUM-20W/X");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("unread count contract must not claim unexecuted evidence");
}

for (const delivered of [
  "exact_owner_unread_count_service",
  "nonnil_identity_validation",
  "exact_tenant_recipient_state_filters",
  "owner_table_count_authority",
  "zero_result_no_oracle",
  "existing_inbox_index_reuse",
  "no_page_derived_total",
  "no_foreign_owner_calls",
  "no_inbox_mutation",
  "semantic_target_not_exposed",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "root_notifications_docs",
  "live_notifications_docs",
  "owner_implementation_ledger",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification unread count contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "bulk and mark-all inbox mutations",
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
    failures.push(`forum notification unread count contract must keep ${residual} explicitly open`);
  }
}
if (contract.not_delivered?.some((item) => item.includes("unread count"))) {
  failures.push("FORUM-20X must remove the exact owner unread count from current residuals");
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
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20X") {
  failures.push("unread count contract must require the canonical ledger through FORUM-20X");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("unread count contract must require FORUM-20H through FORUM-20X delivered sections");
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
  requireText(plan, "FORUM-20A-X provide", "synchronized canonical plan must advance through X");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct NotificationInboxUnreadCountRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub struct NotificationInboxUnreadCount",
  "pub unread_count: u64",
  "pub struct NotificationInboxUnreadCountService",
  "pub async fn count_unread(",
  "validate_request(&request)?",
  "notification::Entity::find()",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  ".filter(notification::Column::State.eq(NotificationState::Unread))",
  ".count(&self.db)",
  "notification inbox unread count identity must not be nil",
]) {
  requireText(owner, marker, `notification unread count owner is missing ${marker}`);
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
  rejectText(owner, forbidden, `unread count owner must preserve its narrow read boundary against ${forbidden}`);
}

const countIndex = owner.indexOf("pub async fn count_unread(");
const tenantIndex = owner.indexOf(
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  countIndex,
);
const recipientIndex = owner.indexOf(
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  countIndex,
);
const stateIndex = owner.indexOf(
  ".filter(notification::Column::State.eq(NotificationState::Unread))",
  countIndex,
);
const aggregateIndex = owner.indexOf(".count(&self.db)", countIndex);
if (
  countIndex < 0 ||
  tenantIndex < 0 ||
  recipientIndex < 0 ||
  stateIndex < 0 ||
  aggregateIndex < 0 ||
  !(countIndex < tenantIndex &&
    tenantIndex < recipientIndex &&
    recipientIndex < stateIndex &&
    stateIndex < aggregateIndex)
) {
  failures.push("unread count must apply tenant recipient and unread-state filters before aggregation");
}

for (const marker of [
  "pub recipient_id: Uuid",
  "pub state: NotificationState",
]) {
  requireText(entities, marker, `notification persistence identity is missing ${marker}`);
}
for (const marker of [
  "CREATE INDEX IF NOT EXISTS idx_notifications_inbox",
  "ON notifications (tenant_id, recipient_id, state, created_at DESC, id DESC)",
]) {
  requireText(migration, marker, `notification inbox count index support is missing ${marker}`);
}
for (const marker of [
  "mod inbox_count;",
  "NotificationInboxUnreadCount",
  "NotificationInboxUnreadCountRequest",
  "NotificationInboxUnreadCountService",
]) {
  requireText(library, marker, `notifications public library surface is missing ${marker}`);
}

for (const marker of [
  "count_tracks_exact_recipient_unread_owner_state",
  "empty_and_foreign_scopes_return_zero_without_an_oracle",
  "nil_count_identity_is_rejected",
  "NotificationInboxUnreadCount { unread_count: 1 }",
  "NotificationInboxUnreadCount { unread_count: 3 }",
  "assert_eq!(stored_after, stored_before)",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `unread count SQLite proof is missing ${marker}`);
}

for (const marker of [
  "exact unread counting",
  "NotificationInboxUnreadCountService",
  "### 6. Exact unread count",
  "tests/inbox_count_sqlite.rs",
  "bulk/mark-all mutations and grouped inbox views",
]) {
  requireText(rootReadme, marker, `notifications root README is missing ${marker}`);
}
for (const marker of [
  "### Exact unread count",
  "NotificationInboxUnreadCountService",
  "stored owner state",
  "tests/inbox_count_sqlite.rs",
  "verify-forum-notification-inbox-unread-count.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20X`",
  "NotificationInboxUnreadCountService",
  "owner table is authoritative",
  "tests/inbox_count_sqlite.rs",
  "scheduled reconciliation",
]) {
  requireText(ownerPlan, marker, `notifications owner implementation plan is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20W" ||
  upstream.upstream_task !== "FORUM-20V" ||
  upstream.composition?.exact_owner_mark_unread_command !== true ||
  !upstream.not_delivered?.includes("canonical unread counts and grouped inbox views")
) {
  failures.push("FORUM-20X must remain linked to the historical FORUM-20W unread-count residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox unread count verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox unread count contract is source-ready.");
