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
  "crates/rustok-forum/contracts/forum-notification-inbox-reconciliation.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const openOwner = read(contract.notifications_open_owner_file ?? "");
const stateOwner = read(contract.notifications_state_owner_file ?? "");
const surface = read(contract.notifications_surface_file ?? "");
const rootReadme = read(contract.notifications_readme ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const ownerPlan = read(contract.notifications_implementation_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification inbox reconciliation contract must use schema_version=1");
}
if (contract.task !== "FORUM-20V" || contract.upstream_task !== "FORUM-20U") {
  failures.push("forum notification inbox reconciliation contract must connect FORUM-20U/V");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("inbox reconciliation must not claim unexecuted evidence");
}

for (const delivered of [
  "bounded_owner_reconciliation_service",
  "nonnil_identity_validation",
  "exact_tenant_recipient_query",
  "non_archived_scope",
  "default_and_hard_page_bounds",
  "bounded_cursor_input",
  "composite_descending_keyset_cursor",
  "nanosecond_cursor_precision",
  "limit_plus_one_raw_scan",
  "open_service_reuse",
  "privacy_before_source_ordering",
  "unavailable_only_archive",
  "state_owner_reuse",
  "archive_timestamp_preservation",
  "durable_idempotent_partial_progress",
  "foreign_calls_outside_transactions",
  "sanitized_progress_response",
  "delivery_attempts_unchanged",
  "public_crate_export",
  "sqlite_contract_proof",
  "root_notifications_docs",
  "live_notifications_docs",
  "owner_implementation_ledger",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification inbox reconciliation must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "mark unread mutation",
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
    failures.push(`forum notification inbox reconciliation must keep ${residual} explicitly open`);
  }
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
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20V") {
  failures.push("inbox reconciliation contract must require the canonical ledger through FORUM-20V");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("inbox reconciliation contract must require FORUM-20H through FORUM-20V delivered sections");
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
  requireText(plan, "FORUM-20A-V provide", "synchronized canonical plan must advance through V");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub struct NotificationInboxReconcileRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub cursor: Option<String>",
  "pub limit: u16",
  "pub struct NotificationInboxReconcilePage",
  "pub scanned: u16",
  "pub archived: u16",
  "pub next_cursor: Option<String>",
  "pub has_more: bool",
  "pub struct NotificationInboxReconcileService",
  "open: NotificationInboxOpenService",
  "state: NotificationInboxStateService",
  "pub async fn reconcile_page(",
  "validate_request(&request)?",
  "notification::Entity::find()",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  ".filter(notification::Column::State.ne(NotificationState::Archived))",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "NotificationInboxOpenService::new",
  ".authorize_open(NotificationInboxOpenRequest {",
  "NotificationInboxOpenDecision::Unavailable",
  "self.state.archive(identity).await?",
  "NotificationInboxStateDecision::Available { changed: true, .. }",
  "MAX_NOTIFICATION_INBOX_CURSOR_BYTES",
  "value.chars().any(char::is_control)",
  "timestamp_subsec_nanos()",
  "Uuid::parse_str(part)",
  "notification inbox reconciliation identity must not be nil",
  "invalid notification inbox reconciliation cursor",
]) {
  requireText(owner, marker, `notification inbox reconciliation owner is missing ${marker}`);
}

const rawQueryIndex = owner.indexOf("notification::Entity::find()");
const rowsLoadedIndex = owner.indexOf(".all(&self.db)");
const openIndex = owner.indexOf(".authorize_open(NotificationInboxOpenRequest {");
const archiveIndex = owner.indexOf("self.state.archive(identity).await?");
if (
  rawQueryIndex < 0 ||
  rowsLoadedIndex < 0 ||
  openIndex < 0 ||
  archiveIndex < 0 ||
  !(rawQueryIndex < rowsLoadedIndex && rowsLoadedIndex < openIndex && openIndex < archiveIndex)
) {
  failures.push("bounded raw selection must complete before open authorization and unavailable archive");
}

for (const forbidden of [
  "begin()",
  "TransactionTrait",
  "target_owner:",
  "target_kind:",
  "target_id:",
  "route:",
  "delivery_attempt",
  "NotificationInboxState::",
]) {
  rejectText(owner, forbidden, `inbox reconciliation must preserve its boundary against ${forbidden}`);
}

for (const marker of [
  "pub struct NotificationInboxOpenService",
  ".evaluate(NotificationRecipientPolicyRequest {",
  ".authorize_target_open(AuthorizeNotificationTargetRequest {",
]) {
  requireText(openOwner, marker, `open-time owner is missing reconciliation dependency ${marker}`);
}
const policyIndex = openOwner.indexOf(".evaluate(NotificationRecipientPolicyRequest {");
const sourceIndex = openOwner.indexOf(".authorize_target_open(AuthorizeNotificationTargetRequest {");
if (policyIndex < 0 || sourceIndex < 0 || policyIndex >= sourceIndex) {
  failures.push("recipient privacy must remain before source target authorization");
}

for (const marker of [
  "pub struct NotificationInboxStateService",
  "pub async fn archive(",
  "archived_at: Set(Some(timestamp.to_owned()))",
  ".filter(notification::Column::State.ne(NotificationState::Archived))",
]) {
  requireText(stateOwner, marker, `state owner is missing reconciliation dependency ${marker}`);
}

for (const marker of [
  "mod inbox_reconcile;",
  "NotificationInboxReconcilePage",
  "NotificationInboxReconcileRequest",
  "NotificationInboxReconcileService",
]) {
  requireText(surface, marker, `notifications public surface is missing ${marker}`);
}

for (const marker of [
  "bounded_pages_archive_only_currently_unavailable_rows",
  "foreign_and_invalid_requests_fail_before_owner_authorization",
  "retryable_failure_stops_after_durable_idempotent_progress",
  "assert_eq!(first.scanned, 2)",
  "assert_eq!(first.archived, 1)",
  "assert_eq!(policy_archived.state, NotificationState::Archived)",
  "assert!(policy_archived.seen_at.is_some())",
  "assert_eq!(source_archived.state, NotificationState::Archived)",
  "assert!(source_archived.read_at.is_some())",
  "restart should skip the already archived row",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `inbox reconciliation SQLite proof is missing ${marker}`);
}

for (const marker of [
  "NotificationInboxReconcileService",
  "bounded exact-recipient reconciliation",
  "tenant-wide scheduled reconciliation",
  "inbox_reconcile_sqlite",
]) {
  requireText(rootReadme, marker, `notifications root README is missing ${marker}`);
}
for (const marker of [
  "### Bounded inbox reconciliation",
  "privacy or source policy",
  "durable and idempotent",
  "tenant-wide scheduling and payload redaction",
  "inbox_reconcile_sqlite",
  "verify-forum-notification-inbox-reconciliation.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}
for (const marker of [
  "### `FORUM-20V`",
  "NotificationInboxReconcileService",
  "tenant-wide scheduled reconciliation",
  "tests/inbox_reconcile_sqlite.rs",
]) {
  requireText(ownerPlan, marker, `notifications owner implementation plan is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20U" ||
  upstream.upstream_task !== "FORUM-20T" ||
  upstream.downstream_task !== "FORUM-20V" ||
  upstream.composition?.exact_owner_state_service !== true
) {
  failures.push("FORUM-20V must remain linked to the FORUM-20U state owner contract");
}

if (failures.length > 0) {
  console.error("Forum notification inbox reconciliation verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox reconciliation contract is source-ready.");
