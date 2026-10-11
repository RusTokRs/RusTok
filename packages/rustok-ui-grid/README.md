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
| `column.ts` | `rustok-grid::column` | `ColumnAlign` (`left`/`center`/`right`), `PinnedSide`, `ColumnWidth`, `GridColumnDef`, `CHECKBOX_COLUMN_ID`, immutable width/configuration builders, `hasFilter`, `hasFilterRow`, `visibleColumns`. |
| `pagination.ts` | `rustok-grid::pagination` | `DEFAULT_PAGE_SIZE`, `PaginationMode` (`paged`/`infinite`), `GridPagination`, overflow-safe page/range helpers, client-side slice bounds and immutable page transitions. JavaScript totals are bounded to safe integers. |
| `sort.ts` | `rustok-grid::sort` | `SortDirection` (`asc`/`desc`), `SortState`, `cycleSort`, `oppositeSortDirection`, normalization and WAI-ARIA sort values. |
| `filter.ts` | `rustok-grid::filter` | `GridFilterType`, `FilterOption`, `FilterValue`, normalized `ColumnFilters`, active-filter queries and immutable set/remove/clear operations. |
| `selection.ts` | `rustok-grid::selection` | `RowSelection` (immutable Set), deterministic id serialization, toggle/select/deselect/retain and all/partial-selection helpers; `addRowsToSelection` preserves selections outside the current page when selecting its rows. |
| `resize.ts` | `rustok-grid::resize` | `ColumnWidths`, persisted-width normalization/clamping, pointer-delta math and keyboard resize steps. |
| `facet.ts` | `rustok-grid::facet` | `FacetDomain` (dictionary/boolean/open), `facetFromBuckets` (cuts at `MAX_GRID_FACET_VALUES` and reports `isTruncated`), `facetFilterOptions`, and the `code=value` selection vocabulary: `selectionEntry`/`splitSelection`/`parseSelection`/`serializeSelection`, `isSelectionSelected` (keys case-insensitive, values exact), `hasSelectionForKey`, `selectionForKey`, `selectionExcept`, `selectionAfterToggle` (order-preserving), `selectionAfterClearKey`, `selectionAfterClear`. |
| `panel.ts` | `rustok-grid::facet_panel` | `FacetPanelLabels` + `ENGLISH_FACET_PANEL_LABELS`, `countLabel`, `buildFacetPanel` (normalizes untrusted facet snapshots, caps at `MAX_GRID_FACETS`, adds hints/markers) and panel selection transitions. |
| `state.ts` | `rustok-grid::state` | Composite `GridState`, runtime normalization, filter/sort transitions that reset pagination, width clamping and selection pruning. |
| `url.ts` | `rustok_ui_core::apply_ui_query_pairs` | `applyQueryPairs` (stable key positions, duplicate-key replacement/removal, blank-value removal and browser form encoding) and `queryParam`. |
| `react/` | `rustok-grid-leptos` | `@rustok/ui-grid/react`: companion adapter providing `DataTableStatic`, `DataTableEmptyState`, `DataTablePaginationBar` for `@tanstack/react-table`. |


## Contract notes

- The root package (`@rustok/ui-grid`) stays strictly framework-free.
- The React companion (`@rustok/ui-grid/react`) provides standard components as a companion to `@tanstack/react-table`.
- Pagination arithmetic is unified: `calculatePageCount` guarantees a floor of 1 page (an empty table reports 1 page, never 0), repairs a zero page size to 1, and rejects totals outside JavaScript's exact-integer range instead of rounding them. Facet totals and bucket counts use the same exact-integer boundary.
- Facet keys compare ASCII-case-insensitively (matching Rust's `eq_ignore_ascii_case`) while values compare exactly: a value is an option id.
- The `code=value` entries are joined with the Rust core's semicolon-list contract; malformed selections are ignored by panel transitions, and one toggle removes every duplicate of the selected entry.
- Persisted Rust state is normalized on deserialization; TypeScript consumers use the corresponding explicit `normalize*` functions at transport/storage boundaries.

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
