#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");

const arg = process.argv[2];
if (!arg) {
  console.error("[verify-page-builder-consumer-readiness] FAIL");
  console.error("usage: node scripts/verify/verify-page-builder-consumer-readiness.mjs <module-slug>");
  process.exit(1);
}

const moduleToCrate = {
  pages: "rustok-pages",
  forum: "rustok-forum",
};

const crateName = moduleToCrate[arg];
if (!crateName) {
  console.error("[verify-page-builder-consumer-readiness] FAIL");
  console.error(`unsupported module '${arg}'. supported: ${Object.keys(moduleToCrate).join(", ")}`);
  process.exit(1);
}

const moduleTomlPath = path.join(repoRoot, "crates", crateName, "rustok-module.toml");
const implPlanPath = path.join(repoRoot, "crates", crateName, "docs", "implementation-plan.md");
const forumFallbackMatrixPath = path.join(repoRoot, "crates", "rustok-forum", "contracts", "evidence", "fw2-fallback-static-matrix.json");
const forumWave1EvidencePath = path.join(repoRoot, "crates", "rustok-forum", "contracts", "evidence", "forum-wave1-rollout-evidence.json");

function fail(message) {
  console.error("[verify-page-builder-consumer-readiness] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function hasPath(root, dottedPath) {
  let current = root;
  for (const segment of dottedPath.split(".")) {
    if (current === null || typeof current !== "object" || !(segment in current)) {
      return false;
    }
    current = current[segment];
  }
  return current !== undefined && current !== null;
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

if (!fs.existsSync(moduleTomlPath)) fail(`missing module manifest: ${moduleTomlPath}`);
if (!fs.existsSync(implPlanPath)) fail(`missing implementation plan: ${implPlanPath}`);

const moduleToml = fs.readFileSync(moduleTomlPath, "utf8");
const implPlan = fs.readFileSync(implPlanPath, "utf8");

const hasConsumerManifestMarkers =
  moduleToml.includes("page_builder") || moduleToml.includes("builder_consumer");

if (!hasConsumerManifestMarkers) {
  fail(`${arg}: no page-builder dependency/builder_consumer markers in manifest`);
}

const mustHaveManifestMarkers = ["contract_version", "builder_contract_version"];
for (const marker of mustHaveManifestMarkers) {
  if (!moduleToml.includes(marker)) {
    fail(`${arg}: manifest missing marker '${marker}'`);
  }
}

if (!implPlan.includes("Execution checkpoint")) {
  fail(`${arg}: implementation-plan missing Execution checkpoint section`);
}

if (!implPlan.match(/FBA|page-builder|builder/mi)) {
  fail(`${arg}: implementation-plan has no FBA/page-builder readiness notes`);
}

if (arg === "pages") {
  const rolloutManifestMarkers = [
    "[fba.builder_consumer.rollout_policy]",
    "audit_trail = \"control_plane_builder_wave_audit\"",
    "before_snapshot_required = true",
    "after_snapshot_required = true",
    "decision_required = true",
    "owner_signoff_required = true",
    "rollback_without_redeploy_target_minutes = 10",
    "pilot_smoke = \"preview -> properties -> publish(dry)\"",
    "runtime_error_rate_above_alert_threshold",
    "publish_latency_p95_above_slo_for_10m",
    "sanitize_failures_above_alert_threshold",
    "storefront_published_read_regression",
    "pages_owned_list_read_menu_paths_stay_available_when_builder_capabilities_are_disabled",
  ];
  for (const marker of rolloutManifestMarkers) {
    if (!moduleToml.includes(marker)) {
      fail(`${arg}: manifest rollout policy missing marker '${marker}'`);
    }
  }

  const rolloutPlanMarkers = [
    "control_plane_builder_wave_audit",
    "before/after snapshots",
    "keep/rollback",
    "owner sign-off",
    "preview -> properties -> publish(dry)",
    "publish p95",
    "<= 10 минут",
    "npm run verify:page-builder:consumer:pages",
  ];
  for (const marker of rolloutPlanMarkers) {
    if (!implPlan.includes(marker)) {
      fail(`${arg}: implementation-plan rollout policy missing marker '${marker}'`);
    }
  }
}

if (arg === "forum") {
  const forumManifestMarkers = [
    "[fba.builder_consumer.degraded_modes]",
    `builder_disabled = "forum_widgets_readonly_keep_forum_routes"`,
    `preview_disabled = "forum_widget_preview_hidden_keep_forum_routes"`,
    `publish_disabled = "forum_widget_publish_feature_disabled_keep_forum_routes"`,
    `fallback_mode = "readonly"`,
    `fallback_mode = "degraded"`,
    `fallback_mode = "hidden"`,
    "builder_off = [",
    "publish_off = [",
    "builder.enabled=false",
    "builder.publish.enabled=false",
  ];
  for (const marker of forumManifestMarkers) {
    if (!moduleToml.includes(marker)) {
      fail(`${arg}: manifest fallback hardening missing marker '${marker}'`);
    }
  }

  const forumPlanMarkers = [
    "FW-2",
    "builder_off",
    "publish_off",
    "readonly",
    "hidden",
    "degraded",
    "npm run verify:page-builder:consumer:forum",
    "без 5xx",
    "fw2-fallback-static-matrix.json",
  ];
  for (const marker of forumPlanMarkers) {
    if (!implPlan.includes(marker)) {
      fail(`${arg}: implementation-plan fallback hardening missing marker '${marker}'`);
    }
  }

  const forumRolloutManifestMarkers = [
    "[fba.builder_consumer.rollout_policy]",
    "audit_trail = \"control_plane_builder_wave_audit\"",
    "before_snapshot_required = true",
    "after_snapshot_required = true",
    "decision_required = true",
    "owner_signoff_required = true",
    "rollback_without_redeploy_target_minutes = 10",
    "pilot_smoke = \"list -> open -> preview -> save_draft -> publish_dry\"",
    "runtime_error_rate_above_alert_threshold",
    "publish_latency_p95_above_slo_for_10m",
    "sanitize_failures_above_alert_threshold",
    "storefront_published_read_regression",
    "forum_owned_list_read_topic_paths_stay_available_when_builder_capabilities_are_disabled",
  ];
  for (const marker of forumRolloutManifestMarkers) {
    if (!moduleToml.includes(marker)) {
      fail(`${arg}: manifest rollout policy missing marker '${marker}'`);
    }
  }

  const forumRolloutPlanMarkers = [
    "FW-4",
    "SLO по времени отклика",
    "<= 10 минут",
    "npm run verify:page-builder:consumer:forum",
    "list -> open -> preview -> save_draft -> publish_dry",
  ];
  for (const marker of forumRolloutPlanMarkers) {
    if (!implPlan.includes(marker)) {
      fail(`${arg}: implementation-plan rollout policy missing marker '${marker}'`);
    }
  }

  if (!fs.existsSync(forumFallbackMatrixPath)) {
    fail(`${arg}: missing FW-2 fallback static matrix: ${forumFallbackMatrixPath}`);
  }

  let forumFallbackMatrix;
  try {
    forumFallbackMatrix = JSON.parse(fs.readFileSync(forumFallbackMatrixPath, "utf8"));
  } catch (error) {
    fail(`${arg}: FW-2 fallback static matrix is not valid JSON: ${error.message}`);
  }

  const routesSource = fs.readFileSync(
    path.join(repoRoot, "crates", "rustok-forum", "src", "controllers", "mod.rs"),
    "utf8",
  );
  const moderationSource = fs.readFileSync(
    path.join(repoRoot, "crates", "rustok-forum", "src", "services", "moderation.rs"),
    "utf8",
  );
  const combinedForumSource = `${routesSource}\n${moderationSource}`;

  const requiredMatrixMarkers = [
    "rustok.forum.fw2_fallback_static_matrix.v1",
    "design_static_ready",
    "builder_off",
    "publish_off",
    "forum-read-routes-survive-builder-off",
    "forum-moderation-routes-survive-publish-off",
    "forum-service-moderation-policy-stays-domain-owned",
    "non_5xx",
    "deferred",
  ];
  const serializedMatrix = JSON.stringify(forumFallbackMatrix);
  for (const marker of requiredMatrixMarkers) {
    if (!serializedMatrix.includes(marker)) {
      fail(`${arg}: FW-2 fallback static matrix missing marker '${marker}'`);
    }
  }

  for (const assertion of forumFallbackMatrix.assertions ?? []) {
    if (assertion.expected_http_class !== "non_5xx") {
      fail(`${arg}: FW-2 assertion '${assertion.id}' must target non_5xx status class`);
    }
    for (const marker of assertion.source_markers ?? []) {
      if (!combinedForumSource.includes(marker)) {
        fail(`${arg}: FW-2 assertion '${assertion.id}' source marker not found: ${marker}`);
      }
    }
  }

  if (!fs.existsSync(forumWave1EvidencePath)) {
    fail(`${arg}: missing Wave 1 rollout evidence packet: ${forumWave1EvidencePath}`);
  }

  let forumWave1Evidence;
  try {
    forumWave1Evidence = JSON.parse(fs.readFileSync(forumWave1EvidencePath, "utf8"));
  } catch (error) {
    fail(`${arg}: Wave 1 rollout evidence packet is not valid JSON: ${error.message}`);
  }

  const requiredWave1Markers = [
    "page_builder_wave_evidence_packet",
    "forum",
    "live",
    "control_plane_builder_wave_audit",
    "live:forum-wave1:2026-06-15",
    "all_on",
    "publish_off",
    "preview_off",
    "builder_off",
    "typed_feature_disabled_error_without_read_5xx",
    "builder_write -> forum_publish -> storefront_read",
  ];
  const serializedWave1 = JSON.stringify(forumWave1Evidence);
  for (const marker of requiredWave1Markers) {
    if (!serializedWave1.includes(marker)) {
      fail(`${arg}: Wave 1 rollout evidence missing marker '${marker}'`);
    }
  }

  if (forumWave1Evidence.wave !== "1" || forumWave1Evidence.mode !== "live") {
    fail(`${arg}: Wave 1 evidence must be live wave 1`);
  }

  const wave1Profiles = forumWave1Evidence.fallback?.profiles ?? [];
  const requiredProfiles = new Set(["all_on", "publish_off", "preview_off", "builder_off"]);
  for (const profile of wave1Profiles) {
    requiredProfiles.delete(profile.name);
    if (profile.read_guarantees?.admin_list_no_5xx !== true) {
      fail(`${arg}: Wave 1 profile '${profile.name}' missing admin list no-5xx guarantee`);
    }
    if (profile.read_guarantees?.admin_read_no_5xx !== true) {
      fail(`${arg}: Wave 1 profile '${profile.name}' missing admin read no-5xx guarantee`);
    }
    if (profile.read_guarantees?.storefront_read_no_5xx !== true) {
      fail(`${arg}: Wave 1 profile '${profile.name}' missing storefront read no-5xx guarantee`);
    }
    if (profile.decision !== "keep") {
      fail(`${arg}: Wave 1 profile '${profile.name}' must record keep decision`);
    }
  }
  if (requiredProfiles.size > 0) {
    fail(`${arg}: Wave 1 evidence missing fallback profiles: ${[...requiredProfiles].join(", ")}`);
  }


  const expectedSmokeKeys = ["list", "open", "preview", "save_draft", "publish_dry"];
  for (const profile of wave1Profiles) {
    for (const key of expectedSmokeKeys) {
      const value = profile.smoke?.[key];
      if (!value) {
        fail(`${arg}: Wave 1 profile '${profile.name}' missing smoke key '${key}'`);
      }
      if (key === "list" || key === "open") {
        if (value !== "pass") {
          fail(`${arg}: Wave 1 profile '${profile.name}' smoke '${key}' must pass`);
        }
      } else if (!["pass", "typed_feature_disabled_error", "readonly_fallback"].includes(value)) {
        fail(`${arg}: Wave 1 profile '${profile.name}' smoke '${key}' has unsupported outcome '${value}'`);
      }
    }
  }

  const metricActual = (name) => {
    const raw = forumWave1Evidence.observability?.metrics?.[name];
    if (typeof raw !== "string" || !raw.startsWith("live_wave1_actual:")) {
      fail(`${arg}: Wave 1 metric '${name}' must be a live_wave1_actual value`);
    }
    const parsed = Number(raw.slice("live_wave1_actual:".length));
    if (!Number.isFinite(parsed)) {
      fail(`${arg}: Wave 1 metric '${name}' is not numeric: ${raw}`);
    }
    return parsed;
  };
  const thresholds = forumWave1Evidence.observability?.slo_thresholds ?? {};
  for (const thresholdName of [
    "preview_p95_ms",
    "publish_p95_ms",
    "sanitize_failure_rate_max",
    "runtime_error_rate_max",
  ]) {
    if (!Number.isFinite(thresholds[thresholdName])) {
      fail(`${arg}: Wave 1 SLO threshold '${thresholdName}' is missing or not numeric`);
    }
  }
  if (metricActual("preview_p95_ms") > thresholds.preview_p95_ms) {
    fail(`${arg}: Wave 1 preview_p95_ms exceeds threshold`);
  }
  if (metricActual("publish_p95_ms") > thresholds.publish_p95_ms) {
    fail(`${arg}: Wave 1 publish_p95_ms exceeds threshold`);
  }
  if (metricActual("sanitize_failure_rate") > thresholds.sanitize_failure_rate_max) {
    fail(`${arg}: Wave 1 sanitize_failure_rate exceeds threshold`);
  }
  if (metricActual("runtime_error_rate") > thresholds.runtime_error_rate_max) {
    fail(`${arg}: Wave 1 runtime_error_rate exceeds threshold`);
  }

  const traceKeys = Object.keys(forumWave1Evidence.observability?.traces ?? {});
  for (const traceKey of ["builder_write_to_forum_publish", "forum_publish_to_storefront_read"]) {
    if (!traceKeys.includes(traceKey)) {
      fail(`${arg}: Wave 1 observability traces missing '${traceKey}'`);
    }
  }
  for (const traceKey of traceKeys) {
    if (traceKey.includes("pages")) {
      fail(`${arg}: Wave 1 observability trace key '${traceKey}' must be forum-owned, not pages-owned`);
    }
  }
  for (const sample of forumWave1Evidence.observability?.trace_samples ?? []) {
    if (!sample.correlation_path?.includes("forum_publish")) {
      fail(`${arg}: Wave 1 trace sample '${sample.trace_id}' missing forum_publish correlation path`);
    }
    if (!["pass", "typed_feature_disabled_error_without_read_5xx"].includes(sample.result)) {
      fail(`${arg}: Wave 1 trace sample '${sample.trace_id}' has unsupported result '${sample.result}'`);
    }
  }

  const refreshPolicy = forumWave1Evidence.refresh_policy ?? {};
  if (refreshPolicy.cadence !== "monthly") {
    fail(`${arg}: Wave 1 refresh policy must require monthly evidence refresh`);
  }
  if (refreshPolicy.required_gate !== "npm run verify:page-builder:consumer:forum") {
    fail(`${arg}: Wave 1 refresh policy must pin the forum consumer gate`);
  }
  if (refreshPolicy.refresh_evidence_required !== true) {
    fail(`${arg}: Wave 1 refresh policy must require refreshed evidence`);
  }
  if (refreshPolicy.stale_evidence_action !== "block_builder_consumer_rollout_until_refreshed") {
    fail(`${arg}: Wave 1 refresh policy must block rollout when evidence is stale`);
  }
  if (!Number.isFinite(refreshPolicy.max_age_days) || refreshPolicy.max_age_days > 45) {
    fail(`${arg}: Wave 1 refresh policy max_age_days must be numeric and <= 45`);
  }
  const parseWaveTimestamp = (value, label) => {
    if (typeof value !== "string" || value.length === 0) {
      fail(`${arg}: Wave 1 ${label} must be a non-empty ISO timestamp`);
    }
    const parsed = Date.parse(value);
    if (!Number.isFinite(parsed)) {
      fail(`${arg}: Wave 1 ${label} is not a valid ISO timestamp: ${value}`);
    }
    return parsed;
  };
  const waveCreatedAt = parseWaveTimestamp(forumWave1Evidence.created_at, "created_at");
  const waveNextDueAt = parseWaveTimestamp(refreshPolicy.next_due_at, "refresh_policy.next_due_at");
  const maxAgeMs = refreshPolicy.max_age_days * 24 * 60 * 60 * 1000;
  if (waveNextDueAt <= waveCreatedAt) {
    fail(`${arg}: Wave 1 refresh_policy.next_due_at must be after created_at`);
  }
  if (waveNextDueAt - waveCreatedAt > maxAgeMs) {
    fail(`${arg}: Wave 1 refresh_policy.next_due_at must not exceed max_age_days from created_at`);
  }
  const now = process.env.RUSTOK_VERIFY_NOW
    ? parseWaveTimestamp(process.env.RUSTOK_VERIFY_NOW, "RUSTOK_VERIFY_NOW")
    : Date.now();
  if (now - waveCreatedAt > maxAgeMs) {
    fail(`${arg}: Wave 1 evidence is older than refresh_policy.max_age_days`);
  }
  if (now > waveNextDueAt) {
    fail(`${arg}: Wave 1 evidence is past refresh_policy.next_due_at and must be refreshed before rollout`);
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
      fail(`${arg}: Wave 1 refresh policy missing required section '${requiredSection}'`);
    }
    if (!hasPath(forumWave1Evidence, requiredSection)) {
      fail(`${arg}: Wave 1 evidence missing required refresh section '${requiredSection}'`);
    }
    const sectionValue = getPath(forumWave1Evidence, requiredSection);
    if (Array.isArray(sectionValue) && sectionValue.length === 0 && requiredSection !== "waivers") {
      fail(`${arg}: Wave 1 evidence refresh section '${requiredSection}' must be a non-empty array`);
    }
    if (
      sectionValue !== null &&
      typeof sectionValue === "object" &&
      !Array.isArray(sectionValue) &&
      Object.keys(sectionValue).length === 0
    ) {
      fail(`${arg}: Wave 1 evidence refresh section '${requiredSection}' must be a non-empty object`);
    }
    if (typeof sectionValue === "string" && sectionValue.trim().length === 0) {
      fail(`${arg}: Wave 1 evidence refresh section '${requiredSection}' must be a non-empty string`);
    }
  }

  if (forumWave1Evidence.observability?.slo_evaluation?.overall !== "pass") {
    fail(`${arg}: Wave 1 evidence must record passing overall SLO evaluation`);
  }
  if (forumWave1Evidence.rollback?.decision !== "keep") {
    fail(`${arg}: Wave 1 evidence must record rollback decision keep`);
  }
  for (const approver of ["platform_on_call", "forum_owner", "builder_owner", "runtime_owner"]) {
    if (forumWave1Evidence.approvals?.[approver] !== "approved") {
      fail(`${arg}: Wave 1 evidence missing approval from ${approver}`);
    }
  }
  if ((forumWave1Evidence.waivers ?? []).length !== 0) {
    fail(`${arg}: Wave 1 evidence must not rely on waivers`);
  }

}

console.log("[verify-page-builder-consumer-readiness] PASS");
console.log(`module=${arg}; crate=${crateName}; consumer_manifest_markers=${hasConsumerManifestMarkers}`);
