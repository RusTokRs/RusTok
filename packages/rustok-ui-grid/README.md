# `@rustok/ui-grid`

Framework-free table toolkit and React DataGrid companion for RusTok — the TypeScript twin of
`crates/ui/rustok-grid`.

## Why it exists

DataGrid state machine and arithmetic are owner-side:
- `rustok-grid` owns column specifications, pagination maths, sort transitions, filters, row selection, and the facet/panel contract.
- UI adapters (`rustok-grid-leptos`, `@rustok/ui-grid/react`) map and render this state without reinventing calculations.

This package is that shared half for TypeScript/Next.js applications. Its root entry (`@rustok/ui-grid`) imports no React, no Next and no fetch client, so it can be consumed from a server component, a route handler or a test runner. Its React companion (`@rustok/ui-grid/react`) provides standard companion components for `@tanstack/react-table`.

## Layout

| Module | Mirrors | Carries |
|---|---|---|
| `column.ts` | `rustok-grid::column` | `ColumnAlign` (`left`/`center`/`right`), `PinnedSide`, `ColumnWidth`, `GridColumnDef`, `CHECKBOX_COLUMN_ID`, `visibleColumns`, `hasFilterRow`. |
| `pagination.ts` | `rustok-grid::pagination` | `DEFAULT_PAGE_SIZE`, `PaginationMode` (`paged`/`infinite`), `GridPagination`, `calculatePageCount`, `clampPage`, `calculateOffset`, `hasPreviousPage`, `hasNextPage`, `dataTableRangeLabel`. |
| `sort.ts` | `rustok-grid::sort` | `SortDirection` (`asc`/`desc`), `SortState`, `cycleSort`, `oppositeSortDirection`, `compareValues`, `ariaSortValue`. |
| `filter.ts` | `rustok-grid::filter` | `GridFilterType`, `FilterOption`, `FilterValue`, `ColumnFilters`, `isFilterActive`, `normalizeFilters`. |
| `selection.ts` | `rustok-grid::selection` | `RowSelection` (immutable Set), `toggleRowSelection`, `selectAllRows`, `selectRow`, `deselectRow`, `deselectAllRows`, `clearSelection`, `retainSelectedRows`, `isRowSelected`, `areAllRowsSelected`, `areSomeRowsSelected`. |
| `resize.ts` | `rustok-grid::resize` | `ColumnWidths` (persisted user width storage), `calculateResizedWidth`, `stepResizedWidth`, `KEYBOARD_RESIZE_STEP`. |
| `state.ts` | `rustok-grid::state` | `GridState`, `createGridState`, `setGridFilter`, `resetGridFilters`, `toggleGridSort`, `pruneGridSelection`, `clampWidths`, `normalizeGridState`. |
| `facet.ts` | `rustok-grid::facet` | `FacetDomain` (dictionary/boolean/open), `facetFromBuckets` (cuts at `MAX_GRID_FACET_VALUES` and reports `isTruncated`), `facetFilterOptions`, and the `code=value` selection vocabulary: `selectionEntry`/`splitSelection`/`parseSelection`/`serializeSelection`, `isSelectionSelected` (keys case-insensitive, values exact), `hasSelectionForKey`, `selectionForKey`, `selectionExcept`, `selectionAfterToggle` (order-preserving), `selectionAfterClearKey`, `selectionAfterClear`. |
| `panel.ts` | `rustok-grid::facet_panel` | `FacetPanelLabels` + `ENGLISH_FACET_PANEL_LABELS`, `countLabel`, `buildFacetPanel` (caps at `MAX_GRID_FACETS`, adds hints, markers and per-bucket `selectionEntry`) and the panel-level transitions. |
| `url.ts` | `rustok_ui_core::apply_ui_query_pairs` | `applyQueryPairs` (existing key keeps its position, blank value removes the key, browser form encoding) and `queryParam`. |
| `react/` | `rustok-grid-leptos` | `@rustok/ui-grid/react`: companion adapter providing `DataTableStatic`, `DataTableEmptyState`, `DataTablePaginationBar` for `@tanstack/react-table`. |


## Contract notes

- The root package (`@rustok/ui-grid`) stays strictly framework-free.
- The React companion (`@rustok/ui-grid/react`) provides standard components as a companion to `@tanstack/react-table`.
- Pagination arithmetic is unified: `calculatePageCount` guarantees a floor of 1 page (an empty table reports 1 page, never 0).
- Keys compare case-insensitively while values compare exactly: a value is an option id.

## Consumers

- `apps/next-frontend/packages/rustok-product/src/catalog/facets.ts` re-exports the selection
  vocabulary under the product names and maps the owner answer into `buildCatalogFacetFiltersView`.
- `apps/next-admin/src/widgets/data-table/data-table-faceted-filter.tsx` uses the shared transitions
  for its multi-select facets.
- `apps/next-admin/src/features/modules/components/modules-list.tsx` uses `DataTableStatic` for build history and module listings.

Both hosts install it through a `file:` dependency (`file:../../packages/rustok-ui-grid`), map it in
`tsconfig.json` → `paths` and list it in `next.config.*` → `transpilePackages`, like every other
workspace package.

Parity with the Rust original is locked by
`scripts/verify/verify-product-ui-grid-facets.mjs` (surface) and `scripts/verify/verify-product-ui-grid-facets.test.mjs`
(behaviour, executing this package through Node's type stripping).
