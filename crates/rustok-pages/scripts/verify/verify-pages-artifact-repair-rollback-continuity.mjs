#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  artifactSet: "crates/rustok-pages/src/services/page/artifact_set.rs",
  rollback: "crates/rustok-pages/src/services/page/rollback.rs",
  manifestMigration: "crates/rustok-pages/src/migrations/m20260722_000009_create_page_rollback_operations.rs",
  test: "crates/rustok-pages/tests/artifact_repair_rollback_continuity_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-artifact-repair-rollback-continuity-source.json",
  packet: "crates/rustok-pages/docs/artifact-repair-rollback-continuity.md",
  actualization: "docs/modules/pages-page-builder-repair-rollback-continuity-actualization-2026-08-07.md",
  registry: "crates/rustok-page-builder/contracts/page-builder-fba-registry.json",
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
  console.error("[verify-pages-artifact-repair-rollback-continuity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const registry = JSON.parse(sources.registry);
const pages = registry.consumers?.find((consumer) => consumer.module_slug === "pages");

if (evidence.format !== "pages_artifact_repair_rollback_continuity_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_artifact_repair_rollback_continuity_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("evidence validation must remain unexecuted");
}
for (const key of [
  "strict_publish_manifest_is_tried_first",
  "fallback_only_handles_rollback_target_unavailable",
  "database_errors_are_not_masked",
  "fallback_requires_current_published_page",
  "fallback_requires_current_artifact_set_hash_to_equal_publish_receipt",
  "fallback_requires_complete_retained_publish_provenance",
  "fallback_revalidates_provenance_hashes",
  "fallback_requires_surviving_manifest_rows_to_match_retained_provenance",
  "fallback_requires_unchanged_locales_to_retain_original_manifest_rows",
  "fallback_requires_missing_repaired_manifest_row_to_have_absent_source_artifact",
  "fallback_requires_at_least_one_explicitly_rebuilt_and_activated_locale",
  "fallback_requires_exact_rebuild_receipt_for_replaced_artifact",
  "fallback_requires_exact_activation_receipt_for_replaced_artifact",
  "fallback_requires_activation_after_source_publish_and_not_after_current_page_version",
  "historical_rollback_targets_still_require_original_manifest",
  "historical_target_missing_manifest_is_rejected",
  "surviving_manifest_identity_mismatch_is_rejected",
  "missing_current_manifest_with_live_source_artifact_is_rejected",
  "physical_loss_rebuild_activation_then_rollback_is_source_covered",
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
  "postgres_scenario_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`evidence source_contract.${key} must remain false`);
  }
}

for (const marker of [
  "load_strict_publish_manifest_in_tx",
  "load_publish_manifest_rows_in_tx",
  "load_recovered_current_publish_set_in_tx",
  "Err(PagesError::RollbackTargetUnavailable(strict_message))",
  "Err(PagesError::Database(error)) => Err(PagesError::Database(error))",
  "artifact_set_hash(&current_members)? != operation.artifact_set_hash",
  "page_publish_rebuild_source::Entity::find()",
  "verify_rebuild_source_for_rollback",
  "rebuild_source_provenance_hash",
  "let surviving_manifest = load_publish_manifest_rows_in_tx",
  "surviving publish manifest locale",
  "unchanged locale",
  "source_artifact_exists_in_tx",
  "is missing its manifest while the source artifact still exists",
  "let mut repaired_locales = 0usize",
  "repaired_locales += 1",
  "repaired_locales == 0",
  "requires at least one explicitly rebuilt and activated locale",
  "load_rebuild_for_current_artifact_in_tx",
  "load_activation_for_current_artifact_in_tx",
  "activation.result_version <= operation.result_version",
  "activation.result_version > current_page.version",
  "current.artifact_id == source.artifact_id",
]) {
  need(sources.artifactSet, marker, "artifact-set owner");
}
requireOrdered(
  sources.artifactSet,
  [
    "load_strict_publish_manifest_in_tx(txn, operation).await",
    "Err(PagesError::RollbackTargetUnavailable(strict_message))",
    "load_recovered_current_publish_set_in_tx(txn, operation).await",
  ],
  "strict-manifest-first ordering",
);
for (const marker of [
  "load_publish_manifest_rows_in_tx(txn, operation).await?",
  "verify_members_in_tx",
  "artifact manifest failed hash validation",
]) {
  need(sources.artifactSet, marker, "strict manifest path");
}
for (const marker of [
  "page_static_landing_artifact::Entity::insert",
  "page_publish_operation_artifact::ActiveModel",
  "page_publish_rebuild_source::ActiveModel",
]) {
  forbid(sources.artifactSet, marker, "rollback recovery must not recreate immutable repair authority");
}

requireOrdered(
  sources.rollback,
  [
    "load_current_published_set_in_tx",
    "find_previous_publish_target_in_tx",
  ],
  "rollback current-set before target selection",
);
requireOrdered(
  sources.rollback,
  [
    "if operation.artifact_set_hash == current_artifact_set_hash",
    "continue;",
    "let manifest = load_publish_manifest_in_tx(txn, &operation).await?;",
  ],
  "historical target must differ before manifest load",
);

for (const marker of [
  "fk_page_publish_operation_artifacts_artifact",
  ".on_delete(ForeignKeyAction::Cascade)",
]) {
  need(sources.manifestMigration, marker, "manifest artifact FK");
}

for (const marker of [
  "rollback_continues_after_physical_loss_rebuild_and_activation_on_postgres",
  "historical_target_still_requires_original_manifest_on_postgres",
  "surviving_manifest_identity_mismatch_is_not_healed_by_repair_on_postgres",
  "missing_current_manifest_is_not_healed_while_source_artifact_still_exists_on_postgres",
  "remove_current_source_artifact",
  "rebuild_immutable_artifact",
  "replace_rebuilt_artifact_binding",
  "UPDATE page_publish_operation_artifacts SET artifact_hash",
  "rollback_to_previous",
  "rollback.target_publish_operation_id",
  "Err(PagesError::RollbackTargetUnavailable(_))",
]) {
  need(sources.test, marker, "PostgreSQL harness source");
}

if (!pages) {
  failures.push("Page Builder FBA registry is missing Pages consumer");
} else {
  if (pages.metadata_properties?.legacy_form !== "PageMetadataEditor_removed") {
    failures.push("registry must mark PageMetadataEditor removed");
  }
  const uniqueKey = pages.materialization_persistence?.unique_key ?? [];
  if (!uniqueKey.includes("instance_key")) {
    failures.push("registry materialization unique key must include instance_key");
  }
  const continuity = pages.artifact_repair?.rollback_continuity;
  if (continuity?.strict_publish_manifest_first !== true) {
    failures.push("registry repair rollback must remain strict-manifest-first");
  }
  if (continuity?.current_cursor_only !== true) {
    failures.push("registry repair rollback fallback must remain current-cursor-only");
  }
  if (continuity?.requires_explicit_repair_chain !== true) {
    failures.push("registry repair rollback fallback must require an explicit repair chain");
  }
  if (continuity?.surviving_manifest_rows_must_match_retained_provenance !== true) {
    failures.push("registry surviving manifest rows must match retained provenance");
  }
  if (continuity?.missing_manifest_row_requires_absent_source_artifact !== true) {
    failures.push("registry missing repaired manifest row must require absent source artifact");
  }
  if (continuity?.historical_target_original_manifest_required !== true) {
    failures.push("registry historical rollback target must require original manifest");
  }
  if (pages.artifact_rollback?.historical_target_provenance_fallback !== false) {
    failures.push("registry must forbid historical target provenance fallback");
  }
  if (pages.artifact_repair?.automatic_audit_to_rebuild !== false
      || pages.artifact_repair?.automatic_rebuild_to_activation !== false
      || pages.artifact_repair?.automatic_activation_to_rollback !== false) {
    failures.push("registry must keep automatic repair/activation/rollback chaining disabled");
  }
}

for (const marker of [
  "Explicit Artifact Repair Rollback Continuity",
  "at least one locale must be explicitly rebuilt and activated",
  "surviving manifest row",
  "source artifact still exists",
  "Historical target remains strict",
  "Physical-loss continuity",
]) {
  need(sources.packet, marker, "repair rollback packet");
}
for (const marker of [
  "Repair-to-Rollback Continuity Actualization",
  "repair-rollback-continuity-source-ready",
  "at least one rebuilt/activated locale",
  "Every surviving manifest row remains authoritative evidence",
  "historical source artifact must be absent as well",
  "Historical targets stay fail-closed",
  "Page Builder registry parity correction",
]) {
  need(sources.actualization, marker, "parity actualization");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-repair-rollback-continuity] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

console.log("[verify-pages-artifact-repair-rollback-continuity] PASS");
