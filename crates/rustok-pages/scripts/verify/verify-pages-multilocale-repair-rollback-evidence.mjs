#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  service: "crates/rustok-pages/src/services/page/artifact_set.rs",
  rollback: "crates/rustok-pages/src/services/page/rollback.rs",
  activation: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  test: "crates/rustok-pages/tests/artifact_multilocale_repair_rollback_evidence_postgres.rs",
  repeatedTest: "crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-multilocale-repair-rollback-evidence-source.json",
  latestOverlay: "docs/modules/pages-page-builder-repeated-artifact-loss-recovery-actualization-2026-08-07.md",
  fba: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const requireOrdered = (source, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = source.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
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
  console.error("[verify-pages-multilocale-repair-rollback-evidence] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_multilocale_repair_rollback_evidence_source_v3") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_multilocale_repair_rollback_latest_state_evidence_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
for (const key of [
  "strict_publish_manifest_is_tried_first",
  "fallback_only_handles_current_repaired_cursor",
  "database_errors_are_not_masked",
  "current_artifact_set_must_match_source_publish_hash",
  "complete_retained_publish_provenance_required",
  "surviving_manifest_rows_remain_authoritative",
  "unchanged_locales_require_original_manifest_rows",
  "missing_repaired_manifest_requires_source_artifact_absent",
  "current_repaired_artifact_requires_exact_rebuild_receipt",
  "current_repaired_artifact_requires_exact_activation_receipt",
  "activation_request_hash_is_recomputed_canonically",
  "physical_loss_activation_prefix_uses_publish_or_exact_rollback_activation_anchor",
  "physical_loss_activation_prefix_is_bounded_to_256_receipts",
  "physical_loss_activation_prefix_query_is_bounded_to_257_rows",
  "physical_loss_activation_prefix_requires_contiguous_expected_and_result_versions",
  "physical_loss_activation_prefix_tracks_latest_repair_state_per_locale",
  "physical_loss_activation_prefix_allows_repeat_only_after_prior_rebuilt_artifact_absence",
  "physical_loss_activation_prefix_requires_exact_same_publish_sources",
  "physical_loss_activation_prefix_revalidates_rebuild_receipts",
  "physical_loss_activation_prefix_proves_required_locale_with_current_replacement_artifact_id",
  "physical_loss_activation_prefix_revalidates_latest_current_rebuilt_artifact_identity",
  "physical_loss_activation_prefix_stops_after_all_current_lost_manifest_locales_are_proven",
  "historical_rollback_targets_still_require_original_manifest_and_live_artifacts",
  "postgres_repeated_loss_rollback_success_source_ready",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`evidence source_contract.${key} must be true`);
  }
}
for (const key of [
  "tests_run",
  "static_verifier_run",
  "cargo_run",
  "formatting_run",
  "migration_run",
  "database_scenario_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`evidence source_contract.${key} must remain false`);
  }
}

for (const marker of [
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_FORMAT",
  "PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT",
  "MAX_RECOVERED_ACTIVATION_PREFIX",
  "physically_lost_manifest_locales",
  "verify_physical_loss_activation_prefix_in_tx",
  "current_members: &[ArtifactSetMember]",
  "required_current_artifacts",
  "let mut latest_by_locale",
  "let mut proven_required_locales",
  "recovery_artifact_if_present_for_rollback_in_tx",
  "repeated a locale while its prior rebuilt artifact still exists",
  "required_locales.is_subset(&proven_required_locales)",
  "activation.replacement_artifact_id",
  "latest rebuilt artifact drifted from its receipt",
  "activation.request_hash != expected_request_hash",
  "source_artifact_exists_in_tx",
]) {
  need(sources.service, marker, "rollback recovery service");
}
requireOrdered(sources.service, [
  "load_strict_publish_manifest_in_tx(txn, operation).await",
  "load_recovered_current_publish_set_in_tx(txn, operation).await",
], "strict manifest before recovery fallback");
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "PagesCacheInvalidationRuntime",
  "GraphQL",
  "OpenAPI",
]) {
  forbid(sources.service, marker, "rollback recovery boundary");
}

for (const marker of [
  "find_previous_publish_target_in_tx",
  "operation.artifact_set_hash == current_artifact_set_hash",
  "load_publish_manifest_in_tx(txn, &operation).await?",
]) {
  need(sources.rollback, marker, "historical target boundary");
}
for (const marker of [
  "MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  "latest_by_locale",
  "recovery_artifact_if_present_in_tx",
  "a repeated locale still has its prior rebuilt immutable artifact",
  "operation.request_hash != expected_request_hash",
]) {
  need(sources.activation, marker, "activation admission source");
}
for (const marker of [
  "rollback_continues_after_two_locale_physical_loss_recovery_on_postgres",
  "rollback_rejects_repaired_cursor_with_noncanonical_activation_request_hash_on_postgres",
  "rollback_rejects_individually_valid_but_noncontiguous_activation_prefix_on_postgres",
  "assert_rollback_rejected_without_binding_change",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "existing rollback PostgreSQL packet");
}
for (const marker of [
  "rollback_continues_after_same_locale_is_recovered_twice_on_postgres",
  "remove_current_rebuilt_binding_and_artifact",
  "rollback-after-repeated-loss-v1",
]) {
  need(sources.repeatedTest, marker, "repeated-loss rollback packet");
}
for (const marker of [
  "Repeated Artifact-Loss Recovery Actualization",
  "latest-state-per-locale",
  "rollback reconstruction",
  "execution remains pending",
]) {
  need(sources.latestOverlay, marker, "latest parity overlay");
}
for (const marker of [
  "latest_repair_state_per_locale",
  "repeated_locale_recovery_supported",
  "pages_repeated_artifact_loss_recovery_verifier",
]) {
  need(sources.fba, marker, "Page Builder FBA registry");
}

if (failures.length > 0) {
  console.error("[verify-pages-multilocale-repair-rollback-evidence] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-multilocale-repair-rollback-evidence] PASS");
