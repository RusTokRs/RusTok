#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-postgres-source.json",
));
const harness = read("crates/rustok-pages/tests/immutable_artifact_integrity_audit_postgres.rs");
const owner = read("crates/rustok-pages/src/services/page/artifact_integrity_audit.rs");
const continuation = read(
  "docs/modules/pages-page-builder-artifact-audit-postgres-continuation-2026-08-07.md",
);
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (content, value, label) => {
  if (content.includes(value)) failures.push(`${label}: forbidden ${value}`);
};
const requireOrder = (content, values, label) => {
  let previous = -1;
  for (const value of values) {
    const index = content.indexOf(value, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${value}`);
      return;
    }
    previous = index;
  }
};
const count = (content, value) => content.split(value).length - 1;

if (evidence.format !== "pages_immutable_artifact_integrity_audit_postgres_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_immutable_artifact_integrity_audit_postgres_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const key of [
  "source_guard",
  "postgres_harness",
  "valid_canonical_and_rebuilt",
  "bounded_truncation",
  "corrupt_payload",
  "partial_materialization",
  "shared_lock_blocking",
  "owner_lock_source_binding",
  "hashed_findings",
]) {
  if (evidence.validation?.[key] !== false) failures.push(`validation.${key} must remain false`);
}
for (const [key, expected] of Object.entries({
  postgres_environment_gated: true,
  isolated_postgres_schema_per_run: true,
  real_outbox_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_reviewed_publish_used: true,
  real_explicit_rebuild_used: true,
  real_audit_owner_used: true,
  canonical_reviewed_publish_body_revision_used: true,
  valid_canonical_and_rebuilt_audit_source_present: true,
  bounded_max_records_truncation_source_present: true,
  corrupt_payload_hashed_finding_source_present: true,
  partial_materialization_hashed_finding_source_present: true,
  shared_page_row_lock_primitive_source_present: true,
  shared_artifact_projection_lock_primitive_source_present: true,
  shared_artifact_record_lock_primitive_source_present: true,
  concurrent_artifact_update_uses_postgres_lock_timeout: true,
  lock_timeout_error_required_for_blocked_update: true,
  blocked_update_rolls_back_without_payload_change: true,
  owner_postgres_page_scan_uses_lock_shared: true,
  owner_postgres_artifact_id_scan_uses_lock_shared: true,
  owner_postgres_artifact_record_scan_uses_lock_shared: true,
  finding_code_is_static: true,
  finding_locale_is_hashed: true,
  finding_record_identity_is_hashed: true,
  finding_diagnostic_is_hashed: true,
  audit_identity_is_hashed: true,
  production_code_changed: false,
  database_schema_changed: false,
  public_transport_changed: false,
  automatic_repair_added: false,
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

for (const marker of [
  "RUSTOK_PAGES_TEST_DATABASE_URL",
  "postgres://",
  "postgresql://",
  "CREATE SCHEMA",
  "OutboxModule",
  "PagesModule.migrations()",
  "CREATE TABLE tenant_modules",
  "Sha256::digest",
  'format!("{}\\0{}", body.format, body.content)',
  ".publish_reviewed(",
  ".rebuild_immutable_artifact(",
  ".audit_immutable_artifact_integrity(",
  "PAGE_ARTIFACT_INTEGRITY_INVALID",
]) requireText(harness, marker, "audit PostgreSQL harness foundation");

requireOrder(harness, [
  "let complete = service",
  "max_records: Some(2)",
  "assert_eq!(complete.scanned_artifact_count, 2);",
  "assert_eq!(complete.valid_artifact_count, 2);",
  "let bounded = service",
  "max_records: Some(1)",
  "assert!(bounded.truncated);",
  "assert_shared_scan_locks_block_artifact_update(",
], "valid bounded audit and lock ordering");

requireOrder(harness, [
  "let locked_page = page::Entity::find_by_id(page_id)",
  ".lock_shared()",
  "let selected_ids = page_static_landing_artifact::Entity::find()",
  ".select_only()",
  ".limit(2)",
  ".lock_shared()",
  "let locked_artifact = page_static_landing_artifact::Entity::find_by_id(artifact_id)",
  ".lock_shared()",
  "SET LOCAL lock_timeout = '100ms'",
  "UPDATE page_static_landing_artifacts SET document_html = $1 WHERE id = $2",
  "expect_err(\"concurrent artifact update must be blocked by shared scan locks\")",
  ".contains(\"lock timeout\")",
  "updater.rollback().await?;",
  "locker.commit().await?;",
  "assert_eq!(stored.document_html, before_document);",
], "PostgreSQL shared-lock blocking packet");

requireOrder(harness, [
  'active.document_html = Set("<main>corrupted immutable payload</main>".to_string());',
  "let corrupt_result = service",
  "assert_eq!(corrupt_result.invalid_artifact_count, 1);",
  "PAGE_ARTIFACT_INTEGRITY_INVALID",
], "corrupt payload audit ordering");
requireOrder(harness, [
  "assert!(artifact.runtime_snapshots.is_some());",
  "active.runtime_snapshots = Set(None);",
  "let partial_result = service",
  "assert_eq!(partial_result.invalid_artifact_count, 1);",
], "partial materialization audit ordering");

if (count(owner, ".lock_shared()") < 3) {
  failures.push("audit owner must retain at least three shared-lock calls for page/id/record scans");
}
requireOrder(owner, [
  "let page_query = ||",
  "DbBackend::Postgres | DbBackend::MySql =>",
  "page_query().lock_shared().one(&txn).await?",
  "let artifact_id_query = ||",
  "DbBackend::Postgres | DbBackend::MySql =>",
  "artifact_id_query()",
  ".lock_shared()",
  ".into_tuple::<Uuid>()",
  "let record_query = ||",
  "DbBackend::Postgres | DbBackend::MySql =>",
  "record_query().lock_shared().one(&txn).await?",
], "audit owner PostgreSQL lock-backed scan ordering");

for (const marker of [
  "let fetch_limit = u64::from(max_records).saturating_add(1);",
  "let truncated = artifact_ids.len() > max_records as usize;",
  "hex_sha256(record.locale.as_bytes())",
  "artifact_record_identity_hash(&record)?",
  "hex_sha256(error.to_string().as_bytes())",
  '"Stored landing materialization evidence is partial"',
]) requireText(owner, marker, "audit owner bounded/hash contract");

for (const marker of [
  "immutable-artifact-audit-postgres-harness-source-ready",
  "pages_immutable_artifact_integrity_audit_postgres_source_unvalidated",
  "lock_timeout",
  "canonical and rebuilt",
  "partial materialization",
  "max_records=1",
  "intentionally not run",
]) requireText(continuation, marker, "audit PostgreSQL continuation");

for (const forbidden of [
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
  "audit_page_artifacts(",
]) forbidText(harness, forbidden, "audit harness transport/automatic-repair boundary");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit-postgres] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-immutable-artifact-integrity-audit-postgres] PASS source_ready=true execution=pending");
