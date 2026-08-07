#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  service: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  migration: "crates/rustok-pages/src/migrations/m20260807_000015_create_page_artifact_binding_replacements.rs",
  test: "crates/rustok-pages/tests/artifact_loss_multilocale_activation_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
  actualization: "docs/modules/pages-page-builder-multilocale-activation-recovery-actualization-2026-08-07.md",
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
  console.error("[verify-pages-artifact-loss-multilocale-activation-recovery-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v3") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_multilocale_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
for (const key of [
  "missing_binding_first_recovery_accepts_publish_result_version_equal_current_expected",
  "missing_binding_sequential_recovery_requires_contiguous_activation_version_chain",
  "missing_binding_sequential_recovery_requires_same_source_publish",
  "missing_binding_sequential_recovery_requires_other_unique_locales",
  "missing_binding_sequential_recovery_revalidates_prior_rebuild_and_provenance",
  "missing_binding_sequential_recovery_recomputes_prior_activation_request_hash",
  "missing_binding_sequential_recovery_requires_prior_repaired_binding_still_active",
  "missing_binding_sequential_recovery_requires_prior_rebuilt_artifact_identity",
  "missing_binding_sequential_recovery_rejects_unexplained_version_gap",
  "missing_binding_sequential_recovery_rejects_prior_activation_for_target_locale",
  "missing_binding_sequential_recovery_is_bounded",
  "postgres_multilocale_recovery_harness_source_ready",
  "postgres_multilocale_success_source_ready",
  "postgres_unexplained_version_drift_rejection_source_ready",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`evidence source_contract.${key} must be true`);
  }
}

for (const marker of [
  "MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  "publish.result_version > expected_version",
  "publish.result_version < expected_version",
  "checked_sub(publish.result_version)",
  "operations.len() != version_gap",
  "operation.expected_version != cursor",
  "operation.result_version != cursor + 1",
  "operation.locale == target_locale",
  "prior_locales.insert(operation.locale.clone())",
  "stable_replacement_hash(&(",
  "operation.request_hash != expected_request_hash",
  "load_rebuild_operation_in_tx",
  "load_rebuild_source_in_tx",
  "ensure_rebuild_matches_source(&prior_rebuild, &prior_source)?",
  "prior_rebuild.source_publish_operation_id != publish.id",
  "prior_source.operation_id != publish.id",
  "operation.expected_current_artifact_id != prior_rebuild.source_artifact_id",
  "operation.replacement_artifact_id != prior_rebuild.rebuilt_artifact_id",
  "load_binding_for_update_in_tx",
  "prior_binding.artifact_id != prior_rebuild.rebuilt_artifact_id",
  "prior_artifact.instance_key != prior_rebuild.artifact_instance_key",
  "cursor = operation.result_version",
  "cursor != expected_version",
]) {
  need(sources.service, marker, "sequential recovery service");
}
requireOrdered(
  sources.service,
  [
    "publish.result_version > expected_version",
    "publish.result_version < expected_version",
    "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  ],
  "direct-version then sequential-version admission",
);
requireOrdered(
  sources.service,
  [
    "operations.len() != version_gap",
    "let mut cursor = publish.result_version",
    "for operation in operations",
    "load_rebuild_operation_in_tx",
    "load_rebuild_source_in_tx",
    "load_binding_for_update_in_tx",
    "cursor = operation.result_version",
    "cursor != expected_version",
  ],
  "sequential receipt verification ordering",
);
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "page_body::Column::Content",
  "body.content",
  "delete_by_id(rebuild.source_artifact_id)",
  "PagesCacheInvalidationRuntime",
]) {
  forbid(sources.service, marker, "sequential recovery boundary");
}

for (const marker of [
  "idx_page_artifact_binding_replacements_result",
  "ResultVersion",
]) {
  need(sources.migration, marker, "existing receipt index");
}

for (const marker of [
  "missing_binding_activation_recovers_two_lost_locales_sequentially_on_postgres",
  "missing_binding_activation_rejects_unexplained_version_between_locales_on_postgres",
  "remove_binding_manifest_and_source_artifact",
  "multilocale-activate-en-v1",
  "expected_version: en_activation.version",
  "fr_activation.version, fixture.publish_version + 2",
  "events_before_activation + 4",
  "advanced.version = Set(en_activation.version + 1)",
  "not fully explained",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "multi-locale PostgreSQL packet source");
}

for (const marker of [
  "Sequential multi-locale version chain",
  "bounded to at most 256 prior activation steps",
  "every prior locale is unique and different from the locale currently being recovered",
  "unexplained lifecycle/version increment",
  "artifact_loss_multilocale_activation_recovery_postgres.rs",
]) {
  need(sources.packet, marker, "recovery packet");
}
for (const marker of [
  "Multi-Locale Artifact-Loss Activation Recovery Actualization",
  "multilocale-missing-binding-recovery-source-ready",
  "same-publish activation chain",
  "Unexplained version drift remains fail-closed",
]) {
  need(sources.actualization, marker, "parity actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-multilocale-activation-recovery-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-artifact-loss-multilocale-activation-recovery-postgres] PASS");
