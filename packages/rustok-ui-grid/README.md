# `@rustok/ui-grid`

Framework-free facet and query-state toolkit for RusTok tables — the TypeScript twin of
`crates/ui/rustok-grid`.

## Why it exists

Facet counting is owner-side: a product owner counts the buckets (`CatalogService::*_catalog_facets`),
`rustok-grid` owns the panel contract, and every adapter only maps the answer and renders it. The
Leptos surfaces got that split in the product audit remediation waves; the Next surfaces used to keep
a private copy of the same rules inside one module package, which meant a second table (the admin
data-table host) could not reuse them.

This package is that shared half. It imports no React, no Next and no fetch client, so it can be
consumed from a server component, a client widget, a route handler or a test runner.

## Layout

| Module | Mirrors | Carries |
|---|---|---|
| `facet.ts` | `rustok-grid::facet` | `FacetDomain` (dictionary/boolean/open), `facetFromBuckets` (cuts at `MAX_GRID_FACET_VALUES` and reports `isTruncated`), `facetFilterOptions`, and the `code=value` selection vocabulary: `selectionEntry`/`splitSelection`/`parseSelection`/`serializeSelection`, `isSelectionSelected` (keys case-insensitive, values exact), `hasSelectionForKey`, `selectionForKey`, `selectionExcept`, `selectionAfterToggle` (order-preserving), `selectionAfterClearKey`, `selectionAfterClear`. |
| `panel.ts` | `rustok-grid::facet_panel` | `FacetPanelLabels` + `ENGLISH_FACET_PANEL_LABELS`, `countLabel`, `buildFacetPanel` (caps at `MAX_GRID_FACETS`, adds hints, markers and per-bucket `selectionEntry`) and the panel-level transitions. |
| `url.ts` | `rustok_ui_core::apply_ui_query_pairs` | `applyQueryPairs` (existing key keeps its position, blank value removes the key, browser form encoding) and `queryParam`. |

## Contract notes

- The panel never builds a link. It reports `selectionEntry`/`selected` and the adapter turns a
  selection into an href (the storefront), a router push or a local filter update.
- Copy is never owned here: an adapter passes `FacetPanelLabels`, because only the adapter knows the
  locale. The count template is code-owned (`({count})` by default) and substituted textually, so it
  must not become a Fluent message — `{count}` would parse as a message reference there.
- Keys compare case-insensitively while values compare exactly: a value is an option id.

## Consumers

- `apps/next-frontend/packages/rustok-product/src/catalog/facets.ts` re-exports the selection
  vocabulary under the product names and maps the owner answer into `buildCatalogFacetFiltersView`.
- `apps/next-admin/src/widgets/data-table/data-table-faceted-filter.tsx` uses the shared transitions
  for its multi-select facets.

Both hosts install it through a `file:` dependency (`file:../../packages/rustok-ui-grid`), map it in
`tsconfig.json` → `paths` and list it in `next.config.*` → `transpilePackages`, like every other
workspace package.

Parity with the Rust original is locked by
`scripts/verify/verify-ui-grid-facets.mjs` (surface) and `scripts/verify/verify-ui-grid-facets.test.mjs`
(behaviour, executing this package through Node's type stripping).
