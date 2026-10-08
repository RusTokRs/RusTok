#!/usr/bin/env node

// SEO bulk lists are keyset-paged by the owning providers. The SEO layer passes an
// opaque cursor through, and no list reports `total` or walks offsets.

import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const exists = (relativePath) => existsSync(fileURLToPath(new URL(relativePath, root)));

const failures = [];
const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

const seoTargets = read('crates/modules/rustok-seo-targets/src/lib.rs');
const readModel = read('crates/modules/rustok-seo/src/services/bulk_read_model.rs');
const dto = read('crates/modules/rustok-seo/src/dto.rs');
const adminCore = read('crates/modules/rustok-seo/admin/src/core.rs');
const adminBulk = read('crates/modules/rustok-seo/admin/src/sections/bulk.rs');
const legacy = read('crates/modules/rustok-seo/src/services/bulk_legacy.rs');

// Trait contract: one keyset page per call, walk provided once for batch readers.
for (const [value, label] of [
  ['pub struct SeoTargetBulkPageRequest', 'page request type'],
  ['pub struct SeoBulkSummaryPage', 'page result type'],
  ['async fn list_bulk_summaries_page(', 'page trait method'],
  ['const SEO_BULK_WALK_BATCH: u64', 'walk batch constant'],
  ['non-advancing cursor', 'walk cursor progress guard'],
]) {
  requireText(seoTargets, value, `rustok-seo-targets ${label}`);
}

// SEO layer: cursor in, cursor out; no page numbers and no totals.
for (const [value, label] of [
  ['list_bulk_summaries_page(', 'provider page call'],
  ['after: cursor.as_deref()', 'cursor passed through'],
  ['MAX_BULK_SCAN_BATCHES', 'bounded batches per request'],
  ['non-advancing cursor', 'scan cursor progress guard'],
]) {
  requireText(readModel, value, `bulk read model ${label}`);
}
for (const [value, label] of [
  ['.skip(offset)', 'offset skip'],
  ['input.page', 'page input'],
  ['total: ', 'total field'],
]) {
  forbidText(readModel, value, `bulk read model ${label}`);
}

requireText(dto, 'pub after: Option<String>,', 'SeoBulkListInput cursor');
requireText(dto, 'pub next_cursor: Option<String>,', 'SeoBulkPage cursor');
forbidText(dto, 'pub total: i32,', 'SeoBulkPage total');

// Providers implement the page method. The shared walk is the only full-scan path.
const providers = [
  ['crates/modules/rustok-blog/src/integrations/seo_targets.rs', 'blog'],
  ['crates/modules/rustok-forum/src/seo_targets.rs', 'forum'],
  ['crates/modules/rustok-forum/src/seo_audience_targets.rs', 'forum audience'],
  ['crates/modules/rustok-pages/src/seo_targets.rs', 'pages'],
  ['crates/modules/rustok-product/src/seo_targets.rs', 'product'],
  ['crates/modules/rustok-taxonomy/src/seo_targets.rs', 'taxonomy'],
];
for (const [file, label] of providers) {
  const source = read(file);
  requireText(source, 'async fn list_bulk_summaries_page(', `${label} provider`);
  forbidText(source, 'async fn list_bulk_summaries(', `${label} provider overrides walk`);
}

// Owner keysets: each order has a unique tie-breaker and no offset.
requireText(read('crates/modules/rustok-pages/src/services/page/read.rs'),
  'order_by_desc(page::Column::Id)', 'pages keyset tie-breaker');
requireText(read('crates/modules/rustok-product/src/services/catalog/queries.rs'),
  'scan_published_product_ids(', 'product id keyset scan');
requireText(read('crates/modules/rustok-taxonomy/src/seo_targets.rs'),
  'taxonomy_term::Column::Id.gt(after)', 'taxonomy keyset');
requireText(read('crates/modules/rustok-forum/src/services/category_taxonomy_read.rs'),
  'forum_category::Column::Id.gt(after)', 'forum category keyset');

// Legacy compiled path no longer carries the offset list or the page field.
forbidText(legacy, 'pub async fn list_bulk_items(', 'legacy offset list');
forbidText(legacy, 'page: input.page', 'legacy job filter page');

// Admin: cursor navigation, no total label.
requireText(adminCore, 'pub after: Option<String>,', 'admin filter cursor');
forbidText(adminCore, 'pub page: i32,', 'admin filter page');
forbidText(adminBulk, 'Total scope items', 'admin total label');
requireText(adminBulk, 'Next page', 'admin next page control');

// Sitemap scans walk the same keyset; pages keep the channel-less public scope.
const pagesSeo = read('crates/modules/rustok-pages/src/seo_targets.rs');
const productSeo = read('crates/modules/rustok-product/src/seo_targets.rs');
requireText(pagesSeo, 'scan_public_published_pages(', 'pages sitemap keyset');
forbidText(pagesSeo, 'list_public_visible(', 'pages sitemap offset');
requireText(read('crates/modules/rustok-pages/src/services/page/read.rs'),
  'apply_public_page_channel_filter(select, tenant_id, None)', 'pages public scan channel scope');
requireText(productSeo, 'scan_published_product_ids(request.tenant_id, None, after, BULK_FETCH_SIZE)',
  'product sitemap keyset');
forbidText(productSeo, 'list_published_products_with_locale_fallback(', 'product sitemap offset');

// Provider loads treat only not-found as absent. Silent `.ok()` is forbidden.
for (const [source, label] of [[pagesSeo, 'pages'], [productSeo, 'product']]) {
  forbidText(source, '.ok();', `${label} provider silent error`);
}
requireText(pagesSeo, 'Err(PagesError::PageNotFound(_)) => Ok(None),', 'pages not-found only');
requireText(productSeo, 'Err(CommerceError::ProductNotFound(_)) => Ok(None),', 'product not-found only');

if (exists('crates/modules/rustok-seo/src/services/bulk_legacy_offset.rs')) {
  failures.push('offset bulk helper file must not exist');
}

if (failures.length > 0) {
  console.error('SEO bulk cursor pagination verification failed:');
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log('SEO bulk cursor pagination verified (providers keyset, no totals, cursor navigation)');
