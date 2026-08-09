#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-execution-contract.json",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-feature-preflight-harness-source.json",
  config: "apps/next-admin/playwright.pages-builder-rollout-feature-preflight.config.ts",
  spec: "apps/next-admin/tests/pages-builder-rollout-feature-preflight/feature-preflight.spec.ts",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  graphqlMod: "crates/rustok-pages/src/graphql/mod.rs",
  pagesCargo: "crates/rustok-pages/Cargo.toml",
  pageBuilderCargo: "crates/rustok-page-builder/Cargo.toml",
  rollout: "crates/rustok-page-builder/src/rollout.rs",
  service: "crates/rustok-page-builder/src/service.rs",
  packet: "docs/modules/pages-page-builder-rollout-feature-preflight-actualization-2026-08-08.md",
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
const packet = read(files.packet);

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
  contract.capability_preflight?.feature_disabled_kind !== "feature-disabled" ||
  contract.capability_preflight?.feature_disabled_code !== "FEATURE_DISABLED"
) failures.push("canonical feature preflight contract drifted");

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

for (const marker of [
  "fullyParallel: false", "workers: 1", "retries: 0", 'trace: "off"',
  'screenshot: "off"', 'video: "off"', 'name: "pages-builder-rollout-feature-preflight-chromium"',
]) need(config, marker, "Playwright config");

for (const marker of [
  "pageBuilderCapabilityPreflight(capability: PREVIEW)",
  "pageBuilderCapabilityPreflight(capability: PROPERTIES)",
  "pageBuilderCapabilityPreflight(capability: PUBLISH)",
  'result.errorKind !== "feature-disabled"',
  'result.errorCode !== "FEATURE_DISABLED"',
  "matrixBrowser.sha256 !== browser.record.sha256",
  "matrixTarget.api_origin_sha256 !== browserTarget.origin_sha256",
  "matrixTarget.deployment_image_digest !== deploymentDigest",
  "withProfile(original.settings, profile.flags)",
  "writePagesSettings(context, settings)",
  "finally {",
  "await writePagesSettings(context, originalSettings)",
  "restoreVerified = true",
  "raw_settings_persisted: false",
  "feature_preflight_executed: true",
  "gate_accepted: false",
  "provider_health_observed: false",
  "renameSync(temporary, location)",
]) need(spec, marker, "feature preflight spec");
for (const marker of [
  "SELECT ", "INSERT ", "UPDATE tenant_modules", "DELETE FROM", "DATABASE_URL",
  "localStorage", "sessionStorage", 'trace: "on"', 'screenshot: "on"', 'video: "on"',
  "gate_accepted: true", "provider_health_observed: true",
]) forbid(spec, marker, "feature preflight spec");

for (const marker of [
  "pub enum GqlPageBuilderCapability",
  '#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]',
  "pub struct GqlPageBuilderCapabilityPreflight",
  "async fn page_builder_capability_preflight(",
  "let required_permission = required_page_builder_permission(capability_kind);",
  "let provider_health = provider_health_snapshot(ctx);",
  "effective_provider_runtime_flags(&flags, provider_health.as_ref())",
  "ensure_capability(&effective_flags, capability_kind)",
  "PageBuilderErrorKind::FeatureDisabled.as_str()",
  "PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE",
  "fn required_page_builder_permission(capability: BuilderCapabilityKind) -> Permission",
  "BuilderCapabilityKind::Preview | BuilderCapabilityKind::Tree",
  "Permission::new(Resource::Pages, Action::Read)",
  "BuilderCapabilityKind::Properties => Permission::new(Resource::Pages, Action::Update)",
  "BuilderCapabilityKind::Publish => Permission::new(Resource::Pages, Action::Publish)",
]) need(owner, marker, "Pages server-owned feature preflight");

const preflightStart = owner.indexOf("async fn page_builder_capability_preflight(");
const permissionMappingStart = owner.indexOf("\nfn required_page_builder_permission(", preflightStart);
if (preflightStart < 0 || permissionMappingStart <= preflightStart) {
  failures.push("non-mutating preflight source slice could not be isolated");
} else {
  const preflight = owner.slice(preflightStart, permissionMappingStart);
  for (const marker of ["save_project(", "render_preview(", ".publish(", "save_document(", "std::fs::"]) {
    forbid(preflight, marker, "non-mutating feature preflight");
  }
}

for (const marker of [
  "pub struct PageBuilderCapabilityPermissions",
  "preview: Permission::new(Resource::Pages, Action::Read)",
  "tree: Permission::new(Resource::Pages, Action::Read)",
  "properties: Permission::new(Resource::Pages, Action::Update)",
  "publish: Permission::new(Resource::Pages, Action::Publish)",
  "PAGE_BUILDER_PAGES_READ_PERMISSION",
  "PAGE_BUILDER_PAGES_UPDATE_PERMISSION",
  "PAGE_BUILDER_PAGES_PUBLISH_PERMISSION",
]) need(service, marker, "Page Builder authorizer permission mapping reference");
for (const marker of [
  "pub fn ensure_capability(",
  "Err(BuilderRolloutError::CapabilityDisabled(capability.as_str()))",
  "pub fn effective_provider_runtime_flags(",
  "provider_health_runtime_flags_only_narrow_configured_rollout",
]) need(rollout, marker, "shared rollout/provider guard");
for (const marker of [
  "Self::CapabilityDisabled(_) => PageBuilderErrorKind::FeatureDisabled",
  "Self::CapabilityDisabled(_) => Some(PAGE_BUILDER_FEATURE_DISABLED_ERROR_CODE)",
  "ensure_capability(&self.flags, BuilderCapabilityKind::Publish)?;",
]) need(service, marker, "canonical Page Builder feature-disabled service contract");

for (const marker of ["GqlPageBuilderCapability", "GqlPageBuilderCapabilityPreflight", "GqlPageBuilderRolloutSnapshot"])
  need(graphqlMod, marker, "Pages GraphQL exports");
forbid(pagesCargo, 'rustok-page-builder = { workspace = true, default-features = false, features = ["server"]', "Pages Page Builder dependency");
for (const marker of ['default = ["server"]', '"dep:rustok-api"', '"dep:rustok-core"'])
  need(pageBuilderCargo, marker, "Page Builder feature boundary");

if (evidence.format !== "pages_builder_rollout_feature_preflight_harness_source_v1") failures.push("feature preflight source evidence format drifted");
if (evidence.status !== "pages_builder_rollout_feature_preflight_harness_source_unvalidated") failures.push("feature preflight source evidence status drifted");
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) failures.push("feature preflight source evidence execution must remain empty");
for (const [key, value] of Object.entries(evidence.validation ?? {})) if (value !== false) failures.push(`feature preflight validation.${key} must remain false`);
for (const key of [
  "server_owned_non_mutating_preflight_added", "preflight_requires_exact_routed_tenant",
  "preflight_uses_page_builder_permission_mapping", "preflight_uses_shared_rollout_guard",
  "preflight_allowed_path_has_no_error_contract", "preflight_disabled_kind_is_feature_disabled",
  "preflight_disabled_code_is_FEATURE_DISABLED", "browser_predecessor_required",
  "rollout_matrix_predecessor_required", "predecessors_same_source_required",
  "rollout_matrix_must_bind_exact_browser_hash", "api_origin_and_deployment_digest_must_match_predecessors",
  "original_settings_restored_in_finally", "restore_semantically_verified", "output_is_atomic",
]) if (evidence.source_contract?.[key] !== true) failures.push(`source_contract.${key} must be true`);
for (const key of [
  "direct_sql_used", "raw_database_access_used", "raw_module_settings_persisted",
  "raw_graphql_bodies_persisted", "credentials_or_storage_contents_persisted",
  "automatic_gate_acceptance", "automatic_source_mutation", "automatic_ffa_fba_promotion",
  "provider_health_observed", "gate_accepted", "forum_wave_accepted", "ffa_promoted", "fba_promoted",
]) if (evidence.source_contract?.[key] !== false) failures.push(`source_contract.${key} must remain false`);

for (const marker of [
  "source-ready / maintainer-execution-pending", "feature-disabled / FEATURE_DISABLED",
  "PageBuilderCapabilityPermissions", "source-lock", "ensure_capability",
  "browser -> rollout matrix -> feature preflight", "No tests, Node verifiers, Cargo commands",
]) need(packet, marker, "feature preflight actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-feature-preflight-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-feature-preflight-harness] PASS source_ready=true execution=not_run gate_accepted=false");
