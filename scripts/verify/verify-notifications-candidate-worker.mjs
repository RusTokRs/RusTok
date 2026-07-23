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
const moduleConsumer = read(contract.module_policy_consumer ?? "");
const moduleTransition = read(contract.module_policy_transition ?? "");
const moduleExecutor = read("crates/rustok-modules/src/executor.rs");
const candidateTest = read(contract.tests?.[0] ?? "");
const moduleTest = read(contract.tests?.[1] ?? "");
const library = read("crates/rustok-notifications/src/lib.rs");

if (contract.slice !== "NOTIFY-03C" || contract.schema_version !== 5) {
  failures.push("candidate worker contract must identify NOTIFY-03C schema 5");
}
if (!contract.promoted_by_slices?.includes("NOTIFY-03H")) {
  failures.push("candidate worker contract must record NOTIFY-03H commit guard promotion");
}
if (contract.enablement?.default_enabled !== false
  || contract.enablement?.production_uses_guarded_constructor !== true) {
  failures.push("candidate worker must remain default-off and use a guarded production constructor");
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
if (contract.tenant_capability_gate?.preclaim_authority
    !== "EffectiveModulePolicyService::resolve"
  || contract.tenant_capability_gate?.checked_before_each_candidate_claim !== true
  || contract.tenant_capability_gate?.observed_policy_revision_forwarded_to_commit !== true
  || contract.tenant_capability_gate?.policy_error_fails_closed !== true
  || contract.tenant_capability_gate?.disabled_tenant_calls_recipient_policy !== false
  || contract.tenant_capability_gate?.disabled_tenant_calls_source_provider !== false) {
  failures.push("candidate worker preclaim gate must resolve and forward the effective policy revision");
}
for (const field of [
  "inside_final_notification_transaction",
  "after_candidate_lease_validation",
  "before_preference_recheck",
  "before_notification_insert",
  "module_owner_resolves_tenant_overrides",
  "revision_match_required",
  "enabled_module_required",
  "revision_change_retryable",
  "disabled_commit_retryable",
  "postgres_serializes_with_lifecycle_tenant_toggle",
]) {
  if (contract.commit_time_guard?.[field] !== true) {
    failures.push(`candidate commit guard contract must set ${field}=true`);
  }
}
if (contract.commit_time_guard?.lifecycle_cursor_consumer_key !== "module.lifecycle"
  || contract.commit_time_guard?.server_reads_tenant_modules_directly !== false
  || contract.commit_time_guard?.rejected_commit_creates_notification !== false
  || contract.commit_time_guard?.sqlite_behavioral_evidence_only !== true
  || contract.commit_time_guard?.atomic_with_manifest_or_security_mutation !== false) {
  failures.push("candidate commit guard scope or degraded evidence contract is invalid");
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
  "NotificationCandidateWorkItem",
  "NotificationCandidatePolicyDeferral",
  "NotificationTenantCapabilityCommitGuard",
  "new_with_commit_guard",
  "claimable_candidate_work",
  "claimable_candidate_ids",
  "defer_candidate",
  "process_candidate_with_policy_revision",
  "FanoutItemStatus::Pending",
  "FanoutItemStatus::RetryableError",
  "FanoutItemStatus::Processing",
  "AttemptCount.eq(current.attempt_count)",
  "order_by_asc(candidate_item::Column::CreatedAt)",
  "order_by_asc(candidate_item::Column::Id)",
  ".limit(self.batch_size as u64)",
]) {
  requireText(owner, marker, `notification candidate owner driver is missing ${marker}`);
}
reject(
  owner,
  /notification::Entity|delivery_attempt::Entity/,
  "candidate owner driver must not create notifications outside canonical candidate service",
);

for (const marker of [
  "pub trait NotificationTenantCapabilityCommitGuard",
  "NotificationTenantCapabilityCommitRequest",
  "NotificationTenantCapabilityCommitDecision",
  "new_with_commit_guard",
  "process_candidate_with_policy_revision",
  "commit_guard.evaluate",
  "TenantCapabilityDisabled",
  "TenantPolicyRevisionChanged",
  "claim_candidate(item_id, worker_id)",
  "LeaseOwner.eq(worker_id)",
  "LeaseExpiresAt.gt",
]) {
  requireText(candidate, marker, `canonical candidate commit path is missing ${marker}`);
}
requireOrder(
  candidate,
  [
    "ensure_candidate_lease(&current, worker_id)?",
    "commit_guard.evaluate(&txn, request).await",
    "self.preference_allows_in_app(&txn, &current, job).await?",
    "notification::Entity::insert(active)",
  ],
  "commit guard must run after lease validation and before preference recheck/notification insert",
);

for (const marker of [
  "lock_current_revision_in_transaction",
  "lock_and_resolve_static_policy_in_transaction",
  "ModuleEffectivePolicyQuery::new_with_context",
  "load_tenant_overrides(transaction, tenant_id)",
  "SELECT module_slug, enabled FROM tenant_modules",
  '" FOR UPDATE"',
]) {
  requireText(moduleConsumer, marker, `module policy owner guard is missing ${marker}`);
}
for (const marker of [
  "publish_and_advance",
  "apply_in_transaction(transaction, tenant_id, consumer_key, transition)",
]) {
  requireText(moduleTransition, marker, `module lifecycle transition path is missing ${marker}`);
}
requireOrder(
  moduleExecutor,
  [
    "TenantModuleStateStore::persist(transaction, state_request)",
    "publish_and_advance(",
    '"module.lifecycle"',
  ],
  "lifecycle state and policy cursor transition must share one transaction",
);

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
  "shared_get::<ModuleRegistry>()",
  "EffectiveModulePolicyService::resolve",
  "policy.policy_revision().to_string()",
  "ServerNotificationTenantCapabilityCommitGuard",
  "PlatformCompositionService::active_manifest",
  "SeaOrmModulePolicyRevisionConsumer::new",
  "lock_and_resolve_static_policy_in_transaction",
  "MODULE_LIFECYCLE_POLICY_CONSUMER",
  "NotificationCandidateWorker::new_with_commit_guard",
  "candidate_work_policy_revision",
  "process_candidate_with_policy_revision",
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
    "NotificationCandidateWorker::new_with_commit_guard",
    "tokio::spawn",
  ],
  "worker startup must validate readiness and compose commit guard before spawn",
);
requireOrder(
  server,
  [
    "for work in work_items",
    "if *stop_rx.borrow()",
    "candidate_work_policy_revision",
    "process_candidate_with_policy_revision",
  ],
  "shutdown and revisioned tenant policy must precede each canonical candidate claim",
);
reject(
  server,
  /tenant_modules|notification_fanout_items|candidate_item::Entity|Column::LeaseOwner|SELECT\s/i,
  "server worker must not read module or Notifications private tables directly",
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
  "NotificationTenantCapabilityCommitGuard",
  "NotificationTenantCapabilityCommitRequest",
  "NotificationTenantCapabilityCommitDecision",
  "NotificationCandidateWorker",
]) {
  requireText(library, marker, `Notifications facade is missing ${marker}`);
}
for (const marker of [
  "worker_selection_is_bounded_and_uses_candidate_lease_path",
  "tenant_policy_deferral_removes_candidate_from_bounded_head",
  "commit_policy_revision_change_rolls_back_notification_and_retries_candidate",
  "RevisionChangedCommitGuard",
  "new_with_commit_guard",
  "process_candidate_with_policy_revision",
  "NOTIFICATION_TENANT_POLICY_REVISION_CHANGED",
  "revision rejection must not create notifications",
  "MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE + 1",
]) {
  requireText(candidateTest, marker, `candidate worker SQLite evidence is missing ${marker}`);
}
for (const marker of [
  "static_policy_resolves_tenant_override_under_lifecycle_cursor_lock",
  "lock_and_resolve_static_policy_in_transaction",
  "module.lifecycle",
  "assert!(!disabled.contains(\"notifications\"))",
  "guard locks but never advances",
]) {
  requireText(moduleTest, marker, `module policy cursor SQLite evidence is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notification candidate worker verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notification candidate worker commit-time tenant policy boundary verified.");
