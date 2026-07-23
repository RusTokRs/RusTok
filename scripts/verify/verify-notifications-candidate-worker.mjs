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

if (contract.slice !== "NOTIFY-03C") {
  failures.push("candidate worker contract must identify NOTIFY-03C");
}
if (contract.enablement?.default_enabled !== false) {
  failures.push("candidate worker must remain disabled by default");
}
if (contract.enablement?.requires_candidate_worker_ready !== true) {
  failures.push("candidate worker startup must require candidate_worker_ready");
}
if (contract.bounded_loop?.default_batch_size !== 32) {
  failures.push("candidate worker default batch must remain 32");
}
if (contract.bounded_loop?.maximum_batch_size !== 64) {
  failures.push("candidate worker hard batch maximum must remain 64");
}

for (const marker of [
  "DEFAULT_NOTIFICATION_CANDIDATE_BATCH_SIZE: usize = 32",
  "MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE: usize = 64",
  "claimable_candidate_ids",
  "FanoutItemStatus::Pending",
  "FanoutItemStatus::RetryableError",
  "FanoutItemStatus::Processing",
  "LeaseExpiresAt.lt",
  "order_by_asc(candidate_item::Column::CreatedAt)",
  "order_by_asc(candidate_item::Column::Id)",
  ".limit(self.batch_size as u64)",
  "self.service.process_candidate",
]) {
  requireText(owner, marker, `notification candidate owner driver is missing ${marker}`);
}
requireOrder(
  owner,
  [
    "claimable_candidate_ids().await?",
    "for item_id in item_ids",
    "self.process_candidate(item_id).await",
  ],
  "bounded convenience batch must select before processing through the canonical service",
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
  "StopHandle",
  "claimable_candidate_ids().await",
  "if *stop_rx.borrow()",
  "worker.process_candidate(item_id).await",
  "NotificationError::LeaseUnavailable",
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
    "NotificationCandidateWorker::new",
    "tokio::spawn",
  ],
  "worker startup must validate enablement and readiness before spawn",
);
requireOrder(
  server,
  [
    "for item_id in item_ids",
    "if *stop_rx.borrow()",
    "worker.process_candidate(item_id).await",
  ],
  "shutdown must be checked before each new candidate claim",
);
reject(
  server,
  /notification_fanout_items|candidate_item::Entity|Column::LeaseOwner/,
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
  "worker_selection_is_bounded_and_uses_candidate_lease_path",
  "assert_eq!(selected.len(), 32)",
  "worker.process_candidate(selected[0])",
  "MAX_NOTIFICATION_CANDIDATE_BATCH_SIZE + 1",
]) {
  requireText(test, marker, `candidate worker SQLite evidence is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Notification candidate worker verification failed:\n");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Notification candidate worker boundary verified.");
