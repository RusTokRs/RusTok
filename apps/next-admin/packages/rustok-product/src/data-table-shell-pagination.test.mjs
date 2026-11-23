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
  assert.match(shellSource, /getRowId,\s*routeQuery,\s*pageCount,/);
});

test('the URL remains authoritative when table filters change via browser history', () => {
  assert.match(hookSource, /lastUrlRouteSignature = React\.useRef/);
  assert.match(
    hookSource,
    /lastUrlRouteSignature\.current !== currentRouteSignature/
  );
  assert.match(hookSource, /setColumnFilters\(initialColumnFilters\)/);
});

test('overlapping local filter navigations and browser history cannot replay stale debounces', () => {
  assert.match(hookSource, /filterNavigationVersion = React\.useRef\(0\)/);
  assert.match(hookSource, /version !== filterNavigationVersion\.current/);
  assert.match(
    hookSource,
    /pendingLocalRouteSignatures\.current\.indexOf\(currentRouteSignature\)/
  );
  assert.match(hookSource, /pendingLocalRouteSignatures\.current\.push\(expectedRouteSignature\)/);
  assert.match(hookSource, /pendingRouteWrites\.length > 0/);
  assert.match(
    hookSource,
    /pendingRouteWrites\[pendingRouteWrites\.length - 1\] !== expectedRouteSignature/
  );
  assert.match(hookSource, /pendingLocalColumnFilterSignature = React\.useRef<string \| null>\(null\)/);
  assert.match(hookSource, /pendingLocalColumnFilterSignature\.current = columnFiltersSignature\(next\)/);
  assert.match(hookSource, /pendingLocalColumnFilterSignature\.current !== null/);
  assert.match(
    hookSource,
    /pendingLocalColumnFilterSignature\.current ===\s*columnFiltersSignature\(initialColumnFilters\)/
  );
  assert.match(hookSource, /no-op URL write has no route response to acknowledge it/);


  assert.match(
    hookSource,
    /pendingLocalRouteSignatures\.current\.splice\(matchedLocalNavigation, 1\)/
  );
  assert.match(hookSource, /pendingLocalRouteSignatures\.current = \[\]/);
  assert.match(
    hookSource,
    /window\.addEventListener\(\s*'popstate',\s*invalidatePendingFilterNavigation\s*\)/
  );
  assert.match(
    hookSource,
    /window\.removeEventListener\(\s*'popstate',\s*invalidatePendingFilterNavigation\s*\)/
  );
  assert.match(hookSource, /invalidatePendingFilterNavigation = \(\) => \{/);

  assert.match(hookSource, /useIsomorphicLayoutEffect\(/);
  assert.match(hookSource, /filterNavigationVersion\.current \+= 1/);
  assert.match(
    hookSource,
    /debouncedSetFilterValues\(filterUpdates, filterNavigationVersion\.current\)/
  );
  assert.match(hookSource, /routeQuerySignature\(routeQuery\)/);
  assert.match(hookSource, /routeQuery\?: Record<string, string>/);
  assert.match(hookSource, /pendingLocalRouteSignatures\.current\.indexOf\(currentRouteSignature\)/);
  assert.match(hookSource, /Object\.prototype\.hasOwnProperty\.call\(expectedQuery, PAGE_KEY\)/);
});

test('Product table passes non-table facet state into navigation reconciliation', () => {
  assert.match(productTableSource, /routeQuery=\{routeQuery\}/);
  assert.match(shellSource, /routeQuery\?: Record<string, string>/);
  assert.match(shellSource, /routeQuery,/);
});

test('catalog controls refresh their uncontrolled defaults after facet navigation', () => {
  assert.match(catalogFilterSource, /key=\{categoryId \?\? ''\}/);
  assert.match(catalogFilterSource, /key=\{attributeFilters\.join\(';'\)\}/);
  assert.match(catalogFilterSource, /key=\{sortBy \?\? 'published_at'\}/);
  assert.match(catalogFilterSource, /key=\{sortDirection \?\? 'desc'\}/);
});
