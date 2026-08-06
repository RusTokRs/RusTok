import fs from 'node:fs';
import path from 'node:path';

const root = process.cwd();
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const requireText = (source, needle, label) => {
  if (!source.includes(needle)) {
    throw new Error(`missing ${label}: ${needle}`);
  }
};
const rejectText = (source, needle, label) => {
  if (source.includes(needle)) {
    throw new Error(`forbidden ${label}: ${needle}`);
  }
};

const owner = read('crates/rustok-forum/src/services/public_discovery.rs');
const services = read('crates/rustok-forum/src/services/mod.rs');
const lib = read('crates/rustok-forum/src/lib.rs');
const seo = read('crates/rustok-forum/src/seo_audience_targets.rs');
const legacySeo = read('crates/rustok-forum/src/seo_targets.rs');
const searchEngine = read('crates/rustok-search/src/engine.rs');
const searchIngestion = read('crates/rustok-search/src/ingestion.rs');
const storefrontCore = read('crates/rustok-forum/storefront/src/core.rs');
const contract = JSON.parse(
  read('crates/rustok-forum/contracts/forum-public-discovery-seo.json'),
);
const upstream = JSON.parse(
  read('crates/rustok-forum/contracts/forum-category-audience-read.json'),
);

for (const marker of [
  'pub struct ForumPublicDiscoveryService',
  'ForumCategoryAudienceReadService',
  'ForumTopicAudienceReadService',
  'get_public_category_with_locale_fallback',
  'get_public_topic_with_locale_fallback',
  'get_public_storefront_visible_with_locale_fallback',
]) {
  requireText(owner, marker, 'exact public discovery owner');
}
requireText(services, 'mod public_discovery;', 'public discovery module');
requireText(
  services,
  'pub use public_discovery::ForumPublicDiscoveryService;',
  'public discovery service export',
);
requireText(lib, 'ForumPublicDiscoveryService', 'crate public discovery export');
requireText(lib, 'mod seo_audience_targets;', 'exact SEO wrapper module');
requireText(lib, 'mod seo_targets;', 'legacy SEO mapper module preservation');
requireText(
  lib,
  'seo_audience_targets::ForumCategorySeoTargetProvider',
  'exact category SEO registration',
);
requireText(
  lib,
  'seo_audience_targets::ForumTopicSeoTargetProvider',
  'exact topic SEO registration',
);

for (const marker of [
  'SeoTargetLoadScope::Authoring',
  'get_public_category_with_locale_fallback',
  'get_public_topic_with_locale_fallback',
  'async fn resolve_route(',
  'async fn list_bulk_summaries(',
  'async fn sitemap_candidates(',
  'category_provider().load_target',
  'topic_provider().load_target',
  'ForumCategoryRouteService',
  'ForumTopicRouteService',
  'parse_canonical_forum_route',
]) {
  requireText(seo, marker, 'exact SEO wrapper');
}
for (const forbidden of ['CategoryService', 'TopicService', 'SecurityContext::system()']) {
  rejectText(seo, forbidden, 'SEO wrapper policy duplication');
}
for (const marker of [
  'pub struct ForumCategorySeoTargetProvider',
  'pub struct ForumTopicSeoTargetProvider',
  'map_category_response',
  'map_topic_response',
  'parse_forum_route',
]) {
  requireText(legacySeo, marker, 'legacy SEO mapping preservation');
}

for (const marker of [
  'const FORUM_SOURCE_MODULE: &str = "forum"',
  'const FORUM_CATEGORY_ENTITY_TYPE: &str = "forum_category"',
  'const FORUM_TOPIC_ENTITY_TYPE: &str = "forum_topic"',
  'const FORUM_STOREFRONT_ROUTE: &str = "/modules/forum"',
  'value.source_module == FORUM_SOURCE_MODULE',
  '?category={}',
  '?topic={}',
  'canonical_url_derives_forum_category_and_topic_routes',
  'canonical_url_rejects_spoofed_forum_source_entity_pairs',
]) {
  requireText(searchEngine, marker, 'canonical Forum Search result routes');
}
for (const marker of [
  'Some(format!("/{locale}/forum/c/{slug}"))',
  'Some(format!("/{locale}/forum/t/{short_id}/{slug}"))',
  'category_href(item.effective_locale.as_str(), item.slug.as_str())',
  'topic_href(',
]) {
  requireText(storefrontCore, marker, 'Forum canonical storefront route');
}
rejectText(storefrontCore, '?category={category_id}', 'retired category UUID card route');
rejectText(storefrontCore, '?topic={topic_id}', 'retired topic UUID card route');
rejectText(
  searchIngestion,
  'DomainEvent::ForumTopicCreated',
  'undelivered Forum Search projection ingestion',
);

if (contract.task !== 'FORUM-20BI') throw new Error('unexpected task');
for (const key of [
  'anonymous_public_only',
  'category_inherited_base_floor_enforced',
  'category_richer_layers_enforced',
  'topic_open_status_enforced',
  'topic_route_channel_enforced',
  'topic_inherited_category_layers_enforced',
  'topic_local_narrowing_enforced',
  'authentication_role_trust_group_and_explicit_user_targets_absent',
  'missing_foreign_and_denied_targets_are_indistinguishable',
]) {
  if (!contract.discovery_boundary[key]) {
    throw new Error(`contract must lock discovery boundary ${key}`);
  }
}
for (const key of [
  'authoring_scope_preserves_managed_legacy_load',
  'public_target_load_uses_exact_discovery',
  'public_route_resolution_uses_exact_discovery',
  'bulk_summaries_filter_exact_public_targets',
  'sitemap_candidates_filter_exact_public_targets',
  'legacy_mapping_and_schema_payload_preserved',
]) {
  if (!contract.seo_boundary[key]) {
    throw new Error(`contract must lock SEO boundary ${key}`);
  }
}
for (const key of [
  'forum_category_result_route_added',
  'forum_topic_result_route_added',
  'canonical_source_entity_pair_required',
  'spoofed_source_entity_pairs_fail_closed',
  'forum_storefront_query_keys_reused',
]) {
  if (!contract.search_boundary[key]) {
    throw new Error(`contract must lock Search boundary ${key}`);
  }
}
for (const key of [
  'forum_projection_consumer_wired',
  'forum_search_documents_written',
  'forum_index_storage_changed',
  'forum_event_ingestion_changed',
]) {
  if (contract.search_boundary[key]) {
    throw new Error(`contract must keep undelivered Search boundary false: ${key}`);
  }
}
if (
  upstream.downstream_completion !==
  'crates/rustok-forum/contracts/forum-public-discovery-seo.json'
) {
  throw new Error('category-read handoff must point to FORUM-20BI completion');
}
for (const key of ['search_index_changed', 'seo_changed', 'deep_link_changed']) {
  if (!upstream.read_boundary[key]) {
    throw new Error(`upstream contract must advance ${key}`);
  }
}
if (!upstream.compatibility.search_change_is_route_contract_only) {
  throw new Error('upstream contract must bound Search change to route contract');
}
if (upstream.compatibility.forum_search_projection_consumer_wired) {
  throw new Error('upstream contract must not claim Forum Search projection wiring');
}
if (contract.downstream_task !== 'FORUM-20BJ') {
  throw new Error('unexpected downstream task');
}

console.log('forum exact public discovery and SEO composition verified');
