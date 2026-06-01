#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..", "..", "..");
const templatePath = path.join(
  repoRoot,
  "crates",
  "rustok-page-builder",
  "contracts",
  "page-builder-wave-evidence-template.json",
);

function fail(message) {
  console.error("[verify-page-builder-wave-evidence-template] FAIL");
  console.error(`- ${message}`);
  process.exit(1);
}

function expectArrayIncludes(array, values, label) {
  if (!Array.isArray(array)) {
    fail(`${label} must be an array`);
  }
  for (const value of values) {
    if (!array.includes(value)) {
      fail(`${label} missing '${value}'`);
    }
  }
}

if (!fs.existsSync(templatePath)) {
  fail(`missing file: ${path.relative(repoRoot, templatePath)}`);
}

const template = JSON.parse(fs.readFileSync(templatePath, "utf8"));
const requiredProfiles = [
  "all_on",
  "publish_off",
  "preview_off",
  "builder_off",
];

if (template.schema_version !== 1) {
  fail(`unsupported schema_version: ${template.schema_version}`);
}
if (template.artifact !== "page_builder_wave_evidence_template") {
  fail(`unexpected artifact: ${template.artifact}`);
}
if (template.contract !== "grapesjs_v1") {
  fail(`unexpected contract: ${template.contract}`);
}

expectArrayIncludes(
  template.required_profiles,
  requiredProfiles,
  "required_profiles",
);
expectArrayIncludes(
  template.required_sections?.metadata?.provider,
  [
    "builder_contract_version",
    "consumer_min_version",
    "health_profile",
    "degraded_modes",
  ],
  "metadata.provider",
);
expectArrayIncludes(
  template.required_sections?.metadata?.consumer,
  ["dependency_profile", "fallback_matrix", "toggle_profiles"],
  "metadata.consumer",
);
expectArrayIncludes(
  template.required_sections?.control_plane?.change_set,
  ["tenant_id", "wave", "change_set_id", "trace_id"],
  "control_plane.change_set",
);
expectArrayIncludes(
  template.required_sections?.control_plane?.snapshots,
  [
    "flags_before",
    "flags_after",
    "module_health_before",
    "module_health_after",
  ],
  "control_plane.snapshots",
);
expectArrayIncludes(
  template.required_sections?.fallback?.profiles,
  requiredProfiles,
  "fallback.profiles",
);
expectArrayIncludes(
  template.required_sections?.fallback?.smoke_steps,
  ["list", "open", "preview", "save_draft", "publish_dry"],
  "fallback.smoke_steps",
);
expectArrayIncludes(
  template.required_sections?.observability?.metrics,
  [
    "preview_p95_ms",
    "publish_p95_ms",
    "sanitize_failure_rate",
    "runtime_error_rate",
  ],
  "observability.metrics",
);
expectArrayIncludes(
  template.required_sections?.rollback?.required_fields,
  ["decision", "reason", "rollback_reference", "timestamp"],
  "rollback.required_fields",
);
expectArrayIncludes(
  template.required_sections?.approvals,
  ["platform_on_call", "pages_owner", "builder_owner", "runtime_owner"],
  "approvals",
);
expectArrayIncludes(
  template.waiver_policy?.blocked_for_wave_1,
  ["anti_drift", "fallback_read_5xx", "rbac_regression"],
  "waiver_policy.blocked_for_wave_1",
);

console.log("[verify-page-builder-wave-evidence-template] PASS");
