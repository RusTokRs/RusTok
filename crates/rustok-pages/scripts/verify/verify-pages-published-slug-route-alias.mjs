#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-published-slug-route-alias-source.json",
));
const migration = read("crates/rustok-pages/src/migrations/m20260805_000010_create_page_route_aliases.rs");
const migrations = read("crates/rustok-pages/src/migrations/mod.rs");
const entity = read("crates/rustok-pages/src/entities/page_route_alias.rs");
const entities = read("crates/rustok-pages/src/entities/mod.rs");
const route = read("crates/rustok-pages/src/services/page/route.rs");
const metadata = read("crates/rustok-pages/src/services/page/metadata.rs");
const persistence = read("crates/rustok-pages/src/services/page/persistence.rs");
const seo = read("crates/rustok-pages/src/seo_targets.rs");
const regression = read("crates/rustok-pages/tests/page_published_slug_route_alias_sqlite.rs");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read("docs/modules/pages-page-builder-published-slug-route-alias-packet-2026-08-05.md");

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

if (evidence.format !== "pages_published_slug_route_alias_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_published_slug_route_alias_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "page_route_alias_table_added",
  "route_alias_claim_unique_by_tenant_locale_slug",
  "route_alias_history_has_no_page_foreign_key",
  "transport_neutral_route_resolver_added",
  "canonical_redirect_and_gone_dispositions_defined",
  "canonical_descriptor_requires_published_page",
  "current_and_alias_claim_collision_fails_closed",
  "published_slug_rename_appends_redirect_in_metadata_transaction",
  "draft_slug_rename_does_not_append_public_history",
  "historical_slug_claim_cannot_be_reused",
  "redirect_recomputes_current_target_slug",
  "multiple_published_renames_chain_to_current_canonical",
  "localized_canonical_seo_route_added",
  "localized_hreflang_alternates_added",
  "legacy_unprefixed_module_route_remains_parseable",
  "stable_route_not_found_error_code_added",
  "stable_route_conflict_error_code_added",
  "focused_sqlite_regression_added",
  "production_pages_metadata_behavior_changed",
  "production_pages_route_resolution_added",
  "production_pages_seo_canonical_behavior_changed",
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
  "http_route_changed",
  "host_redirect_response_added",
  "delete_tombstones_added",
  "historical_backfill_added",
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
  ".table(PageRouteAliases::Table)",
  "PageRouteAliases::TenantId",
  "PageRouteAliases::PageId",
  "PageRouteAliases::Locale",
  ".string_len(32)",
  "PageRouteAliases::Slug",
  "PageRouteAliases::Disposition",
  "PageRouteAliases::TargetPageId",
  "PageRouteAliases::TargetLocale",
  "PageRouteAliases::Reason",
  'name("idx_page_route_aliases_claim")',
  ".col(PageRouteAliases::TenantId)",
  ".col(PageRouteAliases::Locale)",
  ".col(PageRouteAliases::Slug)",
  ".unique()",
]) need(migration, marker, "route alias migration");
forbid(migration, ".foreign_key(", "append-only route alias migration");
need(migrations, "m20260805_000010_create_page_route_aliases", "migration registry");
for (const marker of [
  '#[sea_orm(table_name = "page_route_aliases")]',
  "pub page_id: Uuid",
  "pub disposition: String",
  "pub target_page_id: Option<Uuid>",
  "pub target_locale: Option<String>",
]) need(entity, marker, "route alias entity");
need(entities, "pub mod page_route_alias;", "entity registry");
need(entities, "PageRouteAlias", "entity export");

for (const marker of [
  'PAGE_ROUTE_NOT_FOUND: &str = "PAGE_ROUTE_NOT_FOUND"',
  'PAGE_ROUTE_RESOLUTION_CONFLICT: &str = "PAGE_ROUTE_RESOLUTION_CONFLICT"',
  "pub enum PageRouteDisposition",
  "Canonical",
  "Redirect",
  "Gone",
  "pub struct PageRouteService",
  "pub async fn canonical_descriptor(",
  "pub async fn resolve(",
  "PagesError::Rich(Box::new(",
]) need(route, marker, "route owner");

const canonical = between(
  route,
  "pub async fn canonical_descriptor(",
  "pub async fn resolve(",
  "canonical descriptor",
);
ordered(canonical, [
  "normalize_locale(locale)?",
  "page::Entity::find_by_id(page_id)",
  "storage_to_status(&page.status)? != ContentStatus::Published",
  "page_translation::Entity::find()",
  "PageRouteDescriptor {",
  "page_route_path(&locale, &slug)",
], "published canonical descriptor ordering");

const resolver = between(
  route,
  "pub async fn resolve(",
  "pub(super) async fn ensure_route_alias_claim_available_in_tx(",
  "route resolver",
);
ordered(resolver, [
  "load_current_published_routes",
  "page_route_alias::Entity::find()",
  "match (current.as_slice(), aliases.as_slice())",
  "([route], [])",
  "PageRouteDisposition::Canonical",
  "([], [alias])",
  "ROUTE_DISPOSITION_GONE",
  "PageRouteDisposition::Gone",
  "ROUTE_DISPOSITION_REDIRECT",
  ".canonical_descriptor(tenant_id, target_page_id, target_locale)",
  "PageRouteDisposition::Redirect",
  "([], []) => Err(page_route_not_found())",
  "_ => Err(page_route_resolution_conflict())",
], "canonical redirect gone conflict ordering");

const claim = between(
  persistence,
  "pub(super) async fn ensure_slug_unique_in_tx(",
  "pub(super) async fn replace_translations_in_tx(",
  "route claim policy",
);
ordered(claim, [
  "page_translation::Entity::find()",
  "return Err(PagesError::duplicate_slug",
  "ensure_route_alias_claim_available_in_tx",
], "current then historical claim ordering");

const metadataWrite = between(
  metadata,
  "pub async fn patch_metadata(",
  "\n    }\n}",
  "metadata write",
);
ordered(metadataWrite, [
  "self.ensure_slug_unique_in_tx(",
  "record_published_slug_redirects_in_tx(",
  "active.update(&txn).await?",
  "self.replace_translations_in_tx(",
  "DomainEvent::NodeUpdated",
  "txn.commit().await?",
], "metadata alias transaction ordering");

const aliasWriter = between(
  route,
  "pub(super) async fn record_published_slug_redirects_in_tx(",
  "async fn record_redirect_alias_in_tx(",
  "published rename writer",
);
ordered(aliasWriter, [
  "storage_to_status(page_status)? != ContentStatus::Published",
  "return Ok(())",
  "existing_by_locale",
  "if old_slug == new_slug",
  "record_redirect_alias_in_tx(",
], "published-only alias ordering");

for (const marker of [
  'format!("/{locale}/modules/pages?slug={slug}")',
  "page_route_for_slug(&effective_locale, translation.slug.as_deref())",
  "page_route_for_slug(&item.locale, Some(slug.as_str()))",
  'None => format!("/{locale}/modules/pages")',
  "matches_module_path(&parsed, \"pages\")",
]) need(seo, marker, "localized canonical SEO route");

for (const marker of [
  "published_slug_renames_create_immutable_redirects_and_reserve_old_claims",
  "draft_only_slug_renames_do_not_create_public_route_history",
  'resolve(tenant_id, "en", "company")',
  'for old_slug in ["about", "about-us"]',
  "PageRouteDisposition::Redirect",
  'Some("/en/modules/pages?slug=company")',
  "PagesError::DuplicateSlug",
  'create_draft(&service, tenant_id, "Replacement", "draft-old")',
]) need(regression, marker, "SQLite source regression");

for (const marker of [
  "published-slug-route-alias-source-ready",
  "Published slug route aliases: source-ready",
  "Localized canonical Pages routes",
  "Host redirect/gone response remains open",
]) need(plan, marker, "canonical plan");
for (const marker of [
  "immutable redirects for published slug renames",
  "Old published slug claims cannot be reused",
  "Host redirect responses, delete tombstones and historical backfill remain open",
]) need(localPlan, marker, "Pages local plan");
for (const marker of [
  "source-ready / execution-pending",
  "published slug rename",
  "append-only route ledger",
  "draft-only rename",
  "Execution evidence remains pending",
]) need(packet, marker, "route alias packet");

for (const text of [migration, route, metadata, seo, regression]) {
  forbid(text, "Iggy", "Pages routing slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-slug-route-alias] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-published-slug-route-alias] PASS source_ready=true execution=pending");
