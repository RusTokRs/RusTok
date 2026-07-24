#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(__dirname, "..", "..", "..", "..");

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
const forumFallbackMatrixPath = path.join(
  repoRoot,
  "crates",
  "rustok-forum",
  "contracts",
  "evidence",
  "fw2-fallback-static-matrix.json",
);
const forumWave1EvidencePath = path.join(
  repoRoot,
  "crates",
  "rustok-forum",
  "contracts",
  "evidence",
  "forum-wave1-rollout-evidence.json",
);

function fail(message) {
  console.error("[verify-page-builder-consumer-readiness] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function readJson(filePath, label) {
  if (!fs.existsSync(filePath)) fail(`missing ${label}: ${filePath}`);
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    fail(`${label} is not valid JSON: ${error.message}`);
  }
}

function requireMarkers(source, markers, label) {
  for (const marker of markers) {
    if (!source.includes(marker)) {
      fail(`${label} missing marker '${marker}'`);
    }
  }
}

function requireExactNames(values, expected, label) {
  const names = values.map((value) => value?.name ?? value);
  const remaining = new Set(names);
  for (const name of expected) {
    if (!remaining.delete(name)) fail(`${label} missing '${name}'`);
  }
  if (remaining.size > 0 || names.length !== expected.length) {
    fail(`${label} contains unsupported values: ${[...remaining].join(", ")}`);
  }
}

function validateExpectedProfiles(profiles) {
  const requiredProfiles = ["all_on", "publish_off", "preview_off", "builder_off"];
  requireExactNames(profiles, requiredProfiles, "Forum source-ready fallback profiles");
  const allowed = new Set([
    "expected_pass",
    "expected_typed_feature_disabled",
    "expected_readonly_fallback",
  ]);
  for (const profile of profiles) {
    for (const key of ["list", "open", "preview", "save_draft", "publish_dry"]) {
      const value = profile.expected_smoke?.[key];
      if (!allowed.has(value)) {
        fail(`Forum source-ready profile '${profile.name}' has unsupported expected_smoke.${key}`);
      }
      if ((key === "list" || key === "open") && value !== "expected_pass") {
        fail(`Forum source-ready profile '${profile.name}' must keep ${key} available`);
      }
    }
    for (const key of ["admin_list_no_5xx", "admin_read_no_5xx", "storefront_read_no_5xx"]) {
      if (profile.expected_read_guarantees?.[key] !== true) {
        fail(`Forum source-ready profile '${profile.name}' missing expected read guarantee '${key}'`);
      }
    }
  }
}

function validateObservedProfiles(profiles) {
  const requiredProfiles = ["all_on", "publish_off", "preview_off", "builder_off"];
  requireExactNames(profiles, requiredProfiles, "Forum live fallback profiles");
  const allowed = new Set(["pass", "typed_feature_disabled_error", "readonly_fallback"]);
  for (const profile of profiles) {
    for (const key of ["list", "open", "preview", "save_draft", "publish_dry"]) {
      const value = profile.smoke?.[key];
      if (!allowed.has(value)) {
        fail(`Forum live profile '${profile.name}' has unsupported smoke.${key}`);
      }
      if ((key === "list" || key === "open") && value !== "pass") {
        fail(`Forum live profile '${profile.name}' must pass ${key}`);
      }
    }
    for (const key of ["admin_list_no_5xx", "admin_read_no_5xx", "storefront_read_no_5xx"]) {
      if (profile.read_guarantees?.[key] !== true) {
        fail(`Forum live profile '${profile.name}' missing read guarantee '${key}'`);
      }
    }
    if (profile.decision !== "keep") {
      fail(`Forum live profile '${profile.name}' must record keep decision`);
    }
  }
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
requireMarkers(moduleToml, ["contract_version", "builder_contract_version"], `${arg}: manifest`);
requireMarkers(
  implPlan,
  ["## Current state", "## FFA/FBA status", "## Open results"],
  `${arg}: implementation-plan`,
);
if (!implPlan.match(/FBA|page.builder|builder/mi)) {
  fail(`${arg}: implementation-plan has no Page Builder readiness notes`);
}

if (arg === "pages") {
  requireMarkers(
    moduleToml,
    [
      "[fba.builder_consumer.rollout_policy]",
      'audit_trail = "control_plane_builder_wave_audit"',
      "before_snapshot_required = true",
      "after_snapshot_required = true",
      "decision_required = true",
      "owner_signoff_required = true",
      "rollback_without_redeploy_target_minutes = 10",
      'pilot_smoke = "preview -> properties -> publish(dry)"',
      "runtime_error_rate_above_alert_threshold",
      "publish_latency_p95_above_slo_for_10m",
      "sanitize_failures_above_alert_threshold",
      "storefront_published_read_regression",
      "pages_owned_list_read_menu_paths_stay_available_when_builder_capabilities_are_disabled",
    ],
    `${arg}: manifest rollout policy`,
  );
}

if (arg === "forum") {
  requireMarkers(
    moduleToml,
    [
      "[fba.builder_consumer.degraded_modes]",
      'builder_disabled = "forum_widgets_readonly_keep_forum_routes"',
      'preview_disabled = "forum_widget_preview_hidden_keep_forum_routes"',
      'publish_disabled = "forum_widget_publish_feature_disabled_keep_forum_routes"',
      'fallback_mode = "readonly"',
      'fallback_mode = "degraded"',
      'fallback_mode = "hidden"',
      "builder_off = [",
      "publish_off = [",
      "builder.enabled=false",
      "builder.publish.enabled=false",
      "[fba.builder_consumer.rollout_policy]",
      'audit_trail = "control_plane_builder_wave_audit"',
      "before_snapshot_required = true",
      "after_snapshot_required = true",
      "decision_required = true",
      "owner_signoff_required = true",
      "rollback_without_redeploy_target_minutes = 10",
      'pilot_smoke = "list -> open -> preview -> save_draft -> publish_dry"',
      "runtime_error_rate_above_alert_threshold",
      "publish_latency_p95_above_slo_for_10m",
      "sanitize_failures_above_alert_threshold",
      "storefront_published_read_regression",
      "forum_owned_list_read_topic_paths_stay_available_when_builder_capabilities_are_disabled",
    ],
    `${arg}: manifest fallback and rollout policy`,
  );

  const fallbackMatrix = readJson(forumFallbackMatrixPath, "Forum FW-2 fallback static matrix");
  const serializedMatrix = JSON.stringify(fallbackMatrix);
  for (const marker of [
    "rustok.forum.fw2_fallback_static_matrix.v1",
    "design_static_ready",
    "builder_off",
    "publish_off",
    "forum-read-routes-survive-builder-off",
    "forum-moderation-routes-survive-publish-off",
    "forum-service-moderation-policy-stays-domain-owned",
    "non_5xx",
    "deferred",
  ]) {
    if (!serializedMatrix.includes(marker)) {
      fail(`${arg}: FW-2 fallback static matrix missing marker '${marker}'`);
    }
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
  for (const assertion of fallbackMatrix.assertions ?? []) {
    if (assertion.expected_http_class !== "non_5xx") {
      fail(`${arg}: FW-2 assertion '${assertion.id}' must target non_5xx status class`);
    }
    for (const marker of assertion.source_markers ?? []) {
      if (!combinedForumSource.includes(marker)) {
        fail(`${arg}: FW-2 assertion '${assertion.id}' source marker not found: ${marker}`);
      }
    }
  }

  const evidence = readJson(forumWave1EvidencePath, "Forum Wave 1 evidence packet");
  if (
    evidence.artifact !== "page_builder_wave_evidence_packet" ||
    evidence.module_slug !== "forum" ||
    evidence.wave !== "1"
  ) {
    fail("forum: Wave evidence identity drifted");
  }

  if (evidence.mode === "source_ready") {
    if (evidence.schema_version !== 2) {
      fail("forum: source-ready Wave evidence must use schema_version 2");
    }
    if (evidence.provenance !== "synthetic_fixture") {
      fail("forum: source-ready Wave evidence provenance must be synthetic_fixture");
    }
    if (evidence.execution_status !== "not_run_by_implementation_agent") {
      fail("forum: source-ready Wave evidence must not claim maintainer execution");
    }
    validateExpectedProfiles(evidence.static_readiness?.fallback_profiles ?? []);
    if (
      evidence.observed_run?.required !== true ||
      evidence.observed_run?.status !== "not_run" ||
      evidence.observed_run?.blocked_by !== "pages_reference_consumer_gate"
    ) {
      fail("forum: source-ready Wave evidence must keep the observed run pending behind the pages gate");
    }
    if (
      evidence.observed_run?.required_correlation_path !==
      "builder_write -> forum_publish -> storefront_read"
    ) {
      fail("forum: source-ready Wave correlation path drifted");
    }
    requireExactNames(
      evidence.observed_run?.required_profiles ?? [],
      ["all_on", "publish_off", "preview_off", "builder_off"],
      "Forum observed-run profiles",
    );
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
        fail(`forum: source-ready Wave evidence must not materialize live-only section '${liveOnlyKey}'`);
      }
    }
    const serialized = JSON.stringify(evidence);
    for (const marker of ["live_wave1_actual:", "trace-forum-wave1-live", '"approved"']) {
      if (serialized.includes(marker)) {
        fail(`forum: source-ready Wave evidence contains forbidden live marker '${marker}'`);
      }
    }
  } else if (evidence.mode === "live") {
    if (evidence.provenance !== "observed_control_plane") {
      fail("forum: live Wave evidence provenance must be observed_control_plane");
    }
    if (evidence.execution_status !== "maintainer_verified") {
      fail("forum: live Wave evidence must be maintainer_verified");
    }
    validateObservedProfiles(evidence.fallback?.profiles ?? []);
    if (evidence.control_plane?.audit_trail !== "control_plane_builder_wave_audit") {
      fail("forum: live Wave evidence missing control-plane audit trail");
    }
    for (const metric of [
      "preview_p95_ms",
      "publish_p95_ms",
      "sanitize_failure_rate",
      "runtime_error_rate",
    ]) {
      const value = evidence.observability?.metrics?.[metric];
      if (typeof value !== "string" || !value.startsWith("live_wave1_actual:")) {
        fail(`forum: live Wave metric '${metric}' lacks observed provenance`);
      }
    }
    for (const trace of ["builder_write_to_forum_publish", "forum_publish_to_storefront_read"]) {
      if (typeof evidence.observability?.traces?.[trace] !== "string") {
        fail(`forum: live Wave evidence missing trace '${trace}'`);
      }
    }
    if (evidence.rollback?.decision !== "keep") {
      fail("forum: live Wave evidence must record rollback decision keep");
    }
    for (const approver of ["platform_on_call", "forum_owner", "builder_owner", "runtime_owner"]) {
      if (evidence.approvals?.[approver] !== "approved") {
        fail(`forum: live Wave evidence missing approval from ${approver}`);
      }
    }
    if ((evidence.waivers ?? []).length !== 0) {
      fail("forum: live Wave evidence must not rely on waivers");
    }
  } else {
    fail("forum: Wave evidence mode must be source_ready or live");
  }
}

console.log("[verify-page-builder-consumer-readiness] PASS");
console.log(`module=${arg}; crate=${crateName}; consumer_manifest_markers=${hasConsumerManifestMarkers}`);
