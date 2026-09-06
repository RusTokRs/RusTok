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

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-delete-route-tombstone-source.json",
));
const migration = read(
  "crates/rustok-pages/src/migrations/m20260806_000011_create_page_route_publications.rs",
);
const migrations = read("crates/rustok-pages/src/migrations/mod.rs");
const entity = read("crates/rustok-pages/src/entities/page_route_publication.rs");
const entities = read("crates/rustok-pages/src/entities/mod.rs");
const route = read("crates/rustok-pages/src/services/page/route.rs");
const lifecycle = read("crates/rustok-pages/src/services/page/lifecycle.rs");
const regression = read("crates/rustok-pages/tests/page_delete_route_tombstone_sqlite.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const actualization = read("docs/modules/page-builder-parity-actualization-2026-08-06.md");
const packet = read("docs/modules/pages-page-builder-delete-route-tombstone-packet-2026-08-06.md");

if (evidence.format !== "pages_delete_route_tombstone_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_delete_route_tombstone_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "published_route_snapshot_table_added",
  "snapshot_claim_unique_by_tenant_locale_slug",
  "snapshot_history_survives_page_delete",
  "published_routes_snapshotted_before_unpublish",
  "published_routes_snapshotted_before_archive",
  "never_published_draft_routes_not_snapshotted",
  "delete_tombstones_written_in_owner_transaction",
  "current_public_route_becomes_gone",
  "multiple_public_route_snapshots_become_gone",
  "existing_redirect_history_preserved",
  "redirect_to_deleted_page_folds_to_gone",
  "deleted_public_claims_cannot_be_reused",
  "draft_only_deleted_claim_can_be_reused",
  "focused_sqlite_regression_added",
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
  "cache_policy_changed",
  "event_schema_changed",
  "optional_event_infrastructure_changed",
  "historical_backfill_added",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}

for (const marker of [
  ".table(PageRoutePublications::Table)",
  'name("idx_page_route_publications_claim")',
  ".col(PageRoutePublications::TenantId)",
  ".col(PageRoutePublications::Locale)",
  ".col(PageRoutePublications::Slug)",
  ".unique()",
  'name("idx_page_route_publications_page")',
]) need(migration, marker, "published route snapshot migration");
forbid(migration, ".foreign_key(", "forward-only published route snapshot migration");
need(migrations, "m20260806_000011_create_page_route_publications", "migration registry");
for (const marker of [
  '#[sea_orm(table_name = "page_route_publications")]',
  "pub tenant_id: Uuid",
  "pub page_id: Uuid",
  "pub locale: String",
  "pub slug: String",
]) need(entity, marker, "published route snapshot entity");
need(entities, "pub mod page_route_publication;", "entity registry");
need(entities, "PageRoutePublication", "entity export");

for (const marker of [
  "record_published_route_snapshots_in_tx",
  "storage_to_status(page_status)? != ContentStatus::Published",
  "page_route_publication::Entity::find()",
  "record_delete_route_tombstones_in_tx",
  'const PAGE_DELETED_ROUTE_REASON: &str = "Page deleted"',
  "record_gone_alias_in_tx",
  "Preserve immutable redirect history",
  "page_has_gone_tombstone",
  "PageRouteDisposition::Gone",
]) need(route, marker, "route owner");

ordered(lifecycle, [
  "if existing.status == \"published\"",
  "record_delete_route_tombstones_in_tx(&txn, tenant_id, page_id).await?",
  "page_translation::Entity::delete_many()",
  "page::Entity::delete_by_id(page_id)",
  "DomainEvent::NodeDeleted",
  "txn.commit().await?",
], "delete owner transaction ordering");
ordered(lifecycle, [
  "if transition == PageTransition::Publish",
  "} else {",
  "record_published_route_snapshots_in_tx(",
  "let mut active: page::ActiveModel = existing.into()",
  "active.update(&txn).await?",
], "public route snapshot transition ordering");

for (const marker of [
  "delete_turns_every_retained_public_route_into_gone_without_rewriting_redirects",
  "deleting_a_never_published_draft_does_not_reserve_its_slug",
  'for slug in ["about", "about-us", "company"]',
  "PageRouteDisposition::Gone",
  "PagesError::DuplicateSlug",
  'create_draft(&service, tenant_id, "Replacement", "temporary")',
]) need(regression, marker, "SQLite source regression");

for (const marker of [
  "delete-route-tombstone-source-ready",
  "Delete route tombstones: source-ready",
  "route-history-import-source-ready",
  "Historical route import: source-ready",
]) need(plan, marker, "canonical plan");
for (const marker of [
  "Delete route tombstones",
  "never-published drafts",
  "Page Builder behavior is unchanged",
]) need(actualization, marker, "parity actualization");
for (const marker of [
  "source-ready / execution-pending",
  "published route snapshot ledger",
  "preserves redirect history",
  "Execution evidence remains pending",
]) need(packet, marker, "delete tombstone packet");

for (const text of [migration, route, lifecycle, regression]) {
  forbid(text, "Iggy", "Pages route deletion slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-delete-route-tombstone] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-delete-route-tombstone] PASS source_ready=true execution=pending");
