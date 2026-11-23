import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const shellSource = await readFile(
  new URL('../../../src/widgets/data-table/data-table-shell.tsx', import.meta.url),
  'utf8'
);
const hookSource = await readFile(
  new URL('../../../src/shared/hooks/use-data-table.ts', import.meta.url),
  'utf8'
);

test('server data table forwards its resolved page size into useDataTable defaults', () => {
  assert.match(shellSource, /defaultPageSize = 20/);
  assert.match(
    shellSource,
    /initialState:\s*\{\s*pagination:\s*\{\s*pageIndex:\s*0,\s*pageSize\s*\}\s*\}/s
  );
  assert.match(hookSource, /initialState\?\.pagination\?\.pageSize \?\? 10/);
});

test('text filter values restored from the URL remain scalar strings', () => {
  assert.match(hookSource, /filters\.push\(\{\s*id: key,\s*value\s*\}\)/s);
  assert.doesNotMatch(hookSource, /value\.split\(/);
});

const productTableSource = await readFile(
  new URL('./components/products/product-table/product-table.tsx', import.meta.url),
  'utf8'
);
const catalogFilterSource = await readFile(
  new URL('./components/products/product-table/product-catalog-filters.tsx', import.meta.url),
  'utf8'
);

test('product catalog page selection is keyed by canonical product identity', () => {
  assert.match(productTableSource, /getRowId=\{\(product\) => product\.id\}/);
  assert.match(productTableSource, /defaultPageSize=\{perPage\}/);
  assert.match(shellSource, /getRowId\?: \(row: TData, index: number\) => string/);
  assert.match(shellSource, /getRowId,\s*pageCount,/);
});

test('the URL remains authoritative when table filters change via browser history', () => {
  assert.match(hookSource, /lastUrlColumnFilters = React\.useRef/);
  assert.match(hookSource, /lastUrlColumnFilters\.current !== routeFilters/);
  assert.match(hookSource, /setColumnFilters\(initialColumnFilters\)/);
});

test('browser history invalidates pending debounced filter writes without cancelling newer local edits', () => {
  assert.match(hookSource, /filterNavigationVersion = React\.useRef\(0\)/);
  assert.match(hookSource, /version !== filterNavigationVersion\.current/);
  assert.match(hookSource, /pendingLocalFilterSignature\.current === routeFilters/);
  assert.match(hookSource, /React\.useLayoutEffect\(/);
  assert.match(hookSource, /filterNavigationVersion\.current \+= 1/);
  assert.match(
    hookSource,
    /debouncedSetFilterValues\(filterUpdates, filterNavigationVersion\.current\)/
  );
});

test('catalog controls refresh their uncontrolled defaults after facet navigation', () => {
  assert.match(catalogFilterSource, /key=\{categoryId \?\? ''\}/);
  assert.match(catalogFilterSource, /key=\{attributeFilters\.join\(';'\)\}/);
  assert.match(catalogFilterSource, /key=\{sortBy \?\? 'published_at'\}/);
  assert.match(catalogFilterSource, /key=\{sortDirection \?\? 'desc'\}/);
});
