#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-execution-contract.json",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-harness-source.json",
  config: "apps/next-admin/playwright.pages-builder-rollout-matrix.config.ts",
  spec: "apps/next-admin/tests/pages-builder-rollout-matrix/runtime-matrix.spec.ts",
  gate: "crates/modules/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  owner: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  adminMain: "apps/admin/src/main.rs",
};
const failures = [];
const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
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
if (failures.length) {
  console.error("[verify-pages-builder-rollout-runtime-matrix-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const gate = JSON.parse(read(files.gate));
const config = read(files.config);
const spec = read(files.spec);
const owner = read(files.owner);
const adminMain = read(files.adminMain);

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-builder-rollout-runtime-matrix" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) failures.push("matrix execution contract identity drifted");

if (
  contract.predecessor?.same_source_commit_required !== true ||
  contract.predecessor?.same_api_origin_hash_required !== true ||
  contract.predecessor?.same_admin_origin_hash_required !== true ||
  contract.predecessor?.deployment_digest_required !== true ||
  contract.predecessor?.tenant_rollout_must_be_unexecuted !== true
) failures.push("matrix predecessor identity boundary drifted");
if (
  contract.fixtures?.api_origin_environment !== "RUSTOK_PAGES_ROLLOUT_MATRIX_API_ORIGIN" ||
  contract.fixtures?.admin_origin_environment !== "RUSTOK_PAGES_ROLLOUT_MATRIX_ADMIN_ORIGIN" ||
  contract.fixtures?.api_storage_state_environment !== "RUSTOK_PAGES_ROLLOUT_MATRIX_API_STORAGE_STATE" ||
  contract.fixtures?.admin_storage_state_environment !== "RUSTOK_PAGES_ROLLOUT_MATRIX_ADMIN_STORAGE_STATE"
) failures.push("matrix origin/auth fixture contract drifted");

const expectedProfiles = [
  ["all_on", true, true, true, true, "unobserved"],
  ["publish_off", true, true, true, false, "degraded"],
  ["preview_off", true, false, true, false, "degraded"],
  ["builder_off", false, false, false, false, "unavailable"],
];
const actualProfiles = contract.profiles?.map((profile) => [
  profile.id,
  profile.flags?.builder_enabled,
  profile.flags?.preview_enabled,
  profile.flags?.properties_enabled,
  profile.flags?.publish_enabled,
  profile.provider_state,
]);
if (JSON.stringify(actualProfiles) !== JSON.stringify(expectedProfiles)) {
  failures.push("matrix profile set or flags drifted");
}

if (
  contract.settings_authority?.read_operation !== "tenantModules" ||
  contract.settings_authority?.write_operation !== "updateModuleSettings" ||
  contract.settings_authority?.module_slug !== "pages" ||
  contract.settings_authority?.direct_sql_allowed !== false ||
  contract.settings_authority?.raw_database_access_allowed !== false ||
  contract.settings_authority?.restore_original_settings_in_finally !== true ||
  contract.settings_authority?.verify_semantic_restore_after_finally !== true
) failures.push("matrix production settings authority or restore contract drifted");

if (
  contract.output?.format !== "pages_builder_rollout_runtime_matrix_v1" ||
  contract.output?.status !== "four_profile_runtime_matrix_passed_owner_review_pending" ||
  contract.output?.atomic_replace !== true ||
  contract.output?.automatic_gate_acceptance !== false ||
  contract.output?.automatic_source_mutation !== false ||
  contract.output?.automatic_ffa_fba_promotion !== false
) failures.push("matrix output boundary drifted");

for (const relativePath of contract.required_source_files ?? []) {
  if (!fs.existsSync(absolute(relativePath))) {
    failures.push(`required source file is missing: ${relativePath}`);
  }
}
, marker, "matrix actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-runtime-matrix-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-runtime-matrix-harness] PASS source_ready=true execution=not_run gate_accepted=false targets=api+admin auth=split owner_sources=hash_bound");
