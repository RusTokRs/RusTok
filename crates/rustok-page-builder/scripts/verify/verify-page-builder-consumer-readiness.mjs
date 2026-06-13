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

function fail(message) {
  console.error("[verify-page-builder-consumer-readiness] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
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

console.log("[verify-page-builder-consumer-readiness] PASS");
console.log(`module=${arg}; crate=${crateName}; consumer_manifest_markers=${hasConsumerManifestMarkers}`);
