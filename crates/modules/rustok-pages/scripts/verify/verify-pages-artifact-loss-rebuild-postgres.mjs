#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
const failures = [];

const files = {
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-artifact-loss-rebuild-postgres-source.json",
  harness: "crates/modules/rustok-pages/tests/artifact_loss_rebuild_postgres.rs",
  reviewedPublish: "crates/modules/rustok-pages/src/services/page/reviewed_publish.rs",
  rebuildOwner: "crates/modules/rustok-pages/src/services/page/artifact_rebuild.rs",
  artifactOwner: "crates/modules/rustok-pages/src/services/page_builder_artifact.rs",
  provenanceMigration: "crates/modules/rustok-pages/src/migrations/m20260806_000013_create_page_publish_rebuild_sources.rs",
  rebuildMigration: "crates/modules/rustok-pages/src/migrations/m20260806_000014_add_explicit_artifact_rebuild.rs",
};

for (const [label, relativePath] of Object.entries(files)) {
  const absolute = path.join(repoRoot, relativePath);
  if (!fs.existsSync(absolute)) {
    failures.push(`${label}: missing ${relativePath}`);
    continue;
  }
  const stats = fs.lstatSync(absolute);
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relativePath} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-rebuild-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const requireOrder = (source, markers, label) => {
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
const sliceBetween = (source, start, end, label) => {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) {
    failures.push(`${label}: missing ${start}`);
    return "";
  }
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) {
    failures.push(`${label}: missing ${end}`);
    return "";
  }
  return source.slice(startIndex, endIndex);
};
const countText = (source, marker) => source.split(marker).length - 1;

if (evidence.format !== "pages_artifact_loss_rebuild_postgres_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_artifact_loss_rebuild_postgres_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const value of Object.values(evidence.validation ?? {})) {
  if (value !== false) failures.push("all validation flags must remain false");
}

for (const [key, expected] of Object.entries({
  postgres_environment_gated: true,
  isolated_postgres_schema_per_run: true,
  real_outbox_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_page_service_publish_reviewed_used: true,
  reviewed_publish_revision_matches_owner_updated_at_snapshot: true,
  retained_provenance_captured_before_loss: true,
  canonical_artifact_model_captured_before_loss: true,
  binding_reference_removed_before_artifact_loss: true,
  publish_manifest_reference_removed_before_artifact_loss: true,
  source_artifact_row_deleted_before_rebuild: true,
  retained_provenance_survives_source_artifact_loss: true,
  provenance_migration_has_no_artifact_fk: true,
  rebuild_receipt_migration_has_no_source_artifact_fk: true,
  rebuild_owner_does_not_load_source_artifact_row: true,
  rebuild_uses_retained_provenance_after_source_artifact_loss: true,
  rebuilt_model_matches_pre_loss_canonical_model_except_storage_identity_and_created_at: true,
  rebuild_receipt_retains_missing_source_artifact_id_as_evidence: true,
  rebuild_appends_exactly_one_artifact: true,
  rebuild_appends_exactly_one_receipt: true,
  rebuild_preserves_retained_provenance: true,
  rebuild_does_not_recreate_binding: true,
  rebuild_preserves_page_version_and_status: true,
  rebuild_emits_no_lifecycle_events: true,
  exact_replay_reuses_operation_and_artifact: true,
  exact_replay_adds_no_artifact_receipt_or_event: true,
  automatic_repair_added: false,
  automatic_activation_added: false,
  production_code_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  ffa_promoted: false,
  fba_promoted: false,
  tests_run: false,
  source_verifier_run: false,
  cargo_run: false,
  formatting_run: false,
  postgres_run: false,
  workflows_or_ci_run: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

, marker, "artifact-loss rebuild continuation");

for (const forbidden of [
  "replace_rebuilt_artifact_binding(",
  "audit_page_artifacts(",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
]) forbid(sources.harness, forbidden, "artifact-loss harness automatic/transport boundary");

if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-rebuild-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-artifact-loss-rebuild-postgres] PASS source_ready=true execution=pending");
