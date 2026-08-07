#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  harness: "crates/rustok-pages/tests/explicit_artifact_repair_postgres.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-postgres-source.json",
  continuation: "docs/modules/pages-page-builder-repair-postgres-continuation-2026-08-07.md",
  rebuildOwner: "crates/rustok-pages/src/services/page/artifact_rebuild.rs",
  activationOwner: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  rebuildMigration: "crates/rustok-pages/src/migrations/m20260806_000014_add_explicit_artifact_rebuild.rs",
  activationMigration: "crates/rustok-pages/src/migrations/m20260807_000015_create_page_artifact_binding_replacements.rs",
  transactionalBus: "crates/rustok-outbox/src/transactional.rs",
};
const absolute = (relative) => path.join(repoRoot, relative);
const read = (relative) => fs.readFileSync(absolute(relative), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
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

for (const [label, relative] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relative))) {
    failures.push(`${label}: missing regular file ${relative}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relative));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relative} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relative]) => [label, read(relative)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_explicit_artifact_repair_postgres_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_repair_postgres_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "postgres_environment_gated",
  "isolated_postgres_schema_per_run",
  "real_outbox_module_migrations_used",
  "real_pages_module_migrations_used",
  "minimal_pages_module_enablement_fixture_used",
  "transactional_outbox_transport_used",
  "reviewed_publish_creates_rebuild_provenance",
  "mutable_current_body_is_not_rebuild_authority",
  "corrupted_source_artifact_is_retained",
  "rebuild_appends_distinct_operation_bound_artifact",
  "rebuild_preserves_binding_and_page_version",
  "rebuild_exact_replay_is_single_receipt_single_artifact",
  "rebuild_idempotency_conflict_adds_no_receipt_or_artifact",
  "rebuild_receipt_unique_constraint_is_exercised",
  "rebuild_receipt_conflict_rolls_back_prior_page_marker",
  "stale_current_artifact_activation_is_rejected",
  "stale_activation_adds_no_receipt_and_preserves_binding",
  "activation_switches_exact_locale_binding",
  "activation_advances_page_version_once",
  "activation_retains_source_and_replacement_artifacts",
  "activation_writes_one_node_updated_and_one_node_published",
  "activation_exact_replay_adds_no_version_or_receipt",
  "activated_rebuild_reuse_is_rejected",
  "activation_receipt_unique_rebuild_constraint_is_exercised",
  "activation_receipt_conflict_rolls_back_prior_page_marker",
  "activation_receipt_conflict_rolls_back_prior_outbox_row",
]) {
  if (contract[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "automatic_audit_to_rebuild",
  "automatic_rebuild_to_activation",
  "production_code_changed",
  "database_schema_changed",
  "public_transport_changed",
  "ffa_promoted",
  "fba_promoted",
  "tests_run",
  "source_verifier_run",
  "cargo_run",
  "formatting_run",
  "postgres_run",
  "lifecycle_handler_run",
  "cache_generation_observation_run",
  "workflows_or_ci_run",
]) {
  if (contract[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  'const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL"',
  'env::var("DATABASE_URL")',
  "struct TestDatabase",
  "CREATE SCHEMA",
  "DROP SCHEMA IF EXISTS",
  "SET search_path",
  "OutboxModule",
  "PagesModule",
  "enable_pages_module(&db, tenant_id).await?",
  "CREATE TABLE tenant_modules",
  '"pages".into()',
  "OutboxTransport::new(db.clone())",
  "PageService::new(db.clone(), event_bus.clone())",
  "Mutable draft must not become rebuild authority",
  "<main>corrupted retained artifact</main>",
  "rebuild_immutable_artifact(",
  "PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT",
  "assert_rebuild_receipt_constraint_rolls_back_page_marker",
  "replace_rebuilt_artifact_binding(",
  "PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT",
  "assert_activation_lifecycle_pair",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
  "assert_activation_receipt_conflict_rolls_back_page_and_outbox",
  "publish_in_tx_with_envelope_id(",
  "SysEvents::find_by_id(rolled_back_event_id)",
  "txn.rollback().await?;",
]) need(sources.harness, marker, "PostgreSQL repair harness");

for (const marker of [
  "CREATE TABLE page_artifact_rebuild_operations",
  "CREATE TABLE page_artifact_binding_replacement_operations",
  "CREATE TABLE page_static_landing_artifacts",
  "CREATE TABLE page_publish_rebuild_sources",
]) forbid(sources.harness, marker, "fixture-owned production table boundary");

const rebuildConstraint = sliceBetween(
  sources.harness,
  "async fn assert_rebuild_receipt_constraint_rolls_back_page_marker(",
  "async fn assert_activation_lifecycle_pair(",
  "rebuild receipt rollback fixture",
);
requireOrder(
  rebuildConstraint,
  [
    "let txn = db.begin().await?;",
    "marker.update(&txn).await?;",
    "duplicate.insert(&txn).await",
    "assert!(duplicate_insert.is_err());",
    "txn.rollback().await?;",
  ],
  "rebuild receipt rollback ordering",
);

const activationConstraint = sliceBetween(
  sources.harness,
  "async fn assert_activation_receipt_conflict_rolls_back_page_and_outbox(",
  "async fn outbox_event_ids(",
  "activation receipt rollback fixture",
);
requireOrder(
  activationConstraint,
  [
    "let txn = db.begin().await?;",
    "marker.update(&txn).await?;",
    ".publish_in_tx_with_envelope_id(",
    "duplicate.insert(&txn).await",
    "assert!(duplicate_insert.is_err());",
    "txn.rollback().await?;",
    "SysEvents::find_by_id(rolled_back_event_id)",
  ],
  "activation page event receipt rollback ordering",
);

for (const marker of [
  'name("idx_page_artifact_rebuild_operations_idempotency")',
  ".col(PageArtifactRebuildOperations::TenantId)",
  ".col(PageArtifactRebuildOperations::PageId)",
  ".col(PageArtifactRebuildOperations::IdempotencyKey)",
  ".unique()",
]) need(sources.rebuildMigration, marker, "rebuild migration constraint");
for (const marker of [
  'name("idx_page_artifact_binding_replacements_rebuild")',
  ".col(PageArtifactBindingReplacementOperations::TenantId)",
  ".col(PageArtifactBindingReplacementOperations::PageId)",
  ".col(PageArtifactBindingReplacementOperations::RebuildOperationId)",
  ".unique()",
]) need(sources.activationMigration, marker, "activation migration constraint");

const rebuildOwner = sliceBetween(
  sources.rebuildOwner,
  "pub async fn rebuild_immutable_artifact(",
  "async fn load_source_in_tx(",
  "rebuild owner",
);
requireOrder(
  rebuildOwner,
  [
    "let txn = self.db.begin().await?;",
    "PageBuilderArtifactService::append_rebuilt_in_tx(",
    "page_artifact_rebuild_operation::ActiveModel",
    ".insert(&txn)",
    "txn.commit().await?;",
  ],
  "rebuild artifact receipt commit ordering",
);

const activationOwner = sliceBetween(
  sources.activationOwner,
  "pub async fn replace_rebuilt_artifact_binding(",
  "async fn load_rebuild_operation_in_tx(",
  "activation owner",
);
requireOrder(
  activationOwner,
  [
    "let txn = self.db.begin().await?;",
    "PageBuilderArtifactService::bind_existing_body_in_tx(",
    "active.update(&txn).await?;",
    "DomainEvent::NodeUpdated",
    "DomainEvent::NodePublished",
    "page_artifact_binding_replacement_operation::ActiveModel",
    ".insert(&txn)",
    "txn.commit().await?;",
  ],
  "activation binding page events receipt commit ordering",
);

for (const marker of [
  "pub async fn publish_in_tx_with_envelope_id",
  "outbox.write_to_outbox(txn, envelope).await?;",
  "Ok(envelope_id)",
]) need(sources.transactionalBus, marker, "transactional event bus");

for (const marker of [
  "explicit-artifact-repair-postgres-harness-source-ready",
  "real `OutboxModule` and `PagesModule` migrations",
  "Rebuild receipt constraint rollback",
  "Durable lifecycle pair",
  "Activation receipt conflict rollback",
  "pages_explicit_artifact_repair_postgres_source_unvalidated",
  "intentionally not run",
]) need(sources.continuation, marker, "PostgreSQL repair continuation");

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-postgres] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-explicit-artifact-repair-postgres] PASS source_ready=true postgres_execution=pending");
