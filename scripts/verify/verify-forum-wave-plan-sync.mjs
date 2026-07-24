#!/usr/bin/env node

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");

const planPath = "crates/rustok-forum/docs/implementation-plan.md";
const evidencePath =
  "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json";
const verifierPath = "scripts/verify/verify-forum-wave-plan-sync.mjs";
const verifierTestPath = "scripts/verify/verify-forum-wave-plan-sync.test.mjs";
const failures = [];

function read(relativePath) {
  const absolutePath = path.join(repoRoot, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: required file is missing`);
    return "";
  }
  return readFileSync(absolutePath, "utf8");
}

function requireText(source, marker, message) {
  if (!source.includes(marker)) failures.push(message);
}

function requireCompactText(source, marker, message) {
  const compactSource = source.replace(/\s+/g, " ").trim();
  const compactMarker = marker.replace(/\s+/g, " ").trim();
  if (!compactSource.includes(compactMarker)) failures.push(message);
}

function parseJson(source, relativePath) {
  try {
    return JSON.parse(source);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

const plan = read(planPath);
const evidence = parseJson(read(evidencePath), evidencePath);

const ledgerMatch = plan.match(
  /^\| `FORUM-32` \| `([^`]+)` \| ([^\n]+) \|$/m,
);
if (!ledgerMatch) {
  failures.push(`${planPath}: FORUM-32 program-ledger row is missing`);
} else {
  const [, status, result] = ledgerMatch;
  if (status !== "in_progress") {
    failures.push(`${planPath}: FORUM-32 must remain in_progress while observed rollout evidence is pending`);
  }
  const normalizedResult = result.toLowerCase();
  for (const marker of ["observed", "evidence", "remain"]) {
    if (!normalizedResult.includes(marker)) {
      failures.push(`${planPath}: FORUM-32 ledger result must keep observed evidence explicitly pending: ${marker}`);
    }
  }
}

const cardStartMarker = "## `FORUM-32` — Page Builder and widget evolution";
const cardEndMarker = "## `FORUM-33` — analytics, observability and reconciliation";
const cardStart = plan.indexOf(cardStartMarker);
const cardEnd = plan.indexOf(cardEndMarker, cardStart + cardStartMarker.length);
let card = "";
if (cardStart < 0 || cardEnd < 0 || cardEnd <= cardStart) {
  failures.push(`${planPath}: FORUM-32 task card boundary is missing`);
} else {
  card = plan.slice(cardStart, cardEnd);
}

requireText(
  card,
  "**Status:** `in_progress`",
  `${planPath}: FORUM-32 card must remain in_progress`,
);
requireCompactText(
  card,
  "Replace the synthetic Wave packet with an observed tenant control-plane run",
  `${planPath}: FORUM-32 must require replacement of the synthetic packet with an observed run`,
);
requireCompactText(
  card,
  "after the `pages` reference-consumer gate",
  `${planPath}: FORUM-32 must retain the pages reference-consumer blocker`,
);
requireCompactText(
  card,
  "Page Builder stays optional; forum routes must not depend on provider availability.",
  `${planPath}: FORUM-32 must retain the optional-provider degraded-mode guarantee`,
);
for (const command of [
  "npm run verify:page-builder:consumer:forum",
  "npm run verify:forum:wave-evidence-freshness",
]) {
  requireText(
    card,
    command,
    `${planPath}: FORUM-32 verification set is missing ${command}`,
  );
}

if (evidence.schema_version !== 2) {
  failures.push(`${evidencePath}: expected schema_version 2`);
}
if (evidence.artifact !== "page_builder_wave_evidence_packet") {
  failures.push(`${evidencePath}: Wave evidence artifact identity drifted`);
}
if (evidence.module_slug !== "forum" || evidence.wave !== "1") {
  failures.push(`${evidencePath}: expected Forum Wave 1 identity`);
}
if (evidence.mode !== "source_ready") {
  failures.push(
    `${evidencePath}: canonical plan still records a pending observed run, so evidence mode must remain source_ready`,
  );
}
if (evidence.provenance !== "synthetic_fixture") {
  failures.push(`${evidencePath}: source-ready provenance must remain synthetic_fixture`);
}
if (evidence.execution_status !== "not_run_by_implementation_agent") {
  failures.push(`${evidencePath}: source-ready evidence must not claim maintainer execution`);
}
if (evidence.observed_run?.required !== true || evidence.observed_run?.status !== "not_run") {
  failures.push(`${evidencePath}: observed tenant run must remain required and not_run`);
}
if (evidence.observed_run?.blocked_by !== "pages_reference_consumer_gate") {
  failures.push(`${evidencePath}: observed tenant run blocker must remain pages_reference_consumer_gate`);
}
if (
  evidence.observed_run?.required_correlation_path !==
  "builder_write -> forum_publish -> storefront_read"
) {
  failures.push(`${evidencePath}: observed-run correlation path drifted`);
}

if (!(evidence.static_readiness?.source_contracts ?? []).includes(verifierPath)) {
  failures.push(`${evidencePath}: source contracts must register ${verifierPath}`);
}
for (const command of [
  `node ${verifierPath}`,
  `node ${verifierTestPath}`,
]) {
  if (!(evidence.verification?.no_compile_gates ?? []).includes(command)) {
    failures.push(`${evidencePath}: no-compile verification set is missing ${command}`);
  }
}

const deferred = evidence.deferred ?? [];
if (
  !deferred.some(
    (item) =>
      typeof item === "string" &&
      item.includes("observed tenant control-plane run") &&
      item.includes("pages reference-consumer gate"),
  )
) {
  failures.push(`${evidencePath}: deferred observed-run boundary is missing`);
}

for (const forbiddenLiveKey of [
  "created_at",
  "control_plane",
  "fallback",
  "observability",
  "rollback",
  "approvals",
  "refresh_policy",
  "refresh_history",
]) {
  if (Object.prototype.hasOwnProperty.call(evidence, forbiddenLiveKey)) {
    failures.push(`${evidencePath}: source-ready packet must not materialize live-only key ${forbiddenLiveKey}`);
  }
}

if (failures.length > 0) {
  console.error("forum Wave plan/evidence synchronization verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("forum Wave plan/evidence synchronization verification passed");
