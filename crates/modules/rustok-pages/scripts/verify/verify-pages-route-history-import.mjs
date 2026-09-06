#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];

const need = (text, marker, label) => {
  if (!text.includes(marker)) failures.push(`${label}: missing ${marker}`);
};
const forbid = (text, marker, label) => {
  if (text.includes(marker)) failures.push(`${label}: forbidden ${marker}`);
};
const ordered = (text, markers, label) => {
  let previous = -1;
  for (const marker of markers) {
    const index = text.indexOf(marker, previous + 1);
    if (index < 0) {
      failures.push(`${label}: missing or out of order ${marker}`);
      return;
    }
    previous = index;
  }
};
const between = (text, start, end, label) => {
  const from = text.indexOf(start);
  const to = from < 0 ? -1 : text.indexOf(end, from + start.length);
  if (from < 0 || to < 0) {
    failures.push(`${label}: unable to locate source slice`);
    return "";
  }
  return text.slice(from, to);
};

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-route-history-import-source.json",
));
const migration = read(
  "crates/rustok-pages/src/migrations/m20260806_000012_create_page_route_history_imports.rs",
);
const migrations = read("crates/rustok-pages/src/migrations/mod.rs");
const entity = read("crates/rustok-pages/src/entities/page_route_history_import.rs");
const entities = read("crates/rustok-pages/src/entities/mod.rs");
const service = read("crates/rustok-pages/src/services/page/route_history_import.rs");
const pageServices = read("crates/rustok-pages/src/services/page/mod.rs");
const services = read("crates/rustok-pages/src/services/mod.rs");
const pagesLib = read("crates/rustok-pages/src/lib.rs");
const regression = read("crates/rustok-pages/tests/page_route_history_import_sqlite.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read(
  "docs/modules/pages-page-builder-route-history-import-packet-2026-08-06.md",
);

if (evidence.format !== "pages_route_history_import_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_route_history_import_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "explicit_route_history_import_owner_added",
  "pages_manage_authorization_required",
  "batch_minimum_is_one",
  "batch_maximum_is_one_hundred",
  "batch_is_single_transaction",
  "source_is_normalized_and_bounded",
  "source_record_id_is_bounded",
  "locale_and_slug_use_pages_normalization",
  "immutable_provenance_receipt_table_added",
  "receipt_has_no_page_foreign_key",
  "receipt_unique_by_tenant_source_record",
  "canonical_request_hash_retained",
  "exact_receipt_replay_is_idempotent",
  "provenance_payload_drift_fails_closed",
  "current_route_owner_overlap_fails_closed",
  "retained_snapshot_owner_overlap_fails_closed",
  "incompatible_alias_overlap_fails_closed",
  "existing_page_import_adds_snapshot_without_gone",
  "existing_page_delete_uses_imported_snapshot",
  "missing_page_import_adds_gone_route",
  "existing_redirect_history_is_preserved",
  "redirect_only_missing_page_requires_terminal_anchor",
  "failed_import_rolls_back_receipts_and_snapshots",
  "stable_import_conflict_error_code_added",
  "automatic_history_inference_is_not_claimed",
  "focused_sqlite_regression_added",
  "production_pages_route_history_behavior_changed",
  "database_schema_changed",
  "migration_changed",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "production_page_builder_behavior_changed",
  "page_body_schema_changed",
  "page_artifact_schema_changed",
  "graphql_schema_changed",
  "rest_http_api_changed",
  "host_route_changed",
  "cache_policy_changed",
  "event_schema_changed",
  "optional_event_infrastructure_changed",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

for (const marker of [
  ".table(PageRouteHistoryImports::Table)",
  "PageRouteHistoryImports::TenantId",
  "PageRouteHistoryImports::Source",
  "PageRouteHistoryImports::SourceRecordId",
  "PageRouteHistoryImports::RequestHash",
  "PageRouteHistoryImports::PageId",
  "PageRouteHistoryImports::Locale",
  "PageRouteHistoryImports::Slug",
  "PageRouteHistoryImports::PageWasMissing",
  "PageRouteHistoryImports::ImportedBy",
  "PageRouteHistoryImports::ImportedAt",
  'name("idx_page_route_history_imports_source")',
  'name("idx_page_route_history_imports_route")',
  'name("idx_page_route_history_imports_audit")',
  ".unique()",
]) need(migration, marker, "route history import migration");
forbid(migration, ".foreign_key(", "forward-only import receipt migration");
need(migrations, "m20260806_000012_create_page_route_history_imports", "migration registry");
for (const marker of [
  '#[sea_orm(table_name = "page_route_history_imports")]',
  "pub tenant_id: Uuid",
  "pub source: String",
  "pub source_record_id: String",
  "pub request_hash: String",
  "pub page_id: Uuid",
  "pub locale: String",
  "pub slug: String",
  "pub page_was_missing: bool",
  "pub imported_by: Option<Uuid>",
]) need(entity, marker, "route history import entity");
need(entities, "pub mod page_route_history_import;", "entity registry");
need(entities, "PageRouteHistoryImport", "entity export");

for (const marker of [
  'MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS: usize = 100',
  'PAGE_ROUTE_HISTORY_IMPORT_CONFLICT: &str = "PAGE_ROUTE_HISTORY_IMPORT_CONFLICT"',
  "pub struct PageRouteHistoryImportService",
  "pub async fn import_public_routes(",
  "enforce_scope(&security, Resource::Pages, Action::Manage)?",
  "let txn = self.db.begin().await?",
  "normalize_source(&input.source)?",
  "normalize_locale(&item.locale)?",
  "normalize_slug(&item.slug)?",
  "import_request_hash(",
  "page_route_history_import::Entity::find()",
  "verify_receipt(receipt, tenant_id, &source, item)?",
  "ensure_route_in_tx(&txn, tenant_id, item, terminal).await?",
  "page_route_history_import::ActiveModel",
  "page_has_terminal_gone_alias",
  "txn.commit().await?",
  "Historical page route import",
]) need(service, marker, "route history import owner");

const owner = between(
  service,
  "pub async fn import_public_routes(",
  "fn prepare_input(",
  "route history import owner",
);
ordered(owner, [
  "enforce_scope(&security, Resource::Pages, Action::Manage)?",
  "prepare_input(tenant_id, input)?",
  "self.db.begin().await?",
  "page_route_history_import::Entity::find()",
  "load_page_for_import(&txn, item.page_id).await?",
  "verify_receipt(receipt, tenant_id, &source, item)?",
  "ensure_route_in_tx(&txn, tenant_id, item, terminal).await?",
  "page_route_history_import::ActiveModel",
  "page_has_terminal_gone_alias",
  "txn.commit().await?",
], "authorization provenance route receipt terminal commit ordering");

const routeEnsure = between(
  service,
  "async fn ensure_route_in_tx(",
  "async fn page_has_terminal_gone_alias(",
  "import route composition",
);
ordered(routeEnsure, [
  "page_translation::Entity::find()",
  "Historical route import overlaps a current route claim",
  "page_route_publication::Entity::find()",
  "page_route_publication::ActiveModel",
  "page_route_alias::Entity::find()",
  "ambiguous current and alias ownership",
  "ROUTE_DISPOSITION_GONE",
  "HISTORICAL_ROUTE_IMPORT_REASON",
  "ROUTE_DISPOSITION_REDIRECT",
], "current snapshot alias composition ordering");

for (const marker of [
  "mod route_history_import;",
  "PageRouteHistoryImportService",
  "PAGE_ROUTE_HISTORY_IMPORT_CONFLICT",
  "MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS",
]) need(pageServices, marker, "page service export");
for (const marker of [
  "PageRouteHistoryImportService",
  "PageRouteHistoryImportItem",
  "ImportPageRouteHistoryInput",
]) need(services, marker, "service root export");
for (const marker of [
  "PageRouteHistoryImport",
  "PageRoutePublication",
  "PageRouteHistoryImportService",
  "PAGE_ROUTE_HISTORY_IMPORT_CONFLICT",
]) need(pagesLib, marker, "crate root export");

for (const marker of [
  "existing_page_import_is_replay_safe_and_becomes_gone_after_delete",
  "deleted_page_import_preserves_redirects_and_rejects_provenance_drift",
  "redirect_only_deleted_page_import_fails_closed_and_rolls_back",
  'import_input("LEGACY-EXPORT"',
  "PageRouteDisposition::Gone",
  "PAGE_ROUTE_HISTORY_IMPORT_CONFLICT",
  "failed import receipts should not commit",
  "failed import snapshots should not commit",
]) need(regression, marker, "SQLite source regression");

for (const marker of [
  "route-history-import-source-ready",
  "Historical route import: source-ready",
  "automatic historical inference remains deliberately unsupported",
]) need(plan, marker, "canonical plan");
for (const marker of [
  "historical route import owner",
  "provenance receipts",
  "Automatic inference remains open by design",
]) need(localPlan, marker, "Pages local plan");
for (const marker of [
  "source-ready / execution-pending",
  "explicit operator import",
  "forward-only `page_route_history_imports` receipt",
  "redirect-only missing-page import",
  "Execution evidence remains pending",
]) need(packet, marker, "route history import packet");

for (const text of [migration, service, regression, packet]) {
  forbid(text, "Iggy", "Pages route history import slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-route-history-import] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-route-history-import] PASS source_ready=true execution=pending mode=explicit_import");
