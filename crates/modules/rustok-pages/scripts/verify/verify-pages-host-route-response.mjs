#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");
const read = (file) => fs.readFileSync(path.join(root, file), "utf8");
const failures = [];

const evidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-host-route-response-source.json",
));
const historicalEvidence = JSON.parse(read(
  "crates/rustok-pages/contracts/evidence/pages-published-slug-route-alias-source.json",
));
const pagesLib = read("crates/rustok-pages/src/lib.rs");
const adapter = read(
  "crates/rustok-pages/storefront/src/transport/host_route_adapter.rs",
);
const transport = read("crates/rustok-pages/storefront/src/transport/mod.rs");
const storefrontLib = read("crates/rustok-pages/storefront/src/lib.rs");
const host = read("apps/storefront/src/lib.rs");
const harness = read(
  "crates/rustok-pages/storefront/tests/host_route_decision_sqlite.rs",
);
const plan = read("docs/modules/pages-page-builder-parity-continuation-plan.md");
const localPlan = read("crates/rustok-pages/docs/implementation-plan.md");
const packet = read(
  "docs/modules/pages-page-builder-host-route-response-packet-2026-08-06.md",
);

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

if (evidence.format !== "pages_host_route_response_source_v1") {
  failures.push(`evidence format mismatch: ${evidence.format}`);
}
if (evidence.status !== "pages_host_route_response_source_unvalidated") {
  failures.push(`evidence status mismatch: ${evidence.status}`);
}
if (!Array.isArray(evidence.execution) || evidence.execution.length !== 0) {
  failures.push("execution evidence must remain empty");
}
for (const [key, value] of Object.entries(evidence.validation ?? {})) {
  if (value !== false) failures.push(`validation.${key} must remain false`);
}
for (const key of [
  "registered_route_decision_server_function_added",
  "trusted_tenant_context_used",
  "trusted_request_context_used",
  "channel_module_admission_precedes_route_owner",
  "requested_tenant_platform_locale_candidates_used",
  "real_page_route_service_used",
  "route_not_found_maps_to_not_found_decision",
  "route_conflict_maps_to_conflict_decision",
  "gone_alias_maps_to_gone_decision",
  "target_page_public_lifecycle_rechecked",
  "target_page_channel_visibility_rechecked",
  "target_lifecycle_race_fails_closed",
  "canonical_location_percent_encoded",
  "host_route_decision_precedes_seo_and_render",
  "exact_localized_canonical_continues_ssr",
  "legacy_unprefixed_route_redirects_permanently",
  "noncanonical_slug_redirects_permanently",
  "immutable_alias_redirects_permanently",
  "gone_route_returns_410",
  "unknown_route_returns_404",
  "ambiguous_route_returns_409",
  "route_runtime_failure_returns_503",
  "terminal_route_responses_are_private_no_store",
  "registered_sqlite_axum_harness_added",
  "production_pages_storefront_behavior_changed",
  "production_storefront_host_behavior_changed",
]) {
  if (evidence.source_contract?.[key] !== true) {
    failures.push(`source_contract.${key} must be true`);
  }
}
for (const key of [
  "production_page_builder_behavior_changed",
  "database_schema_changed",
  "migration_changed",
  "page_body_schema_changed",
  "page_artifact_schema_changed",
  "graphql_schema_changed",
  "rest_http_api_changed",
  "cache_policy_changed",
  "event_schema_changed",
  "optional_event_infrastructure_changed",
  "delete_tombstones_added",
  "historical_backfill_added",
  "ffa_promoted",
  "fba_promoted",
]) {
  if (evidence.source_contract?.[key] !== false) {
    failures.push(`source_contract.${key} must be false`);
  }
}
if (
  evidence.server_function?.path !== "/api/fn/pages/route-decision" ||
  evidence.server_function?.codec !== "application/x-www-form-urlencoded" ||
  evidence.regression?.path !==
    "crates/rustok-pages/storefront/tests/host_route_decision_sqlite.rs" ||
  evidence.regression?.test !==
    "registered_host_route_decision_respects_admission_aliases_and_terminal_states" ||
  evidence.host?.redirect_status !== 308 ||
  evidence.host?.gone_status !== 410 ||
  evidence.host?.not_found_status !== 404 ||
  evidence.host?.conflict_status !== 409 ||
  evidence.host?.runtime_failure_status !== 503 ||
  evidence.host?.terminal_cache_control !== "private, no-store"
) {
  failures.push("host route response registration is invalid");
}

if (historicalEvidence.source_contract?.host_redirect_response_added !== false) {
  failures.push("historical published-slug evidence must retain host_redirect_response_added=false");
}
for (const marker of ["PAGE_ROUTE_NOT_FOUND", "PAGE_ROUTE_RESOLUTION_CONFLICT"]) {
  need(pagesLib, marker, "rustok-pages root route error export");
}

for (const marker of [
  "pub enum StorefrontPageRouteDisposition",
  "Canonical",
  "Redirect",
  "Gone",
  "NotFound",
  "Conflict",
  "pub struct StorefrontPageRouteDecision",
  'endpoint = "pages/route-decision"',
  "expect_context::<HostRuntimeContext>()",
  "leptos_axum::extract::<rustok_api::RequestContext>()",
  "leptos_axum::extract::<rustok_api::TenantContext>()",
  "PageRouteService::new(runtime_ctx.db_clone())",
  "PAGE_ROUTE_NOT_FOUND",
  "PAGE_ROUTE_RESOLUTION_CONFLICT",
  "PageService::new(runtime_ctx.db_clone(), event_bus.clone())",
  "SecurityContext::public_read()",
  "ContentStatus::Published",
  "is_visible_for_public_channel(",
  "PagesError::PageNotFound(_)",
  "PagesError::Forbidden(_)",
  "encoded_page_route_path(",
  "form_urlencode_component(",
]) need(adapter, marker, "Pages host route adapter");

const serverFn = between(
  adapter,
  "async fn storefront_page_route_native(",
  '#[cfg(not(feature = "ssr"))]',
  "registered route decision server function",
);
ordered(serverFn, [
  "let request_context = leptos_axum::extract",
  "let tenant_context = leptos_axum::extract",
  "let (tenant_id, fallback_locale)",
  "ChannelService::new(runtime_ctx.db_clone())",
  ".is_module_enabled(channel_id, MODULE_SLUG)",
  "let requested_locale = locale",
  "build_locale_candidates(",
  "Some(requested_locale)",
  "Some(fallback_locale.as_str())",
  "Some(PLATFORM_FALLBACK_LOCALE)",
  "PageRouteService::new(runtime_ctx.db_clone())",
  ".resolve(tenant_id, candidate_locale.as_str(), page_slug.as_str())",
  "PageRouteDisposition::Gone",
  "PageService::new(runtime_ctx.db_clone(), event_bus.clone())",
  ".get_with_locale_fallback(",
  "page.status != ContentStatus::Published",
  "is_visible_for_public_channel(",
  "StorefrontPageRouteDisposition::Redirect",
  "StorefrontPageRouteDisposition::Canonical",
], "admission locale owner target ordering");

need(transport, "mod host_route_adapter;", "Pages storefront transport registry");
need(transport, "resolve_storefront_page_route", "Pages storefront transport export");
need(storefrontLib, "StorefrontPageRouteDecision", "Pages storefront crate export");
need(storefrontLib, "resolve_storefront_page_route", "Pages storefront crate export");

for (const marker of [
  "const PAGES_ROUTE_SEGMENT: &str = \"pages\"",
  "const PRIVATE_NO_STORE: &str = \"private, no-store\"",
  "async fn resolve_pages_route_response(",
  "rustok_pages_storefront::resolve_storefront_page_route(",
  "fn pages_route_response_from_decision(",
  "StatusCode::PERMANENT_REDIRECT",
  "StatusCode::GONE",
  "StatusCode::NOT_FOUND",
  "StatusCode::CONFLICT",
  "StatusCode::SERVICE_UNAVAILABLE",
  "private_permanent_redirect",
  "private_status_response",
  "exact_localized_canonical_page_route_continues_ssr",
  "legacy_or_noncanonical_page_route_redirects_privately",
  "alias_gone_missing_and_conflict_stop_before_ssr",
]) need(host, marker, "storefront host response composition");

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
  "render_module_page_with_nonce(",
], "Pages route decision before canonical composition and generic SEO/render");

for (const marker of [
  '#![cfg(feature = "ssr")]',
  "use rustok_pages_storefront as _;",
  'const SERVER_FN_PATH: &str = "/api/fn/pages/route-decision"',
  "handle_server_fns_with_context",
  "provide_context(host.clone())",
  "SysEventsMigration.up(&manager).await?",
  "for migration in ChannelModule.migrations()",
  "for migration in PagesModule.migrations()",
  "create_and_rename_published_page",
  'translation("About", "about")',
  'translation("About us", "about-us")',
  'translation("Company", "company")',
  'call_route(&app, &tenant, &channel_context, "about", "en")',
  'call_route(&app, &tenant, &channel_context, "about-us", "en")',
  'call_route(&app, &tenant, &channel_context, "company", "en")',
  'call_route(&app, &tenant, &channel_context, "missing", "en")',
  '"removed"',
  '"gone"',
  '"conflict"',
  "set_pages_enabled(&channel_service, channel.id, false)",
  "assert!(!denied_alias.body.contains(\"redirect\"))",
]) need(harness, marker, "registered SQLite/Axum route decision harness");

for (const marker of [
  "host-route-response-source-ready",
  "Pages host route response: source-ready",
  "route decision precedes SEO and SSR rendering",
  "delete-route-tombstone-source-ready",
  "route-history-import-source-ready",
]) need(plan, marker, "canonical Pages/Page Builder plan");
for (const marker of [
  "registered Pages host route decision server function",
  "Exact localized canonical routes continue SSR",
  "Delete tombstones retain every route",
  "historical route import owner",
]) need(localPlan, marker, "Pages local plan");
for (const marker of [
  "source-ready / execution-pending",
  "308 Permanent Redirect",
  "410 Gone",
  "404 Not Found",
  "409 Conflict",
  "private, no-store",
  "Execution evidence remains pending",
]) need(packet, marker, "host route response packet");

for (const text of [adapter, host, harness, packet]) {
  forbid(text, "Iggy", "Pages host routing slice");
}

if (failures.length > 0) {
  console.error("[verify-pages-host-route-response] FAIL");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}
console.log("[verify-pages-host-route-response] PASS source_ready=true execution=pending");
