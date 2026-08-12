#!/usr/bin/env node

import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const failures = [];

function read(relativePath) {
  try {
    return readFileSync(path.join(repoRoot, relativePath), "utf8");
  } catch (error) {
    failures.push(`${relativePath}: ${error.message}`);
    return "";
  }
}

function json(relativePath) {
  const source = read(relativePath);
  try {
    return JSON.parse(source);
  } catch (error) {
    failures.push(`${relativePath}: invalid JSON: ${error.message}`);
    return {};
  }
}

function requireValue(condition, message) {
  if (!condition) failures.push(message);
}

function requireText(source, marker, label) {
  if (!source.includes(marker)) failures.push(`${label}: missing '${marker}'`);
}

const contractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-wave-observed-acceptance-source.json";
const wavePath = "crates/rustok-forum/contracts/evidence/forum-wave1-rollout-evidence.json";
const runnerPath = "scripts/evidence/accept-forum-page-builder-wave.mjs";
const testsPath = "scripts/evidence/accept-forum-page-builder-wave.test.mjs";
const lineagePath = "scripts/verify/verify-forum-wave-admission-lineage.mjs";
const actualizationPath =
  "docs/modules/forum-page-builder-wave-observed-acceptance-actualization-2026-08-12.md";
const workflowPath = ".github/workflows/pages-page-builder-provider-health.yml";

const contract = json(contractPath);
const wave = json(wavePath);
const runner = read(runnerPath);
const tests = read(testsPath);
const lineage = read(lineagePath);
const actualization = read(actualizationPath);
const workflow = read(workflowPath);

requireValue(
  contract.format === "forum_page_builder_wave_observed_acceptance_source_v1" &&
    contract.status === "source_ready_maintainer_execution_pending" &&
    contract.module === "forum" &&
    contract.wave === "1",
  `${contractPath}: identity drifted`,
);
requireValue(
  contract.owner_decision?.runner === runnerPath &&
    JSON.stringify(contract.owner_decision?.decisions) ===
      JSON.stringify(["accept_observed_wave_evidence", "reject"]),
  `${contractPath}: owner decision contract drifted`,
);
requireValue(
  contract.output?.format === "forum_page_builder_wave_observed_acceptance_v1" &&
    contract.output?.accepted_status ===
      "owner_accepted_observed_control_plane_wave_promotion_review_pending" &&
    contract.output?.rejected_status === "owner_rejected_observed_control_plane_wave",
  `${contractPath}: output contract drifted`,
);
for (const key of [
  "accepted_packet_does_not_itself_promote_ffa_or_fba",
  "accepted_packet_does_not_mutate_control_plane_or_rollout",
  "accepted_packet_does_not_assert_current_provider_health",
  "accepted_packet_does_not_claim_cryptographic_origin_to_repo_digest_binding",
]) {
  requireValue(contract.promotion_boundary?.[key] === true, `${contractPath}: ${key} must remain true`);
}

const ownerCursor = wave.observed_run?.owner_acceptance;
requireValue(
  ownerCursor?.required === true &&
    ownerCursor?.source_status === "source_ready_maintainer_execution_pending" &&
    ownerCursor?.format === "forum_page_builder_wave_observed_acceptance_v1" &&
    ownerCursor?.accepted_status ===
      "owner_accepted_observed_control_plane_wave_promotion_review_pending" &&
    ownerCursor?.execution_status === "maintainer_execution_pending",
  `${wavePath}: owner acceptance cursor drifted`,
);

for (const marker of [
  "--wave-evidence",
  "--admission",
  "--owner-id",
  "accept_observed_wave_evidence",
  "contract.output.accepted_status",
  "RUSTOK_FORUM_WAVE_EVIDENCE_PATH",
  "RUSTOK_FORUM_WAVE_ADMISSION_PATH",
  "verify-forum-wave-evidence-freshness.mjs",
  "verify-forum-wave-admission-lineage.mjs",
  "git\", [\"rev-parse\", \"HEAD\"]",
  "control_plane_or_rollout_mutated: false",
  "current_provider_health_asserted: false",
  "cryptographic_origin_to_repo_digest_binding_claimed: false",
  "ffa_promoted: false",
  "fba_promoted: false",
  "raw_input_paths_persisted: false",
]) requireText(runner, marker, runnerPath);

for (const forbidden of ["fetch(", "@playwright/test", "chromium", 'spawnSync("cargo"']) {
  if (runner.includes(forbidden)) failures.push(`${runnerPath}: forbidden execution marker '${forbidden}'`);
}

for (const label of [
  "accepts fresh lineage-verified observed Forum Wave evidence",
  "retains explicit reject without promotion",
  "rejects source-ready Wave evidence at owner review",
  "rejects invalid owner identifier",
  "rejects unsupported owner decision",
  "rejects stale observed Wave evidence",
  "rejects retained admission hash drift",
  "rejects admission source-commit drift",
  "rejects admission privacy overclaim",
  "rejects live Wave that drops the lineage verifier from refresh gates",
]) requireText(tests, label, testsPath);

requireText(lineage, "live Wave latest refresh", lineagePath);

for (const marker of [
  "forum-wave-observed-owner-acceptance-source-ready",
  "retrospective owner-decision packet",
  "accept_observed_wave_evidence",
  "does not promote FFA/FBA",
  "maintainer execution remains pending",
]) requireText(actualization, marker, actualizationPath);

for (const marker of [
  contractPath,
  runnerPath,
  testsPath,
  "Verify Forum Wave observed owner acceptance source",
  "Forum Wave observed owner acceptance synthetic tests",
]) requireText(workflow, marker, workflowPath);

if (failures.length > 0) {
  console.error("[verify-forum-page-builder-wave-observed-acceptance] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-forum-page-builder-wave-observed-acceptance] PASS");
console.log(
  "module=forum; wave=1; observed_owner_acceptance=source_ready_maintainer_execution_pending",
);
