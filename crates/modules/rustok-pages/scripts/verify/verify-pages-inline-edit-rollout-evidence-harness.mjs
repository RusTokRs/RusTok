#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  contract:
    "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-rollout-execution-contract.json",
  evidence:
    "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-rollout-evidence-harness-source.json",
  browserContract:
    "crates/modules/rustok-pages/contracts/evidence/pages-inline-edit-browser-execution-contract.json",
  assembler: "scripts/evidence/assemble-pages-inline-edit-rollout-evidence.mjs",
  executionPlan: "docs/modules/pages-page-builder-inline-edit-execution-plan.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const exact = (actual, expected, label) => {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) failures.push(`${label} drifted`);
};

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-rollout-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const browserContract = JSON.parse(read(files.browserContract));
const assembler = read(files.assembler);
const plan = read(files.executionPlan);

if (contract.schema_version !== 1) failures.push("contract schema_version must be 1");
if (contract.module !== "pages") failures.push("contract module must be pages");
if (contract.packet !== "pages-inline-edit-rollout-execution") {
  failures.push("contract packet identity drifted");
}
if (contract.status !== "source_ready_maintainer_execution_pending") {
  failures.push("contract source status drifted");
}
if (
  contract.runtime_control_boundary !==
  "observes_external_rollout_without_mutating_configuration_or_deployment"
) {
  failures.push("runtime control boundary drifted");
}
exact(
  contract.browser_input,
  {
    format: "pages_inline_edit_browser_execution_v1",
    status: "browser_execution_passed_rollout_pending",
    same_source_commit_required: true,
    deployment_digest_required: true,
    rollout_boundary_must_be_open: true,
  },
  "browser input contract",
);
if (
  contract.browser_input.format !== browserContract.output?.format ||
  contract.browser_input.status !== browserContract.output?.status
) {
  failures.push("rollout browser input is not tied to the browser output contract");
}
if (contract.observation_input?.format !== "pages_inline_edit_rollout_observation_v1") {
  failures.push("observation input format drifted");
}
if (contract.observation_input?.flag_key !== "pages.builder.inline_edit.enabled") {
  failures.push("rollout flag key drifted");
}
exact(
  contract.observation_input?.phases,
  ["ffa", "fba"],
  "rollout observation phases",
);
exact(
  contract.observation_input?.required_admission_facts,
  [
    "pages_module_enabled_required",
    "direct_user_required",
    "authenticated_session_required",
    "pages_update_required",
  ],
  "rollout admission facts",
);
exact(
  contract.observation_input?.required_monitoring_series,
  [
    "save_conflicts",
    "authorization_denials",
    "grant_verification_failures",
    "client_load_failures",
  ],
  "rollout monitoring series",
);
if (contract.observation_input?.raw_monitoring_logs_allowed !== false) {
  failures.push("raw monitoring logs must remain forbidden");
}
if (contract.phases?.ffa?.output_status !== "ffa_observation_passed_fba_pending") {
  failures.push("FFA output status drifted");
}
if (contract.phases?.ffa?.previous_ffa_packet_required !== false) {
  failures.push("FFA must not require a previous FFA packet");
}
if (contract.phases?.ffa?.rollback_rehearsal_required !== false) {
  failures.push("FFA must not claim the later rollback rehearsal");
}
if (contract.phases?.fba?.output_status !== "fba_rollout_evidence_complete") {
  failures.push("FBA output status drifted");
}
, marker, "rollout evidence packet");
for (const marker of [
  "inline-edit-rollout-evidence-harness-source-ready",
  "verify-pages-inline-edit-rollout-evidence-harness.mjs",
  "assemble-pages-inline-edit-rollout-evidence.mjs",
  "ffa_observation_passed_fba_pending",
  "fba_rollout_evidence_complete",
  "rollout evidence harness: source-ready",
  "rollout execution: pending",
]) need(plan, marker, "active execution plan");

if (failures.length > 0) {
  console.error("[verify-pages-inline-edit-rollout-evidence-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-inline-edit-rollout-evidence-harness] PASS rollout_harness_source_ready=true execution=pending ffa=false fba=false",
);
