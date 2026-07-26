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
  "crates/rustok-forum/contracts/forum-notification-inbox-group-listing.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.notifications_owner_file ?? "");
const openOwner = read(contract.notifications_open_owner_file ?? "");
const entity = read(contract.notifications_entity_file ?? "");
const migration = read(contract.notifications_migration_file ?? "");
const candidate = read(contract.notifications_candidate_file ?? "");
const library = read(contract.notifications_lib_file ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("group-listing contract must use schema_version=1");
}
if (contract.task !== "FORUM-20AC" || contract.upstream_task !== "FORUM-20AB") {
  failures.push("group-listing contract must connect FORUM-20AB/20AC");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("group-listing contract must not claim unexecuted evidence");
}

for (const key of [
  "bounded_group_list_owner",
  "nonnil_owner_identity_validation",
  "bounded_group_key_validation",
  "exact_group_filter_before_authorization",
  "optional_exact_state_filter",
  "shared_page_bounds",
  "shared_versioned_cursor",
  "stable_descending_selection",
  "raw_progress_cursor",
  "recipient_privacy_before_source",
  "sparse_group_pages",
  "retryable_failure_aborts_page",
  "existing_group_index_reused",
  "existing_inbox_read_model_reused",
  "no_route_or_structural_target_exposure",
  "inbox_state_unchanged",
  "delivery_attempts_unchanged",
  "sqlite_contract_proof",
  "owner_contract_note",
]) {
  if (contract.composition?.[key] !== true) {
    failures.push(`group-listing contract must record ${key}`);
  }
}
for (const key of ["source_group_key_population", "grouped_aggregate_summary"]) {
  if (contract.composition?.[key] !== false) {
    failures.push(`group-listing contract must keep ${key} false`);
  }
}
if (
  !contract.not_delivered?.includes(
    "source or notifications grouping policy and production group key population",
  )
) {
  failures.push("production group-key population must remain open");
}
if (
  !contract.not_delivered?.includes(
    "grouped aggregate summaries unread totals and latest-item projections",
  )
) {
  failures.push("group aggregate summaries must remain open");
}

const sync = contract.canonical_plan_sync ?? {};
if (sync.required_ledger_through !== "FORUM-20AC") {
  failures.push("canonical ledger must be required through FORUM-20AC");
}
if (sync.status === "pending") {
  if (sync.current_plan_through !== "FORUM-20G") {
    failures.push("pending plan sync must identify FORUM-20G");
  }
  requireText(plan, "FORUM-20A-G provide", "pending plan sync must remain grounded through G");
  rejectText(plan, "### Delivered in `FORUM-20AC`", "canonical plan sync status is stale");
}

for (const marker of [
  "pub const MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES: usize = 191",
  "pub struct NotificationInboxGroupListRequest",
  "pub group_key: String",
  "pub state: Option<NotificationState>",
  "pub cursor: Option<String>",
  "pub limit: u16",
  "pub struct NotificationInboxGroupListService",
  "NotificationInboxOpenService::new",
  "validate_request(&request)?",
  ".map(decode_inbox_cursor)",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  ".filter(notification::Column::GroupKey.eq(request.group_key.as_str()))",
  ".filter(notification::Column::State.eq(state))",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "rows.truncate(limit as usize)",
  "rows.last().map(encode_inbox_cursor)",
  ".authorize_open(NotificationInboxOpenRequest",
  "NotificationInboxOpenDecision::Allowed",
  "NotificationInboxPage {",
  "request.group_key != request.group_key.trim()",
  "request.group_key.chars().any(char::is_control)",
  "notification inbox group list identity must not be nil",
  "notification inbox group key must contain between 1 and",
]) {
  requireText(owner, marker, `group-listing owner is missing ${marker}`);
}
for (const forbidden of [
  "update_many()",
  "ActiveModel {",
  "Set(",
  "delivery_attempt::",
  "NotificationInboxStateService",
]) {
  rejectText(owner, forbidden, `group-listing owner must not use ${forbidden}`);
}

const groupFilter = owner.indexOf(
  ".filter(notification::Column::GroupKey.eq(request.group_key.as_str()))",
);
const rawLoad = owner.indexOf(".all(&self.db)", groupFilter);
const authorize = owner.indexOf(".authorize_open(NotificationInboxOpenRequest", rawLoad);
if (
  groupFilter < 0 ||
  rawLoad < 0 ||
  authorize < 0 ||
  !(groupFilter < rawLoad && rawLoad < authorize)
) {
  failures.push("group-listing owner must filter/load one raw group page before authorization");
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
  failures.push("group listing must preserve recipient privacy before source authorization");
}

requireText(entity, "pub group_key: Option<String>", "notification entity must retain group_key");
requireText(
  migration,
  "CREATE INDEX IF NOT EXISTS idx_notifications_group",
  "group listing must reuse the existing group index",
);
requireText(
  migration,
  "ON notifications (tenant_id, recipient_id, group_key, created_at DESC)",
  "group index must remain tenant/recipient/group scoped",
);
requireText(
  candidate,
  "group_key: Set(None)",
  "production group-key population must remain explicitly open",
);

for (const marker of [
  "MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES",
  "NotificationInboxGroupListRequest",
  "NotificationInboxGroupListService",
]) {
  requireText(library, marker, `public library is missing ${marker}`);
}

for (const marker of [
  "exact_group_sparse_pages_exclude_other_groups_and_preserve_progress",
  "state_filter_missing_foreign_and_invalid_group_requests_fail_closed",
  "retryable_group_authorization_failure_aborts_without_partial_result_or_mutation",
  "group_listing_limits_reuse_shared_inbox_bounds",
  "group_key: Set(Some(group_key.to_string()))",
  "delivery_attempt::Entity::find()",
  "invalid-cursor",
  "MAX_NOTIFICATION_INBOX_GROUP_KEY_BYTES + 1",
]) {
  requireText(proof, marker, `SQLite proof is missing ${marker}`);
}

for (const marker of [
  "# FORUM-20AC bounded notification group listing",
  "NotificationInboxGroupListService",
  "idx_notifications_group",
  "Current production candidate finalization still stores `group_key = NULL`",
  "tests/inbox_group_listing_sqlite.rs",
]) {
  requireText(note, marker, `owner note is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20AB" ||
  upstream.composition?.bounded_selected_state_owner !== true ||
  !upstream.not_delivered?.includes("grouped inbox views")
) {
  failures.push("FORUM-20AC must remain linked to the FORUM-20AB grouped-view residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox group-listing verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox group-listing contract is source-ready.");
