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

const contractPath =
  "crates/rustok-notifications/contracts/notifications-candidate-worker.json";
const contract = JSON.parse(read(contractPath) || "{}");
const owner = read(contract.owner_driver ?? "");
const candidate = read(contract.candidate_service ?? "");
const runtime = read(contract.policy_runtime ?? "");
const server = read(contract.server_worker ?? "");
const composition = read(contract.policy_composition ?? "");
const bootstrap = read(contract.bootstrap ?? "");
const test = read(contract.tests?.[0] ?? "");
const library = read("crates/rustok-notifications/src/lib.rs");

if (contract.slice !== "NOTIFY-03C" || contract.schema_version !== 4) {
  failures.push("candidate worker contract must identify NOTIFY-03C schema 4");
}
if (!contract.promoted_by_slices?.includes("NOTIFY-03G")) {
  failures.push("candidate worker contract must record NOTIFY-03G tenant gate promotion");
}
if (contract.enablement?.default_enabled !== false) {
  failures.push("candidate worker must remain disabled by default");
}
if (contract.enablement?.requires_candidate_worker_ready !== true
  || contract.enablement?.requires_module_registry !== true) {
  failures.push("candidate worker startup must require policy readiness and module registry");
}
if (contract.bounded_loop?.default_batch_size !== 32
  || contract.bounded_loop?.maximum_batch_size !== 64
  || contract.bounded_loop?.tenant_scoped_work_item !== true) {
  failures.push("candidate worker bounded tenant-scoped selection contract is invalid");
}
if (contract.tenant_capability_gate?.authority !== "EffectiveModulePolicyService::is_enabled"
  || contract.tenant_capability_gate?.checked_before_each_candidate_claim !== true
  || contract.tenant_capability_gate?.policy_error_fails_closed !== true
  || contract.tenant_capability_gate?.disabled_tenant_calls_recipient_policy !== false
  || contract.tenant_capability_gate?.disabled_tenant_calls_source_provider !== false
  || contract.tenant_capability_gate?.atomic_with_concurrent_tenant_disable !== false) {
  failures.push("candidate worker must enforce a bounded non-atomic tenant gate before recipient/source calls");
}
for (const field of [
  "candidate_transitions_to_retryable_error",
  "attempt_count_incremented",
  "lease_fields_cleared",
  "stable_error_code_persisted",
  "cas_rejects_concurrent_claim",
  "prevents_disabled_tenant_head_of_line_starvation",
  "prevents_post_fanout_processing_without_tenant_recheck",
]) {
  if (contract.tenant_policy_backoff?.[field] !== true) {
    failures.push(`candidate tenant backoff contract must set ${field}=true`);
  }
}
if (contract.tenant_policy_backoff?.disabled_retry_seconds !== 300
  || contract.tenant_policy_backoff?.policy_unavailable_retry_seconds !== 30) {
  failures.push("candidate tenant retry delays must remain 300/30 seconds");
}

for (const marker of [
  "DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE: usize = 32",
  "MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE: usize = 64",
  "TENANT_DISABLED_RETRY_SECONDS: i64 = 300",
  "TENANT_POLICY_UNAVAILABLE_RETRY_SECONDS: i64 = 30",
  "NotificationCandidateWorkItem",
  "NotificationCandidatePolicyDeferral",
  "claimable_candidate_work",
  "claimable_candidate_ids",
  "defer_candidate",
  "FanoutItemStatus::Pending",
  "FanoutItemStatus::RetryableError",
  "FanoutItemStatus::Processing",
  "LeaseExpiresAt.lt",
  "AttemptCount.eq(current.attempt_count)",
  "next_attempt_at: Set(Some(",
  "lease_owner: Set(None)",
  "lease_expires_at: Set(None)",
  "NOTIFICATION_TENANT_CAPABILITY_DISABLED",
  "NOTIFICATION_TENANT_POLICY_UNAVAILABLE",
  "order_by_asc(candidate_item::Column::CreatedAt)",
  "order_by_asc(candidate_item::Column::Id)",
  ".limit(self.batch_size as u64)",
  "process_candidate(item_id, self.worker_id.as_str())",
]) {
  requireText(owner, marker, `notification candidate owner driver is missing ${marker}`);
}
requireOrder(
  owner,
  [
    "let work_items = self.claimable_candidate_work().await?",
    "for work in work_items",
    "self.process_candidate(work.item_id).await",
  ],
  "trusted bounded convenience batch must select before canonical processing",
);
reject(
  owner,
  /notification::Entity|delivery_attempt::Entity/,
  "candidate owner driver must not create notifications outside canonical candidate service",
);

for (const marker of [
  "claim_candidate(item_id, worker_id)",
  "FanoutItemStatus::Processing",
  "LeaseOwner.eq(worker_id)",
  "LeaseExpiresAt.gt",
]) {
  requireText(candidate, marker, `canonical candidate lease path is missing ${marker}`);
}

for (const marker of [
  "candidate_worker_enabled: bool",
  "with_candidate_worker_enabled",
  "self.relation_ports_ready && self.candidate_worker_enabled",
  "policy_arc",
]) {
  requireText(runtime, marker, `candidate worker readiness runtime is missing ${marker}`);
}

for (const marker of [
  "RUSTOK_NOTIFICATIONS_CANDIDATE_WORKER_ENABLED",
  "with_candidate_worker_enabled(candidate_worker_enabled_from_environment())",
  "Invalid notification candidate worker enable flag; keeping worker disabled",
]) {
  requireText(composition, marker, `candidate worker enable composition is missing ${marker}`);
}

for (const marker of [
  "runs_background_workers()",
  "candidate_worker_enabled()",
  "relation_ports_ready()",
  "candidate_worker_ready()",
  "notification_source_registry_from_extensions",
  "shared_get::<ModuleRegistry>()",
  "EffectiveModulePolicyService::is_enabled",
  "TenantNotificationPolicy",
  "candidate_work_is_enabled",
  "NotificationCandidatePolicyDeferral::TenantDisabled",
  "NotificationCandidatePolicyDeferral::PolicyUnavailable",
  "defer_candidate(work, reason).await",
  "claimable_candidate_work().await",
  "worker.process_candidate(work.item_id).await",
  "NotificationError::LeaseUnavailable",
  "policy lookup failed closed",
  "StopHandle",
  "tokio::select!",
]) {
  requireText(server, marker, `server candidate worker is missing ${marker}`);
}
requireOrder(
  server,
  [
    "candidate_worker_enabled()",
    "relation_ports_ready()",
    "candidate_worker_ready()",
    "shared_get::<ModuleRegistry>()",
    "NotificationCandidateWorker::new",
    "tokio::spawn",
  ],
  "worker startup must validate enablement, policy readiness and module registry before spawn",
);
requireOrder(
  server,
  [
    "for work in work_items",
    "if *stop_rx.borrow()",
    "candidate_work_is_enabled",
    "worker.process_candidate(work.item_id).await",
  ],
  "shutdown and tenant policy must be checked before each canonical candidate claim",
);
reject(
  server,
  /notification_fanout_items|candidate_item::Entity|Column::LeaseOwner|SELECT.+notification/i,
  "server worker must not read Notifications private tables directly",
);

requireOrder(
  bootstrap,
  [
    "bootstrap_app_runtime",
    "start_notification_candidate_worker_if_ready",
    "connect_runtime_workers_with_runtime",
  ],
  "candidate worker must start only after runtime composition and before shared worker connection completes",
);

for (const marker of [
  "NotificationCandidateWorkItem",
  "NotificationCandidatePolicyDeferral",
  "NotificationCandidateWorker",
  "DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE",
]) {
  requireText(library, marker, `Notifications facade is missing ${marker}`);
}
for (const marker of [
  "worker_selection_is_bounded_and_uses_candidate_lease_path",
  "claimable_candidate_work",
  "work.tenant_id == tenant_id",
  "process_candidate(selected[0].item_id)",
  "tenant_policy_deferral_removes_candidate_from_bounded_head",
  "NotificationCandidatePolicyDeferral::TenantDisabled",
  "NOTIFICATION_TENANT_CAPABILITY_DISABLED",
  "later enabled work should reach bounded head",
  "assert_ne!(next_page[0].item_id, deferred.item_id)",
  "MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE + 1",
]) {
  requireText(test, marker, `candidate worker SQLite evidence is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notification candidate worker verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notification candidate worker tenant policy boundary verified.");
