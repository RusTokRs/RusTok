#!/usr/bin/env node

// Source-level guard for the cross-owner F6/F7 contract. Runtime behavior is
// exercised by the Forum SQLite tests; this guard does not replace cargo test.
import { readFileSync } from 'node:fs';

import { containsMarker } from './lib/forum-source-verification.mjs';

function source(path) {
  return readFileSync(path, 'utf8');
}
// Markers are compared on a layout-normalized copy: rustfmt wraps method chains and
// `impl Trait for Type`, and that is a formatting fact, not a contract change.
function includes(path, ...markers) {
  const text = source(path);
  for (const marker of markers) {
    if (!containsMarker(text, marker)) throw new Error(`${path}: missing ${marker}`);
  }
  return text;
}

const forum = 'crates/modules/rustok-forum/src';
const native = 'crates/modules/rustok-forum/storefront/src/transport';
includes(`${forum}/services/engagement_mode.rs`,
  'pub async fn anonymous_reading_allowed(', 'FORUM_PUBLIC_SETTINGS_UNAVAILABLE',
  'self.module_settings(tenant_id).await?.allow_anonymous_reading');
for (const owner of [
  'category_audience_visibility.rs', 'topic_audience_visibility.rs',
  'category_audience_read.rs', 'topic_audience_list.rs',
  'search_result_eligibility.rs',
]) {
  includes(`${forum}/services/${owner}`, 'anonymous_reading_allowed(tenant_id)');
}
for (const owner of ['topic_audience_read.rs', 'reply_audience_read.rs']) {
  includes(`${forum}/services/${owner}`, '.is_topic_visible(');
}
const gql = includes(`${forum}/graphql/storefront_audience_topics.rs`,
  'ctx.data_opt::<AuthContext>()',
  '.list_authenticated_storefront_visible_with_audience_context(',
  '.list_public_storefront_visible_with_locale_fallback(');
if (gql.indexOf('.list_authenticated_storefront_visible_with_audience_context(') >
    gql.indexOf('.list_public_storefront_visible_with_locale_fallback(')) {
  throw new Error('GraphQL topic list must choose the authenticated owner first');
}
includes(`${forum}/graphql/runtime_data.rs`,
  '(Some(reader), None) => ForumSettingsProviders::default().with_static_reader(reader)');
includes(`${forum}/graphql/mod.rs`, '.is_module_enabled_for_tenant(tenant.id, channel_id, "forum")');
includes(`${forum}/graphql/storefront_read_state.rs`,
  'super::require_public_forum_channel_enabled(ctx).await?');

includes(`${native}/native_server_adapter.rs`,
  'pub(super) async fn forum_channel_enabled(',
  '.is_module_enabled_for_tenant(tenant_id, channel_id, "forum")',
  '.with_settings_providers(settings_providers.clone())');
for (const path of [
  `${native}/native_server_adapter_pages.rs`,
  `${native}/native_server_adapter_bulk.rs`,
  `${native}/native_server_adapter_category_route.rs`,
  `${native}/native_server_adapter_topic_route.rs`,
]) {
  includes(path, 'forum_channel_enabled(&runtime_ctx, tenant.id, &request).await?');
}
const nativeMain = source(`${native}/native_server_adapter.rs`);
const nativePages = source(`${native}/native_server_adapter_pages.rs`);
for (const [path, text] of [[`${native}/native_server_adapter.rs`, nativeMain],
                          [`${native}/native_server_adapter_pages.rs`, nativePages]]) {
  if (/auth\.as_ref\(\)\.filter\(\|auth\|\s*\{\s*has_any_effective_permission[\s\S]{0,140}?\}\)\s*\{[\s\S]{0,2500}?list_public_storefront_visible_with_locale_fallback/.test(text)) {
    throw new Error(`${path}: a signed-in caller must not borrow a public listing fallback`);
  }
}
for (const path of [
  'apps/server/src/services/forum_search_category_scope.rs',
  'apps/server/src/services/forum_search_result_eligibility.rs',
]) {
  const text = includes(path, 'if let Some(auth) = request.auth.as_ref() {',
    'with_static_reader(reader)');
  if (text.includes('request.auth.as_ref().filter(')) {
    throw new Error(`${path}: an authenticated Search caller was downgraded to public`);
  }
}
// The retired Forum-only execution pipeline must stay retired: no storefront Search
// document may be authorized outside the declared-corpus executor.
for (const retiredPath of [
  'crates/modules/rustok-search/src/forum_storefront_execution.rs',
  'crates/modules/rustok-search/src/forum_storefront_execution_public.rs',
  'crates/modules/rustok-search/src/storefront_result_eligibility.rs',
]) {
  let exists = true;
  try {
    source(retiredPath);
  } catch (error) {
    exists = false;
  }
  if (exists) throw new Error(`${retiredPath}: retired file must not exist`);
}
includes('crates/modules/rustok-search/src/audience_security.rs',
  'public_discovery_allowed(module, tenant_id)',
  'SearchDictionaryService::apply_storefront_query_rules(',
  'execute_audience_eligible_search');
// Search never hardcodes a module as gated: the audience scope is the only place a
// module is restricted, and it is applied inside the SQL filter so totals, facets,
// ordering and pagination stay authorized.
includes('crates/modules/rustok-search/src/pg_engine.rs',
  'sd.document_id AS document_id',
  '(source_module NOT IN ({gated_params}) OR document_id IN ({authorized_params}))');
includes('crates/modules/rustok-search/src/audience_security.rs',
  'SearchAudienceClass',
  'pub fn declared_source_modules(&self) -> Vec<String>',
  'None => {\n                tracing::warn!(',
  'MAX_SEARCH_AUDIENCE_SCAN_DOCUMENTS');
includes('crates/modules/rustok-search/src/graphql/query.rs',
  'SEARCH_AUDIENCE_REGISTRY_UNAVAILABLE_MESSAGE');
includes('crates/modules/rustok-search/src/suggestions.rs',
  'source_module IN ({})',
  'authorize_suggestion_documents(');
includes('crates/modules/rustok-search/src/dictionaries.rs',
  'if !allow_extra_pins || has_catalog_filters(query)',
  'storefront_payload_visible_for_channel(&payload, &entity_type, channel)');
includes('apps/server/src/services/forum_search_result_eligibility.rs',
  'async fn public_forum_discovery_allowed(', '.public_discovery_allowed(tenant_id)');
includes(`${forum}/search_projection.rs`,
  'fn build_with_extensions(', 'anonymous_reading_allowed(tenant_id)');
includes('crates/modules/rustok-seo/src/services/targets.rs',
  'static_settings_reader: self.static_settings_reader.clone()');
const seoTargets = includes(`${forum}/seo_audience_targets.rs`,
  'if !discovery.anonymous_reading_allowed(tenant_id).await?',
  'async fn public_sitemap_requires_refresh(',
  'ForumTopicAudienceListService::new(runtime.db.clone(), runtime.event_bus.clone())',
  'last_visible_cursor = item.sort_key.map(|key| key.encode())',
  'visible.last().map(|candidate| candidate.target_id.to_string())');
const mapper = includes(`${forum}/seo_targets.rs`,
  'pub(crate) fn seo_settings_providers(runtime: &SeoTargetRuntimeContext)',
  'runtime.static_settings_reader.clone()');
if ((mapper.match(/\.with_settings_providers\(seo_settings_providers\(runtime\)\)/g) ?? []).length !== 2) {
  throw new Error('Forum legacy SEO mapper must receive settings for both public topic loads and sitemap lists');
}
if (seoTargets.includes('let next_cursor = page.next_cursor;') ||
    seoTargets.includes('next_cursor: page.next_cursor')) {
  throw new Error('Forum public SEO bulk cursor must never forward an unfiltered candidate cursor');
}
includes(`${forum}/seo_audience_targets_sqlite_tests.rs`,
  'open_forum_seo_loads_topics_and_pages_only_public_owner_candidates',
  'public topic SEO load must receive settings',
  'first public category SEO page', 'first public topic SEO page');
const sitemapHooks = [...seoTargets.matchAll(/async fn public_sitemap_requires_refresh\([\s\S]*?(?=async fn sitemap_candidates\()/g)];
if (sitemapHooks.length !== 2 || sitemapHooks.some(([hook]) =>
  !hook.includes('.anonymous_reading_allowed(tenant_id)') || !hook.includes('Ok(true)')
)) {
  throw new Error('Forum sitemap must always recheck current owner candidates, including when anonymous reading is enabled');
}
includes('crates/modules/rustok-seo/src/services/sitemaps.rs',
  '.public_sitemap_requires_refresh(&self.target_runtime(), tenant_id)',
  'self.render_current_public_sitemap(tenant, path).await',
  'render_live_public_sitemap(&urls, &origin, &settings, path)');
includes('crates/modules/rustok-seo/src/controllers/mod.rs',
  '.public_sitemap_index(&tenant)', '.public_sitemap_file(&tenant, name.as_str())',
  '(CACHE_CONTROL, "no-store")');
includes('apps/next-frontend/src/app/sitemap.ts',
  'export const dynamic = "force-dynamic"');
includes(`${forum}/notification_source.rs`,
  'host.shared_get::<SharedStaticModuleSettingsReader>()', 'self.visibility_service()');
includes(`${forum}/reaction_subject.rs`,
  'self.visibility_service()', '.with_settings_providers(settings)');
console.log('Forum anonymous-discovery, channel and signed-in owner source wiring verified');
