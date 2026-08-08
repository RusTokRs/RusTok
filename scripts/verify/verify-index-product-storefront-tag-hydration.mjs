#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => fs.readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-index-product-storefront-tag-hydration] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const portPath = 'crates/rustok-product/src/storefront_tag_read_port.rs';
const port = requireMarkers(portPath, [
  'const MAX_STOREFRONT_TAG_HYDRATION_PRODUCTS: usize = 48;',
  'pub struct ProductStorefrontTagHydrationRequest',
  'pub product_ids: Vec<Uuid>',
  'pub fallback_locale: String',
  'pub struct ProductStorefrontTagHydration',
  'pub struct ProductStorefrontTagHydrationItem',
  'pub trait ProductStorefrontTagReadPort',
  'async fn hydrate_storefront_product_tags(',
  'impl ProductStorefrontTagReadPort for CatalogService',
  'entities::product::Column::TenantId.eq(tenant_id)',
  'entities::product::Column::Id.is_in(request.product_ids.clone())',
  'products.len() != request.product_ids.len()',
  '.load_product_tag_map(',
  'context.locale.as_str()',
  'Some(request.fallback_locale.as_str())',
  'request.product_ids',
  'tags: tags_by_product.remove(&product_id).unwrap_or_default()',
  'product.storefront_tag_product_missing',
]);
for (const forbidden of [
  'rustok_index',
  'IndexQueryPage',
  'IndexValue',
]) {
  if (port.includes(forbidden)) fail(`${portPath} Product owner capability must not depend on Index: ${forbidden}`);
}

const ownerTagsPath = 'crates/rustok-product/src/services/catalog/tags.rs';
requireMarkers(ownerTagsPath, [
  'pub async fn load_product_tag_map(',
  'product_tag::Column::ProductId.is_in(product_ids.clone())',
  'TaxonomyService::new(self.db.clone())',
  '.resolve_term_names(tenant_id, &ordered_term_ids, locale, fallback_locale)',
  'metadata_has_tags_field(&product.metadata)',
  'normalize_tag_names(&extract_metadata_tags(&product.metadata))',
]);

const taxonomyPath = 'crates/rustok-taxonomy/src/services.rs';
requireMarkers(taxonomyPath, [
  'pub async fn resolve_term_names(',
  'resolve_by_locale_with_fallback(',
  '.unwrap_or_else(|| term.canonical_key.clone())',
]);

const legacyEvidencePath = 'crates/rustok-commerce/tests/product_taxonomy_tags.rs';
requireMarkers(legacyEvidencePath, [
  'legacy_metadata_tags_are_used_as_read_fallback_but_not_exposed_publicly',
  '"tags": ["legacy", "sale", "legacy"]',
  'vec!["legacy".to_string(), "sale".to_string()]',
]);

const sourcePath = 'crates/rustok-distribution/src/product_index/product.rs';
const productSource = requireMarkers(sourcePath, [
  'product_tag_ids AS (',
  'jsonb_agg(product_tag.term_id ORDER BY product_tag.term_id) AS tag_ids',
  'LEFT JOIN product_tag_ids tags',
  "COALESCE(tags.tag_ids, '[]'::jsonb) AS tag_ids",
  'many_field("tag_ids", IndexValueType::Uuid, true, false)',
]);
if (productSource.includes('metadata.tags') || productSource.includes("metadata->'tags'")) {
  fail(`${sourcePath} must not invent tag identities from legacy metadata strings`);
}

const runtimePath = 'crates/rustok-product/src/runtime.rs';
const runtime = requireMarkers(runtimePath, [
  'storefront_tag_read_port: Option<Arc<dyn ProductStorefrontTagReadPort>>',
  'storefront_tag_read_port: None',
  '.with_storefront_tag_read_port(catalog)',
  'pub fn with_storefront_tag_read_port(',
  'pub fn storefront_tag_read_port(&self)',
  'pub fn external(read_port: Arc<dyn ProductCatalogReadPort>)',
]);
const externalStart = runtime.indexOf('pub fn external(read_port: Arc<dyn ProductCatalogReadPort>)');
const withTagStart = runtime.indexOf('pub fn with_storefront_tag_read_port(');
if (externalStart < 0 || withTagStart <= externalStart) fail('Product external runtime boundary markers are missing');
const externalBody = runtime.slice(externalStart, withTagStart);
if (externalBody.includes('with_storefront_tag_read_port')) {
  fail(`${runtimePath} external Product runtime must not silently install an embedded tag provider`);
}

const executorPath = 'crates/rustok-distribution/src/product_index/storefront_shadow_executor.rs';
const executor = requireMarkers(executorPath, [
  'pub(crate) tag_hydration:',
  'Option<Result<ProductStorefrontTagHydration, ProductStorefrontIndexTagHydrationError>>',
  'ProductStorefrontIndexTagHydrationError',
  'TagReadPortUnavailable',
  'let tag_hydration = match projected.as_ref()',
  'self.hydrate_projected_tags(context, fallback_locale, projected)',
  'async fn hydrate_projected_tags(',
  '.storefront_tag_read_port()',
  '.map(|item| item.entity_id)',
  'ProductStorefrontTagHydrationRequest',
  'product_ids,',
  'fallback_locale,',
  '.hydrate_storefront_product_tags(',
]);
for (const forbidden of [
  'TaxonomyService',
  'taxonomy_term',
  'product_tag::',
  'DatabaseConnection',
  'query_all(',
  'query_one(',
]) {
  if (executor.includes(forbidden)) {
    fail(`${executorPath} must consume Product owner tag capability, not read storage/Taxonomy directly: ${forbidden}`);
  }
}
const projectedPosition = executor.indexOf('let projected = self');
const hydrationPosition = executor.indexOf('let tag_hydration = match projected.as_ref()');
if (projectedPosition < 0 || hydrationPosition <= projectedPosition) {
  fail('tag hydration must begin only after the raw Index page exists');
}

const publicProjectionPath = 'crates/rustok-distribution/src/product_index/storefront_projection.rs';
requireMarkers(publicProjectionPath, [
  'value(&projected.items[0], "tag_ids")',
  'IndexValue::List(vec![IndexValue::Uuid(tag_id)])',
]);

console.log('[verify-index-product-storefront-tag-hydration] Product IDs from the fixed raw Index page drive bounded Product-owned tag hydration with Taxonomy and legacy metadata semantics retained');
