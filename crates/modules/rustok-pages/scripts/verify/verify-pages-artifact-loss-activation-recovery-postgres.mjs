#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  service: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  bindingOwner: "crates/rustok-pages/src/services/page_builder_artifact.rs",
  test: "crates/rustok-pages/tests/artifact_loss_activation_recovery_postgres.rs",
  rollbackTest: "crates/rustok-pages/tests/artifact_loss_after_rollback_activation_recovery_postgres.rs",
  repeatedTest: "crates/rustok-pages/tests/artifact_repeated_loss_recovery_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-binding-replacement-source.json",
  packet: "crates/rustok-pages/docs/explicit-immutable-artifact-loss-activation-recovery.md",
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
  console.error("[verify-pages-artifact-loss-activation-recovery-postgres] FAIL");
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
  "missing_binding_recovery_requires_source_artifact_absent",
  "missing_binding_recovery_requires_retained_source_body_identity",
  "missing_binding_recovery_requires_exact_source_publish_operation",
  "missing_binding_direct_publish_anchor_accepts_publish_result_version_equal_current_expected",
  "missing_binding_rollback_activation_anchor_is_supported",
  "missing_binding_rollback_anchor_recomputes_canonical_request_hash",
  "missing_binding_sequential_recovery_rejects_unexplained_version_gap",
  "missing_binding_sequential_recovery_tracks_latest_repair_state_per_locale",
  "missing_binding_sequential_recovery_allows_repeated_locale_only_after_prior_rebuilt_artifact_absence",
  "existing_binding_mismatch_never_falls_back_to_recovery",
  "postgres_recovery_harness_source_ready",
  "postgres_success_recovery_case_source_ready",
  "postgres_source_artifact_present_rejection_source_ready",
  "postgres_stale_publish_version_rejection_source_ready",
  "postgres_rollback_activated_recovery_harness_source_ready",
  "postgres_repeated_loss_recovery_harness_source_ready",
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
  "binding_replacement_run",
  "workflows_or_ci_run",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`evidence source_contract.${key} must remain false`);
  }
}

for (const marker of [
  "let page_body_id = match binding",
  "binding.page_body_id != source.page_body_id",
  "binding.artifact_id != input.expected_current_artifact_id",
  "ensure_missing_binding_recovery_in_tx",
  "page_static_landing_artifact::Entity::find_by_id(rebuild.source_artifact_id)",
  "source_artifact.is_some()",
  "page_body::Entity::find_by_id(source.page_body_id)",
  "page_publish_operation::Entity::find_by_id(source.operation_id)",
  "publish.id != rebuild.source_publish_operation_id",
  "resolve_missing_binding_recovery_anchor_in_tx",
  "rollback.request_hash != expected_request_hash",
  "ensure_sequential_missing_binding_recovery_version_chain_in_tx",
  "let mut latest_by_locale",
  "recovery_artifact_if_present_in_tx",
  "PageBuilderArtifactService::bind_existing_body_in_tx",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
]) {
  need(sources.service, marker, "recovery service");
}
for (const marker of [
  "sanitize_static_landing_project",
  "compile_materialized_static_landing",
  "append_rebuilt_in_tx",
  "page_body::Column::Content",
  "body.content",
  "delete_by_id(rebuild.source_artifact_id)",
  "PagesCacheInvalidationRuntime",
]) {
  forbid(sources.service, marker, "recovery boundary");
}
need(sources.bindingOwner, "pub(crate) async fn bind_existing_body_in_tx", "binding owner");

for (const marker of [
  "missing_binding_activation_recovers_after_physical_source_artifact_loss_on_postgres",
  "missing_binding_activation_rejects_when_source_artifact_still_exists_on_postgres",
  "missing_binding_activation_rejects_stale_source_publish_version_on_postgres",
  "RUSTOK_PAGES_TEST_DATABASE_URL",
]) {
  need(sources.test, marker, "direct-publish PostgreSQL recovery source");
}
for (const marker of [
  "rollback_activated_publish_recovers_two_lost_locales_sequentially_on_postgres",
  "rollback_activated_recovery_rejects_noncanonical_rollback_anchor_hash_on_postgres",
]) {
  need(sources.rollbackTest, marker, "rollback-activated regression source");
}
for (const marker of [
  "missing_binding_activation_recovers_same_locale_after_rebuilt_artifact_is_lost_again_on_postgres",
  "repeated_locale_recovery_rejects_missing_binding_while_prior_rebuilt_artifact_still_exists_on_postgres",
]) {
  need(sources.repeatedTest, marker, "repeated-loss regression source");
}
for (const marker of [
  "Missing-binding recovery admission",
  "Direct publish anchor",
  "Exact rollback activation anchor",
  "Sequential multi-locale and repeated-loss version chain",
  "Existing-binding path remains strict",
  "Repeated recovery",
]) {
  need(sources.packet, marker, "recovery packet");
}

if (failures.length > 0) {
  console.error("[verify-pages-artifact-loss-activation-recovery-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}
console.log("[verify-pages-artifact-loss-activation-recovery-postgres] PASS");
