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

const requiredGates = [
  "npm run verify:page-builder:consumer:forum",
  "npm run verify:forum:wave-evidence-freshness",
  "npm run test:verify:forum:wave-evidence-freshness",
];
const requiredProfiles = ["all_on", "publish_off", "preview_off", "builder_off"];
const requiredObservedSections = [
  "control_plane.audit_trail",
  "fallback.profiles",
  "observability.metrics",
  "observability.traces",
  "rollback.decision",
  "approvals",
  "waivers",
];

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

function getPath(root, dottedPath) {
  let current = root;
  for (const segment of dottedPath.split(".")) {
    if (current === null || typeof current !== "object" || !(segment in current)) {
      return undefined;
    }
    current = current[segment];
  }
  return current;
}

function assertMaterializedSection(root, dottedPath) {
  const value = getPath(root, dottedPath);
  if (value === undefined || value === null) {
    fail(`evidence packet missing required section ${dottedPath}`);
  }
  if (Array.isArray(value) && value.length === 0 && dottedPath !== "waivers") {
    fail(`evidence section ${dottedPath} must be a non-empty array`);
  }
  if (
    value !== null &&
    typeof value === "object" &&
    !Array.isArray(value) &&
    Object.keys(value).length === 0
  ) {
    fail(`evidence section ${dottedPath} must be a non-empty object`);
  }
  if (typeof value === "string" && value.trim().length === 0) {
    fail(`evidence section ${dottedPath} must be a non-empty string`);
  }
}

function requireExactNames(values, expected, label) {
  const names = values.map((value) => value?.name ?? value);
  const actual = new Set(names);
  for (const name of expected) {
    if (!actual.delete(name)) {
      fail(`${label} missing ${name}`);
    }
  }
  if (actual.size > 0 || names.length !== expected.length) {
    fail(`${label} contains unsupported values: ${[...actual].join(", ")}`);
  }
}

function validateSourceReady(evidence) {
  if (evidence.schema_version !== 2) {
    fail("source-ready evidence must use schema_version 2");
  }
  if (evidence.provenance !== "synthetic_fixture") {
    fail("source-ready evidence provenance must be synthetic_fixture");
  }
  if (evidence.execution_status !== "not_run_by_implementation_agent") {
    fail("source-ready evidence must not claim maintainer execution");
  }
  if (typeof evidence.prepared_on !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(evidence.prepared_on)) {
    fail("source-ready prepared_on must be an ISO date");
  }

  const profiles = evidence.static_readiness?.fallback_profiles ?? [];
  requireExactNames(profiles, requiredProfiles, "source-ready fallback profiles");
  const allowedOutcomes = new Set([
    "expected_pass",
    "expected_typed_feature_disabled",
    "expected_readonly_fallback",
  ]);
  for (const profile of profiles) {
    for (const key of ["list", "open", "preview", "save_draft", "publish_dry"]) {
      const outcome = profile.expected_smoke?.[key];
      if (!allowedOutcomes.has(outcome)) {
        fail(`source-ready profile '${profile.name}' has invalid expected_smoke.${key}`);
      }
      if ((key === "list" || key === "open") && outcome !== "expected_pass") {
        fail(`source-ready profile '${profile.name}' must keep ${key} available`);
      }
    }
    for (const key of ["admin_list_no_5xx", "admin_read_no_5xx", "storefront_read_no_5xx"]) {
      if (profile.expected_read_guarantees?.[key] !== true) {
        fail(`source-ready profile '${profile.name}' missing expected read guarantee ${key}`);
      }
    }
  }

  const observedRun = evidence.observed_run ?? {};
  if (observedRun.required !== true || observedRun.status !== "not_run") {
    fail("source-ready evidence must keep the observed run required and not_run");
  }
  if (observedRun.blocked_by !== "pages_reference_consumer_gate") {
    fail("source-ready observed run must remain blocked by pages_reference_consumer_gate");
  }
  if (observedRun.required_correlation_path !== "builder_write -> forum_publish -> storefront_read") {
    fail("source-ready observed run correlation path drifted");
  }
  requireExactNames(observedRun.required_profiles ?? [], requiredProfiles, "observed-run profiles");
  for (const section of requiredObservedSections) {
    if (!(observedRun.required_evidence ?? []).includes(section)) {
      fail(`source-ready observed run missing required evidence ${section}`);
    }
  }

  if (evidence.verification?.execution_status !== "not_run_by_implementation_agent") {
    fail("source-ready verification must not claim execution");
  }
  for (const gate of requiredGates) {
    if (!(evidence.verification?.no_compile_gates ?? []).includes(gate)) {
      fail(`source-ready verification missing no-compile gate ${gate}`);
    }
  }

  for (const liveOnlyKey of [
    "created_at",
    "control_plane",
    "fallback",
    "observability",
    "rollback",
    "approvals",
    "waivers",
    "refresh_history",
    "refresh_policy",
  ]) {
    if (Object.prototype.hasOwnProperty.call(evidence, liveOnlyKey)) {
      fail(`source-ready evidence must not materialize live-only section ${liveOnlyKey}`);
    }
  }

  const serialized = JSON.stringify(evidence);
  for (const forbiddenMarker of ["live_wave1_actual:", "trace-forum-wave1-live", '"approved"']) {
    if (serialized.includes(forbiddenMarker)) {
      fail(`source-ready evidence contains forbidden live marker ${forbiddenMarker}`);
    }
  }

  console.log("[verify-forum-wave-evidence-freshness] PASS");
  console.log(
    `module=forum; wave=1; mode=source_ready; prepared_on=${evidence.prepared_on}; observed_run=not_run`,
  );
}

function validateLive(evidence) {
  if (evidence.provenance !== "observed_control_plane") {
    fail("live evidence provenance must be observed_control_plane");
  }
  if (evidence.execution_status !== "maintainer_verified") {
    fail("live evidence execution_status must be maintainer_verified");
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
  const now = process.env.RUSTOK_VERIFY_NOW
    ? parseTimestamp(process.env.RUSTOK_VERIFY_NOW, "RUSTOK_VERIFY_NOW")
    : Date.now();
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
    ...requiredObservedSections,
    "refresh_history.latest_refresh",
  ]) {
    if (!(refreshPolicy.required_sections ?? []).includes(requiredSection)) {
      fail(`refresh_policy.required_sections missing ${requiredSection}`);
    }
    assertMaterializedSection(evidence, requiredSection);
  }

  const latestRefresh = evidence.refresh_history?.latest_refresh ?? {};
  if (parseTimestamp(latestRefresh.refreshed_at, "refresh_history.latest_refresh.refreshed_at") !== createdAt) {
    fail("refresh_history.latest_refresh.refreshed_at must match evidence created_at");
  }
  if (latestRefresh.verified_by !== refreshPolicy.owner) {
    fail("refresh_history.latest_refresh.verified_by must match refresh_policy.owner");
  }
  for (const gate of requiredGates) {
    if (!(latestRefresh.no_compile_gates ?? []).includes(gate)) {
      fail(`refresh_history.latest_refresh.no_compile_gates missing ${gate}`);
    }
  }
  for (const section of refreshPolicy.required_sections ?? []) {
    if (!(latestRefresh.sections_refreshed ?? []).includes(section)) {
      fail(`refresh_history.latest_refresh.sections_refreshed missing ${section}`);
    }
  }

  for (const metricName of [
    "preview_p95_ms",
    "publish_p95_ms",
    "sanitize_failure_rate",
    "runtime_error_rate",
  ]) {
    const value = evidence.observability?.metrics?.[metricName];
    if (typeof value !== "string" || !value.startsWith("live_wave1_actual:")) {
      fail(`live metric ${metricName} must use live_wave1_actual provenance`);
    }
  }
  for (const traceName of ["builder_write_to_forum_publish", "forum_publish_to_storefront_read"]) {
    if (typeof evidence.observability?.traces?.[traceName] !== "string") {
      fail(`live evidence missing observed trace ${traceName}`);
    }
  }
  if (evidence.rollback?.decision !== "keep") {
    fail("live evidence must record rollback decision keep");
  }
  for (const approver of ["platform_on_call", "forum_owner", "builder_owner", "runtime_owner"]) {
    if (evidence.approvals?.[approver] !== "approved") {
      fail(`live evidence missing approval from ${approver}`);
    }
  }
  if ((evidence.waivers ?? []).length !== 0) {
    fail("live evidence must not rely on waivers");
  }

  console.log("[verify-forum-wave-evidence-freshness] PASS");
  console.log(
    `module=forum; wave=1; mode=live; created_at=${evidence.created_at}; next_due_at=${refreshPolicy.next_due_at}; max_age_days=${refreshPolicy.max_age_days}`,
  );
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

if (evidence.module_slug !== "forum" || evidence.wave !== "1") {
  fail("evidence packet must describe Wave 1 for the forum module");
}

if (evidence.mode === "source_ready") {
  validateSourceReady(evidence);
} else if (evidence.mode === "live") {
  validateLive(evidence);
} else {
  fail("evidence mode must be source_ready or live");
}
