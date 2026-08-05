#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (relativePath) => fs.readFileSync(path.join(repoRoot, relativePath), "utf8");
const failures = [];

const files = {
  evidence: "crates/rustok-pages/contracts/evidence/pages-published-slug-route-alias-source.json",
  migration: "crates/rustok-pages/src/migrations/m20260805_000010_create_page_route_aliases.rs",
  migrations: "crates/rustok-pages/src/migrations/mod.rs",
  entity: "crates/rustok-pages/src/entities/page_route_alias.rs",
  entities: "crates/rustok-pages/src/entities/mod.rs",
  route: "crates/rustok-pages/src/services/page/route.rs",
  pageModule: "crates/rustok-pages/src/services/page/mod.rs",
  metadata: "crates/rustok-pages/src/services/page/metadata.rs",
  persistence: "crates/rustok-pages/src/services/page/persistence.rs",
  seo: "crates/rustok-pages/src/seo_targets.rs",
  regression: "crates/rustok-pages/tests/page_published_slug_route_alias_sqlite.rs",
  plan: "docs/modules/pages-page-builder-parity-continuation-plan.md",
  localPlan: "crates/rustok-pages/docs/implementation-plan.md",
  packet: "docs/modules/pages-page-builder-published-slug-route-alias-packet-2026-08-05.md",
};

const source = Object.fromEntries(
  Object.entries(files).map(([key, relativePath]) => [key, read(relativePath)]),
);
const evidence = JSON.parse(source.evidence);

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
  if (from < 0) {
    failures.push(`${label}: missing start ${start}`);
    return "";
  }
  const to = text.indexOf(end, from + start.length);
  if (to < 0) {
    failures.push(`${label}: missing end ${end}`);
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
  failures.push("source evidence execution must remain empty");
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
  "Table::create()",
  ".table(PageRouteAliases::Table)",
  "PageRouteAliases::TenantId",
  "PageRouteAliases::PageId",
  "PageRouteAliases::Locale",
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
]) {
  need(source.migration, marker, "route alias migration");
}
forbid(source.migration, ".foreign_key(", "append-only route alias migration");
need(
  source.migrations,
  "m20260805_000010_create_page_route_aliases",
  "Pages migration registry",
);
for (const marker of [
  '#[sea_orm(table_name = "page_route_aliases")]',
  "pub page_id: Uuid",
  "pub disposition: String",
  "pub target_page_id: Option<Uuid>",
  "pub target_locale: Option<String>",
]) {
  need(source.entity, marker, "route alias entity");
}
need(source.entities, "pub mod page_route_alias;", "Pages entity registry");
need(source.entities, "PageRouteAlias", "Pages entity export");

for (const marker of [
  'pub const PAGE_ROUTE_NOT_FOUND: &str = "PAGE_ROUTE_NOT_FOUND"',
  'pub const PAGE_ROUTE_RESOLUTION_CONFLICT: &str = "PAGE_ROUTE_RESOLUTION_CONFLICT"',
  "pub enum PageRouteDisposition",
  "Canonical",
  "Redirect",
  "Gone",
  "pub struct PageRouteDescriptor",
  "pub struct PageRouteResolution",
  "pub struct PageRouteService",
  "pub async fn canonical_descriptor(",
  "pub async fn resolve(",
  "PagesError::Rich(Box::new(",
]) {
  need(source.route, marker, "route resolution owner");
}
const canonical = between(
  source.route,
  "pub async fn canonical_descriptor(",
  "pub async fn resolve(",
  "canonical descriptor",
);
ordered(
  canonical,
  [
    "normalize_locale(locale)?",
    "page::Entity::find_by_id(page_id)",
    "storage_to_status(&page.status)? != ContentStatus::Published",
    "page_translation::Entity::find()",
    "PageRouteDescriptor {",
    "page_route_path(&locale, &slug)",
  ],
  "published canonical descriptor ordering",
);
const resolve = between(
  source.route,
  "pub async fn resolve(",
  "pub(super) async fn ensure_route_alias_claim_available_in_tx(",
  "route resolution",
);
ordered(
  resolve,
  [
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
  ],
  "canonical alias conflict resolution ordering",
);

const aliasClaim = between(
  source.persistence,
  "pub(super) async fn ensure_slug_unique_in_tx(",
  "pub(super) async fn replace_translations_in_tx(",
  "slug claim policy",
);
ordered(
  aliasClaim,
  [
    "page_translation::Entity::find()",
    "return Err(PagesError::duplicate_slug",
    "ensure_route_alias_claim_available_in_tx",
  ],
  "current then historical claim ordering",
);
const metadataWrite = between(
  source.metadata,
  "pub async fn patch_metadata(",
  "\n    }\n}",
  "metadata write",
);
ordered(
  metadataWrite,
  [
    "self.ensure_slug_unique_in_tx(",
    "record_published_slug_redirects_in_tx(",
    "active.update(&txn).await?",
    "self.replace_translations_in_tx(",
    "DomainEvent::NodeUpdated",
    "txn.commit().await?",
  ],
  "metadata route alias transaction ordering",
);
const publishedRedirects = between(
  source.route,
  "pub(super) async fn record_published_slug_redirects_in_tx(",
  "async fn record_redirect_alias_in_tx(",
  "published rename alias writer",
);
ordered(
  publishedRedirects,
  [
    "storage_to_status(page_status)? != ContentStatus::Published",
    "return Ok(())",
    "existing_by_locale",
    "if old_slug == new_slug",
    "record_redirect_alias_in_tx(",
  ],
  "published-only alias write ordering",
);

for (const marker of [
  'format!("/{locale}/modules/pages?slug={slug}")',
  "page_route_for_slug(&effective_locale, translation.slug.as_deref())",
  "page_route_for_slug(&item.locale, Some(slug.as_str()))",
  'Some(slug) => format!("/{locale}/modules/pages?slug={slug}")',
  'None => format!("/{locale}/modules/pages")',
  "matches_module_path(&parsed, \"pages\")",
]) {
  need(source.seo, marker, "localized canonical SEO route");
}

for (const marker of [
  "published_slug_renames_create_immutable_redirects_and_reserve_old_claims",
  "draft_only_slug_renames_do_not_create_public_route_history",
  'resolve(tenant_id, "en", "company")',
  'for old_slug in ["about", "about-us"]',
  "PageRouteDisposition::Redirect",
  'Some("/en/modules/pages?slug=company")',
  "PagesError::DuplicateSlug",
  'create_draft(&service, tenant_id, "Replacement", "draft-old")',
]) {
  need(source.regression, marker, "focused SQLite regression");
}

for (const marker of [
  "published-slug-route-alias-source-ready",
  "Published slug route aliases: source-ready",
  "localized canonical Pages routes",
  "host redirect/gone response remains open",
]) {
  need(source.plan, marker, "canonical Pages/Page Builder plan");
}
for (const marker of [
  "immutable redirects for published slug renames",
  "old published slug claims cannot be reused",
  "host redirect responses, delete tombstones and historical backfill remain open",
]) {
  need(source.localPlan, marker, "Pages local plan");
}
for (const marker of [
  "source-ready / execution-pending",
  "published slug rename",
  "append-only route ledger",
  "draft-only rename",
  "Execution evidence remains pending",
]) {
  need(source.packet, marker, "published slug route alias packet");
}

for (const text of [source.migration, source.route, source.metadata, source.seo, source.regression]) {
  forbid(text, "Iggy", "Pages routing slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-published-slug-route-alias] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log(
  "[verify-pages-published-slug-route-alias] PASS source_ready=true execution=pending",
);
