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
  "crates/rustok-forum/contracts/forum-notification-inbox-group-summaries.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const openOwner = read(contract.notifications_open_owner_file ?? "");
const entity = read(contract.notifications_entity_file ?? "");
const indexMigration = read(contract.notifications_index_migration_file ?? "");
const migrationRegistry = read(contract.notifications_migration_registry ?? "");
const library = read(contract.notifications_lib_file ?? "");
const readme = read(contract.notifications_readme ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("group-summary contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AE" || contract.upstream_task !== "FORUM-20AD") {
  failures.push("group-summary contract must connect FORUM-20AD/20AE");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("group-summary contract must not claim unexecuted evidence");
}

for (const key of [
  "bounded_group_summary_owner",
  "nonnil_owner_identity_validation",
  "nonarchived_group_selection",
  "latest_row_per_group",
  "stable_latest_tie_breaker",
  "exact_nonarchived_item_count",
  "exact_unread_count",
  "authorized_latest_item_projection",
  "recipient_privacy_before_source",
  "shared_page_bounds",
  "shared_versioned_cursor",
  "raw_group_progress_cursor",
  "sparse_group_pages",
  "retryable_failure_aborts_page",
  "missing_foreign_archived_empty",
  "dedicated_partial_summary_index",
  "no_route_or_structural_target_exposure",
  "inbox_state_unchanged",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`group-summary contract must record ${key}`);
  }
}
for (const key of [
  "group_level_state_commands",
  "external_transport_adapter",
  "grouped_ui",
]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`group-summary contract must keep ${key} false`);
  }
}
if (
  !contract.not_delivered?.includes(
    "group-level mark-read mark-unread and archive commands",
  )
) {
  failures.push("group-level state commands must remain open");
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20AE") {
  failures.push("canonical ledger must be required through FORUM-20AE");
}
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") {
    failures.push("pending plan sync must identify FORUM-20G");
  }
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded through G");
  rejectText(plan, "### Delivered in `FORUM-20AE`", "canonical plan sync status is stale");
}

for (const marker of [
  "pub struct NotificationInboxGroupSummaryRequest",
  "pub struct NotificationInboxGroupSummary",
  "pub group_key: String",
  "pub item_count: u64",
  "pub unread_count: u64",
  "pub latest_item: NotificationInboxItem",
  "pub struct NotificationInboxGroupSummaryPage",
  "pub struct NotificationInboxGroupSummaryService",
  "NotificationInboxOpenService::new",
  "validate_request(&request)?",
  ".map(decode_inbox_cursor)",
  "DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE",
  "MAX_NOTIFICATION_INBOX_PAGE_SIZE",
  "latest.group_key IS NOT NULL",
  "latest.state <> 'archived'",
  "counted.state <> 'archived'",
  "unread.state = 'unread'",
  "NOT EXISTS",
  "newer.created_at > latest.created_at",
  "newer.created_at = latest.created_at AND newer.id > latest.id",
  "ORDER BY latest.created_at DESC, latest.id DESC",
  "rows.len() > limit as usize",
  "rows.truncate(limit as usize)",
  "encode_summary_cursor",
  ".authorize_open(NotificationInboxOpenRequest",
  "NotificationInboxOpenDecision::Allowed",
  "notification inbox group summary identity must not be nil",
]) {
  requireText(owner, marker, `group-summary owner is missing ${marker}`);
}
for (const forbidden of [
  "update_many()",
  "ActiveModel {",
  "delivery_attempt::",
  "NotificationInboxStateService",
  "NotificationTargetRoute",
]) {
  rejectText(owner, forbidden, `group-summary owner must not use ${forbidden}`);
}

const rawQuery = owner.indexOf("let query_rows = self.db.query_all(statement).await?");
const authorize = owner.indexOf(".authorize_open(NotificationInboxOpenRequest", rawQuery);
if (rawQuery < 0 || authorize < 0 || rawQuery >= authorize) {
  failures.push("group-summary owner must select bounded raw groups before authorization");
}

for (const marker of [
  "pub struct NotificationInboxOpenService",
  ".policy",
  ".evaluate(NotificationRecipientPolicyRequest",
  ".registry",
  ".authorize_target_open(AuthorizeNotificationTargetRequest",
]) {
  requireText(openOwner, marker, `open owner is missing ${marker}`);
}
const privacy = openOwner.indexOf(".evaluate(NotificationRecipientPolicyRequest");
const source = openOwner.indexOf(".authorize_target_open(AuthorizeNotificationTargetRequest");
if (privacy < 0 || source < 0 || privacy >= source) {
  failures.push("group summaries must preserve recipient privacy before source authorization");
}

requireText(entity, "pub group_key: Option<String>", "notification entity must retain group_key");
for (const marker of [
  "pub struct Migration",
  "DatabaseBackend::Postgres | DatabaseBackend::Sqlite",
  "CREATE INDEX IF NOT EXISTS idx_notifications_group_summary",
  "ON notifications (tenant_id, recipient_id, created_at DESC, id DESC, group_key)",
  "WHERE group_key IS NOT NULL AND state <> 'archived'",
  "DROP INDEX IF EXISTS idx_notifications_group_summary",
]) {
  requireText(indexMigration, marker, `group-summary index migration is missing ${marker}`);
}

for (const marker of [
  "mod m20260726_000016_add_notification_group_summary_index;",
  "Box::new(m20260726_000016_add_notification_group_summary_index::Migration)",
  '"m20260726_000016_add_notification_group_summary_index"',
  'vec!["m20260726_000015_populate_notification_group_keys"]',
]) {
  requireText(migrationRegistry, marker, `migration registry is missing ${marker}`);
}

for (const marker of [
  "mod inbox_group_summary;",
  "NotificationInboxGroupSummary",
  "NotificationInboxGroupSummaryPage",
  "NotificationInboxGroupSummaryRequest",
  "NotificationInboxGroupSummaryService",
  "assert_eq!(module.migrations().len(), 7)",
  "assert_eq!(module.migration_dependencies().len(), 7)",
]) {
  requireText(library, marker, `public library is missing ${marker}`);
}

for (const marker of [
  "summaries_count_non_archived_rows_order_latest_and_preserve_sparse_progress",
  "missing_foreign_archived_and_invalid_summary_requests_fail_closed",
  "retryable_summary_authorization_failure_aborts_without_partial_result_or_mutation",
  "group_summary_limits_reuse_shared_inbox_bounds",
  "assert_eq!(first.groups[0].item_count, 3)",
  "assert_eq!(first.groups[0].unread_count, 1)",
  "assert_eq!(first.groups[0].latest_item.id, Uuid::from_u128(108))",
  "assert!(second.groups.is_empty())",
  "Some(\"invalid-cursor\".to_string())",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `SQLite group-summary proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AE bounded notification group summaries",
  "NotificationInboxGroupSummaryService",
  "item_count",
  "unread_count",
  "latest_item",
  "idx_notifications_group_summary",
  "tests/inbox_group_summary_sqlite.rs",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

for (const marker of [
  "NotificationInboxGroupSummaryService",
  "seven ordered PostgreSQL/SQLite migrations",
  "m20260726_000016_add_notification_group_summary_index",
  "tests/inbox_group_summary_sqlite.rs",
  "group-level mark-read, mark-unread, and archive commands",
]) {
  requireText(readme, marker, `notifications README is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AD" ||
  upstream.composition?.owner_level_group_key_population !== true ||
  upstream.composition?.grouped_aggregate_summary !== false ||
  upstream.composition?.group_unread_total !== false ||
  upstream.composition?.latest_item_projection !== false ||
  !upstream.not_delivered?.includes(
    "grouped aggregate summaries unread totals and latest-item projections",
  )
) {
  failures.push("FORUM-20AE must close the FORUM-20AD grouped-summary residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox group-summary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox group-summary contract is source-ready.");
