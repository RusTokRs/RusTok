#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  audit: "crates/rustok-pages/src/services/page/artifact_integrity_audit.rs",
  pageModule: "crates/rustok-pages/src/services/page/mod.rs",
  services: "crates/rustok-pages/src/services/mod.rs",
  lib: "crates/rustok-pages/src/lib.rs",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-source.json",
  packet: "crates/rustok-pages/docs/immutable-artifact-integrity-audit.md",
  actualization: "docs/modules/page-builder-parity-actualization-2026-08-05.md",
};

const absolute = (relativePath) => path.join(repoRoot, relativePath);
const read = (relativePath) => fs.readFileSync(absolute(relativePath), "utf8");
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
  console.error("[verify-pages-immutable-artifact-integrity-audit] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_immutable_artifact_integrity_audit_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "pages_immutable_artifact_integrity_audit_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

for (const [key, expected] of Object.entries({
  command: "PageService::audit_immutable_artifact_integrity",
  result_format: "pages_immutable_artifact_integrity_audit_v1",
  required_permission: "pages:manage",
  required_permission_scope: "all",
  default_max_records: 128,
  hard_max_records: 512,
  max_returned_findings: 64,
  audit_hash_algorithm: "sha256",
})) {
  if (contract[key] !== expected) failures.push(`source_contract.${key} drifted`);
}
for (const key of [
  "owner_scoped_manage_rejected",
  "tenant_and_page_fenced",
  "single_transaction_read_boundary",
  "limit_plus_one_truncation_detection",
  "checks_static_artifact_integrity",
  "checks_materialization_integrity",
  "checks_complete_or_legacy_null_materialization_evidence",
  "checks_owner_identity",
  "checks_document_body_css_size_limits",
  "returns_hashed_record_identity",
  "returns_hashed_diagnostics",
]) {
  if (contract[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "returns_raw_locale",
  "returns_raw_build_hash",
  "returns_raw_materialization_hash",
  "returns_raw_document_html",
  "returns_raw_body_html",
  "returns_raw_css",
  "returns_runtime_snapshots",
  "returns_materialization_identity",
  "returns_internal_error_text",
  "mutates_artifacts",
  "mutates_bindings",
  "emits_events",
  "performs_repair",
  "performs_rebuild",
  "adds_database_schema",
  "adds_graphql_or_http_transport",
  "tests_run",
  "static_verifiers_run",
  "cargo_run",
  "formatting_run",
  "database_run",
  "workflows_or_ci_run",
]) {
  if (contract[key] !== false) failures.push(`source_contract.${key} must remain false`);
}
if (JSON.stringify(contract.stable_order) !== JSON.stringify(["created_at", "id"])) {
  failures.push("source_contract.stable_order drifted");
}
if (Object.hasOwn(contract, "transactional_snapshot_read")) {
  failures.push("source contract must not overstate transaction isolation as a snapshot");
}

for (const marker of [
  "PermissionScope",
  "ConnectionTrait",
  'pub const DEFAULT_PAGE_ARTIFACT_AUDIT_RECORDS: u32 = 128;',
  'pub const MAX_PAGE_ARTIFACT_AUDIT_RECORDS: u32 = 512;',
  'pub const MAX_PAGE_ARTIFACT_AUDIT_FINDINGS: usize = 64;',
  '"pages_immutable_artifact_integrity_audit_v1"',
  "pub struct AuditPageArtifactsInput",
  "pub struct PageArtifactIntegrityFinding",
  "pub struct PageArtifactIntegrityAuditResult",
  "pub async fn audit_immutable_artifact_integrity",
  "enforce_tenant_wide_manage(&security)?",
  "security.get_scope(Resource::Pages, Action::Manage)",
  "PermissionScope::All",
  '"Immutable artifact audit requires tenant-wide pages:manage"',
  "page::Column::TenantId.eq(tenant_id)",
  "page_static_landing_artifact::Column::TenantId.eq(tenant_id)",
  "page_static_landing_artifact::Column::PageId.eq(page_id)",
  ".order_by_asc(page_static_landing_artifact::Column::CreatedAt)",
  ".order_by_asc(page_static_landing_artifact::Column::Id)",
  "u64::from(max_records).saturating_add(1)",
  "let truncated = records.len() > max_records as usize;",
  "let locale_hash = hex_sha256(record.locale.as_bytes());",
  "let record_identity_hash = artifact_record_identity_hash(record)?;",
  "artifact\n        .verify_integrity()",
  "materialized\n                .verify_integrity()",
  "(None, None, None) => Ok(())",
  '"Stored landing materialization evidence is partial"',
  "findings.len() < MAX_PAGE_ARTIFACT_AUDIT_FINDINGS",
  "hex_sha256(error.to_string().as_bytes())",
  "PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT",
  "txn.commit().await?",
]) need(sources.audit, marker, "audit source");
forbid(sources.audit, "use crate::services::rbac::enforce_scope;", "tenant-wide admission");
forbid(sources.audit, "enforce_scope(&security", "tenant-wide admission");

const auditMethod = sliceBetween(
  sources.audit,
  "pub async fn audit_immutable_artifact_integrity",
  "fn enforce_tenant_wide_manage",
  "audit command",
);
requireOrdered(
  auditMethod,
  [
    "enforce_tenant_wide_manage(&security)?",
    "let txn = self.db.begin().await?;",
    "page::Entity::find_by_id(page_id)",
    "page_static_landing_artifact::Entity::find()",
    "let audit_hash = stable_hash",
    "txn.commit().await?;",
  ],
  "audit command order",
);
for (const marker of [
  "ActiveModel",
  ".insert(",
  ".update(",
  ".delete(",
  "delete_many",
  "update_many",
  "publish_in_tx",
  "event_bus",
  "rollback_to_previous",
  "compile_materialized_static_landing",
  "sanitize_static_landing_project",
]) forbid(auditMethod, marker, "read-only audit command");

const publicResult = sliceBetween(
  sources.audit,
  "pub struct PageArtifactIntegrityFinding",
  "#[derive(Debug, Clone, Serialize)]",
  "public audit result",
);
for (const marker of [
  "locale: String",
  "build_hash",
  "artifact_hash",
  "content_hash",
  "materialization_hash",
  "document_html",
  "body_html",
  "css:",
  "runtime_snapshots",
  "materialization_identity",
  "message:",
  "error:",
]) forbid(publicResult, marker, "bounded public audit result");
for (const marker of [
  "artifact_id",
  "locale_hash",
  "record_identity_hash",
  "diagnostic_hash",
  "truncated",
  "findings_truncated",
  "audit_hash",
]) need(publicResult, marker, "bounded public audit result");

for (const [label, source] of [
  ["page module", sources.pageModule],
  ["services export", sources.services],
  ["crate export", sources.lib],
]) {
  for (const marker of [
    "AuditPageArtifactsInput",
    "PageArtifactIntegrityAuditResult",
    "PageArtifactIntegrityFinding",
    "MAX_PAGE_ARTIFACT_AUDIT_RECORDS",
    "PAGE_ARTIFACT_INTEGRITY_AUDIT_FORMAT",
  ]) need(source, marker, label);
}
need(sources.pageModule, "mod artifact_integrity_audit;", "page module");

for (const marker of [
  "source-ready / maintainer-validation-pending",
  "PageService::audit_immutable_artifact_integrity",
  "tenant-wide",
  "pages:manage",
  "maximum records: 512",
  "maximum returned findings: 64",
  "truncated = true",
  "SHA-256 locale hash",
  "raw build, artifact, content or materialization hashes",
  "does not repair, rebuild",
  "intentionally not run",
]) need(sources.packet, marker, "audit packet");

for (const marker of [
  "immutable-artifact-integrity-audit-source-ready",
  "read-only immutable artifact integrity audit",
  "repair/rebuild remains open",
  "execution and rollout remain open",
]) need(sources.actualization, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-immutable-artifact-integrity-audit] PASS source_ready=true execution=pending",
);
