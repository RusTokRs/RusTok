#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  artifactSet: "crates/rustok-pages/src/services/page/artifact_set.rs",
  activation: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  rollback: "crates/rustok-pages/src/services/page/rollback.rs",
  test: "crates/rustok-pages/tests/artifact_rollback_activated_repair_rollback_continuity_postgres.rs",
  repeatedTest: "crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-rollback-activated-repair-rollback-continuity-source.json",
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
  console.error("[verify-pages-rollback-activated-repair-rollback-continuity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_rollback_activated_repair_rollback_continuity_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_rollback_activated_repair_rollback_continuity_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
for (const key of [
  "strict_publish_manifest_is_always_attempted_first",
  "repair_fallback_is_current_cursor_only",
  "database_errors_are_not_masked_by_repair_fallback",
  "physical_loss_prefix_uses_publish_or_exact_rollback_activation_anchor",
  "direct_publish_without_matching_rollback_falls_back_to_publish_result_version",
  "rollback_anchor_requires_exact_target_publish_operation",
  "rollback_anchor_requires_exact_target_artifact_set_hash",
  "rollback_anchor_request_hash_is_recomputed",
  "rollback_anchor_expected_version_is_derived_from_result_version_minus_one",
  "latest_matching_rollback_anchor_is_selected",
  "physical_loss_prefix_starts_after_resolved_anchor",
  "physical_loss_prefix_remains_bounded_to_256_receipts",
  "physical_loss_prefix_query_remains_bounded_to_257_rows",
  "historical_rollback_targets_still_require_original_manifest_and_live_artifacts",
  "postgres_three_publish_success_source_ready",
  "postgres_corrupted_rollback_anchor_hash_rejection_source_ready",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
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
    failures.push(`source_contract.${key} must remain false`);
  }
}

for (const marker of [
  'PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT: &str = "page_rollback_operation_v1"',
  "resolve_repair_activation_anchor_in_tx",
  "TargetPublishOperationId.eq(operation.id)",
  "TargetArtifactSetHash",
  "rollback.request_hash != expected_request_hash",
  "resolve_repair_activation_anchor_in_tx(txn, operation, current_page_version).await?",
  ".gt(anchor_version)",
  "let mut cursor = anchor_version",
  ".limit((MAX_RECOVERED_ACTIVATION_PREFIX + 1) as u64)",
  "required_current_artifacts",
  "proven_required_locales",
  "required_locales.is_subset(&proven_required_locales)",
  "recovery_artifact_if_present_for_rollback_in_tx",
]) {
  need(sources.artifactSet, marker, "artifact-set recovery source");
}
for (const marker of [
  'PAGE_ROLLBACK_ACTIVATION_ANCHOR_FORMAT: &str = "page_rollback_operation_v1"',
  "resolve_missing_binding_recovery_anchor_in_tx",
  "TargetPublishOperationId.eq(publish.id)",
  "rollback.request_hash != expected_request_hash",
  "latest_by_locale",
]) {
  need(sources.activation, marker, "activation admission source");
}
for (const marker of [
  "find_current_publish_cursor_in_tx",
  "rollback.target_publish_operation_id",
  "load_publish_manifest_in_tx(txn, &cursor).await?",
]) {
  need(sources.rollback, marker, "rollback cursor source");
}
for (const marker of [
  "rollback_continues_after_rollback_activated_physical_loss_repair_on_postgres",
  "rollback_rejects_repaired_cursor_when_rollback_activation_anchor_hash_is_corrupted_on_postgres",
  "publish-p0-v1",
  "publish-p1-v1",
  "publish-p2-v1",
  "rollback-p2-to-p1-v1",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "rollback-anchor PostgreSQL packet");
}
need(
  sources.repeatedTest,
  "rollback_continues_after_same_locale_is_recovered_twice_on_postgres",
  "repeated-loss rollback regression",
);
for (const marker of [
  "rollback_activated_repair_to_rollback",
  "physical_loss_activation_prefix_anchor",
  "latest_repair_state_per_locale",
  "pages_rollback_activated_repair_rollback_continuity_verifier",
]) {
  need(sources.fba, marker, "Page Builder FBA registry");
}
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "PagesCacheInvalidationRuntime",
  "GraphQL",
  "OpenAPI",
]) {
  forbid(sources.artifactSet, marker, "rollback reconstruction boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-rollback-activated-repair-rollback-continuity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-rollback-activated-repair-rollback-continuity] PASS");
