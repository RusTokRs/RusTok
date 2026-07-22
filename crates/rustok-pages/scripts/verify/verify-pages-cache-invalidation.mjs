#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const repoRoot = path.resolve(path.dirname(__filename), "..", "..", "..", "..");
const read = (relativePath) =>
  fs.readFileSync(path.join(repoRoot, relativePath), "utf8");

const owner = read("crates/rustok-pages/src/cache_invalidation.rs");
const pagesModule = read("crates/rustok-pages/src/lib.rs");
const reviewedPublish = read(
  "crates/rustok-pages/src/services/page/reviewed_publish.rs",
);
const pagesControllers = read("crates/rustok-pages/src/controllers/mod.rs");
const storefrontReader = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const serverAdapter = read(
  "apps/server/src/services/pages_cache_invalidation.rs",
);
const dispatcher = read("apps/server/src/services/module_event_dispatcher.rs");

function fail(message) {
  console.error(`[verify-pages-cache-invalidation] ${message}`);
  process.exit(1);
}

function requireMarker(source, marker, label) {
  if (!source.includes(marker)) fail(`${label} is missing ${marker}`);
}

function forbidMarker(source, marker, label) {
  if (source.includes(marker)) fail(`${label} still contains ${marker}`);
}

function sliceBetween(source, start, end, label) {
  const startIndex = source.indexOf(start);
  if (startIndex < 0) fail(`${label} is missing ${start}`);
  const endIndex = source.indexOf(end, startIndex + start.length);
  if (endIndex < 0) fail(`${label} is missing ${end}`);
  return source.slice(startIndex, endIndex);
}

function requireOrder(source, markers, label) {
  let previous = -1;
  for (const marker of markers) {
    const current = source.indexOf(marker, previous + 1);
    if (current < 0) fail(`${label} is missing ${marker}`);
    if (current <= previous) fail(`${label} has invalid order at ${marker}`);
    previous = current;
  }
}

for (const marker of [
  'PAGES_CACHE_NAMESPACE_FORMAT: &str = "pages_cache_namespace_v1"',
  "pub enum PageCacheScope",
  "PAGE_CACHE_SCOPES",
  "PAGE_CACHE_MUTABLE_SCOPES",
  "Self::Published | Self::Unpublished | Self::Deleted => &PAGE_CACHE_SCOPES",
  "Self::Updated => &PAGE_CACHE_MUTABLE_SCOPES",
  "pub struct PageCacheGenerationSnapshot",
  "pub trait PageCacheInvalidationPort",
  "pub trait PagesCacheReadPort",
  "pub struct PagesCacheInvalidationRuntime",
  "pub struct PagesCacheReadRuntime",
  "receipt.validate_for(&request)?",
  "pub struct PageCacheInvalidationEventHandler",
  "DomainEvent::NodePublished",
  "kind == PAGES_CACHE_ENTITY_KIND",
  "page_cache_namespace(scope: PageCacheScope, tenant_id: Uuid)",
  '":g-{generation}:page:{page_id}:{variant_hash}"',
  "storefront_pages_cache_key",
  "rg-{}:pg-{}:ag-{}",
  "Sha256::digest(variant.as_bytes())",
  "MAX_PAGE_CACHE_KEY_VARIANT_BYTES",
  "MAX_PAGE_CACHE_VALUE_BYTES",
]) {
  requireMarker(owner, marker, "Pages-owned cache contract");
}
for (const forbidden of ["hex::encode", 'cmd("SCAN")', 'cmd("KEYS")']) {
  forbidMarker(owner, forbidden, "Pages-owned bounded generation contract");
}

for (const marker of [
  "pub mod cache_invalidation;",
  "PagesCacheInvalidationRuntime",
  "PagesCacheReadRuntime",
  "PagesCacheReadPort",
  "storefront_pages_cache_key",
  "fn register_event_listeners(",
  ".get::<PagesCacheInvalidationRuntime>()",
  "registry.register(PageCacheInvalidationEventHandler::new(runtime))",
]) {
  requireMarker(pagesModule, marker, "Pages module cache exports and listener registration");
}

for (const marker of [
  "DomainEvent::NodePublished",
  "insert_publish_operation_in_tx",
  "txn.commit().await?",
]) {
  requireMarker(reviewedPublish, marker, "Pages reviewed publish outbox boundary");
}
for (const forbidden of [
  "CacheService",
  "namespace_generations",
  "page_cache_namespace",
  "PagesCacheReadRuntime",
]) {
  forbidMarker(
    reviewedPublish,
    forbidden,
    "reviewed publish must remain event-driven instead of invalidating caches inline",
  );
}

for (const marker of [
  "pub struct ServerPagesCachePort",
  "CacheNamespaceGenerationStore",
  "CacheService",
  "OnceCell<Arc<dyn CacheBackend>>",
  "cache.namespace_generations()",
  "impl PageCacheInvalidationPort for ServerPagesCachePort",
  "for scope in request.scopes()",
  ".bump(&namespace)",
  "receipt.record(*scope, generation.value())",
  "receipt.validate_for(&request)?",
  "impl PagesCacheReadPort for ServerPagesCachePort",
  "for scope in PAGE_CACHE_SCOPES",
  ".read(&namespace)",
  ".get(key)",
  ".set_with_ttl(key, value, ttl)",
]) {
  requireMarker(serverAdapter, marker, "neutral server cache capability adapter");
}
const serverAdapterRuntime = serverAdapter.split("#[cfg(test)]", 1)[0];
for (const forbidden of [
  "PageCacheScope::Route",
  "PageCacheScope::Page",
  "PageCacheScope::Artifact",
  '"route"',
  '"artifact"',
  "redis::",
  'cmd("SCAN")',
  'cmd("KEYS")',
  'cmd("DEL")',
]) {
  forbidMarker(serverAdapterRuntime, forbidden, "server adapter ownership boundary");
}

for (const marker of [
  '#[cfg(feature = "mod-pages")]',
  "ensure_cache_service(ctx)",
  "ServerPagesCachePort::new(&cache)",
  "PagesCacheInvalidationRuntime::new",
  "PagesCacheReadRuntime::new(provider)",
  "build_module_event_dispatcher(registry, bus, db, extensions.as_ref())",
]) {
  requireMarker(dispatcher, marker, "server Pages cache runtime composition");
}
const enrichment = sliceBetween(
  dispatcher,
  "fn enrich_runtime_extensions_after_event_start(",
  '#[cfg(feature = "mod-commerce")]\nfn spawn_paid_order_label_worker_if_enabled',
  "server runtime extension enrichment",
);
for (const marker of [
  "ServerPagesCachePort::new(&cache)",
  "PagesCacheInvalidationRuntime::new",
  "PagesCacheReadRuntime::new(provider)",
]) {
  requireMarker(enrichment, marker, "server runtime extension enrichment");
}
const enrichmentCall = dispatcher.indexOf(
  "let extensions = enrich_runtime_extensions_after_event_start(ctx, extensions)",
);
const dispatcherBuild = dispatcher.indexOf(
  "build_module_event_dispatcher(registry, bus, db, extensions.as_ref())",
);
if (
  enrichmentCall < 0 ||
  dispatcherBuild < 0 ||
  enrichmentCall > dispatcherBuild
) {
  fail("Pages cache runtimes must be composed before module event listeners are built");
}

for (const marker of [
  "shared_get::<PagesCacheReadRuntime>()",
  "generation_snapshot(tenant_id)",
  "storefront_pages_cache_key(",
  "get_json::<StorefrontPagesData>",
  "put_json(cache_key, &data)",
  "storefront_cache_variant(",
]) {
  requireMarker(storefrontReader, marker, "Pages storefront generation-aware reader");
}
const storefrontSsr = sliceBetween(
  storefrontReader,
  '#[cfg(feature = "ssr")]\n    {',
  '#[cfg(not(feature = "ssr"))]',
  "Pages storefront SSR read path",
);
requireOrder(
  storefrontSsr,
  [
    ".is_module_enabled(channel_id, MODULE_SLUG)",
    "shared_get::<PagesCacheReadRuntime>()",
    "get_json::<StorefrontPagesData>",
    "get_by_slug_with_locale_fallback(",
    "load_public_bound_artifact_with_fallback(",
    "put_json(cache_key, &data)",
  ],
  "Pages storefront authorization/cache/source order",
);

for (const marker of [
  "cache: Option<PagesCacheReadRuntime>",
  "shared_get::<PagesCacheReadRuntime>()",
  "PageCacheScope::Artifact",
  "generation_snapshot(tenant_id)",
  "page_cache_key(",
  "get_json::<CachedPublishedLandingArtifact>",
  "load_public_bound_artifact_with_fallback(",
  "put_json(cache_key, &artifact)",
  "artifact_cache_variant(",
]) {
  requireMarker(pagesControllers, marker, "Pages artifact generation-aware delivery");
}
const artifactHandler = sliceBetween(
  pagesControllers,
  "pub async fn get_page_artifact(",
  "#[utoipa::path(\n    post,\n    path = \"/api/admin/pages\"",
  "Pages artifact delivery path",
);
requireOrder(
  artifactHandler,
  [
    "ensure_pages_module_enabled_for_channel(&runtime, &request_context).await?",
    "load_cached_page_artifact(",
    "generation_snapshot(tenant_id)",
    "get_json::<CachedPublishedLandingArtifact>",
    "load_public_bound_artifact_with_fallback(",
    "put_json(cache_key, &artifact)",
  ],
  "Pages artifact authorization/cache/source order",
);

console.log("[verify-pages-cache-invalidation] PASS");
