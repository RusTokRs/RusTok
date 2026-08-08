#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  activation: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  artifactSet: "crates/rustok-pages/src/services/page/artifact_set.rs",
  test: "crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-repeated-artifact-loss-recovery-source.json",
  plan: "crates/rustok-pages/docs/implementation-plan.md",
  overlay: "docs/modules/pages-page-builder-repeated-artifact-loss-recovery-actualization-2026-08-07.md",
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
  console.error("[verify-pages-repeated-artifact-loss-recovery] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
if (evidence.format !== "pages_repeated_artifact_loss_recovery_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_repeated_artifact_loss_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`evidence validation.${key} must remain false`);
}
for (const key of [
  "retained_publish_provenance_remains_rebuild_authority",
  "historical_source_artifact_identity_remains_activation_request_fence",
  "missing_binding_recovery_still_requires_original_source_artifact_absent",
  "repeated_locale_recovery_is_supported",
  "repeated_locale_requires_prior_rebuilt_artifact_physically_absent",
  "repeated_locale_requires_target_binding_absent",
  "post_anchor_chain_tracks_latest_rebuild_per_locale",
  "latest_non_target_repaired_binding_must_remain_active",
  "latest_non_target_rebuilt_artifact_must_remain_present_and_exact",
  "rollback_reconstruction_tracks_latest_rebuild_per_locale",
  "rollback_reconstruction_proves_current_artifact_not_first_locale_occurrence",
  "rollback_reconstruction_requires_superseded_rebuilt_instance_absent_before_repeat",
  "postgres_same_locale_second_loss_success_source_ready",
  "postgres_live_prior_rebuilt_artifact_rejection_source_ready",
  "postgres_other_locale_after_repeat_success_source_ready",
  "postgres_rollback_after_repeated_recovery_success_source_ready",
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
  "latest_by_locale",
  "recovery_artifact_if_present_in_tx",
  "a repeated locale still has its prior rebuilt immutable artifact",
  "target locale binding unexpectedly became active before repeated recovery",
  "target locale prior rebuilt immutable artifact still exists",
  "latest repaired locale binding is no longer active",
  "latest repaired immutable artifact drifted from its rebuild receipt",
  "operation.expected_current_artifact_id != prior_rebuild.source_artifact_id",
  ".limit((MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS + 1) as u64)",
]) {
  need(sources.activation, marker, "activation repeated-loss lineage");
}
for (const marker of [
  "required_current_artifacts",
  "proven_required_locales",
  "latest_by_locale",
  "recovery_artifact_if_present_for_rollback_in_tx",
  "repeated a locale while its prior rebuilt artifact still exists",
  "activation.replacement_artifact_id",
  "required_locales.is_subset(&proven_required_locales)",
  "latest rebuilt artifact drifted from its receipt",
  ".limit((MAX_RECOVERED_ACTIVATION_PREFIX + 1) as u64)",
]) {
  need(sources.artifactSet, marker, "rollback repeated-loss lineage");
}
for (const marker of [
  "missing_binding_activation_recovers_same_locale_after_rebuilt_artifact_is_lost_again_on_postgres",
  "repeated_locale_recovery_rejects_missing_binding_while_prior_rebuilt_artifact_still_exists_on_postgres",
  "another_locale_can_recover_after_repeated_locale_loss_on_postgres",
  "rollback_continues_after_same_locale_is_recovered_twice_on_postgres",
  "remove_current_rebuilt_binding_and_artifact",
  "prior rebuilt immutable artifact still exists",
  "rollback-after-repeated-loss-v1",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "PostgreSQL repeated-loss packet");
}
for (const marker of [
  "Repeated Artifact-Loss Recovery Actualization",
  "latest-state-per-locale",
  "physically absent",
  "execution remains pending",
]) {
  need(sources.overlay, marker, "repeated-loss overlay");
}
for (const marker of [
  "repeated artifact-loss recovery",
  "latest repair state per locale",
  "artifact_repeated_loss_recovery_postgres.rs",
  "verify-pages-repeated-artifact-loss-recovery.mjs",
]) {
  need(sources.plan, marker, "Pages implementation plan");
}
for (const marker of [
  "repeated_locale_recovery_supported",
  "latest_repair_state_per_locale",
  "pages_repeated_artifact_loss_recovery_verifier",
]) {
  need(sources.fba, marker, "Page Builder FBA registry");
}
for (const marker of [
  "automatic_audit_to_repair",
  "compile_materialized_static_landing",
  "sanitize_static_landing_project",
]) {
  forbid(sources.activation, marker, "activation owner boundary");
}

if (failures.length > 0) {
  console.error("[verify-pages-repeated-artifact-loss-recovery] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-repeated-artifact-loss-recovery] PASS");
