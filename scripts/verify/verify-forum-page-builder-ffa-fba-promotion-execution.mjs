#!/usr/bin/env node

import { lstatSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("../../", import.meta.url));
const failures = [];

function read(relativePath) {
  try {
    const location = path.join(repoRoot, relativePath);
    const stat = lstatSync(location);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      failures.push(`${relativePath}: must be a regular non-symlink file`);
      return "";
    }
    return readFileSync(location, "utf8");
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

function normalize(source) {
  return source.replace(/\s+/gu, " ").trim();
}

function requireValue(condition, message) {
  if (!condition) failures.push(message);
}

function requireText(source, marker, label) {
  if (!source.includes(marker) && !normalize(source).includes(normalize(marker))) {
    failures.push(`${label}: missing '${marker}'`);
  }
}

function forbidText(source, marker, label) {
  if (source.includes(marker)) failures.push(`${label}: forbidden '${marker}'`);
}

const contractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-execution-source.json";
const reviewContractPath =
  "crates/rustok-forum/contracts/evidence/forum-page-builder-ffa-fba-promotion-review-source.json";
const runnerPath = "scripts/evidence/execute-forum-page-builder-ffa-fba-promotion.mjs";
const testsPath = "scripts/evidence/execute-forum-page-builder-ffa-fba-promotion.test.mjs";
const resolverPath = "apps/server/src/graphql/module_settings_cas.rs";
const servicePath = "apps/server/src/services/module_rollout_promotion_settings.rs";
const lifecyclePath = "crates/rustok-modules/src/lifecycle_writer.rs";
const storePath = "crates/rustok-modules/src/operation_store.rs";
const snapshotPath = "crates/rustok-pages/src/graphql/builder_rollout.rs";
const actualizationPath =
  "docs/modules/forum-page-builder-ffa-fba-promotion-review-actualization-2026-08-12.md";
const planPath = "docs/modules/pages-page-builder-parity-continuation-plan.md";

const contract = json(contractPath);
const reviewContract = json(reviewContractPath);
const runner = read(runnerPath);
const tests = read(testsPath);
const resolver = read(resolverPath);
const service = read(servicePath);
const lifecycle = read(lifecyclePath);
const store = read(storePath);
const snapshot = read(snapshotPath);
const actualization = read(actualizationPath);
const plan = read(planPath);

requireValue(
  contract.format === "forum_page_builder_ffa_fba_promotion_execution_source_v1" &&
    contract.status === "source_ready_maintainer_execution_pending" &&
    contract.module === "forum" &&
    contract.wave === "1",
  `${contractPath}: identity drifted`,
);
requireValue(
  contract.predecessor?.format === "forum_page_builder_ffa_fba_promotion_review_v1" &&
    contract.predecessor?.required_status ===
      "owner_approved_ffa_fba_promotion_review_execution_pending" &&
    contract.predecessor?.source_commit_must_equal_checkout_head === true &&
    contract.predecessor?.deployment_image_digest_must_be_canonical_repo_digest === true &&
    contract.predecessor?.promotion_decision_must_equal ===
      "approve_ffa_fba_promotion_review" &&
    JSON.stringify(contract.predecessor?.targets_must_equal) ===
      JSON.stringify(["ffa", "fba"]) &&
    contract.predecessor?.separate_control_plane_execution_required_must_be_true === true &&
    contract.predecessor?.wave_next_due_at_must_still_be_future_at_execution === true,
  `${contractPath}: approved-review predecessor boundary drifted`,
);
requireValue(
  contract.target?.settings_module_slug === "pages" &&
    contract.target?.promotion_profile === "all_on" &&
    contract.target?.graphql_path === "/api/graphql" &&
    JSON.stringify(contract.target?.required_permissions) ===
      JSON.stringify(["modules:manage", "pages:read"]) &&
    contract.target?.read_operation === "tenantModules" &&
    contract.target?.write_operation === "compareAndSwapModuleSettings" &&
    contract.target?.postcondition_operation === "pageBuilderRolloutSnapshot" &&
    contract.target?.direct_sql_allowed === false &&
    contract.target?.raw_database_access_allowed === false &&
    contract.target?.preserve_non_builder_settings === true,
  `${contractPath}: target/CAS authority drifted`,
);
for (const environment of [
  "RUSTOK_FORUM_FFA_FBA_PROMOTION_API_ORIGIN",
  "RUSTOK_FORUM_FFA_FBA_PROMOTION_TENANT_SLUG",
  "RUSTOK_FORUM_FFA_FBA_PROMOTION_AUTH_TOKEN",
  "RUSTOK_FORUM_FFA_FBA_PROMOTION_DEPLOYMENT_IMAGE_DIGEST",
]) {
  requireValue(
    Object.values(contract.target ?? {}).includes(environment),
    `${contractPath}: missing target environment ${environment}`,
  );
  requireText(runner, environment, runnerPath);
}
requireValue(
  contract.mutation?.expected_enabled === true &&
    contract.mutation?.expected_lifecycle_revision ===
      "exact current Pages static lifecycle aggregate revision" &&
    contract.mutation?.idempotency_key === "fresh UUID for each mutation attempt" &&
    contract.mutation?.cas_conflict_code === "MODULE_SETTINGS_SNAPSHOT_CONFLICT" &&
    contract.mutation?.cas_conflict_requires_rereview === true &&
    contract.mutation?.already_at_target_is_not_new_execution_evidence === true &&
    contract.mutation?.ordinary_update_module_settings_forbidden === true &&
    Object.values(contract.mutation?.desired_flags ?? {}).every((value) => value === true),
  `${contractPath}: reviewed all_on mutation contract drifted`,
);
for (const key of [
  "attempt_on_confirmed_mutation_postcondition_failure",
  "expected_settings_must_equal_confirmed_applied_settings",
  "expected_lifecycle_revision_must_equal_confirmed_applied_revision",
  "restore_settings_must_equal_original_snapshot",
  "rollback_cas_conflict_requires_manual_reconciliation",
  "ambiguous_transport_outcome_must_not_auto_rollback",
  "rollback_success_still_fails_execution",
  "no_unconditional_overwrite_allowed",
]) {
  requireValue(contract.rollback?.[key] === true, `${contractPath}: ${key} must remain true`);
}
requireValue(
  contract.rollback?.write_operation === "compareAndSwapModuleSettings",
  `${contractPath}: rollback must remain CAS-only`,
);
requireValue(
  contract.output?.format === "forum_page_builder_ffa_fba_promotion_execution_v1" &&
    contract.output?.success_status ===
      "control_plane_change_executed_readiness_promotion_pending" &&
    contract.output?.rolled_back_status ===
      "control_plane_change_postcondition_failed_rolled_back" &&
    contract.output?.manual_reconciliation_status ===
      "control_plane_change_requires_manual_reconciliation" &&
    contract.output?.snapshot_conflict_status ===
      "control_plane_change_snapshot_conflict_rereview_required" &&
    contract.output?.raw_settings_retained === false &&
    contract.output?.raw_graphql_request_or_response_retained === false &&
    contract.output?.authorization_or_cookie_values_retained === false,
  `${contractPath}: bounded output contract drifted`,
);
for (const key of [
  "successful_control_plane_change_is_not_ffa_readiness_promotion",
  "successful_control_plane_change_is_not_fba_readiness_promotion",
  "ffa_parity_verified_requires_separate_evidence_backed_governance_change",
  "fba_transport_verified_requires_separate_evidence_backed_governance_change",
]) {
  requireValue(contract.readiness_boundary?.[key] === true, `${contractPath}: ${key} must remain true`);
}
for (const key of [
  "registry_or_local_plan_status_mutation_by_runner",
  "canonical_source_mutation_by_runner",
]) {
  requireValue(contract.readiness_boundary?.[key] === false, `${contractPath}: ${key} must remain false`);
}

requireValue(
  reviewContract.output?.format === "forum_page_builder_ffa_fba_promotion_review_v1" &&
    reviewContract.output?.approved_status ===
      "owner_approved_ffa_fba_promotion_review_execution_pending" &&
    reviewContract.execution_boundary?.actual_ffa_fba_promotion_remains_separate_maintainer_execution ===
      true,
  `${reviewContractPath}: approved execution predecessor drifted`,
);

for (const marker of [
  "--promotion-review",
  "tenantModules",
  "compareAndSwapModuleSettings",
  "pageBuilderRolloutSnapshot",
  "MODULE_SETTINGS_SNAPSHOT_CONFLICT",
  "requires_rereview === true",
  "Pages settings already match all_on",
  "ambiguous_mutation_outcome_must_not_auto_rollback",
  "confirmed_restored",
  "readinessBoundary()",
  "ffa_promoted: false",
  "fba_promoted: false",
  "raw_module_settings_persisted: false",
  "cryptographic_origin_to_repo_digest_binding_claimed: false",
]) requireText(runner, marker, runnerPath);
forbidText(runner, "updateModuleSettings(", runnerPath);
forbidText(runner, "UPDATE tenant_modules", runnerPath);
forbidText(runner, "SELECT * FROM tenant_modules", runnerPath);

for (const label of [
  "executes approved all_on promotion through CAS and preserves unrelated settings",
  "rejects non-approved promotion review before target requests",
  "rejects promotion review source-commit drift before target requests",
  "rejects promotion review deployment RepoDigest drift before target requests",
  "rejects stale observed Wave lease before target requests",
  "rejects already-all_on target as non-evidence without mutation",
  "records CAS snapshot conflict and requires re-review without rollback",
  "rolls back confirmed mutation when postcondition fails and retains rolled-back receipt",
  "records manual reconciliation when rollback CAS conflicts",
  "records ambiguous mutation without automatic rollback",
]) requireText(tests, label, testsPath);

for (const marker of [
  "async fn compare_and_swap_module_settings",
  "Permission::MODULES_MANAGE",
  "MODULE_SETTINGS_SNAPSHOT_CONFLICT",
  "requires_rereview",
]) requireText(resolver, marker, resolverPath);
for (const marker of [
  "ModuleRolloutPromotionSettingsService",
  "update_if_current",
  "ModuleRolloutPromotionSettingsOutcome::Conflict",
  "update_static_normalized_settings",
  "ModuleLifecycleSettingsCommand",
]) requireText(service, marker, servicePath);
requireText(lifecycle, "update_static_normalized_settings", lifecyclePath);
for (const marker of [
  "StaticTenantLifecycleStore",
  "active_idempotency_key",
  "expected_revision",
]) requireText(store, marker, storePath);
for (const marker of [
  "page_builder_rollout_snapshot",
  "Permission::PAGES_READ",
  "BuilderCapabilityFlags::from_module_settings",
]) requireText(snapshot, marker, snapshotPath);

for (const marker of [
  "forum-ffa-fba-promotion-execution-source-ready",
  "owner_approved_ffa_fba_promotion_review_execution_pending",
  "compareAndSwapModuleSettings",
  "MODULE_SETTINGS_SNAPSHOT_CONFLICT",
  "ambiguous initial mutation outcome",
  "successful tenant/control-plane write is **not** the FFA/FBA readiness-board promotion",
  "control_plane_change_executed_readiness_promotion_pending",
  "maintainer live execution remains pending",
]) requireText(actualization, marker, actualizationPath);
requireText(plan, "separate explicit FFA/FBA promotion review", planPath);
requireText(plan, "accepted observed-Wave owner evidence", planPath);

if (failures.length > 0) {
  console.error("[verify-forum-page-builder-ffa-fba-promotion-execution] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("[verify-forum-page-builder-ffa-fba-promotion-execution] PASS");
console.log(
  "module=forum; wave=1; control_plane_execution=source_ready_blocked_on_approved_review; readiness_promotion=separate_governance_pending",
);
