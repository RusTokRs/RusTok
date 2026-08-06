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
  "crates/rustok-pages/contracts/evidence/pages-storefront-composition-etag-source.json",
));
const routeAdapter = read(
  "crates/rustok-pages/storefront/src/transport/host_route_adapter.rs",
);
const navigationModel = read("crates/rustok-navigation/storefront/src/model.rs");
const navigationLib = read("crates/rustok-navigation/storefront/src/lib.rs");
const navigationUi = read("crates/rustok-navigation/storefront/src/ui/menu.rs");
const composition = read("apps/storefront/src/shared/context/pages_composition.rs");
const host = read("apps/storefront/src/lib.rs");
const cargo = read("apps/storefront/Cargo.toml");
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read(
  "docs/modules/pages-page-builder-storefront-composition-etag-packet-2026-08-06.md",
);

if (evidence.format !== "pages_storefront_composition_etag_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_storefront_composition_etag_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "canonical_route_decision_exposes_route_generation",
  "canonical_route_decision_exposes_page_generation",
  "canonical_route_decision_exposes_artifact_generation",
  "canonical_route_decision_exposes_channel_identity",
  "channel_module_admission_precedes_generation_read",
  "page_publication_and_channel_visibility_precede_generation_read",
  "missing_generation_runtime_disables_etag_without_disabling_ssr",
  "navigation_header_and_footer_are_loaded_once_by_host",
  "navigation_owner_transport_is_reused",
  "preloaded_navigation_snapshot_is_reused_by_ssr_components",
  "seo_owner_context_is_loaded_before_pages_render",
  "composition_etag_binds_pages_generations",
  "composition_etag_binds_canonical_page_locale_and_slug",
  "composition_etag_binds_channel_identity",
  "composition_etag_binds_navigation_payload",
  "composition_etag_binds_seo_payload",
  "composition_etag_binds_rendered_html",
  "composition_etag_uses_sha256",
  "composition_etag_is_canonical_only",
  "terminal_routes_never_claim_composition_etag",
  "strong_weak_and_list_if_none_match_are_supported",
  "matching_if_none_match_returns_304",
  "conditional_304_uses_fully_rendered_document_identity",
  "canonical_etag_responses_use_private_no_cache",
  "terminal_route_responses_remain_private_no_store",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "navigation_ownership_changed",
  "seo_ownership_changed",
  "page_builder_behavior_changed",
  "database_schema_changed",
  "migration_changed",
  "graphql_schema_changed",
  "rest_http_api_changed",
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
  "pub canonical_locale: Option<String>",
  "pub channel_id: Option<String>",
  "pub route_generation: Option<u64>",
  "pub page_generation: Option<u64>",
  "pub artifact_generation: Option<u64>",
  "PagesCacheReadRuntime",
  "generation_snapshot(tenant_id).await",
  "composition ETag disabled",
]) need(routeAdapter, marker, "Pages host route decision generations");

const routeOwner = between(
  routeAdapter,
  "async fn storefront_page_route_native(",
  '#[cfg(not(feature = "ssr"))]',
  "Pages route owner",
);
ordered(routeOwner, [
  ".is_module_enabled(channel_id, MODULE_SLUG)",
  ".resolve(tenant_id, candidate_locale.as_str(), page_slug.as_str())",
  ".get_with_locale_fallback(",
  "page.status != ContentStatus::Published",
  "is_visible_for_public_channel(",
  "shared_get::<PagesCacheReadRuntime>()",
  "generation_snapshot(tenant_id).await",
  "StorefrontPageRouteDecision {",
], "admission route visibility generations ordering");

for (const marker of [
  "pub struct StorefrontNavigationSnapshot",
  "pub header: Option<StorefrontMenu>",
  "pub footer: Option<StorefrontMenu>",
  "pub fn menu(&self, location: StorefrontMenuLocation)",
]) need(navigationModel, marker, "Navigation snapshot model");
for (const marker of [
  "StorefrontNavigationSnapshot",
  "fetch_active_menu",
  "NavigationTransportError",
]) need(navigationLib, marker, "Navigation public composition contract");
for (const marker of [
  "NavigationSnapshotProvider",
  "provide_context(snapshot)",
  "use_context::<StorefrontNavigationSnapshot>()",
  ".menu(location)",
  "Resource::new_blocking",
]) need(navigationUi, marker, "Navigation SSR snapshot reuse");
ordered(navigationUi, [
  "use_context::<StorefrontNavigationSnapshot>()",
  ".menu(location)",
  "Resource::new_blocking",
], "preloaded Navigation before fallback transport");

for (const marker of [
  'PAGES_STOREFRONT_COMPOSITION_FORMAT: &str = "pages_storefront_composition_v1"',
  'PAGES_STOREFRONT_REVALIDATE_CACHE_CONTROL: &str = "private, no-cache"',
  "StorefrontPageRouteDisposition::Canonical",
  "canonical_page_id: decision.canonical_page_id.as_deref()?",
  "route_generation: decision.route_generation?",
  "page_generation: decision.page_generation?",
  "artifact_generation: decision.artifact_generation?",
  "rendered_html_hash: String",
  "rendered_html: &str",
  "Sha256::digest(rendered_html.as_bytes())",
  "seo,",
  "navigation,",
  "Sha256::digest(encoded)",
  "candidate.strip_prefix(\"W/\") == Some(etag)",
  "candidate == \"*\"",
  "composition_etag_is_stable_and_binds_every_dependency",
  "changed rendered HTML should still produce an ETag",
  "incomplete_or_terminal_decisions_do_not_claim_composition_cache_identity",
]) need(composition, marker, "Pages composition ETag contract");
need(cargo, "sha2.workspace = true", "storefront composition dependency");

for (const marker of [
  "fetch_pages_navigation_snapshot",
  "tokio::join!(",
  "fetch_active_menu(StorefrontMenuLocation::Header",
  "fetch_active_menu(StorefrontMenuLocation::Footer",
  "render_canonical_pages_response",
  "fetch_seo_page_context(locale, route_segment, &query_params).await",
  "render_module_page_with_nonce(",
  "Some(navigation.clone())",
  "pages_storefront_composition_etag(",
  "html.as_str()",
  "if_none_match_matches(if_none_match, etag)",
  "not_modified_composition_response(etag)",
  "StatusCode::NOT_MODIFIED",
  "apply_composition_headers",
  "HeaderValue::from_static(PAGES_STOREFRONT_REVALIDATE_CACHE_CONTROL)",
  "get(IF_NONE_MATCH)",
  "const PRIVATE_NO_STORE: &str = \"private, no-store\"",
]) need(host, marker, "storefront Pages owner composition");

const canonicalComposition = between(
  host,
  "async fn render_canonical_pages_response(",
  "fn apply_composition_headers(",
  "canonical Pages host composition",
);
ordered(canonicalComposition, [
  "fetch_seo_page_context(locale, route_segment, &query_params).await",
  "fetch_pages_navigation_snapshot(locale).await",
  "render_module_page_with_nonce(",
  "Some(navigation.clone())",
  "pages_storefront_composition_etag(",
  "html.as_str()",
  "if_none_match_matches(if_none_match, etag)",
  "not_modified_composition_response(etag)",
  "Html(html).into_response()",
  "apply_composition_headers(&mut response, etag.as_str())",
], "SEO Navigation render identity conditional response ordering");

const hostPipeline = between(
  host,
  "async fn render_module_page_response(",
  "fn redirect_response(",
  "storefront module response pipeline",
);
ordered(hostPipeline, [
  "resolve_pages_route_response(",
  "Err(response) => return response",
  "if let Some(decision) = pages_decision",
  "render_canonical_pages_response(",
  "fetch_seo_page_context(locale, route_segment, &query_params).await",
], "route terminal decision before composition and generic SEO");

for (const marker of [
  "storefront-composition-etag-source-ready",
  "Pages storefront Navigation/SEO composition ETag: source-ready",
  "exact final rendered HTML",
  "Cache-Control: private, no-cache",
  "Terminal Pages route responses continue to use `private, no-store`",
]) need(plan, marker, "canonical parity plan");
for (const marker of [
  "storefront-composition-etag-source-ready",
  "Navigation-owned header/footer menus",
  "exact rendered HTML",
  "deterministic SHA-256 ETag",
  "Matching strong, weak or comma-separated `If-None-Match` returns `304`",
]) need(localPlan, marker, "Pages implementation plan");
for (const marker of [
  "source-ready / execution-pending",
  "Navigation-owned header/footer reads",
  "SSR render with the preloaded Navigation snapshot",
  "pages_storefront_composition_v1",
  "exact final rendered HTML document",
  "304 Not Modified",
  "private, no-cache",
  "Execution evidence remains pending",
]) need(packet, marker, "storefront composition packet");

for (const text of [routeAdapter, navigationModel, navigationUi, composition, host, packet]) {
  forbid(text, "Iggy", "Pages storefront composition slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-storefront-composition-etag] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-storefront-composition-etag] PASS source_ready=true execution=pending");
