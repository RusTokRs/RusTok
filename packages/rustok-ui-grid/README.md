# `@rustok/ui-grid`

Framework-free table toolkit for RusTok — the TypeScript twin of `crates/ui/rustok-grid`.

## Why it exists

Table state machines and table arithmetic are owner-side, not component-side:

- `rustok-grid` (Rust) owns column specifications, the cell vocabulary behind filtering and sorting,
  pagination maths, sort transitions, row selection and the facet/panel contract.
- This package owns exactly the same contract for TypeScript/Next hosts.
- Renderers — `rustok-grid-leptos`, the Dioxus adapters, and `apps/next-admin`'s
  `widgets/data-table` — map and draw that state without reinventing calculations.

The root entry (`@rustok/ui-grid`) imports no React, no Next and no fetch client, so it can be
consumed from a server component, a route handler or a test runner. There is deliberately **no**
React companion in this package: the React components that used to live in `src/react/` had zero
consumers, and the Next admin renders its own `DataTableStatic`
(`apps/next-admin/src/widgets/data-table/data-table-static.tsx`), which delegates its math here.
Re-adding a second renderer here would reintroduce the divergence that removal fixed.

## Layout

| Module | Mirrors | Carries |
|---|---|---|
| `column.ts` | `rustok-grid::column` | `ColumnAlign` (`left`/`center`/`right`), `PinnedSide`, `ColumnWidth`, `GridColumnDef`, `CHECKBOX_COLUMN_ID`, immutable width/configuration builders, `hasFilter`, `hasFilterRow`, `visibleColumns`. |
| `cell.ts` | `rustok-grid::cell` | `CellValue` (text/number/boolean/empty) + `CellAccessor`, `matchesFilterValue`, `compareCells`, `filterRows`, `sortRows`, and the primitive comparisons both use (`textContainsIgnoreCase`, `equalsIgnoreAsciiCase`, `compareCodePoints`). |
| `pagination.ts` | `rustok-grid::pagination` | `DEFAULT_PAGE_SIZE`, `DEFAULT_PAGE_SIZE_OPTIONS`, `normalizePageSizeOptions`, `PaginationMode` (`paged`/`infinite`), `GridPagination`, overflow-safe page/range helpers, mode-aware `rangeLabel`, client-side `sliceBounds`/`sliceItems` and immutable page transitions. JavaScript totals are bounded to safe integers. |
| `sort.ts` | `rustok-grid::sort` | `SortDirection` (`asc`/`desc`), `SortState`, `cycleSort` (returns a state, never `null`), `oppositeSortDirection`, normalization and WAI-ARIA sort values. |
| `filter.ts` | `rustok-grid::filter` | `GridFilterType`, `FilterOption`, `FilterValue`, normalized `ColumnFilters`, `isFilterValueActive`/`hasActiveFilters`, structural `isFilterEqual` and immutable `setColumnFilter`/`removeColumnFilter`/`clearColumnFilters` transitions that report `changed`. |
| `selection.ts` | `rustok-grid::selection` | `RowSelection` (immutable Set), deterministic id serialization (`rowSelectionIds`), toggle/select/deselect/retain and all/partial-selection helpers; `addRowsToSelection` preserves selections outside the current page when selecting its rows. |
| `resize.ts` | `rustok-grid::resize` | `ColumnWidths`, persisted-width normalization/clamping/**pruning**, pointer-delta math and keyboard resize steps. |
| `facet.ts` | `rustok-grid::facet` | `FacetDomain` (dictionary/boolean/open), `FacetValue`, `facetFromBuckets` (cuts at `MAX_GRID_FACET_VALUES` and reports `isTruncated`), `facetFilterOptions`, and the `code=value` selection vocabulary: `selectionEntry`/`splitSelection`/`parseSelection`/`serializeSelection`, `isSelectionSelected` (keys ASCII-case-insensitive, values exact), `hasSelectionForKey`, `selectionForKey`, `selectionExcept`, `selectionAfterToggle` (order-preserving), `selectionAfterClearKey`, `selectionAfterClear`. |
| `panel.ts` | `rustok-grid::facet_panel` | `FacetPanelLabels` + `ENGLISH_FACET_PANEL_LABELS`, `countLabel`, `buildFacetPanel` (normalizes untrusted facet snapshots, caps at `MAX_GRID_FACETS`, adds hints/markers), panel selection transitions, and the per-facet views the owner needs for counting (`facetEntriesForCode`, `otherFacetSelectionForCode`). |
| `state.ts` | `rustok-grid::state` | Composite `GridState`, runtime normalization, filter/sort transitions that reset pagination, width clamping, selection pruning, and `applyGridRows` (filter → sort → page) for grids that own their full row set client-side. |
| `url.ts` | `rustok_ui_core::apply_ui_query_pairs` | `applyQueryPairs` (stable key positions, duplicate-key replacement/removal, blank-value removal, fragment preservation, browser form encoding) and `queryParam`. |

## Contract notes

- **Reads repair, writes validate.** `normalize*` functions and every width/filter/sort *read* repair
  half-filled state coming from a URL, localStorage or a serialized snapshot — a missing direction
  means `asc`, a blank column id clears, an out-of-range width clamps, an unknown filter key drops.
  The throwing paths are deliberate and narrow: `assertColumnWidthPx` (write path), a non-safe-integer
  `total`/count (`calculatePageCount`, `normalizeFacetSource`) and an oversized facet page size,
  because those are arithmetic that cannot be repaired without silently lying about a row count.
  Render paths never call the throwing variants: `getStoredColumnWidth`/`getClampedColumnWidth` clamp
  instead of throwing, exactly like `ColumnWidths::get_clamped` in Rust.
- Pagination arithmetic is unified: `calculatePageCount` guarantees a floor of 1 page (an empty table
  reports 1 page, never 0), repairs a zero page size to 1, and rejects totals outside JavaScript's
  exact-integer range instead of rounding them. Facet totals and bucket counts use the same
  exact-integer boundary. `rangeLabel` is mode-aware: an infinite grid reports the window it has
  loaded, not a page range it does not have.
- `rowSelectionIds` sorts by code point. Rust stores ids in a `BTreeSet`, so its serialization is
  sorted; returning raw `Set` insertion order would make one and the same selection serialize
  differently after a different click order, and bulk actions and cache keys compare that list.
- `filterRows`/`sortRows` implement the same trimming, case folding, range and ISO-date rules the
  module admin cores used to hand-roll, so a new grid does not need its own `matches_*_filter`.
  Text filters fold with Unicode `toLowerCase` (a Cyrillic query finds a Cyrillic value) while
  selection keys and select-option ids fold ASCII-only, matching Rust's `eq_ignore_ascii_case`.
  Ordering is code-point ordering, never locale collation, so a page refresh cannot reorder ties.
- A `#fragment` is never part of a query value: `applyQueryPairs` and `queryParam` cut it off before
  parsing and re-append it to the result, so `/products?color=red#reviews` filters on `red`.
- Persisted Rust state is normalized on deserialization; TypeScript consumers call the corresponding
  explicit `normalize*` functions at transport/storage boundaries.

## Consumers

- `apps/next-frontend/packages/rustok-product/src/catalog/facets.ts` re-exports the selection
  vocabulary under the product names and maps the owner answer into `buildCatalogFacetFiltersView`.
- `apps/next-admin/src/widgets/data-table/data-table-faceted-filter.tsx` uses the shared transitions
  for its multi-select facets.
- `apps/next-admin/src/widgets/data-table/data-table-static.tsx` (the Next renderer owner) takes its
  `aria-sort` value and its range label from here instead of re-deriving them.

Both hosts install it through a `file:` dependency (`file:../../packages/rustok-ui-grid`), map it in
`tsconfig.json` → `paths` and list it in `next.config.*` → `transpilePackages`, like every other
workspace package.

## Known Limitations / Pending Implementation

- **Module admin cores are not yet on `cell.ts`.** `rustok-order`/`rustok-pricing` (`admin/src/core/grid.rs`)
  and `rustok-customer`/`rustok-fulfillment`/`rustok-inventory`/`rustok-product` (`admin/src/core.rs`)
  still run their own `matches_*_filter` + `filter_*` loops. They are behaviourally equivalent today,
  so this is duplicated logic rather than a bug; migrating six module crates is the next tracked
  phase and is recorded in `docs/standards/continuous-review-ledger.md`.
- **Module admin screens are not yet on a shared renderer.** The same six crates hand-roll their
  `<table>` markup per adapter instead of using one shared grid view.
- **`queryParam` has no Rust twin.** Reading one query key out of a route string is a
  client/router-boundary concern in TypeScript; the Rust surfaces receive typed server-function
  parameters and never parse a URL themselves. It is listed as an accepted, justified exception in
  `scripts/verify/verify-ui-grid-parity.mjs` rather than answered with a stub.
- **A facet's identity is `code` here and `key` in Rust.** The TS value feeds JSX, where `key` is a
  reserved attribute name, and the product owner calls this field `code` end to end; the field is
  therefore named once, on the TS side, and mapped at the boundary. Also listed as an exception.
- **Verification in this sandbox is static.** `node --test` parity/product gates and
  `scripts/audit/rust_syntax_check.mjs` run here; `cargo check/test -p rustok-grid` does not (no
  Rust toolchain), so the Rust half of this change is compile-verified by CI/maintainer, not by us.

Parity with the Rust original is locked by
`scripts/verify/verify-product-ui-grid-facets.mjs` (product surface),
`scripts/verify/verify-ui-grid-parity.mjs` (name-by-name twin surface, bidirectional, with a reviewed
exception table) and `scripts/verify/verify-product-ui-grid-facets.test.mjs` /
`scripts/verify/verify-ui-grid-parity.test.mjs` (behaviour, executing this package through Node's
type stripping).
