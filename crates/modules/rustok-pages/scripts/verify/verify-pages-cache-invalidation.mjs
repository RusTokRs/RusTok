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
const rollback = read("crates/rustok-pages/src/services/page/rollback.rs");
const pagesControllers = read("crates/rustok-pages/src/controllers/mod.rs");
const storefrontReader = read(
  "crates/rustok-pages/storefront/src/transport/native_server_adapter.rs",
);
const serverAdapter = read(
  "apps/server/src/services/pages_cache_invalidation.rs",
);
const dispatcher = read("apps/server/src/services/module_event_dispatcher.rs");
const correlationEvidence = JSON.parse(
  read(
    "crates/rustok-pages/contracts/evidence/pages-publish-rollback-cache-correlation-source.json",
  ),
);
const correlationRegression = read(
  "crates/rustok-pages/tests/publish_rollback_cache_correlation.rs",
);
const correlationVerifier = read(
  "crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-cache-correlation.mjs",
);

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
  '"{}:g-{generation}:page:{page_id}:{variant_hash}"',
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

const reviewedPublishOwner = sliceBetween(
  reviewedPublish,
  "pub async fn publish_reviewed(",
  "fn require_builder_sources(",
  "Pages reviewed publish owner transaction",
);
requireOrder(
  reviewedPublishOwner,
  [
    "DomainEvent::NodePublished",
    "insert_publish_operation_in_tx",
    "txn.commit().await?",
  ],
  "Pages reviewed publish outbox receipt commit order",
);
for (const forbidden of [
  "CacheService",
  "namespace_generations",
  "page_cache_namespace",
  "PagesCacheReadRuntime",
]) {
  forbidMarker(
    reviewedPublishOwner,
    forbidden,
    "reviewed publish must remain event-driven instead of invalidating caches inline",
  );
}

const rollbackOwner = sliceBetween(
  rollback,
  "pub async fn rollback_to_previous(",
  "async fn find_previous_publish_target_in_tx(",
  "Pages rollback owner transaction",
);
requireOrder(
  rollbackOwner,
  [
    "DomainEvent::NodePublished",
    "insert_rollback_operation_in_tx",
    "txn.commit().await?",
  ],
  "Pages rollback outbox receipt commit order",
);
for (const forbidden of [
  "CacheService",
  "namespace_generations",
  "page_cache_namespace",
  "PagesCacheReadRuntime",
]) {
  forbidMarker(
    rollbackOwner,
    forbidden,
    "rollback must remain event-driven instead of invalidating caches inline",
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

if (
  correlationEvidence.status !==
    "pages_publish_rollback_cache_correlation_source_unvalidated" ||
  !Array.isArray(correlationEvidence.execution) ||
  correlationEvidence.execution.length !== 0 ||
  correlationEvidence.validation?.tests_run !== false ||
  correlationEvidence.validation?.verifiers_run !== false ||
  correlationEvidence.validation?.runtime_proven !== false
) {
  fail("publish/rollback cache correlation evidence status is invalid");
}
for (const marker of [
  "published_event_rotates_generations_and_forces_storefront_and_artifact_miss_refill",
  "struct CorrelatingCachePort",
  "handler.handle(&envelope).await.unwrap()",
  "assert_eq!(requests[0].event_id, envelope.id)",
  "assert_eq!(receipts[0].correlation_id, envelope.correlation_id)",
  "assert_ne!(new_storefront_key, old_storefront_key)",
  "assert_ne!(new_artifact_key, old_artifact_key)",
  "put_json(new_storefront_key.clone(), &refilled_storefront)",
  "put_json(new_artifact_key.clone(), &refilled_artifact)",
]) {
  requireMarker(
    correlationRegression,
    marker,
    "Pages publish/rollback cache correlation regression",
  );
}
for (const marker of [
  "pages_publish_rollback_cache_correlation_source_unvalidated",
  "reviewed publish event receipt commit ordering",
  "rollback event receipt commit ordering",
  "storefront generation miss source refill order",
  "artifact generation miss source refill order",
  "source_ready=true execution=pending",
]) {
  requireMarker(
    correlationVerifier,
    marker,
    "Pages publish/rollback cache correlation verifier",
  );
}

console.log(
  "[verify-pages-cache-invalidation] PASS correlation_source_ready=true execution=pending",
);
