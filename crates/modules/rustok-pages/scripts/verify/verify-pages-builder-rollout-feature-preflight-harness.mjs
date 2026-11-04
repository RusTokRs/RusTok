#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const files = {
  contract: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json",
  config: "apps/next-admin/playwright.pages-builder-rollout-feature-preflight.config.ts",
  spec: "apps/next-admin/tests/pages-builder-rollout-feature-preflight/feature-preflight.spec.ts",
  owner: "crates/modules/rustok-pages/src/graphql/builder_rollout.rs",
  graphqlMod: "crates/modules/rustok-pages/src/graphql/mod.rs",
  pagesCargo: "crates/modules/rustok-pages/Cargo.toml",
  pageBuilderCargo: "crates/modules/rustok-page-builder/Cargo.toml",
  rollout: "crates/modules/rustok-page-builder/src/rollout.rs",
  service: "crates/modules/rustok-page-builder/src/service.rs",
};
const failures = [];
const abs = (value) => path.join(repoRoot, value);
const read = (value) => fs.readFileSync(abs(value), "utf8");
const need = (source, marker, label) => { if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`); };
const forbid = (source, marker, label) => { if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`); };

for (const [label, relativePath] of Object.entries(files)) {
  if (!fs.existsSync(abs(relativePath))) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(abs(relativePath));
  if (!stats.isFile() || stats.isSymbolicLink()) failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
}
if (failures.length) {
  console.error("[verify-pages-builder-rollout-feature-preflight-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}

const contract = JSON.parse(read(files.contract));
const evidence = JSON.parse(read(files.evidence));
const config = read(files.config);
const spec = read(files.spec);
const owner = read(files.owner);
const graphqlMod = read(files.graphqlMod);
const pagesCargo = read(files.pagesCargo);
const pageBuilderCargo = read(files.pageBuilderCargo);
const rollout = read(files.rollout);
const service = read(files.service);

if (
  contract.schema_version !== 1 ||
  contract.module !== "pages" ||
  contract.packet !== "pages-builder-rollout-feature-preflight" ||
  contract.status !== "source_ready_maintainer_execution_pending"
) failures.push("feature preflight execution contract identity drifted");

const expectedProfiles = [
  ["all_on", [true, true, true, true], "allowed", "allowed", "allowed"],
  ["publish_off", [true, true, true, false], "allowed", "allowed", "feature_disabled"],
  ["preview_off", [true, false, true, false], "feature_disabled", "allowed", "feature_disabled"],
  ["builder_off", [false, false, false, false], "feature_disabled", "feature_disabled", "feature_disabled"],
];
const actualProfiles = contract.profiles?.map((profile) => [profile.id, profile.flags, profile.preview, profile.properties, profile.publish]);
if (JSON.stringify(actualProfiles) !== JSON.stringify(expectedProfiles)) failures.push("feature preflight profile contract drifted");

const expectedPermissionMapping = {
  preview: "pages:read",
  tree: "pages:read",
  properties: "pages:update",
  publish: "pages:publish",
};
if (
  contract.capability_preflight?.operation !== "pageBuilderCapabilityPreflight" ||
  contract.capability_preflight?.non_mutating !== true ||
  JSON.stringify(contract.capability_preflight?.permission_mapping_contract) !== JSON.stringify(expectedPermissionMapping) ||
  contract.capability_preflight?.permission_mapping_reference !== "rustok_page_builder::service::PageBuilderCapabilityPermissions" ||
  contract.capability_preflight?.permission_mapping_reference_is_source_locked !== true ||
  contract.capability_preflight?.server_feature_dependency_required_in_pages !== false ||
  contract.capability_preflight?.rollout_guard_owner !== "rustok_page_builder::rollout::ensure_capability" ||
  contract.capability_preflight?.provider_runtime_policy_owner !== "rustok_page_builder::rollout::effective_provider_runtime_flags" ||
  contract.capability_preflight?.feature_disabled_kind !== "feature-disabled" ||
  contract.capability_preflight?.feature_disabled_code !== "FEATURE_DISABLED"
) failures.push("canonical feature preflight contract drifted");

if (
  contract.provider_health_boundary?.harness_purpose !== "rollout_only" ||
  contract.provider_health_boundary?.provider_health_must_remain_unobserved !== true ||
  contract.provider_health_boundary?.observation_operation !== "pageBuilderRolloutSnapshot" ||
  contract.provider_health_boundary?.observe_before_each_profile_preflight !== true ||
  contract.provider_health_boundary?.observe_after_each_profile_preflight !== true ||
  contract.provider_health_boundary?.provider_health_payload_must_be_absent !== true ||
  contract.provider_health_boundary?.fail_if_provider_health_is_observed !== true ||
  contract.provider_health_boundary?.retained_health_payload !== false
) failures.push("rollout-only provider-health observation boundary drifted");

if (
  contract.settings_authority?.read_operation !== "tenantModules" ||
  contract.settings_authority?.write_operation !== "updateModuleSettings" ||
  contract.settings_authority?.module_slug !== "pages" ||
  contract.settings_authority?.direct_sql_allowed !== false ||
  contract.settings_authority?.raw_database_access_allowed !== false ||
  contract.settings_authority?.restore_original_settings_in_finally !== true ||
  contract.settings_authority?.verify_semantic_restore_after_finally !== true
) failures.push("feature preflight settings authority/restore contract drifted");

for (const relativePath of contract.required_source_files ?? []) {
  if (!fs.existsSync(abs(relativePath))) failures.push(`required source file is missing: ${relativePath}`);
}

, marker, "feature preflight actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-feature-preflight-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-feature-preflight-harness] PASS source_ready=true health=verified_unobserved execution=not_run gate_accepted=false");
