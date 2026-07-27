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

const contract = JSON.parse(
  read("crates/rustok-forum/contracts/forum-notification-plan-sync.json") || "{}",
);
const canonical = read(contract.canonical_plan ?? "");
const local = read(contract.notifications_local_plan ?? "");
const owner = read(contract.notifications_owner_readme ?? "");
const live = read(contract.notifications_live_contract ?? "");
const upstream = JSON.parse(read(contract.upstream_contract ?? "") || "{}");
const note = read(contract.owner_note ?? "");

if (
  contract.schema_version !== 1 ||
  contract.task !== "FORUM-20AM" ||
  contract.upstream_task !== "FORUM-20AL" ||
  contract.required_ledger_through !== "FORUM-20AL"
) {
  failures.push("plan sync contract must connect FORUM-20AL/20AM through AL");
}
if (contract.verification?.execution_status !== "not_run_by_implementation_agent") {
  failures.push("plan sync contract must not claim unexecuted evidence");
}
for (const key of [
  "canonical_plan",
  "notifications_local_plan",
  "notifications_owner_readme",
  "notifications_live_contract",
  "upstream_handoff_updated",
]) {
  if (contract.synchronization?.[key] !== true) {
    failures.push(`plan sync contract must record ${key}`);
  }
}
for (const key of ["runtime_behavior_changed", "migration_changed", "dependency_changed"]) {
  if (contract.synchronization?.[key] !== false) {
    failures.push(`plan sync contract must keep ${key} false`);
  }
}

for (const marker of [
  "FORUM-20A-AO provide",
  "### Delivered in `FORUM-20H` through `FORUM-20Q`",
  "### Delivered in `FORUM-20R` through `FORUM-20AF`",
  "### Delivered in `FORUM-20AG` through `FORUM-20AL`",
  "### Delivered in `FORUM-20AM`",
  "### Delivered in `FORUM-20AN`",
  "### Delivered in `FORUM-20AO`",
  "PostgreSQL concurrency",
]) {
  requireText(canonical, marker, `canonical plan is missing ${marker}`);
}
rejectText(
  canonical,
  "current category/topic/reply reads remain unchanged until a later owner-read",
  "canonical plan must not retain the pre-composition FORUM-20G residual",
);

for (const marker of [
  "### `FORUM-20AB`",
  "### `FORUM-20AC / FORUM-20AD`",
  "### `FORUM-20AE / FORUM-20AF`",
  "### `FORUM-20AG / FORUM-20AH`",
  "### `FORUM-20AI / FORUM-20AJ`",
  "### `FORUM-20AK / FORUM-20AL`",
  "### `FORUM-20AM`",
  "### `FORUM-20AN`",
  "### `FORUM-20AO`",
]) {
  requireText(local, marker, `Notifications local plan is missing ${marker}`);
}
rejectText(
  local,
  "No external transport, selected-ID bulk",
  "Notifications local plan must not retain the pre-storefront residual",
);

for (const marker of [
  "NotificationInboxStorefrontPort",
  "### 14. Authenticated storefront transport and grouped UI",
  "GraphQL now",
  "automatically reloads its bootstrap",
]) {
  requireText(owner, marker, `Notifications owner README is missing ${marker}`);
}
for (const marker of [
  "### Authenticated storefront ports, transports, and UI",
  "GraphQL group-state mutations now delegate",
  "automatically reloads its bootstrap",
]) {
  requireText(live, marker, `Notifications live contract is missing ${marker}`);
}

const synchronizationContract =
  "crates/rustok-forum/contracts/forum-notification-plan-sync.json";
for (const key of ["canonical_plan_sync", "notifications_local_plan_sync"]) {
  if (
    upstream[key]?.status !== "synchronized_by_FORUM-20AM" ||
    upstream[key]?.required_ledger_through !== "FORUM-20AL" ||
    upstream[key]?.current_plan_through !== "FORUM-20AL" ||
    upstream[key]?.sync_contract !== synchronizationContract
  ) {
    failures.push(`FORUM-20AL handoff must synchronize ${key} through FORUM-20AL`);
  }
}
if (
  upstream.notifications_owner_docs_sync?.status !== "synchronized_by_FORUM-20AM" ||
  upstream.notifications_owner_docs_sync?.required_contract_through !== "FORUM-20AL" ||
  upstream.notifications_owner_docs_sync?.current_contract_through !== "FORUM-20AL" ||
  upstream.notifications_owner_docs_sync?.sync_contract !== synchronizationContract
) {
  failures.push(
    "FORUM-20AL handoff must synchronize Notifications owner documents through FORUM-20AL",
  );
}

for (const marker of [
  "# FORUM-20AM Forum and Notifications plan synchronization",
  "source-ready / unvalidated",
  "does not claim maintainer-run tests or runtime evidence",
]) {
  requireText(note, marker, `FORUM-20AM owner note is missing ${marker}`);
}

if (failures.length > 0) {
  console.error("Forum notification plan synchronization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Historical FORUM-20AM synchronization remains valid through downstream FORUM-20AO.");
