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
function requireOrder(source, markers, message) {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0 || index <= previous) {
      failures.push(`${message}: missing or out-of-order marker ${marker}`);
      return;
    }
    previous = index;
  }
}
function reject(source, pattern, message) {
  if (pattern.test(source)) failures.push(message);
}

const contract = JSON.parse(read(
  "crates/rustok-notifications/contracts/notifications-fanout-worker.json",
) || "{}");
const owner = read(contract.owner_worker ?? "");
const service = read(contract.canonical_service ?? "");
const server = read(contract.server_worker ?? "");
const bootstrap = read(contract.bootstrap ?? "");
const library = read("crates/rustok-notifications/src/lib.rs");
const test = read(contract.tests?.[0] ?? "");

if (contract.slice !== "NOTIFY-03E" || contract.schema_version !== 2) {
  failures.push("fanout worker contract must identify NOTIFY-03E schema 2");
}
if (contract.runtime?.default_enabled !== false || contract.runtime?.invalid_value_enabled !== false) {
  failures.push("fanout worker must fail closed and remain disabled by default");
}
if (contract.runtime?.requires_materialized_nonempty_source_registry !== true
  || contract.runtime?.requires_module_registry !== true) {
  failures.push("fanout worker requires materialized source and module registries");
}
if (contract.tenant_capability_gate?.authority !== "EffectiveModulePolicyService::is_enabled"
  || contract.tenant_capability_gate?.checked_before_each_source_claim !== true
  || contract.tenant_capability_gate?.checked_before_each_job_claim !== true
  || contract.tenant_capability_gate?.policy_error_fails_closed !== true) {
  failures.push("fanout worker must use the authoritative effective tenant policy before every claim");
}
if (contract.durability?.creates_final_notification_rows !== false
  || contract.durability?.creates_delivery_attempts !== false) {
  failures.push("fanout worker must not create final notifications or deliveries");
}

for (const marker of [
  "DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE: usize = 32",
  "MAX_NOTIFICATION_FANOUT_BATCH_SIZE: usize = 64",
  "DEFAULT_NOTIFICATION_FANOUT_PAGE_SIZE: u16 = 256",
  "MAX_NOTIFICATION_FANOUT_PAGE_SIZE: u16 = 256",
  "NotificationFanoutSourceWorkItem",
  "NotificationFanoutJobWorkItem",
  "claimable_source_inbox_work",
  "claimable_fanout_job_work",
  "tenant_id: row.tenant_id",
  "NotificationSourceInboxStatus::Pending",
  "NotificationSourceInboxStatus::RetryableError",
  "NotificationSourceInboxStatus::Processing",
  "NotificationJobStatus::Pending",
  "NotificationJobStatus::RetryableError",
  "NotificationJobStatus::Leased",
  "order_by_asc(source_inbox::Column::CreatedAt)",
  "order_by_asc(fanout_job::Column::CreatedAt)",
  "materialize_source_event",
  "process_fanout_page",
]) {
  requireText(owner, marker, `fanout owner worker is missing ${marker}`);
}
reject(
  owner,
  /update_many|ActiveModel\s*\{|notification::Entity|delivery_attempt::Entity/,
  "fanout driver must not acquire leases or bypass canonical service persistence",
);
for (const marker of [
  "claim_inbox(inbox_id, worker_id)",
  "claim_job(job_id, worker_id)",
  "describe_event",
  "resolve_audience",
  "persist_fanout_page",
]) {
  requireText(service, marker, `canonical fanout service is missing ${marker}`);
}

for (const marker of [
  "RUSTOK_NOTIFICATIONS_FANOUT_WORKER_ENABLED",
  "runs_background_workers()",
  "notification_source_registry_from_extensions",
  "source_registry.is_empty()",
  "shared_get::<ModuleRegistry>()",
  "EffectiveModulePolicyService::is_enabled",
  "tenant_notifications_enabled",
  "NOTIFICATIONS_MODULE_SLUG",
  "claimable_source_inbox_work().await",
  "materialize_source_inbox(work.inbox_id).await",
  "claimable_fanout_job_work().await",
  "process_fanout_job(work.job_id).await",
  "policy lookup failed closed",
  "StopHandle",
  "tokio::select!",
]) {
  requireText(server, marker, `server fanout worker is missing ${marker}`);
}
requireOrder(
  server,
  [
    "fanout_worker_enabled_from_environment()",
    "notification_source_registry_from_extensions",
    "shared_get::<ModuleRegistry>()",
    "NotificationFanoutWorker::new",
    "tokio::spawn",
  ],
  "fanout worker readiness must be checked before spawn",
);
requireOrder(
  server,
  [
    "for work in source_work",
    "tenant_notifications_enabled(&db, &module_registry, work.tenant_id).await",
    "worker.materialize_source_inbox(work.inbox_id).await",
  ],
  "tenant policy must precede source provider materialization",
);
requireOrder(
  server,
  [
    "for work in job_work",
    "tenant_notifications_enabled(&db, &module_registry, work.tenant_id).await",
    "worker.process_fanout_job(work.job_id).await",
  ],
  "tenant policy must precede audience provider resolution",
);
requireOrder(
  bootstrap,
  [
    "start_notification_outbox_intake_if_enabled",
    "start_notification_fanout_worker_if_ready",
    "start_notification_candidate_worker_if_ready",
  ],
  "notification pipeline workers must start in intake/fanout/candidate order",
);
reject(
  server,
  /tenant_module|module_installation|ModuleControlPlane|SELECT.+module/i,
  "fanout worker must not bypass effective policy through private module tables",
);

for (const marker of [
  "NotificationFanoutWorker",
  "NotificationFanoutSourceWorkItem",
  "NotificationFanoutJobWorkItem",
  "DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE",
]) {
  requireText(library, marker, `Notifications facade is missing ${marker}`);
}
for (const marker of [
  "bounded_worker_materializes_sources_and_pages_without_final_delivery",
  "claimable_source_inbox_work",
  "assert_eq!(first_work[0].tenant_id, tenant_id)",
  "assert_eq!(first.source_selected, 1)",
  "assert_eq!(items.len(), 4)",
  "FanoutItemStatus::Pending",
  "delivery_attempt::Entity::find",
  "MAX_NOTIFICATION_FANOUT_BATCH_SIZE + 1",
  "MAX_NOTIFICATION_FANOUT_PAGE_SIZE + 1",
]) {
  requireText(test, marker, `fanout worker SQLite evidence is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notifications fanout worker verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Notifications source fanout worker boundary verified.");
