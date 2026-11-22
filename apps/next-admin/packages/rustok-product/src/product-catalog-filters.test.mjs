import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const index = readFileSync(new URL('./index.ts', import.meta.url), 'utf8');
const filters = readFileSync(
  new URL('./components/products/product-table/product-catalog-filters.tsx', import.meta.url),
  'utf8'
);
const page = readFileSync(
  new URL('../../../src/app/dashboard/product/page.tsx', import.meta.url),
  'utf8'
);

test('category search options contain structural categories only', () => {
  assert.match(
    index,
    /filter\(\(category\) => category\.kind\.toLowerCase\(\) === 'structural'\)/
  );
});

test('attribute search options require filterable Product or Both scope', () => {
  assert.match(index, /isFilterable\s+isSortable\s+scope/);
  assert.match(
    index,
    /attribute\.isFilterable &&\s*\(attribute\.scope === 'product' \|\| attribute\.scope === 'both'\)/
  );
});

test('facet counts share non-pagination catalog filters', () => {
  const facetFunction =
    index.match(/export async function listAdminCatalogFacets[\s\S]*?\n\}/)?.[0] ?? '';
  assert.match(facetFunction, /vendor: filter\.vendor/);
  assert.match(facetFunction, /categoryId: filter\.categoryId/);
  assert.match(facetFunction, /attributeFilters:/);
  assert.doesNotMatch(facetFunction, /\b(?:page|perPage):/);
});

test('facet navigation resets the page and preserves page size and other filters', () => {
  assert.match(filters, /preservedQuery/);
  assert.match(filters, /query\.set\('page', '1'\)/);
  assert.match(filters, /query\.set\('perPage', String\(perPage\)\)/);
});

test('the server page loads category options, attribute options, and facets', () => {
  assert.match(page, /listCatalogCategorySearchOptions/);
  assert.match(page, /listCatalogAttributeSearchOptions/);
  assert.match(page, /listAdminCatalogFacets/);
  assert.match(page, /facets=\{facets\}/);
});
