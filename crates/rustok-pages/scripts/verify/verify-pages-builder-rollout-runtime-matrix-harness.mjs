#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const files = {
  contract: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-execution-contract.json",
  evidence: "crates/rustok-pages/contracts/evidence/pages-builder-rollout-runtime-matrix-harness-source.json",
  config: "apps/next-admin/playwright.pages-builder-rollout-matrix.config.ts",
  spec: "apps/next-admin/tests/pages-builder-rollout-matrix/runtime-matrix.spec.ts",
  gate: "crates/rustok-pages/contracts/evidence/pages-reference-consumer-gate-source.json",
  owner: "crates/rustok-pages/src/graphql/builder_rollout.rs",
  adminMain: "apps/admin/src/main.rs",
  actualization: "docs/modules/pages-page-builder-rollout-runtime-matrix-harness-actualization-2026-08-08.md",
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
const packet = read(files.actualization);

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
for (const forbidden of [
  "tenant slugs",
  "tenant ids",
  "page ids",
  "authorization headers",
  "cookies",
  "storage state contents",
  "raw module settings",
  "raw GraphQL request or response bodies",
  "traces",
  "screenshots",
  "videos",
]) {
  if (!contract.forbidden_retained_data?.includes(forbidden)) {
    failures.push(`privacy contract is missing ${forbidden}`);
  }
}

for (const marker of [
  "fullyParallel: false",
  "workers: 1",
  "retries: 0",
  'trace: "off"',
  'screenshot: "off"',
  'video: "off"',
  'name: "pages-builder-rollout-matrix-chromium"',
]) need(config, marker, "Playwright config");

for (const marker of [
  "currentCommit()",
  "validatePredecessor",
  "target?.origin_sha256 !== sha256(apiOrigin)",
  "target?.standalone_origin_sha256 !== sha256(adminOrigin)",
  "apiOrigin === adminOrigin",
  "api_storage_state_environment",
  "admin_storage_state_environment",
  "const apiContext = await browser.newContext",
  "const adminContext = await browser.newContext",
  "storageState: apiStorage.path",
  "storageState: adminStorage.path",
  "tenantModulesQuery",
  "updateSettingsMutation",
  "rolloutSnapshotQuery",
  "pagesReadsQuery",
  "withProfile(original.settings, profile.flags)",
  "writePagesSettings(apiContext, profileSettings)",
  "readRolloutSnapshot(apiContext, tenantSlug, profile)",
  "assertPagesReads(apiContext, pageId)",
  "const page = await adminContext.newPage()",
  "assertUiProfile(page, profile)",
  "allowedPreview(page, profile.id === \"all_on\")",
  "deniedPreview(adminContext, previewTemplate)",
  'deniedBrowserIntent(\n            adminContext,\n            pageId,\n            "save",\n            "publish",',
  '"rename_page"',
  '"properties"',
  "finally {",
  "await writePagesSettings(apiContext, originalSettings)",
  "canonicalJson(restored.settings) !== canonicalJson(originalSettings)",
  "restoreVerified = true",
  "Promise.all([apiContext.close(), adminContext.close()])",
  "rmSync(output, { force: true })",
  "renameSync(temporary, location)",
  "storage_states:",
  "api_origin_sha256: sha256(apiOrigin)",
  "admin_origin_sha256: sha256(adminOrigin)",
  "raw_settings_persisted: false",
  "provider_health_observed: false",
  "gate_accepted: false",
  "forum_wave_accepted: false",
  "canonical_source_mutated: false",
]) need(spec, marker, "matrix spec");

for (const marker of [
  "SELECT ",
  "INSERT ",
  "UPDATE tenant_modules",
  "DELETE FROM",
  "DATABASE_URL",
  "localStorage",
  "sessionStorage",
  "trace: \"on\"",
  "screenshot: \"on\"",
  "video: \"on\"",
  "gate_accepted: true",
  "provider_health_observed: true",
  "ffa_promoted: true",
  "fba_promoted: true",
]) forbid(spec, marker, "matrix spec");

for (const marker of [
  "page_builder_rollout_snapshot",
  "Permission::PAGES_READ",
  "tenant_module_settings(db, tenant.id, MODULE_SLUG)",
  "BuilderCapabilityFlags::from_module_settings(&settings)",
  "provider_health_observed: false",
]) need(owner, marker, "Pages rollout owner");
for (const marker of [
  "fetch_pages_builder_rollout_snapshot(",
  "pages_editor_capabilities_for_rollout(",
  "dispatch_pages_browser_intent_with_capabilities(snapshot, envelope, editor_capabilities)",
]) need(adminMain, marker, "standalone browser-intent rollout binding");

if (evidence.format !== "pages_builder_rollout_runtime_matrix_harness_source_v1") {
  failures.push("matrix source evidence format drifted");
}
if (evidence.status !== "pages_builder_rollout_runtime_matrix_harness_source_unvalidated") {
  failures.push("matrix source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("matrix source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`matrix validation.${key} must remain false`);
}
for (const key of [
  "execution_contract_added",
  "playwright_config_added",
  "bounded_matrix_spec_added",
  "predecessor_same_source_required",
  "predecessor_same_api_origin_hash_required",
  "predecessor_same_admin_origin_hash_required",
  "predecessor_immutable_deployment_digest_required",
  "api_and_admin_origins_must_be_distinct",
  "api_operator_storage_state_required",
  "admin_operator_storage_state_required",
  "production_tenant_modules_read_used",
  "production_update_module_settings_used",
  "original_settings_restored_in_finally",
  "restore_semantically_verified",
  "server_owned_rollout_snapshot_checked_per_profile",
  "admin_provider_state_checked_per_profile",
  "authoritative_ssr_preview_checked_per_profile",
  "pages_owned_list_read_checked_per_profile",
  "pages_owned_document_read_checked_per_profile",
  "all_on_publish_probe_is_non_mutating",
  "output_is_atomic",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`matrix source_contract.${key} must be true`);
  }
}
for (const key of [
  "direct_sql_used",
  "raw_database_access_used",
  "raw_module_settings_persisted",
  "raw_graphql_bodies_persisted",
  "raw_preview_request_or_response_persisted",
  "raw_browser_intent_response_persisted",
  "credentials_or_storage_contents_persisted",
  "automatic_gate_acceptance",
  "automatic_source_mutation",
  "automatic_ffa_fba_promotion",
  "provider_health_observed",
  "gate_accepted",
  "forum_wave_accepted",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`matrix source_contract.${key} must remain false`);
  }
}

if (
  gate.accepted !== false ||
  gate.current_boundary?.execution_gate !== "pending" ||
  gate.current_boundary?.provider_health !== "unobserved" ||
  gate.current_boundary?.four_profile_runtime_matrix !==
    "harness_source_ready_maintainer_execution_pending"
) failures.push("Pages gate must remain pending, unaccepted, unobserved and matrix-execution pending");

for (const marker of [
  "source-ready / maintainer-execution-pending",
  "updateModuleSettings",
  "API origin",
  "admin origin",
  "separate reviewed storage-state files",
  "restore",
  "four profiles",
  "No tests, Node verifiers, Cargo commands",
]) need(packet, marker, "matrix actualization");

if (failures.length) {
  console.error("[verify-pages-builder-rollout-runtime-matrix-harness] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-builder-rollout-runtime-matrix-harness] PASS source_ready=true execution=not_run gate_accepted=false targets=api+admin auth=split");
