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

for (const marker of [
  'PAGES_CACHE_NAMESPACE_FORMAT: &str = "pages_cache_namespace_v1"',
  "pub enum PageCacheScope",
  "Route",
  "Page",
  "Artifact",
  "Self::Published | Self::Unpublished | Self::Deleted => &ALL_SCOPES",
  "Self::Updated => &ROUTE_AND_PAGE_SCOPES",
  "pub trait PageCacheInvalidationPort",
  "pub struct PagesCacheInvalidationRuntime",
  "receipt.validate_for(&request)?",
  "pub struct PageCacheInvalidationEventHandler",
  "DomainEvent::NodePublished",
  "kind == PAGES_CACHE_ENTITY_KIND",
  "page_cache_namespace(scope: PageCacheScope, tenant_id: Uuid)",
  '":g-{generation}:page:{page_id}:{}"',
  "hex::encode(variant.as_bytes())",
]) {
  requireMarker(owner, marker, "Pages-owned cache invalidation contract");
}

for (const marker of [
  "pub mod cache_invalidation;",
  "PagesCacheInvalidationRuntime",
  "fn register_event_listeners(",
  ".get::<PagesCacheInvalidationRuntime>()",
  "registry.register(PageCacheInvalidationEventHandler::new(runtime))",
]) {
  requireMarker(pagesModule, marker, "Pages module listener registration");
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
]) {
  forbidMarker(
    reviewedPublish,
    forbidden,
    "reviewed publish must remain event-driven instead of invalidating caches inline",
  );
}

for (const marker of [
  "pub struct ServerPagesCacheInvalidationPort",
  "CacheNamespaceGenerationStore",
  "cache.namespace_generations()",
  "for scope in request.scopes()",
  ".bump(&namespace)",
  "receipt.record(*scope, generation.value())",
  "receipt.validate_for(&request)?",
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
  "ServerPagesCacheInvalidationPort::new",
  "enriched.insert(rustok_pages::PagesCacheInvalidationRuntime::new(provider))",
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
requireMarker(
  enrichment,
  "enriched.insert(rustok_pages::PagesCacheInvalidationRuntime::new(provider))",
  "server runtime extension enrichment",
);
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
  fail("Pages cache runtime must be composed before module event listeners are built");
}

console.log("[verify-pages-cache-invalidation] PASS");
