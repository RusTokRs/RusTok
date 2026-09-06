#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const failures = [];
const files = {
  rebuildOwner: "crates/rustok-pages/src/services/page/artifact_rebuild.rs",
  activationOwner: "crates/rustok-pages/src/services/page/artifact_binding_replacement.rs",
  transportDto: "crates/rustok-pages/src/dto/artifact_repair_transport.rs",
  dtoMod: "crates/rustok-pages/src/dto/mod.rs",
  gql: "crates/rustok-pages/src/graphql/artifact_repair.rs",
  gqlMod: "crates/rustok-pages/src/graphql/mod.rs",
  http: "crates/rustok-pages/src/http/artifact_repair.rs",
  httpMod: "crates/rustok-pages/src/http.rs",
  openapi: "crates/rustok-pages/src/openapi.rs",
  evidence: "crates/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-transport-source.json",
  packet: "crates/rustok-pages/docs/explicit-artifact-repair-transports.md",
  actualization: "docs/modules/page-builder-parity-actualization-2026-08-05.md",
  continuation: "docs/modules/pages-page-builder-rebuild-provenance-continuation-2026-08-06.md",
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
  console.error("[verify-pages-explicit-artifact-repair-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relativePath]) => [label, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_explicit_artifact_repair_transport_source_v1") {
  failures.push("source evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_repair_transport_source_unvalidated") {
  failures.push("source evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("source evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`source evidence validation.${key} must remain false`);
}

for (const [key, expected] of Object.entries({
  rebuild_service_command: "PageService::rebuild_immutable_artifact",
  activation_service_command: "PageService::replace_rebuilt_artifact_binding",
  graphql_rebuild_mutation: "rebuildPageArtifact",
  graphql_activation_mutation: "activateRebuiltPageArtifact",
  http_rebuild_method: "POST",
  http_rebuild_path: "/api/admin/pages/{id}/artifacts/rebuild",
  http_activation_method: "POST",
  http_activation_path: "/api/admin/pages/{id}/artifacts/activate",
  required_permission: "pages:manage",
})) {
  if (contract[key] !== expected) failures.push(`source_contract.${key} drifted`);
}
for (const key of [
  "current_tenant_only",
  "owner_services_recheck_permission_scope_all",
  "graphql_inputs_match_explicit_owner_commands",
  "http_inputs_reuse_owner_dtos",
  "transport_results_are_bounded",
  "transport_results_omit_provenance_source_id",
  "transport_results_omit_publish_operation_id",
  "transport_results_omit_storage_instance_key",
  "transport_results_omit_idempotency_key",
  "transport_results_omit_runtime_context",
  "transport_results_omit_materialization_identity",
  "transport_results_omit_runtime_snapshots",
  "adapters_delegate_to_owner_services",
  "public_error_codes_and_messages_are_static",
  "openapi_registered",
]) {
  if (contract[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "adapters_query_owner_tables",
  "adapters_mutate_artifacts_or_bindings_directly",
  "adapters_emit_events_directly",
  "adapters_return_internal_error_text",
  "audit_automatically_starts_rebuild",
  "rebuild_automatically_starts_activation",
  "adds_admin_ui",
  "adds_database_schema",
  "tests_run",
  "static_verifier_run",
  "cargo_run",
  "formatting_run",
  "graphql_or_http_run",
  "openapi_generation_run",
  "database_scenario_run",
  "workflows_or_ci_run",
]) {
  if (contract[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  "pub async fn rebuild_immutable_artifact",
  "PermissionScope::All",
  "page_artifact_rebuild_operation::ActiveModel",
]) need(sources.rebuildOwner, marker, "rebuild owner");
for (const marker of [
  "pub async fn replace_rebuilt_artifact_binding",
  "PermissionScope::All",
  "page_artifact_binding_replacement_operation::ActiveModel",
  "DomainEvent::NodeUpdated",
  "DomainEvent::NodePublished",
]) need(sources.activationOwner, marker, "activation owner");

for (const marker of [
  "pub struct RebuildPageArtifactTransportResult",
  "pub struct ActivateRebuiltPageArtifactTransportResult",
  "impl From<RebuildPageArtifactResult>",
  "impl From<ReplacePageArtifactBindingResult>",
]) need(sources.transportDto, marker, "transport result DTOs");
for (const marker of [
  "pub source_id:",
  "pub source_publish_operation_id:",
  "pub artifact_instance_key:",
  "pub idempotency_key:",
  "pub runtime:",
  "pub materialization_identity:",
  "pub runtime_snapshots:",
]) forbid(sources.transportDto, marker, "bounded transport result DTOs");
for (const marker of [
  "pub mod artifact_repair_transport;",
  "ActivateRebuiltPageArtifactTransportResult",
  "RebuildPageArtifactTransportResult",
]) need(sources.dtoMod, marker, "DTO wiring");

for (const marker of [
  "pub(crate) struct PageArtifactRepairMutation",
  "async fn rebuild_page_artifact",
  "async fn activate_rebuilt_page_artifact",
  "pub struct RebuildGqlPageArtifactInput",
  "pub struct ActivateGqlRebuiltPageArtifactInput",
  "pub struct GqlRebuildPageArtifactResult",
  "pub struct GqlActivateRebuiltPageArtifactResult",
  "Permission::new(Resource::Pages, Action::Manage)",
  "resolve_current_tenant(tenant, &auth, tenant_id)?",
  "rebuild_immutable_artifact",
  "replace_rebuilt_artifact_binding",
  "map_rebuild_error",
  "map_activation_error",
]) need(sources.gql, marker, "GraphQL transport");

const gqlRebuild = sliceBetween(
  sources.gql,
  "async fn rebuild_page_artifact",
  "async fn activate_rebuilt_page_artifact",
  "GraphQL rebuild mutation",
);
const gqlActivation = sliceBetween(
  sources.gql,
  "async fn activate_rebuilt_page_artifact",
  "#[derive(InputObject)]",
  "GraphQL activation mutation",
);
for (const [label, mutation, ownerCall, mapper] of [
  ["GraphQL rebuild", gqlRebuild, "rebuild_immutable_artifact", "map_rebuild_error"],
  ["GraphQL activation", gqlActivation, "replace_rebuilt_artifact_binding", "map_activation_error"],
]) {
  requireOrdered(
    mutation,
    [
      "require_module_enabled(ctx, MODULE_SLUG).await?",
      "require_pages_manage_permission(ctx)?",
      "resolve_current_tenant(tenant, &auth, tenant_id)?",
      ownerCall,
      mapper,
    ],
    `${label} order`,
  );
}

const gqlRebuildResult = sliceBetween(
  sources.gql,
  "pub struct GqlRebuildPageArtifactResult",
  "pub struct GqlActivateRebuiltPageArtifactResult",
  "GraphQL rebuild result",
);
const gqlActivationResult = sliceBetween(
  sources.gql,
  "pub struct GqlActivateRebuiltPageArtifactResult",
  "impl From<RebuildPageArtifactTransportResult>",
  "GraphQL activation result",
);
for (const [label, result] of [
  ["GraphQL rebuild result", gqlRebuildResult],
  ["GraphQL activation result", gqlActivationResult],
]) {
  for (const marker of [
    "source_id",
    "source_publish_operation_id",
    "artifact_instance_key",
    "idempotency_key",
    "runtime",
    "materialization_identity",
    "runtime_snapshots",
  ]) forbid(result, marker, label);
}
for (const marker of [
  "error.to_string()",
  "page_publish_rebuild_source",
  "page_static_landing_artifact",
  "page_published_landing_artifact",
  "Entity::find",
  ".insert(",
  ".update(",
  ".delete(",
  "audit_immutable_artifact_integrity",
]) forbid(sources.gql, marker, "GraphQL adapter boundary");
for (const marker of [
  "mod artifact_repair;",
  "artifact_repair::PageArtifactRepairMutation",
  "RebuildGqlPageArtifactInput",
  "ActivateGqlRebuiltPageArtifactInput",
]) need(sources.gqlMod, marker, "GraphQL module wiring");

for (const marker of [
  'path = "/api/admin/pages/{id}/artifacts/rebuild"',
  'path = "/api/admin/pages/{id}/artifacts/activate"',
  "request_body = RebuildPageArtifactInput",
  "request_body = ReplacePageArtifactBindingInput",
  "body = RebuildPageArtifactTransportResult",
  "body = ActivateRebuiltPageArtifactTransportResult",
  "Permission::new(Resource::Pages, Action::Manage)",
  "ensure_current_tenant(&tenant, &auth)?",
  "ensure_manage_permission(&auth)?",
  "rebuild_immutable_artifact",
  "replace_rebuilt_artifact_binding",
  "map_rebuild_error",
  "map_activation_error",
]) need(sources.http, marker, "HTTP transport");

const httpRebuild = sliceBetween(
  sources.http,
  "pub async fn rebuild_page_artifact",
  "#[utoipa::path(\n    post,\n    path = \"/api/admin/pages/{id}/artifacts/activate\"",
  "HTTP rebuild handler",
);
const httpActivation = sliceBetween(
  sources.http,
  "pub async fn activate_rebuilt_page_artifact",
  "pub(super) fn router",
  "HTTP activation handler",
);
for (const [label, handler, ownerCall, mapper] of [
  ["HTTP rebuild", httpRebuild, "rebuild_immutable_artifact", "map_rebuild_error"],
  ["HTTP activation", httpActivation, "replace_rebuilt_artifact_binding", "map_activation_error"],
]) {
  requireOrdered(
    handler,
    [
      "ensure_current_tenant(&tenant, &auth)?",
      "ensure_manage_permission(&auth)?",
      ownerCall,
      mapper,
    ],
    `${label} order`,
  );
}
for (const marker of [
  "error.to_string()",
  "page_publish_rebuild_source",
  "page_static_landing_artifact",
  "page_published_landing_artifact",
  "Entity::find",
  ".insert(",
  ".update(",
  ".delete(",
  "audit_immutable_artifact_integrity",
]) forbid(sources.http, marker, "HTTP adapter boundary");
for (const marker of [
  "mod artifact_repair;",
  "pub use artifact_repair::{activate_rebuilt_page_artifact, rebuild_page_artifact};",
  ".merge(artifact_repair::router(runtime)?)",
]) need(sources.httpMod, marker, "HTTP module wiring");

for (const marker of [
  "crate::http::rebuild_page_artifact",
  "crate::http::activate_rebuilt_page_artifact",
  "crate::RebuildPageArtifactInput",
  "crate::RebuildPageArtifactTransportResult",
  "crate::ReplacePageArtifactBindingInput",
  "crate::ActivateRebuiltPageArtifactTransportResult",
]) need(sources.openapi, marker, "OpenAPI registration");

for (const marker of [
  "source-ready / maintainer-validation-pending",
  "rebuildPageArtifact",
  "activateRebuiltPageArtifact",
  "POST /api/admin/pages/{id}/artifacts/rebuild",
  "POST /api/admin/pages/{id}/artifacts/activate",
  "PermissionScope::All",
  "No `PagesError` text",
  "deliberately omit",
  "intentionally not run",
]) need(sources.packet, marker, "transport packet");
for (const marker of [
  "explicit-artifact-repair-transport-source-ready",
  "Bounded tenant-admin repair transports",
  "repair transport execution evidence",
]) {
  need(sources.actualization, marker, "parity actualization");
  need(sources.continuation, marker, "rebuild continuation");
}

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-transport] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log(
  "[verify-pages-explicit-artifact-repair-transport] PASS source_ready=true execution=pending",
);
