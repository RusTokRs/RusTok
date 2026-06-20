#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(__dirname, "..", "..");
const evidencePath = process.env.RUSTOK_FORUM_WAVE_EVIDENCE_PATH
  ? path.resolve(process.env.RUSTOK_FORUM_WAVE_EVIDENCE_PATH)
  : path.join(
      repoRoot,
      "crates",
      "rustok-forum",
      "contracts",
      "evidence",
      "forum-wave1-rollout-evidence.json",
    );

function fail(message) {
  console.error("[verify-forum-wave-evidence-freshness] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function parseTimestamp(value, label) {
  if (typeof value !== "string" || value.length === 0) {
    fail(`${label} must be a non-empty ISO timestamp`);
  }
  const parsed = Date.parse(value);
  if (!Number.isFinite(parsed)) {
    fail(`${label} is not a valid ISO timestamp: ${value}`);
  }
  return parsed;
}

if (!fs.existsSync(evidencePath)) {
  fail(`missing Forum Wave 1 evidence packet: ${evidencePath}`);
}

let evidence;
try {
  evidence = JSON.parse(fs.readFileSync(evidencePath, "utf8"));
} catch (error) {
  fail(`Forum Wave 1 evidence packet is not valid JSON: ${error.message}`);
}

if (evidence.module_slug !== "forum" || evidence.wave !== "1" || evidence.mode !== "live") {
  fail("evidence packet must describe live Wave 1 for the forum module");
}

const refreshPolicy = evidence.refresh_policy ?? {};
if (refreshPolicy.cadence !== "monthly") {
  fail("refresh_policy.cadence must stay monthly");
}
if (refreshPolicy.stale_evidence_action !== "block_builder_consumer_rollout_until_refreshed") {
  fail("refresh_policy.stale_evidence_action must block rollout until evidence is refreshed");
}
if (refreshPolicy.required_gate !== "npm run verify:page-builder:consumer:forum") {
  fail("refresh_policy.required_gate must pin npm run verify:page-builder:consumer:forum");
}
if (!Number.isFinite(refreshPolicy.max_age_days) || refreshPolicy.max_age_days > 45) {
  fail("refresh_policy.max_age_days must be numeric and <= 45");
}

const createdAt = parseTimestamp(evidence.created_at, "created_at");
const nextDueAt = parseTimestamp(refreshPolicy.next_due_at, "refresh_policy.next_due_at");
const now = process.env.RUSTOK_VERIFY_NOW ? parseTimestamp(process.env.RUSTOK_VERIFY_NOW, "RUSTOK_VERIFY_NOW") : Date.now();
const maxAgeMs = refreshPolicy.max_age_days * 24 * 60 * 60 * 1000;

if (nextDueAt <= createdAt) {
  fail("refresh_policy.next_due_at must be after created_at");
}
if (nextDueAt - createdAt > maxAgeMs) {
  fail("refresh_policy.next_due_at must not exceed max_age_days from created_at");
}
if (now - createdAt > maxAgeMs) {
  fail("Forum Wave 1 evidence is older than refresh_policy.max_age_days");
}
if (now > nextDueAt) {
  fail("Forum Wave 1 evidence is past refresh_policy.next_due_at and must be refreshed before rollout");
}

for (const requiredSection of [
  "control_plane.audit_trail",
  "fallback.profiles",
  "observability.metrics",
  "observability.traces",
  "rollback.decision",
  "approvals",
  "waivers",
]) {
  if (!(refreshPolicy.required_sections ?? []).includes(requiredSection)) {
    fail(`refresh_policy.required_sections missing ${requiredSection}`);
  }
}

console.log("[verify-forum-wave-evidence-freshness] PASS");
console.log(
  `module=forum; wave=1; created_at=${evidence.created_at}; next_due_at=${refreshPolicy.next_due_at}; max_age_days=${refreshPolicy.max_age_days}`,
);
