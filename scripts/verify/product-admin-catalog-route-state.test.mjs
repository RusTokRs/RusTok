#!/usr/bin/env node
// Source-level regression guards for the mounted Product admin catalog.
// Runtime SSR/hydration and owner SQL behavior still require the maintainer's tests.
import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const grid = readFileSync(
  new URL('../../crates/modules/rustok-product/admin/src/ui/product_grid.rs', import.meta.url),
  'utf8'
);
const productAdminCore = readFileSync(
  new URL('../../crates/modules/rustok-product/admin/src/core.rs', import.meta.url),
  'utf8'
);
const catalogTransport = readFileSync(
  new URL('../../crates/modules/rustok-product/admin/src/catalog_transport.rs', import.meta.url),
  'utf8'
);
const filters = readFileSync(
  new URL('../../crates/modules/rustok-product/src/services/catalog/attribute_filters.rs', import.meta.url),
  'utf8'
);
const facets = readFileSync(
  new URL('../../crates/modules/rustok-product/src/services/catalog/facets.rs', import.meta.url),
  'utf8'
);
const nextPage = readFileSync(
  new URL('../../apps/next-admin/src/app/dashboard/product/page.tsx', import.meta.url),
  'utf8'
);
const nextFacetPanel = readFileSync(
  new URL('../../apps/next-admin/packages/rustok-product/src/components/products/product-table/product-catalog-filters.tsx', import.meta.url),
  'utf8'
);
const nativeCatalogControls = readFileSync(
  new URL('../../crates/modules/rustok-product/admin/src/catalog_controls.rs', import.meta.url),
  'utf8'
);

test('mounted Leptos catalog uses current router filters for owner and facet requests', () => {
  assert.doesNotMatch(grid, /provide_context\(initial_catalog_controls\.clone\(\)\)/);
  assert.doesNotMatch(catalogTransport, /product_admin_list_input_from_route|ProductAdminRouteControls/);

  assert.match(grid, /let catalog_controls = Memo::new/);
  for (const name of ['route_search', 'route_status', 'route_vendor', 'route_category',
    'route_attributes', 'route_sort_by', 'route_sort_direction', 'route_page']) {
    assert.match(grid, new RegExp(name + '\\.get\\(\\)'));
  }
  assert.match(grid, /res_controls\.get\(\)/);
  assert.match(grid, /facets_controls\.get\(\)/);
  assert.match(grid, /facet_panel_controls\.get\(\)/);
  assert.match(grid, /loaded_controls == controls/);
  assert.match(grid, /catalog_controls\.get_untracked\(\) == requested_controls/);
});

test('URL owner-query changes invalidate accumulated rows and request generation', () => {
  assert.match(grid, /identity != previous_filter_identity\.get_untracked\(\)/);
  assert.match(grid, /set_page_request_nonce\.update/);
  assert.match(grid, /set_infinite_items\.set\(Vec::new\(\)\)/);
  assert.match(grid, /selection\.update\(\|selected\| selected\.clear\(\)\)/);
  assert.match(grid, /name="status"/);
  assert.match(grid, /name="search"/);
});

test('only eligible owner attribute options can appear in filters and facets', () => {
  assert.match(filters, /pavo\.tenant_id = pav\.tenant_id/);
  assert.match(filters, /pao\.attribute_id = pav\.attribute_id/);
  assert.match(filters, /pao\.archived_at IS NULL/);
  assert.match(facets, /Alias::new\("product_attribute_options"\)/);
  assert.match(facets, /Alias::new\("archived_at"\)\)\)\.is_null\(\)/);
});

test('blank and duplicate status query values are canonicalized before hydration', () => {
  assert.match(nextPage, /const hasNonCanonicalStatus/);
  assert.match(nextPage, /!rawStatus\?\.trim\(\)/);
  assert.match(nextPage, /hasNonCanonicalStatus \|\|/);
});

test('vendor, category, sort and attribute filters are canonical before hydration', () => {
  for (const name of [
    'hasNonCanonicalVendor',
    'hasNonCanonicalCategoryId',
    'hasNonCanonicalSortBy',
    'hasNonCanonicalSortDirection',
    'hasNonCanonicalAttributeFilters'
  ]) {
    assert.match(nextPage, new RegExp(name));
  }
  assert.match(nextPage, /const trimmedStatus = rawStatus\?\.trim\(\) \?\? ''/);
  assert.match(nextPage, /rawStatus !== trimmedStatus/);
  assert.match(nextPage, /return \[\.\.\.new Set\(normalized\)\]/);
  assert.match(nextPage, /query\.set\('vendor', vendor\)/);
  assert.match(nextPage, /query\.set\('category_id', categoryId\)/);
  assert.match(nextPage, /query\.set\('sort_by', sortBy\)/);
  assert.match(nextPage, /query\.set\('sort_direction', sortDirection\)/);
  assert.match(nextPage, /query\.set\('attribute_filters', canonicalAttributeFilters\)/);
});

test('legacy search URLs are migrated into the visible title column filter', () => {
  assert.match(nextPage, /const rawTitle = pickParam\(params\.title\)/);
  assert.match(nextPage, /const rawSearch = pickParam\(params\.search\)/);
  assert.match(nextPage, /const trimmedTitle = rawTitle\?\.trim\(\) \?\? ''/);
  assert.match(nextPage, /rawTitle !== trimmedTitle/);

  assert.match(nextPage, /hasLegacySearch/);
  assert.match(nextPage, /query\.set\('title', search\)/);
  assert.match(nextPage, /query\.delete\('search'\)/);
});

test('Next catalog normalizes typed selection grammar and prioritizes active facets', () => {
  assert.match(nextPage, /entry\.indexOf\('='\)/);
  assert.match(nextPage, /selectedFacetCodes/);
  assert.match(nextPage, /orderedFacetCodes/);
});

test('Next and native admin facets agree with owner case-insensitive attribute codes', () => {
  assert.match(nextPage, /trim\(\)\.toLowerCase\(\)/);
  assert.match(nextFacetPanel, /function hasFacetCode\(/);
  assert.match(nextFacetPanel, /function hasFacetValue\(/);
  assert.match(nativeCatalogControls, /code\.trim\(\)\.to_ascii_lowercase\(\)/);
});

test('Leptos catalog filters are pushed down before server pagination', () => {
  assert.match(grid, /filters=filters/);
  assert.match(grid, /new_filters\s*\.get\("title"\)/);
  assert.match(grid, /new_filters\s*\.get\("status"\)/);
  assert.match(grid, /new_filters\s*\.get\("vendor"\)/);
  assert.match(grid, /filter_query_writer\.update\(/);
  assert.doesNotMatch(grid, /filter_products\(/);
  assert.match(productAdminCore, /assert_eq!\(\s*filterable,\s*vec!\["title", "vendor", "status"\]\)/);
  assert.match(productAdminCore, /GridColumnDef::new\("product_type".*?\.not_filterable\(\)/s);
  assert.match(productAdminCore, /GridColumnDef::new\("created_at".*?\.not_filterable\(\)/s);
});
