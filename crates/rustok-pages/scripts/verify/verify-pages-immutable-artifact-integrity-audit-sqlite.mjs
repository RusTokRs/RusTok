#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-sqlite-source.json",
));
const harness = read("crates/rustok-pages/tests/immutable_artifact_integrity_audit_sqlite.rs");
const owner = read("crates/rustok-pages/src/services/page/artifact_integrity_audit.rs");
const continuation = read(
  "docs/modules/pages-page-builder-artifact-audit-sqlite-continuation-2026-08-07.md",
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

if (evidence.status !== "pages_immutable_artifact_integrity_audit_sqlite_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const key of [
  "source_guard",
  "sqlite_harness",
  "authorization_all_none",
  "valid_canonical_and_rebuilt",
  "bounded_truncation",
  "corrupt_payload",
  "partial_materialization",
  "hashed_findings",
]) {
  if (evidence.validation?.[key] !== false) failures.push(`validation.${key} must remain false`);
}
for (const [key, expected] of Object.entries({
  sqlite_isolated_database_per_test: true,
  real_outbox_sys_events_migration_used: true,
  real_channel_module_migrations_used: true,
  real_pages_module_migrations_used: true,
  real_reviewed_publish_used: true,
  real_explicit_rebuild_used: true,
  real_audit_owner_used: true,
  pages_manage_present_resolves_all: true,
  pages_manage_absent_resolves_none: true,
  manage_absent_rejected_before_audit_reads: true,
  canonical_artifact_audits_valid: true,
  rebuilt_artifact_audits_valid: true,
  requested_record_limit_sets_truncated: true,
  corrupt_payload_reports_hashed_invalid_finding: true,
  partial_materialization_reports_hashed_invalid_finding: true,
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
  sqlite_run: false,
  workflows_or_ci_run: false,
})) {
  if (evidence.source_contract?.[key] !== expected) {
    failures.push(`source_contract.${key} must be ${expected}`);
  }
}

for (const test of [
  "audit_manage_scope_is_all_or_none_and_public_read_is_denied",
  "audit_accepts_canonical_and_rebuilt_records_and_truncates_at_requested_limit",
  "audit_reports_corrupted_immutable_payload_with_hashed_finding",
  "audit_reports_partial_materialization_evidence_without_exposing_payloads",
]) {
  requireText(harness, `async fn ${test}()`, "audit SQLite harness");
  if (!evidence.harness?.tests?.includes(test)) failures.push(`missing harness registration ${test}`);
}

for (const marker of [
  "SysEventsMigration.up(&manager).await?",
  "for migration in ChannelModule.migrations()",
  "for migration in PagesModule.migrations()",
  "SecurityContext::system()",
  "SecurityContext::public_read()",
  "PermissionScope::All",
  "PermissionScope::None",
  ".publish_reviewed(",
  ".rebuild_immutable_artifact(",
  ".audit_immutable_artifact_integrity(",
  "PAGE_ARTIFACT_INTEGRITY_INVALID",
]) requireText(harness, marker, "audit SQLite harness foundation");

requireOrder(harness, [
  "let complete = service",
  "max_records: Some(2)",
  "assert_eq!(complete.scanned_artifact_count, 2);",
  "assert_eq!(complete.valid_artifact_count, 2);",
  "let bounded = service",
  "max_records: Some(1)",
  "assert!(bounded.truncated);",
], "canonical/rebuilt bounded audit ordering");
requireOrder(harness, [
  'active.document_html = Set("<main>corrupted immutable payload</main>".to_string());',
  ".audit_immutable_artifact_integrity(",
  "assert_eq!(result.invalid_artifact_count, 1);",
  "assert_eq!(finding.code, PAGE_ARTIFACT_INTEGRITY_INVALID);",
], "corruption audit ordering");
requireOrder(harness, [
  "assert!(artifact.runtime_snapshots.is_some());",
  "active.runtime_snapshots = Set(None);",
  ".audit_immutable_artifact_integrity(",
  "assert_eq!(result.invalid_artifact_count, 1);",
], "partial materialization audit ordering");

for (const marker of [
  "DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS",
  "MAX_PAGE_ARTIFACT_AUDIT_RECORDS",
  "MAX_PAGE_ARTIFACT_AUDIT_FINDINGS",
  "PermissionScope::All",
  "let fetch_limit = u64::from(max_records).saturating_add(1);",
  "let truncated = artifact_ids.len() > max_records as usize;",
  "PAGE_ARTIFACT_INTEGRITY_INVALID",
  '"Stored landing materialization evidence is partial"',
  "hex_sha256(record.locale.as_bytes())",
  "artifact_record_identity_hash(&record)?",
  "hex_sha256(error.to_string().as_bytes())",
]) requireText(owner, marker, "audit owner contract");

for (const marker of [
  "immutable-artifact-audit-sqlite-harness-source-ready",
  "pages_immutable_artifact_integrity_audit_sqlite_source_unvalidated",
  "canonical and rebuilt",
  "partial materialization",
  "max_records=1",
  "intentionally not run",
]) requireText(continuation, marker, "audit SQLite continuation");

for (const forbidden of [
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
  "audit_page_artifacts(",
]) forbidText(harness, forbidden, "audit harness transport/automatic-repair boundary");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit-sqlite] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-immutable-artifact-integrity-audit-sqlite] PASS source_ready=true execution=pending");
