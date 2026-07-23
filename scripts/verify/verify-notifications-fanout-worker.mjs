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
const workerTest = read(contract.tests?.[0] ?? "");
const deferralTest = read(contract.tests?.[1] ?? "");

if (contract.slice !== "NOTIFY-03E" || contract.schema_version !== 3) {
  failures.push("fanout worker contract must identify NOTIFY-03E schema 3");
}
if (!contract.promoted_by_slices?.includes("NOTIFY-03F")) {
  failures.push("fanout worker contract must record NOTIFY-03F backoff promotion");
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
  failures.push("fanout worker must use authoritative effective tenant policy before every claim");
}
for (const field of [
  "source_and_job_rows_transition_to_retryable_error",
  "attempt_count_incremented",
  "lease_fields_cleared",
  "stable_error_code_persisted",
  "cas_rejects_concurrent_claim",
  "prevents_disabled_tenant_head_of_line_starvation",
]) {
  if (contract.tenant_policy_backoff?.[field] !== true) {
    failures.push(`tenant backoff contract must set ${field}=true`);
  }
}
if (contract.tenant_policy_backoff?.disabled_retry_seconds !== 300
  || contract.tenant_policy_backoff?.policy_unavailable_retry_seconds !== 30) {
  failures.push("tenant policy retry delays must remain 300/30 seconds");
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
  "TENANT_DISABLED_RETRY_SECONDS: i64 = 300",
  "TENANT_POLICY_UNAVAILABLE_RETRY_SECONDS: i64 = 30",
  "NotificationFanoutSourceWorkItem",
  "NotificationFanoutJobWorkItem",
  "NotificationFanoutPolicyDeferral",
  "claimable_source_inbox_work",
  "claimable_fanout_job_work",
  "defer_source_inbox",
  "defer_fanout_job",
  "NotificationSourceInboxStatus::RetryableError",
  "NotificationJobStatus::RetryableError",
  "AttemptCount.eq(current.attempt_count)",
  "next_attempt_at: Set(Some(",
  "lease_owner: Set(None)",
  "lease_expires_at: Set(None)",
  "NOTIFICATION_TENANT_CAPABILITY_DISABLED",
  "NOTIFICATION_TENANT_POLICY_UNAVAILABLE",
  "order_by_asc(source_inbox::Column::CreatedAt)",
  "order_by_asc(fanout_job::Column::CreatedAt)",
  "materialize_source_event",
  "process_fanout_page",
]) {
  requireText(owner, marker, `fanout owner worker is missing ${marker}`);
}
reject(
  owner,
  /notification::Entity|delivery_attempt::Entity/,
  "fanout driver must not create final notifications or delivery attempts",
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
  "TenantNotificationPolicy",
  "source_work_is_enabled",
  "job_work_is_enabled",
  "NotificationFanoutPolicyDeferral::TenantDisabled",
  "NotificationFanoutPolicyDeferral::PolicyUnavailable",
  "defer_source_inbox(work, reason).await",
  "defer_fanout_job(work, reason).await",
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
    "source_work_is_enabled",
    "materialize_source_inbox(work.inbox_id)",
  ],
  "tenant policy/backoff must precede source provider materialization",
);
requireOrder(
  server,
  [
    "for work in job_work",
    "job_work_is_enabled",
    "process_fanout_job(work.job_id)",
  ],
  "tenant policy/backoff must precede audience provider resolution",
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
  "NotificationFanoutPolicyDeferral",
  "DEFAULT_NOTIFICATION_FANOUT_BATCH_SIZE",
]) {
  requireText(library, marker, `Notifications facade is missing ${marker}`);
}
for (const marker of [
  "bounded_worker_materializes_sources_and_pages_without_final_delivery",
  "claimable_source_inbox_work",
  "assert_eq!(first_work[0].tenant_id, tenant_id)",
  "assert_eq!(items.len(), 4)",
  "FanoutItemStatus::Pending",
  "delivery_attempt::Entity::find",
  "MAX_NOTIFICATION_FANOUT_BATCH_SIZE + 1",
  "MAX_NOTIFICATION_FANOUT_PAGE_SIZE + 1",
]) {
  requireText(workerTest, marker, `fanout worker SQLite evidence is missing ${marker}`);
}
for (const marker of [
  "tenant_policy_deferral_removes_disabled_work_from_bounded_head",
  "NotificationFanoutPolicyDeferral::TenantDisabled",
  "NotificationSourceInboxStatus::RetryableError",
  "NOTIFICATION_TENANT_CAPABILITY_DISABLED",
  "later enabled work should reach bounded head",
  "assert_ne!(next_page[0].inbox_id, deferred.inbox_id)",
]) {
  requireText(deferralTest, marker, `fanout policy deferral evidence is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notifications fanout worker verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("Notifications source fanout worker and tenant backoff boundary verified.");
