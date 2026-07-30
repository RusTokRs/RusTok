#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";

const root = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(".");
const failures = [];

function read(relativePath) {
  const target = path.join(root, relativePath);
  if (!existsSync(target)) {
    failures.push(`${relativePath}: expected file is missing`);
    return "";
  }
  return readFileSync(target, "utf8");
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
}

function rejectMarker(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
}

const inboxPath = "crates/rustok-search/src/forum_inbox.rs";
const ingestionPath = "crates/rustok-search/src/ingestion.rs";
const manifestPath = "crates/rustok-search/Cargo.toml";
const migrationPath =
  "crates/rustok-search/src/migrations/m20260730_000009_create_search_projection_inbox.rs";
const migrationRegistryPath = "crates/rustok-search/src/migrations/mod.rs";
const libPath = "crates/rustok-search/src/lib.rs";
const rustTestPath = "crates/rustok-search/tests/forum_projection_inbox_contract.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-inbox-ordering.json";
const approvedReplyPath = "crates/rustok-forum/contracts/forum-approved-reply-search.json";
const notePath = "crates/rustok-forum/docs/forum-20bp-search-inbox-ordering.md";

const inbox = read(inboxPath);
const ingestion = read(ingestionPath);
const manifest = read(manifestPath);
const migration = read(migrationPath);
const migrationRegistry = read(migrationRegistryPath);
const lib = read(libPath);
const rustTest = read(rustTestPath);
const note = read(notePath);

let contract = null;
let approvedReply = null;
for (const [label, source, assign] of [
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [approvedReplyPath, read(approvedReplyPath), (value) => { approvedReply = value; }],
]) {
  try {
    assign(JSON.parse(source));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

for (const marker of [
  "CREATE TABLE IF NOT EXISTS search_projection_inbox",
  "event_id UUID PRIMARY KEY",
  "envelope_json JSONB NOT NULL",
  "'retryable_error'",
  "'dead_letter'",
  "CREATE TABLE IF NOT EXISTS search_projection_watermarks",
  "PRIMARY KEY (tenant_id, source_module, scope_key)",
  "idx_search_projection_inbox_due",
  "DatabaseBackend::Postgres",
  "DatabaseBackend::Sqlite",
]) {
  requireMarker(migration, marker, migrationPath);
}
rejectMarker(migration, "search_projection_rollup_forum_watermark", migrationPath);
requireMarker(
  migrationRegistry,
  "mod m20260730_000009_create_search_projection_inbox;",
  migrationRegistryPath,
);
requireMarker(
  migrationRegistry,
  "Box::new(m20260730_000009_create_search_projection_inbox::Migration)",
  migrationRegistryPath,
);

for (const marker of [
  "pub(crate) enum ForumProjectionScope",
  "DomainEvent::ForumTopicCreated",
  "DomainEvent::ForumReplyStatusChanged",
  '("search", _) | ("forum", _) | ("forum_topic", Some(_))',
  '("forum_category", Some(category_id))',
  "serde_json::to_value(envelope)?",
  "SqlValue::Json(Some(Box::new(envelope_json)))",
  "ON CONFLICT (event_id) DO NOTHING",
  "status IN ('pending', 'retryable_error')",
  "ORDER BY revision_at ASC, event_id ASC",
  "FOR UPDATE",
  "due_at > Utc::now()",
  "pg_try_advisory_xact_lock(hashtextextended($1, 0))",
  "AS acquired",
  "search:{FORUM_SOURCE_MODULE}:{tenant_id}:{FULL_SCOPE_KEY}",
  "load_effective_watermark",
  "load_watermark(transaction, tenant_id, FULL_SCOPE_KEY)",
  "incoming_event_id.as_bytes() > watermark_event_id.as_bytes()",
  'Some("stale_revision")',
  "MAX_ATTEMPTS: u32 = 12",
  "retry_exhausted",
  "ON CONFLICT (tenant_id, source_module, scope_key)",
]) {
  requireMarker(inbox, marker, inboxPath);
}
for (const forbidden of [
  "pg_advisory_xact_lock(",
  "OnceLock",
  "OwnedMutexGuard",
  "rustok_forum::",
  "SystemTime::now",
]) {
  rejectMarker(inbox, forbidden, inboxPath);
}

for (const marker of [
  "forum_inbox: Option<ForumProjectionInbox>",
  "ForumProjectionInbox::new",
  "ForumProjectionScope::for_event(&envelope.event)",
  "inbox.enqueue(envelope, &scope).await?",
  "reconcile_forum_inbox",
  "FORUM_INBOX_EVENT_BATCH",
  "FORUM_INBOX_OPPORTUNISTIC_BATCH",
  "claim.complete().await?",
  "claim.retry(&error).await?",
  "projector.rebuild_tenant(envelope.tenant_id).await",
  "projector.delete_tenant(tenant_id).await",
  'refresh_entity(envelope.tenant_id, "forum_category"',
  "Forum projection inbox opportunistic reconciliation failed",
]) {
  requireMarker(ingestion, marker, ingestionPath);
}
rejectMarker(
  ingestion,
  '.refresh_entity(envelope.tenant_id, "forum_topic"',
  ingestionPath,
);

requireMarker(lib, "mod forum_inbox;", libPath);
requireMarker(lib, "SearchIngestionHandler::with_forum_source", libPath);
rejectMarker(lib, "rustok_forum::", libPath);
requireMarker(manifest, "rustok-content.workspace = true", manifestPath);
requireMarker(manifest, "[dev-dependencies]", manifestPath);
rejectMarker(manifest.split("[dev-dependencies]")[0], "tokio.workspace = true", manifestPath);

for (const marker of [
  "durable_inbox_stores_replayable_envelopes_and_terminal_state",
  "forum_events_are_strictly_ordered_by_envelope_revision",
  "handler_has_no_direct_forum_projection_bypass",
  "module_registers_inbox_without_new_runtime_dependency",
  "pg_try_advisory_xact_lock",
]) {
  requireMarker(rustTest, marker, rustTestPath);
}

for (const marker of [
  "FORUM-20BP",
  "ULID-backed UUID",
  "pg_try_advisory_xact_lock",
  "strict retry barrier",
  "does **not** advance the full-scope watermark",
  "12 attempts",
  "does **not** add a startup worker",
  "FORUM-20BQ",
  "No tests, Cargo commands, formatting, verifiers, workflows or CI were run",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BP") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BO") failures.push(`${contractPath}: unexpected upstream task`);
  if (contract.downstream_task !== "FORUM-20BQ") failures.push(`${contractPath}: unexpected downstream task`);
  if (contract.approved_reply_contract !== approvedReplyPath) failures.push(`${contractPath}: approved reply handoff drift`);

  for (const key of [
    "search_owns_inbox_storage",
    "search_projection_inbox_added",
    "search_projection_watermarks_added",
    "source_event_id_is_primary_dedupe_key",
    "full_event_envelope_is_persisted",
    "retryable_state_recorded",
    "attempt_count_and_next_attempt_recorded",
    "last_error_is_bounded",
  ]) {
    if (contract.storage_boundary?.[key] !== true) failures.push(`${contractPath}: storage ${key} drift`);
  }
  for (const key of [
    "search_documents_schema_changed",
    "forum_owner_storage_changed",
    "second_platform_event_log_added",
  ]) {
    if (contract.storage_boundary?.[key] !== false) failures.push(`${contractPath}: storage ${key} must remain false`);
  }
  if (contract.storage_boundary?.maximum_attempts !== 12) failures.push(`${contractPath}: retry bound drift`);

  for (const key of [
    "ordering_is_timestamp_then_event_id",
    "duplicate_event_id_is_idempotent",
    "stale_or_equal_revision_is_skipped",
    "invalid_stored_envelope_is_dead_lettered",
    "category_scope_has_specific_watermark",
    "category_effective_watermark_includes_last_full_scope_watermark",
    "full_scope_watermark_is_advanced_only_by_full_scope_work",
  ]) {
    if (contract.revision_boundary?.[key] !== true) failures.push(`${contractPath}: revision ${key} drift`);
  }
  for (const key of [
    "source_domain_event_schema_changed",
    "source_owner_revision_field_added",
    "new_reindex_target_string_added",
    "category_completion_advances_full_scope_watermark",
  ]) {
    if (contract.revision_boundary?.[key] !== false) failures.push(`${contractPath}: revision ${key} must remain false`);
  }

  for (const key of [
    "postgresql_try_advisory_transaction_lock_added",
    "lock_acquisition_is_non_blocking",
    "lock_scope_is_tenant_wide_forum_projection",
    "full_and_targeted_forum_operations_share_one_lock",
    "candidate_order_is_oldest_revision_first",
    "oldest_retry_backoff_blocks_newer_claims",
    "inbox_row_is_selected_for_update_after_lock",
    "watermark_is_checked_before_projection",
    "projection_finishes_before_watermark_commit",
    "watermark_and_terminal_inbox_state_commit_atomically",
    "competing_claims_release_database_connection_without_waiting",
    "same_process_pool_starvation_by_waiting_claims_prevented",
    "cross_process_serialization_remains_database_owned",
    "older_topic_or_reply_event_cannot_overwrite_newer_module_disable",
  ]) {
    if (contract.serialization_boundary?.[key] !== true) failures.push(`${contractPath}: serialization ${key} drift`);
  }
  if (contract.serialization_boundary?.projection_and_inbox_use_same_database_transaction !== false) {
    failures.push(`${contractPath}: transaction boundary drift`);
  }

  for (const key of [
    "enqueue_commits_before_projection_claim",
    "projection_failure_is_persisted_as_retryable",
    "retry_exhaustion_is_dead_lettered",
    "crash_before_claim_leaves_pending_work",
    "crash_during_projection_rolls_back_processing_claim",
    "crash_after_projection_before_terminal_commit_replays_idempotently",
    "forum_event_delivery_runs_bounded_reconciliation",
    "other_search_events_run_bounded_opportunistic_reconciliation",
  ]) {
    if (contract.recovery_boundary?.[key] !== true) failures.push(`${contractPath}: recovery ${key} drift`);
  }
  for (const key of [
    "startup_reconciliation_sweep_added",
    "idle_tenant_periodic_sweep_added",
    "external_queue_or_scheduler_dependency_added",
  ]) {
    if (contract.recovery_boundary?.[key] !== false) failures.push(`${contractPath}: recovery ${key} must remain false`);
  }

  for (const [key, expected] of Object.entries({
    workspace_root_dependency_changed: false,
    search_crate_runtime_dependency_changed: false,
    cargo_lock_changed: false,
    migration_added: true,
    postgresql_runtime_required_for_reconciliation: true,
    sqlite_schema_parity_added: true,
    ffa_status_changed: false,
    fba_status_changed: false,
  })) {
    if (contract.compatibility?.[key] !== expected) failures.push(`${contractPath}: compatibility ${key} drift`);
  }
}

if (approvedReply) {
  if (approvedReply.search_inbox_ordering_contract !== contractPath) {
    failures.push(`${approvedReplyPath}: inbox handoff drift`);
  }
  if (approvedReply.downstream_task !== "FORUM-20BQ") {
    failures.push(`${approvedReplyPath}: downstream task must advance to FORUM-20BQ`);
  }
  if (approvedReply.persistence_boundary?.consumer_envelope_revision_guard_added !== true) {
    failures.push(`${approvedReplyPath}: consumer ordering completion not recorded`);
  }
  if (approvedReply.persistence_boundary?.out_of_order_owner_revision_guard_added !== false) {
    failures.push(`${approvedReplyPath}: owner revision must remain explicitly absent`);
  }
  if (approvedReply.remaining_scope?.some((entry) => entry.includes("owner revision ordering and durable inbox"))) {
    failures.push(`${approvedReplyPath}: completed inbox work remains open`);
  }
}

if (failures.length > 0) {
  console.error("Forum Search inbox ordering verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search inbox ordering contract verified");
