#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  owner: "crates/rustok-pages/src/services/page/artifact_integrity_audit.rs",
  gql: "crates/rustok-pages/src/graphql/artifact_integrity_audit.rs",
  gqlMod: "crates/rustok-pages/src/graphql/mod.rs",
  http: "crates/rustok-pages/src/http/artifact_integrity_audit.rs",
  httpMod: "crates/rustok-pages/src/http.rs",
  openapi: "crates/rustok-pages/src/openapi.rs",
  evidence:
    "crates/rustok-pages/contracts/evidence/pages-immutable-artifact-integrity-audit-transport-source.json",
  packet: "crates/rustok-pages/docs/immutable-artifact-integrity-audit-transport.md",
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
  console.error("[verify-pages-immutable-artifact-integrity-audit-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_immutable_artifact_integrity_audit_transport_source_v1") {
  failures.push("source evidence format drifted");
}
if (
  evidence.status !==
  "pages_immutable_artifact_integrity_audit_transport_source_unvalidated"
) {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

for (const [key, expected] of Object.entries({
  service_command: "PageService::audit_immutable_artifact_integrity",
  graphql_mutation: "auditPageArtifacts",
  http_method: "POST",
  http_path: "/api/admin/pages/{id}/artifacts/audit",
  required_permission: "pages:manage",
})) {
  if (contract[key] !== expected) failures.push(`source_contract.${key} drifted`);
}
for (const key of [
  "current_tenant_only",
  "service_rechecks_permission_scope_all",
  "graphql_input_uses_bounded_max_records_only",
  "http_input_reuses_owner_dto",
  "graphql_result_reuses_bounded_owner_fields",
  "http_result_reuses_owner_dto",
  "openapi_registered",
  "adapters_delegate_to_owner_service",
  "public_error_codes_are_static",
]) {
  if (contract[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "adapters_query_artifact_tables",
  "adapters_mutate_artifacts_or_bindings",
  "adapters_emit_events",
  "returns_raw_locale",
  "returns_stored_artifact_hashes",
  "returns_document_html",
  "returns_body_html",
  "returns_css",
  "returns_runtime_snapshots",
  "returns_materialization_identity",
  "returns_internal_error_text",
  "adds_admin_ui",
  "adds_database_schema",
  "tests_run",
  "static_verifiers_run",
  "cargo_run",
  "formatting_run",
  "http_or_graphql_run",
  "database_run",
  "workflows_or_ci_run",
]) {
  if (contract[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  "PermissionScope::All",
  "pub async fn audit_immutable_artifact_integrity",
  "MAX_PAGE_ARTIFACT_AUDIT_RECORDS: u32 = 512",
  "MAX_PAGE_ARTIFACT_AUDIT_FINDINGS: usize = 64",
]) need(sources.owner, marker, "owner service");

for (const marker of [
  "pub(crate) struct PageArtifactIntegrityAuditMutation",
  "async fn audit_page_artifacts",
  "pub struct AuditGqlPageArtifactsInput",
  "pub max_records: Option<i32>",
  "pub struct GqlPageArtifactIntegrityAuditResult",
  "pub struct GqlPageArtifactIntegrityFinding",
  "Permission::new(Resource::Pages, Action::Manage)",
  "resolve_current_tenant(tenant, &auth, tenant_id)?",
  "audit_immutable_artifact_integrity(tenant_id, page_security(&auth), id, input)",
  '"PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT"',
  '"PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED"',
  '"Immutable artifact audit failed"',
]) need(sources.gql, marker, "GraphQL transport");

const gqlMutation = sliceBetween(
  sources.gql,
  "async fn audit_page_artifacts",
  "#[derive(InputObject)]",
  "GraphQL mutation",
);
requireOrdered(
  gqlMutation,
  [
    "require_module_enabled(ctx, MODULE_SLUG).await?",
    "require_pages_manage_permission(ctx)?",
    "resolve_current_tenant(tenant, &auth, tenant_id)?",
    "audit_immutable_artifact_integrity",
    ".map_err(map_artifact_audit_error)",
  ],
  "GraphQL mutation order",
);

for (const marker of [
  "mod artifact_integrity_audit;",
  "artifact_integrity_audit::PageArtifactIntegrityAuditMutation",
  "AuditGqlPageArtifactsInput",
  "GqlPageArtifactIntegrityAuditResult",
  "GqlPageArtifactIntegrityFinding",
]) need(sources.gqlMod, marker, "GraphQL module wiring");

for (const marker of [
  'path = "/api/admin/pages/{id}/artifacts/audit"',
  "request_body = AuditPageArtifactsInput",
  "body = PageArtifactIntegrityAuditResult",
  "ensure_current_tenant(&tenant, &auth)?",
  "ensure_manage_permission(&auth)?",
  "Permission::new(Resource::Pages, Action::Manage)",
  "audit_immutable_artifact_integrity(tenant.id, page_security(&auth), id, input)",
  '"PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT"',
  '"PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED"',
  '"Immutable artifact audit failed"',
]) need(sources.http, marker, "HTTP transport");

const httpHandler = sliceBetween(
  sources.http,
  "pub async fn audit_page_artifacts",
  "pub(super) fn router",
  "HTTP handler",
);
requireOrdered(
  httpHandler,
  [
    "ensure_current_tenant(&tenant, &auth)?",
    "ensure_manage_permission(&auth)?",
    "audit_immutable_artifact_integrity",
    ".map_err(map_artifact_audit_error)",
  ],
  "HTTP handler order",
);

for (const source of [gqlMutation, httpHandler]) {
  for (const marker of [
    "page_static_landing_artifact",
    "Entity::find",
    "document_html",
    "body_html",
    "runtime_snapshots",
    "materialization_identity",
    ".insert(",
    ".update(",
    ".delete(",
    "event_bus.emit",
    "error.to_string()",
  ]) forbid(source, marker, "transport adapter");
}

for (const marker of [
  "mod artifact_integrity_audit;",
  "pub use artifact_integrity_audit::audit_page_artifacts;",
  ".merge(artifact_integrity_audit::router(runtime)?)",
]) need(sources.httpMod, marker, "HTTP module wiring");

for (const marker of [
  "crate::http::audit_page_artifacts",
  "crate::AuditPageArtifactsInput",
  "crate::PageArtifactIntegrityAuditResult",
  "crate::PageArtifactIntegrityFinding",
]) need(sources.openapi, marker, "OpenAPI registration");

for (const marker of [
  "source-ready / maintainer-validation-pending",
  "auditPageArtifacts",
  "POST /api/admin/pages/{id}/artifacts/audit",
  "PermissionScope::All",
  "PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT",
  "PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED",
  "does not expose raw locale",
  "intentionally not run",
]) need(sources.packet, marker, "transport packet");

for (const marker of [
  "immutable-artifact-integrity-audit-transport-source-ready",
  "GraphQL and HTTP audit transport",
  "repair/rebuild remains open",
  "execution and rollout remain open",
]) need(sources.actualization, marker, "parity actualization");

if (failures.length > 0) {
  console.error("[verify-pages-immutable-artifact-integrity-audit-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-immutable-artifact-integrity-audit-transport] PASS source_ready=true execution=pending",
);
