#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..", "..");
const failures = [];
const files = {
  test: "crates/modules/rustok-pages/tests/explicit_artifact_repair_request_contract.rs",
  evidence: "crates/modules/rustok-pages/contracts/evidence/pages-explicit-artifact-repair-request-contract-source.json",
  transportDoc: "crates/modules/rustok-pages/docs/explicit-artifact-repair-transports.md",
  auditDoc: "crates/modules/rustok-pages/docs/immutable-artifact-integrity-audit.md",
  auditTransportDoc: "crates/modules/rustok-pages/docs/immutable-artifact-integrity-audit-transport.md",
  rebuildDoc: "crates/modules/rustok-pages/docs/explicit-immutable-artifact-rebuild.md",
  activationDoc: "crates/modules/rustok-pages/docs/explicit-immutable-artifact-binding-replacement.md",
};
const absolute = (relative) => path.join(repoRoot, relative);
const read = (relative) => fs.readFileSync(absolute(relative), "utf8");
const need = (source, marker, label) => {
  if (!source.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (source, marker, label) => {
  if (source.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};

for (const [label, relative] of Object.entries(files)) {
  if (!fs.existsSync(absolute(relative))) {
    failures.push(`${label}: missing ${relative}`);
    continue;
  }
  const stats = fs.lstatSync(absolute(relative));
  if (!stats.isFile() || stats.isSymbolicLink()) {
    failures.push(`${label}: ${relative} must be a regular non-symlink file`);
  }
}
if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-request-contract] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(1);
}

const sources = Object.fromEntries(
  Object.entries(files).map(([label, relative]) => [label, read(relative)]),
);
const evidence = JSON.parse(sources.evidence);
const contract = evidence.source_contract ?? {};

if (evidence.format !== "pages_explicit_artifact_repair_request_contract_source_v1") {
  failures.push("evidence format drifted");
}
if (evidence.status !== "pages_explicit_artifact_repair_request_contract_source_unvalidated") {
  failures.push("evidence status drifted");
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("evidence execution must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "graphql_requests_are_executed_by_harness",
  "http_requests_are_dispatched_by_harness",
  "graphql_module_enablement_is_backed_by_minimal_tenant_modules_fixture",
  "current_tenant_mismatch_is_rejected",
  "missing_pages_manage_is_rejected",
  "pages_manage_present_reaches_owner_validation",
  "pages_manage_snapshot_resolves_permission_scope_all",
  "pages_manage_absent_resolves_permission_scope_none",
  "pages_manage_permission_scope_own_is_not_currently_representable",
  "owner_permission_scope_all_guard_remains_defense_in_depth",
  "graphql_permission_error_is_static",
  "graphql_validation_error_is_static",
  "http_permission_error_is_static",
  "http_validation_error_is_static",
  "nil_page_validation_prevents_owner_database_writes",
]) {
  if (contract[key] !== true) failures.push(`source_contract.${key} must be true`);
}
for (const key of [
  "automatic_audit_to_rebuild",
  "automatic_rebuild_to_activation",
  "tests_run",
  "source_verifier_run",
  "cargo_run",
  "formatting_run",
  "graphql_or_http_run",
  "database_write_scenario_run",
  "workflows_or_ci_run",
]) {
  if (contract[key] !== false) failures.push(`source_contract.${key} must remain false`);
}

for (const marker of [
  "Schema::build(",
  "PagesQuery::default()",
  "PagesMutation::default()",
  "GraphqlRequest::new(query).variables(variables)",
  "rustok_pages::http::axum_router(&host)?",
  ".oneshot(http_request(",
  "TenantContextExtension(tenant)",
  "AuthContextExtension(auth)",
  "Permission::new(Resource::Pages, Action::Manage)",
  "PermissionScope::All",
  "PermissionScope::None",
  "PermissionScope::Own",
  "CREATE TABLE tenant_modules",
  '"pages".into()',
  '"PERMISSION_DENIED"',
  '"PAGES_PERMISSION_DENIED"',
  '"PAGE_ARTIFACT_REPAIR_INVALID_INPUT"',
  '"Invalid immutable artifact rebuild input"',
  '"Invalid rebuilt artifact activation input"',
  "Uuid::nil()",
]) need(sources.test, marker, "request contract test");

for (const marker of [
  "PagesModule.migrations",
  "page_publish_rebuild_source",
  "page_artifact_rebuild_operation",
  "page_artifact_binding_replacement_operation",
  "page_static_landing_artifact",
  "page_published_landing_artifact",
  "Entity::find",
  "Entity::insert",
  "rebuild_immutable_artifact(",
  "replace_rebuilt_artifact_binding(",
]) forbid(sources.test, marker, "request contract test");

for (const [label, source] of [
  ["transport doc", sources.transportDoc],
  ["audit doc", sources.auditDoc],
  ["audit transport doc", sources.auditTransportDoc],
  ["rebuild doc", sources.rebuildDoc],
  ["activation doc", sources.activationDoc],
]) {
  need(source, "PermissionScope::All", label);
  need(source, "PermissionScope::None", label);
}

for (const [label, source, marker] of [
  [
    "transport doc",
    sources.transportDoc,
    "An owner-scoped Manage grant may pass the adapter's coarse permission check but is rejected by the owner command before writes.",
  ],
  [
    "audit doc",
    sources.auditDoc,
    "An owner-scoped `pages:manage` permission does not authorize reading retained page-wide artifacts.",
  ],
  [
    "audit transport doc",
    sources.auditTransportDoc,
    "owner-scoped Manage rejection by the service",
  ],
  ["rebuild doc", sources.rebuildDoc, "Owner-scoped Manage is insufficient."],
  ["activation doc", sources.activationDoc, "Owner-scoped Manage is insufficient."],
]) forbid(source, marker, label);

if (failures.length > 0) {
  console.error("[verify-pages-explicit-artifact-repair-request-contract] FAIL");
  failures.forEach((failure) => console.error(`- ${failure}`));
  process.exit(Math.min(failures.length, 255));
}
console.log("[verify-pages-explicit-artifact-repair-request-contract] PASS source_ready=true execution=pending");
