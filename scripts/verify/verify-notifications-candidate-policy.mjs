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
  "crates/rustok-notifications/contracts/notifications-candidate-policy.json";
const contract = JSON.parse(read(contractPath) || "{}");
const migration = read(contract.migration ?? "");
const service = read(contract.service ?? "");
const library = read("crates/rustok-notifications/src/lib.rs");
const model = read("crates/rustok-notifications/src/model.rs");
const migrations = read("crates/rustok-notifications/src/migrations/mod.rs");
const test = read("crates/rustok-notifications/tests/candidate_sqlite.rs");
const workerTest = read("crates/rustok-notifications/tests/candidate_worker_sqlite.rs");

if (contract.slice !== "NOTIFY-03B/07A" || contract.schema_version !== 7) {
  failures.push("machine contract must identify NOTIFY-03B/07A schema 7");
}
if (!contract.promoted_by_slices?.includes("NOTIFY-03H")) {
  failures.push("candidate policy contract must record NOTIFY-03H promotion");
}
if (contract.recipient_privacy_policy?.injected_trait_required !== true
  || contract.recipient_privacy_policy?.allow_all_default_forbidden !== true) {
  failures.push("candidate privacy policy must be an injected fail-closed owner contract");
}
if (contract.preference_policy?.rechecked_inside_final_transaction !== true) {
  failures.push("preference must be rechecked in the final notification transaction");
}
if (contract.source_authorization?.authorize_target_open_before_creation !== true) {
  failures.push("source authorization must run before notification creation");
}
if (contract.final_notification?.creates_delivery_attempts !== false
  || contract.final_notification?.candidate_completion_same_transaction !== true
  || contract.final_notification?.tenant_policy_guard_same_transaction !== true) {
  failures.push("final notification transaction must include tenant guard and candidate completion without channel delivery");
}
if (contract.upstream_runtime?.candidate_commit_policy_revision_guard_delivered !== true
  || contract.upstream_runtime?.candidate_guard_serializes_postgres_lifecycle_tenant_disable !== true
  || contract.upstream_runtime?.candidate_guard_atomic_with_manifest_or_security_mutation !== false) {
  failures.push("candidate commit policy guard scope is invalid");
}

for (const marker of [
  "m20260722_000012_add_candidate_processing",
  "m20260722_000011_create_notification_source_inbox",
]) {
  requireText(migrations, marker, `migration registry is missing ${marker}`);
}
for (const marker of [
  "ADD COLUMN IF NOT EXISTS attempt_count",
  "ADD COLUMN IF NOT EXISTS next_attempt_at",
  "ADD COLUMN IF NOT EXISTS lease_owner",
  "ADD COLUMN IF NOT EXISTS lease_expires_at",
  "status IN ('pending', 'processing', 'processed', 'skipped', 'retryable_error', 'failed')",
  "notification_fanout_item_tenant_guard_insert",
  "notification_fanout_item_tenant_guard_update",
  "idx_notification_fanout_item_recovery",
]) {
  requireText(migration, marker, `candidate migration is missing ${marker}`);
}
requireOrder(
  migration,
  [
    "ALTER TABLE notification_fanout_items RENAME TO",
    "CREATE TABLE notification_fanout_items",
    "INSERT INTO notification_fanout_items",
    "DROP TABLE notification_fanout_items_before_candidate_processing",
    "CREATE TRIGGER notification_fanout_item_tenant_guard_insert",
  ],
  "SQLite candidate rebuild must preserve rows before restoring tenant guards",
);
for (const marker of ["Processing", "RetryableError", "FanoutItemStatus"]) {
  requireText(model, marker, `candidate state model is missing ${marker}`);
}

for (const marker of [
  "pub trait NotificationRecipientPolicy",
  "pub trait NotificationTenantCapabilityCommitGuard",
  "pub struct NotificationCandidateService",
  "new_with_commit_guard",
  "process_candidate_with_policy_revision",
  "NotificationTenantCapabilityCommitDecision::Disabled",
  "NotificationTenantCapabilityCommitDecision::RevisionChanged",
  "preference_specificity",
  "NotificationRecipientPolicyDecision::Suppress",
  "authorize_target_open",
  "preference_allows_in_app(&txn",
  "notification::Entity::insert",
  "OnConflict::columns",
  "ensure_notification_identity",
  "LeaseExpiresAt.gt(completion_time)",
  "FanoutItemStatus::Processed",
  "FanoutItemStatus::Skipped",
  "FanoutItemStatus::RetryableError",
]) {
  requireText(service, marker, `candidate owner service is missing ${marker}`);
}
requireOrder(
  service,
  [
    "preference_allows_in_app(&self.db",
    "self.policy.evaluate",
    "authorize_target_open",
    "persist_final_notification",
  ],
  "candidate checks must remain preference, privacy, source authorization, then persistence",
);
requireOrder(
  service,
  [
    "let txn = self.db.begin().await?",
    "ensure_candidate_lease(&current, worker_id)?",
    "commit_guard.evaluate(&txn, request).await",
    "preference_allows_in_app(&txn",
    "notification::Entity::insert",
    "FanoutItemStatus::Processed",
    "txn.commit().await?",
  ],
  "final transaction must guard tenant policy before preference, insert, and completion",
);
reject(
  service,
  /struct\s+(?:AllowAll|Permissive|DefaultAllow).*NotificationRecipientPolicy/s,
  "candidate owner must not provide an allow-all privacy implementation",
);
reject(
  service,
  /delivery_attempt::ActiveModel|notification_delivery_attempts|DeliveryStatus::Pending/,
  "candidate finalization must not enqueue delivery attempts",
);
reject(
  service,
  /rustok_profiles::|profile::Entity|block::Entity|user_blocks|tenant_modules/,
  "notifications owner must use injected ports instead of reading foreign private tables",
);

for (const marker of [
  "NotificationCandidateService",
  "NotificationRecipientPolicy",
  "NotificationTenantCapabilityCommitGuard",
  "NotificationCandidateProcessResult",
  "module.migrations().len(), 5",
]) {
  requireText(library, marker, `notifications facade is missing ${marker}`);
}
for (const marker of [
  "candidates_require_preference_privacy_and_source_authorization",
  "NotificationDeliveryMode::Off",
  "NotificationRecipientSuppression::Blocked",
  "NOTIFICATION_RECIPIENT_POLICY_FAILURE",
  "FanoutItemStatus::RetryableError",
  "delivery_count, 0",
  "notification_rows.len(), 1",
]) {
  requireText(test, marker, `candidate SQLite scenario is missing ${marker}`);
}
for (const marker of [
  "commit_policy_revision_change_rolls_back_notification_and_retries_candidate",
  "NOTIFICATION_TENANT_POLICY_REVISION_CHANGED",
  "revision rejection must not create notifications",
]) {
  requireText(workerTest, marker, `candidate commit guard scenario is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notifications candidate policy verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notifications candidate policy and commit guard boundary verified.");
