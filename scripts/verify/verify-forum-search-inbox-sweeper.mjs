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

const reconcilerPath = "crates/rustok-search/src/forum_reconciliation.rs";
const inboxPath = "crates/rustok-search/src/forum_inbox.rs";
const libPath = "crates/rustok-search/src/lib.rs";
const workerPath = "apps/server/src/services/forum_search_inbox_worker.rs";
const servicesPath = "apps/server/src/services/mod.rs";
const bootstrapPath = "apps/server/src/services/server_bootstrap.rs";
const rustTestPath = "crates/rustok-search/tests/forum_projection_sweeper_contract.rs";
const contractPath = "crates/rustok-forum/contracts/forum-search-inbox-sweeper.json";
const orderingPath = "crates/rustok-forum/contracts/forum-search-inbox-ordering.json";
const notePath = "crates/rustok-forum/docs/forum-20bq-search-inbox-sweeper.md";

const reconciler = read(reconcilerPath);
const inbox = read(inboxPath);
const lib = read(libPath);
const worker = read(workerPath);
const services = read(servicesPath);
const bootstrap = read(bootstrapPath);
const rustTest = read(rustTestPath);
const note = read(notePath);

let contract = null;
let ordering = null;
for (const [label, source, assign] of [
  [contractPath, read(contractPath), (value) => { contract = value; }],
  [orderingPath, read(orderingPath), (value) => { ordering = value; }],
]) {
  try {
    assign(JSON.parse(source));
  } catch (error) {
    failures.push(`${label}: invalid JSON: ${error.message}`);
  }
}

for (const marker of [
  "pub struct ForumProjectionReconciler",
  "SELECT DISTINCT ON (tenant_id)",
  "status IN ('pending', 'retryable_error')",
  "ORDER BY tenant_id, ingest_sequence ASC",
  "next_attempt_at <= CURRENT_TIMESTAMP",
  "ORDER BY ingest_sequence ASC",
  "DEFAULT_FORUM_SWEEP_TENANT_LIMIT: usize = 32",
  "DEFAULT_FORUM_SWEEP_EVENT_LIMIT: usize = 64",
  "MAX_FORUM_SWEEP_TENANT_LIMIT: usize = 256",
  "MAX_FORUM_SWEEP_EVENT_LIMIT: usize = 256",
  "self.inbox.claim_next(tenant_id).await?",
  "claim.complete().await?",
  "claim.retry(&error).await?",
  "self.forum_projector.rebuild_tenant",
  "self.forum_projector.refresh_entity",
  "self.forum_projector.delete_tenant",
]) {
  requireMarker(reconciler, marker, reconcilerPath);
}
for (const forbidden of [
  "UPDATE search_projection_inbox",
  "INSERT INTO search_projection_watermarks",
  "SELECT DISTINCT tenant_id FROM search_projection_inbox",
]) {
  rejectMarker(reconciler, forbidden, reconcilerPath);
}
requireMarker(inbox, "pg_try_advisory_xact_lock", inboxPath);
requireMarker(lib, "pub use forum_reconciliation", libPath);

for (const marker of [
  "runs_background_workers()",
  "ForumSearchInboxWorkerHandle",
  "search_projection_source_registry_from_extensions",
  "supports_background_reconciliation()",
  "tokio::spawn(forum_search_inbox_worker_loop",
  "Duration::from_secs(5)",
  "DEFAULT_FORUM_SWEEP_TENANT_LIMIT",
  "DEFAULT_FORUM_SWEEP_EVENT_LIMIT",
  "StopHandle",
  "stop_rx.changed()",
  "Forum Search inbox sweep failed",
]) {
  requireMarker(worker, marker, workerPath);
}
for (const forbidden of [
  "search_projection_inbox",
  "search_projection_watermarks",
  "ReindexRequested",
  "pg_try_advisory_xact_lock",
]) {
  rejectMarker(worker, forbidden, workerPath);
}
requireMarker(services, "pub mod forum_search_inbox_worker;", servicesPath);
requireMarker(bootstrap, "start_forum_search_inbox_worker_if_ready", bootstrapPath);

for (const marker of [
  "due_tenant_discovery_preserves_oldest_event_retry_barrier",
  "sweeper_reuses_search_owned_claim_projection_and_retry_owners",
  "sweeper_replay_scope_matches_event_ingestion_scope",
  "host_worker_runs_startup_periodic_and_shutdown_aware_sweeps",
]) {
  requireMarker(rustTest, marker, rustTestPath);
}
for (const marker of [
  "FORUM-20BQ",
  "immediately after startup",
  "every five seconds",
  "oldest non-terminal Forum inbox row",
  "The server worker does not query",
  "No tests, Cargo commands, formatting, verifiers, workflows or CI were run",
]) {
  requireMarker(note, marker, notePath);
}

if (contract) {
  if (contract.task !== "FORUM-20BQ") failures.push(`${contractPath}: unexpected task`);
  if (contract.upstream_task !== "FORUM-20BP") failures.push(`${contractPath}: unexpected upstream task`);
  if (contract.downstream_task !== "FORUM-20BR") failures.push(`${contractPath}: unexpected downstream task`);
  if (contract.ordering_contract !== orderingPath) failures.push(`${contractPath}: ordering handoff drift`);
  for (const [boundary, keys] of Object.entries({
    discovery_boundary: [
      "search_owns_due_tenant_query",
      "oldest_non_terminal_event_per_tenant_is_selected",
      "tenant_is_due_only_when_oldest_event_is_due",
      "newer_due_event_cannot_bypass_older_backoff",
      "due_tenants_order_by_oldest_revision",
    ],
    execution_boundary: [
      "search_reconciler_owns_projection_replay",
      "existing_search_projectors_are_reused",
      "existing_forum_inbox_claim_owner_is_reused",
      "existing_tenant_wide_forum_advisory_lock_is_reused",
      "retry_and_dead_letter_semantics_preserved",
      "watermark_semantics_preserved",
    ],
    lifecycle_boundary: [
      "host_owned_worker_added",
      "worker_starts_only_when_background_workers_run",
      "worker_requires_forum_projection_source",
      "worker_requires_postgresql",
      "startup_sweep_runs_before_first_sleep",
      "periodic_sweep_added",
      "worker_uses_shared_stop_handle",
      "duplicate_worker_start_in_one_process_prevented",
      "multi_process_workers_allowed",
      "cross_process_execution_serialized_by_database",
      "sweep_failure_is_logged_and_retried_on_next_interval",
    ],
    recovery_boundary: [
      "startup_reconciliation_sweep_added",
      "idle_tenant_periodic_sweep_added",
      "pending_work_recovered_without_new_domain_event",
      "retryable_work_recovered_after_backoff_without_new_domain_event",
      "lock_contention_leaves_work_durable",
      "tenant_failure_does_not_remove_durable_work",
    ],
  })) {
    for (const key of keys) {
      if (contract[boundary]?.[key] !== true) failures.push(`${contractPath}: ${boundary}.${key} drift`);
    }
  }
  for (const [boundary, keys] of Object.entries({
    discovery_boundary: ["server_reads_inbox_tables_directly", "unbounded_tenant_scan_added", "unbounded_event_drain_added"],
    execution_boundary: ["server_constructs_projection_sql", "server_mutates_inbox_state", "new_domain_event_added", "new_reindex_target_added"],
    lifecycle_boundary: ["runtime_sweep_failure_stops_server", "external_scheduler_dependency_added"],
    recovery_boundary: ["retry_exhaustion_behavior_changed", "ordering_key_changed", "cross_producer_clock_skew_fully_resolved"],
  })) {
    for (const key of keys) {
      if (contract[boundary]?.[key] !== false) failures.push(`${contractPath}: ${boundary}.${key} must remain false`);
    }
  }
  if (contract.discovery_boundary?.default_tenant_limit !== 32) failures.push(`${contractPath}: tenant limit drift`);
  if (contract.discovery_boundary?.default_event_limit_per_tenant !== 64) failures.push(`${contractPath}: event limit drift`);
  if (contract.lifecycle_boundary?.poll_interval_seconds !== 5) failures.push(`${contractPath}: poll interval drift`);
}

if (ordering) {
  if (ordering.search_inbox_sweeper_contract !== contractPath) {
    failures.push(`${orderingPath}: sweeper handoff drift`);
  }
  if (ordering.recovery_completion?.startup_reconciliation_sweep_added !== true) {
    failures.push(`${orderingPath}: startup completion missing`);
  }
  if (ordering.recovery_completion?.idle_tenant_periodic_sweep_added !== true) {
    failures.push(`${orderingPath}: periodic completion missing`);
  }
  if (ordering.downstream_task !== "FORUM-20BQ") {
    failures.push(`${orderingPath}: historical downstream task drift`);
  }
}

if (failures.length > 0) {
  console.error("Forum Search inbox sweeper verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Forum Search inbox sweeper contract verified");
