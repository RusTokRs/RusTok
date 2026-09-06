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
  rollbackTest: "crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs",
  repeatedTest: "crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
  latestOverlay: "docs/modules/pages-page-builder-repeated-artifact-loss-recovery-actualization-2026-08-07.md",
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
if (evidence.format !== "pages_explicit_artifact_binding_replacement_source_v5") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_binding_replacement_repeated_loss_recovery_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
for (const key of [
  "missing_binding_direct_publish_anchor_accepts_publish_result_version_equal_current_expected",
  "missing_binding_rollback_activation_anchor_is_supported",
  "missing_binding_sequential_recovery_requires_contiguous_activation_version_chain",
  "missing_binding_sequential_recovery_requires_same_source_publish",
  "missing_binding_sequential_recovery_tracks_latest_repair_state_per_locale",
  "missing_binding_sequential_recovery_allows_repeated_locale_only_after_prior_rebuilt_artifact_absence",
  "missing_binding_sequential_recovery_revalidates_prior_rebuild_and_provenance",
  "missing_binding_sequential_recovery_recomputes_prior_activation_request_hash",
  "missing_binding_sequential_recovery_requires_latest_non_target_binding_still_active",
  "missing_binding_sequential_recovery_requires_latest_non_target_rebuilt_artifact_identity",
  "missing_binding_sequential_recovery_requires_target_binding_absent",
  "missing_binding_sequential_recovery_requires_latest_target_prior_rebuilt_artifact_absent_before_repeat",
  "missing_binding_sequential_recovery_rejects_unexplained_version_gap",
  "missing_binding_sequential_recovery_is_bounded",
  "missing_binding_sequential_recovery_query_is_physically_bounded",
  "postgres_multilocale_recovery_harness_source_ready",
  "postgres_multilocale_success_source_ready",
  "postgres_unexplained_version_drift_rejection_source_ready",
  "postgres_rollback_activated_multilocale_success_source_ready",
  "postgres_repeated_loss_recovery_harness_source_ready",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`evidence source_contract.${key} must be true`);
  }
}

for (const marker of [
  "MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS",
  "resolve_missing_binding_recovery_anchor_in_tx",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  "checked_sub(anchor_version)",
  ".gt(anchor_version)",
  ".limit((MAX_SEQUENTIAL_RECOVERY_ACTIVATIONS + 1) as u64)",
  "operations.len() != version_gap",
  "let mut cursor = anchor_version",
  "let mut latest_by_locale",
  "operation.expected_version != cursor",
  "operation.result_version != cursor + 1",
  "operation.request_hash != expected_request_hash",
  "ensure_rebuild_matches_source(&prior_rebuild, &prior_source)?",
  "prior_rebuild.source_publish_operation_id != publish.id",
  "recovery_artifact_if_present_in_tx",
  "a repeated locale still has its prior rebuilt immutable artifact",
  "target locale prior rebuilt immutable artifact still exists",
  "latest repaired locale binding is no longer active",
  "latest repaired immutable artifact drifted from its rebuild receipt",
  "cursor != expected_version",
]) {
  need(sources.service, marker, "sequential recovery service");
}
requireOrdered(sources.service, [
  "publish.result_version > expected_version",
  "let anchor_version = if publish.result_version == expected_version",
  "resolve_missing_binding_recovery_anchor_in_tx",
  "if anchor_version < expected_version",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
], "publish-or-rollback then sequential admission");
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

for (const marker of ["idx_page_artifact_binding_replacements_result", "ResultVersion"]) {
  need(sources.migration, marker, "existing receipt index");
}
for (const marker of [
  "missing_binding_activation_recovers_two_lost_locales_sequentially_on_postgres",
  "missing_binding_activation_rejects_unexplained_version_between_locales_on_postgres",
  "expected_version: en_activation.version",
  "fr_activation.version, fixture.publish_version + 2",
  "not fully explained",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "direct multi-locale PostgreSQL packet source");
}
for (const marker of [
  "rollback_activated_publish_recovers_two_lost_locales_sequentially_on_postgres",
  "expected_version: fixture.rollback_version",
  "expected_version: en_activation.version",
]) {
  need(sources.rollbackTest, marker, "rollback-activated multi-locale source");
}
for (const marker of [
  "missing_binding_activation_recovers_same_locale_after_rebuilt_artifact_is_lost_again_on_postgres",
  "repeated_locale_recovery_rejects_missing_binding_while_prior_rebuilt_artifact_still_exists_on_postgres",
  "another_locale_can_recover_after_repeated_locale_loss_on_postgres",
]) {
  need(sources.repeatedTest, marker, "repeated-loss PostgreSQL packet source");
}
for (const marker of [
  "Sequential multi-locale and repeated-loss version chain",
  "latest repair state per locale",
  "prior rebuilt instance is physically absent",
  "artifact_repeated_loss_recovery_postgres.rs",
]) {
  need(sources.packet, marker, "recovery packet");
}
for (const marker of [
  "Repeated Artifact-Loss Recovery Actualization",
  "latest-state-per-locale",
  "execution remains pending",
]) {
  need(sources.latestOverlay, marker, "latest parity actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-multilocale-activation-recovery-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-artifact-loss-multilocale-activation-recovery-postgres] PASS");
