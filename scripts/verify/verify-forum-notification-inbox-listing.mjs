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
  "crates/rustok-forum/contracts/forum-notification-inbox-listing.json";
const contract = JSON.parse(read(contractPath) || "{}");
const inbox = read(contract.notifications_owner_file ?? "");
const surface = read(contract.notifications_surface_file ?? "");
const entities = read(contract.notifications_entity_file ?? "");
const migration = read(contract.notifications_migration_file ?? "");
const rootReadme = read(contract.notifications_readme ?? "");
const docs = read(contract.notifications_live_contract ?? "");
const ownerPlan = read(contract.notifications_implementation_plan ?? "");
const proof = read(contract.sqlite_proof ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const plan = read(contract.canonical_plan ?? "");

if (contract.schema_version !== 1) {
  failures.push("forum notification inbox listing contract must use schema_version=1");
}
if (contract.task !== "FORUM-20T" || contract.upstream_task !== "FORUM-20S") {
  failures.push("forum notification inbox listing contract must connect FORUM-20S/T");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("inbox listing must not claim unexecuted evidence");
}

for (const delivered of [
  "bounded_owner_listing_service",
  "exact_tenant_recipient_query",
  "optional_exact_state_filter",
  "composite_descending_keyset_cursor",
  "nanosecond_cursor_precision",
  "limit_plus_one_raw_scan",
  "open_service_reuse",
  "privacy_and_source_filtering",
  "last_scanned_raw_cursor",
  "sparse_page_progress",
  "typed_sanitized_read_model",
  "dedicated_route_and_target_fields_not_exposed",
  "retryable_failure_atomicity",
  "read_state_unchanged",
  "delivery_attempts_unchanged",
  "public_crate_export",
  "sqlite_contract_proof",
  "root_notifications_docs",
  "live_notifications_docs",
  "owner_implementation_ledger",
]) {
  if (contract.composition?.[delivered] !== true) {
    failures.push(`forum notification inbox listing contract must record ${delivered} as delivered`);
  }
}

for (const residual of [
  "seen read and archive state mutations",
  "channel delivery enqueue and transports",
  "delivery-time target authorization",
  "host trust facts adapter",
  "host channel membership facts adapter",
  "initially non-public topic-created descriptor materialization",
  "search index SEO and deep-link migration",
  "PostgreSQL and cross-consumer runtime evidence",
]) {
  if (!contract.not_delivered?.includes(residual)) {
    failures.push(`forum notification inbox listing contract must keep ${residual} explicitly open`);
  }
}
if (contract.not_delivered?.includes("bounded inbox listing API")) {
  failures.push("FORUM-20T must remove bounded inbox listing from current residuals");
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
];
const planSync = contract.canonical_plan_sync ?? {};
if (planSync.required_ledger_through !== "FORUM-20T") {
  failures.push("inbox listing contract must require the canonical ledger through FORUM-20T");
}
if (JSON.stringify(planSync.required_delivered_sections) !== JSON.stringify(deliveredSlices)) {
  failures.push("inbox listing contract must require FORUM-20H through FORUM-20T delivered sections");
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
  requireText(plan, "FORUM-20A-T provide", "synchronized canonical plan must advance through T");
  for (const slice of deliveredSlices) {
    requireText(plan, `### Delivered in \`${slice}\``, `canonical plan is missing ${slice}`);
  }
} else {
  failures.push("canonical_plan_sync.status must be pending or synchronized");
}

for (const marker of [
  "pub const DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE: u16 = 20",
  "pub const MAX_NOTIFICATION_INBOX_PAGE_SIZE: u16 = 64",
  "const INBOX_CURSOR_VERSION: &str = \"i1\"",
  "pub struct NotificationInboxListRequest",
  "pub tenant_id: Uuid",
  "pub recipient_id: Uuid",
  "pub state: Option<NotificationState>",
  "pub cursor: Option<String>",
  "pub limit: u16",
  "pub fn bounded_limit(&self) -> u64",
  "DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE",
  "requested.min(MAX_NOTIFICATION_INBOX_PAGE_SIZE)",
  "pub struct NotificationInboxItem",
  "pub source: NotificationSourceSlug",
  "pub notification_type: NotificationTypeKey",
  "pub template_key: NotificationTemplateKey",
  "pub actor_id: Option<Uuid>",
  "pub priority: NotificationPriority",
  "pub state: NotificationState",
  "pub template_data: NotificationTemplateData",
  "pub created_at: DateTime<FixedOffset>",
  "pub struct NotificationInboxPage",
  "pub items: Vec<NotificationInboxItem>",
  "pub next_cursor: Option<String>",
  "pub has_more: bool",
  "pub struct NotificationInboxListService",
  "open: NotificationInboxOpenService",
  "pub async fn list_page(",
  "notification::Entity::find()",
  ".filter(notification::Column::TenantId.eq(request.tenant_id))",
  ".filter(notification::Column::RecipientId.eq(request.recipient_id))",
  "notification::Column::State.eq(state)",
  "notification::Column::CreatedAt.lt(cursor.created_at.to_owned())",
  "notification::Column::CreatedAt.eq(cursor.created_at)",
  "notification::Column::Id.lt(cursor.id)",
  ".order_by_desc(notification::Column::CreatedAt)",
  ".order_by_desc(notification::Column::Id)",
  ".limit(limit + 1)",
  "let has_more = rows.len() > limit as usize",
  "rows.truncate(limit as usize)",
  "rows.last().map(encode_inbox_cursor)",
  ".authorize_open(NotificationInboxOpenRequest {",
  "if matches!(decision, NotificationInboxOpenDecision::Allowed { .. })",
  "items.push(materialize_inbox_item(stored)?)",
  "stored.created_at.timestamp()",
  "stored.created_at.timestamp_subsec_nanos()",
  "DateTime::<Utc>::from_timestamp(seconds, nanos)",
  "Uuid::parse_str(part)",
  ".filter(|id| !id.is_nil())",
  "serde_json::from_value(stored.template_data_json)?",
]) {
  requireText(inbox, marker, `notification inbox listing owner is missing ${marker}`);
}

const itemStart = inbox.indexOf("pub struct NotificationInboxItem");
const pageStart = inbox.indexOf("pub struct NotificationInboxPage");
if (itemStart < 0 || pageStart <= itemStart) {
  failures.push("notification inbox listing read model boundaries are missing");
} else {
  const itemBlock = inbox.slice(itemStart, pageStart);
  for (const forbidden of ["route:", "target:", "target_id", "target_owner", "target_kind"]) {
    rejectText(itemBlock, forbidden, `inbox list read model must not expose ${forbidden}`);
  }
}

const queryIndex = inbox.indexOf("notification::Entity::find()");
const cursorIndex = inbox.indexOf("rows.last().map(encode_inbox_cursor)");
const openIndex = inbox.indexOf(".authorize_open(NotificationInboxOpenRequest {");
const materializeIndex = inbox.indexOf("items.push(materialize_inbox_item(stored)?)");
if (
  queryIndex < 0 ||
  cursorIndex < 0 ||
  openIndex < 0 ||
  materializeIndex < 0 ||
  !(queryIndex < cursorIndex && cursorIndex < openIndex && openIndex < materializeIndex)
) {
  failures.push("raw scan and last-scanned cursor must precede open authorization and materialization");
}

for (const forbidden of [
  "notification::ActiveModel",
  "delivery_attempt::",
  "seen_at: Set(",
  "read_at: Set(",
  "archived_at: Set(",
  ".update(&self.db)",
  ".delete(&self.db)",
]) {
  rejectText(inbox, forbidden, `inbox listing must not mutate owner state through ${forbidden}`);
}

for (const marker of [
  "DEFAULT_NOTIFICATION_INBOX_PAGE_SIZE",
  "MAX_NOTIFICATION_INBOX_PAGE_SIZE",
  "NotificationInboxItem",
  "NotificationInboxListRequest",
  "NotificationInboxListService",
  "NotificationInboxPage",
]) {
  requireText(surface, marker, `notifications public surface is missing ${marker}`);
}
for (const marker of [
  "pub source_slug: String",
  "pub notification_type: String",
  "pub template_key: String",
  "pub actor_id: Option<Uuid>",
  "pub priority: NotificationPriorityValue",
  "pub state: NotificationState",
  "pub template_data_json: Json",
  "pub created_at: DateTimeWithTimeZone",
]) {
  requireText(entities, marker, `notification persistence read identity is missing ${marker}`);
}
for (const marker of [
  "CREATE INDEX IF NOT EXISTS idx_notifications_inbox",
  "ON notifications (tenant_id, recipient_id, state, created_at DESC, id DESC)",
]) {
  requireText(migration, marker, `notification inbox persistence index is missing ${marker}`);
}

for (const marker of [
  "sparse_pages_advance_by_raw_rows_and_return_only_currently_authorized_items",
  "state_filter_foreign_recipient_and_invalid_cursor_fail_closed_before_authorization",
  "retryable_policy_and_source_failures_abort_pages_without_mutating_rows",
  "NotificationInboxListService::new",
  "NotificationInboxListRequest",
  "NotificationRecipientPolicyDecision::Suppress",
  "NotificationOpenAuthorization::Unavailable",
  "empty raw page should still advance its cursor",
  "second raw page should advance its cursor",
  "foreign recipient page should be indistinguishably empty",
  "invalid inbox cursor must fail before authorization",
  "retryable recipient policy failure must abort the whole page",
  "retryable source owner failure must abort the whole page",
  "NOTIFICATION_SOURCE_PROVIDER_FAILURE",
  "delivery_attempt::Entity::find()",
]) {
  requireText(proof, marker, `inbox listing SQLite proof is missing ${marker}`);
}

for (const marker of [
  "### Bounded authorized inbox listing",
  "A request defaults to 20 rows, is capped at 64",
  "Each scanned row is passed through `NotificationInboxOpenService`",
  "adds no dedicated route or structural target owner, kind, or ID fields",
  "last scanned raw row rather than the last returned item",
  "empty page with a next cursor",
  "seen/read/archive state APIs",
  "inbox_listing_sqlite",
  "verify-forum-notification-inbox-listing.mjs",
]) {
  requireText(docs, marker, `notifications live contract is missing ${marker}`);
}

for (const marker of [
  "bounded authorized inbox listing",
  "NotificationInboxOpenService",
  "NotificationInboxListService",
  "default page of 20 and a hard cap of 64",
  "empty page with a next cursor",
  "seen/read/archive mutation APIs",
]) {
  requireText(rootReadme, marker, `notifications root README is missing ${marker}`);
}

for (const marker of [
  "Exact inbox open and bounded listing services are now owner-public",
  "### `FORUM-20R / FORUM-20S`",
  "### `FORUM-20T`",
  "default/hard limits 20/64",
  "empty page with a next cursor",
  "seen/read/archive commands",
  "inbox_listing_sqlite",
  "verify-forum-notification-inbox-listing.mjs",
]) {
  requireText(ownerPlan, marker, `notifications owner implementation plan is missing ${marker}`);
}

if (
  upstream.schema_version !== 1 ||
  upstream.task !== "FORUM-20S" ||
  upstream.upstream_task !== "FORUM-20R" ||
  upstream.downstream_task !== "FORUM-20T" ||
  upstream.composition?.privacy_before_source_authorization !== true ||
  !upstream.not_delivered?.includes("bounded inbox listing API")
) {
  failures.push("FORUM-20T must remain linked to the FORUM-20S privacy contract and its listing residual");
}

if (failures.length > 0) {
  console.error("Forum notification inbox listing verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum notification inbox listing contract is source-ready.");
