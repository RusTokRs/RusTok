# rustok-grid

Framework-agnostic DataGrid state machine, column math, and filtering engine for the RusToK platform.

## Architecture

Following Hexagonal / Ports & Adapters architecture, `rustok-grid` is 100% pure Rust with zero web framework dependencies. Runtime code depends only on `serde`; `serde_json` is a development-only dependency for serialization-normalization tests.

- **Columns**: column definitions, alignment, sizing, and pin configuration.
- **Resize**: column resizing math with min/max clamps (pointer and keyboard).
- **Filters**: column filter specifications (Text, Select, NumberRange, DateRange, Boolean).
- **Cells**: what a grid knows about one row (`CellValue`) — the matching and ordering rules every
  filter and sort shares, so a table maps its rows onto cells instead of writing its own predicate
  and comparator loops.
- **Facets**: server-computed facet descriptors (buckets with counts), the enumerable/open domain split, truncation, and the drill-down rule that excludes a facet's own selection while counting it.
- **Facet panel**: framework-free panel state built from those descriptors — selection markers, rendered counts, bounded/unbounded hints, and the toggle/clear rules every adapter shares.
- **Sorting**: deterministic single-column sort state machine. A server-driven grid forwards it to
  its backend; a grid over locally available rows orders them with `SortState::sort_items`.
- **Selection**: row selection, bulk action tracking, all/partial helpers.
- **Pagination**: dual pagination models (classic paged vs infinite scroll), with overflow-safe
  bounds, one-based display indexes, the fetch `offset()` a server query needs, `slice_items()` for
  client-side paging, and one page-size picker list (`DEFAULT_PAGE_SIZE_OPTIONS`,
  `GridPagination::normalize_page_size_options`) shared by every adapter.
- **State**: unified grid state composite.

## Invariants

Every type repairs its own state, so no renderer and no server handler has to defend itself against nonsense:

| Type | Invariant |
| --- | --- |
| `ColumnWidth` | `min <= current <= max`; builders and serde deserialization normalize reversed bounds and clamp `current`; `.width(px)` widens the range instead of discarding configured bounds. |
| `ColumnWidths` | Persisted widths are clamped against the *current* column definition on read and on write (`get_clamped` / `set_clamped`); `prune_to` drops the overrides of columns that no longer exist, which `GridState::clamp_widths` does on every clamp. |
| `CellValue` | A non-finite number is `Empty`, never a comparison; `matches` reads only normalized `FilterValue`s and `compare` is a total order (empty < boolean < number < text, text by code point), so no locale or collation can reorder a page between refreshes. |
| `ColumnFilters` | Mutation, `normalize()`, and serde deserialization keep only normalized, non-empty values: text is trimmed, `NaN`/infinite numbers are dropped, and reversed number/date ranges are swapped. "Key present" therefore means "filter active". `set`/`clear` return whether anything actually changed. |
| `SortState` | `column_id` and `direction` are either both set or both unset; deserialization and `normalize()` repair half-filled/blank states. |
| `GridPagination` | `page ∈ 1..=total_pages()`, `page_size >= 1`, `total_pages() >= 1`; serde deserialization and `normalize()` restore the invariants. Page-count ceiling uses quotient/remainder without overflow (`u64` totals), and `go_to_page` returns the *effective* page. |
| `RowSelection` | Ordered (deterministic serialization); `is_all_selected` on an empty page is `false`, `retain_ids` prunes rows that left the data set. |
| `GridFacet` | Construction/deserialization cut buckets at `MAX_GRID_FACET_VALUES` and report truncation; open domains never carry buckets; public-field mutation can be repaired with `normalize()`, `other_selection` excludes the facet's own entries, and panel construction normalizes its snapshot before rendering. |
| `FacetPanel` | Renders at most `MAX_GRID_FACETS` facets, keeps selection order on toggle, and never owns UI copy: labels and markers always come from the adapter. |

`GridState` composes all of the above, normalizes filters/sort/pagination during deserialization and
through `normalize()`, and offers `set_filter()`, `toggle_sort()` and `clamp_widths()` so that a
filter or sort change always resets pagination to page 1. Width overrides require the current column
definitions before they can be clamped.

`GridState::apply_rows(items, cell_of)` is the client-side path: it filters, sorts and pages in that
order and returns the rows to render. The order is a contract — paging before sorting would sort one
page, sorting before filtering would sort rows about to be dropped — and the TypeScript twin's
`applyGridRows` uses the same one.

The id of the synthetic selection column is exported as `CHECKBOX_COLUMN_ID` — adapters must never hard-code the string.

## Facets

Facet *counts* belong to the module that owns the data: a grid adapter never counts rows, and a
domain module never renders. The contract in between is this crate:

- the owning module computes buckets (value, label, count) under the current filter set and
  returns them as `GridFacet` values. `GridFacet::from_buckets` enforces the value limit and sets
  `is_truncated`; `GridFacet::open` describes a facet with an unbounded domain (free text,
  numbers, dates) that has a total but no enumerable buckets;
- `FacetDomain` tells the adapter how to render the facet: a dictionary (single or multi select),
  a boolean toggle row, or a free-form input;
- `selection_except` / `GridFacet::other_selection` implement the drill-down rule — a facet is
  counted with every *other* active filter, so its counts describe what the user would get by
  choosing a bucket instead of what is already selected;
- `FacetValue::to_filter_option` / `GridFacet::filter_options` turn buckets into the same
  `FilterOption` vocabulary the filter row already uses, so an adapter renders counts without a
  second code path;
- `FacetPanel::build` turns descriptors plus the active selection into the panel an adapter draws:
  selection markers, rendered counts, the unbounded-domain hint, the truncation hint, and the
  per-facet and panel-wide clear actions. `FacetPanel::toggle` / `clear_key` / `clear` return the
  selection the adapter must persist, so no adapter re-implements the `key=value` vocabulary.
  Copy lives in `FacetPanelLabels` and is supplied per locale by the adapter — this crate ships
  English defaults only as a neutral fallback.

## Adapters

- [`rustok-grid-leptos`](./leptos): SSR-first Leptos 0.8 `<DataGrid />` UI adapter.
- [`rustok-grid-dioxus`](./dioxus): Dioxus 0.7 `DataGrid` UI adapter with the same component set, props and markup.

Both adapters render this crate's state and compute nothing of their own: the page-size picker list
comes from `DEFAULT_PAGE_SIZE_OPTIONS` (re-exported, not repeated), a resize handle is drawn only for
a column that is both `resizable` and flexible, `aria-sort` comes from `SortState::aria_sort_for`, and
the select-all control carries `role="checkbox"` with a tri-state `aria-checked`.

## Parity with the TypeScript twin

[`packages/rustok-ui-grid`](../../../packages/rustok-ui-grid) is the same state machine for the Next
hosts, module by module. Two gates keep the twins from drifting:

- `npm run verify:ui:grid-parity` — the public surface, name by name and in both directions, with an
  explicit reviewed table of renamed twins and justified one-sided symbols (`scripts/verify/verify-ui-grid-parity.mjs`).
- `npm run test:verify:ui:grid-parity` — behaviour, run against the TypeScript package through Node's
  type stripping; the Rust mirror cases live in this crate's `#[cfg(test)]` modules.

## Known Limitations / Pending Implementation

- **Module admin cores still filter and sort by hand.** `rustok-order`/`rustok-pricing`
  (`admin/src/core/grid.rs`) and `rustok-customer`/`rustok-fulfillment`/`rustok-inventory`/`rustok-product`
  (`admin/src/core.rs`) each carry their own `matches_*_filter` + `filter_*` loop. They agree with
  `CellValue::matches` today, so this is duplicated logic rather than a defect; moving them onto
  `ColumnFilters::retain_matching` / `SortState::sort_items` is the next tracked phase
  (`docs/standards/continuous-review-ledger.md`).
- **Module admin screens do not share a renderer.** The same six crates hand-roll their `<table>`
  markup per adapter instead of using one grid view over this state machine.
- **Grid chrome copy is still literal English in both adapters.** "Rows per page:", "Showing", the
  mode toggle and the resize/filter tooltips are hardcoded, while the facet panel already takes
  `FacetPanelLabels`. The fix is one label struct in this crate (same shape, English defaults)
  consumed by both adapters and passed down per locale — deliberately not shipped as an unverifiable
  sweep across the two adapter crates, and tracked in the ledger.
- **Page reset differs by mode between adapters.** The Leptos grid resets to page 1 on a filter or
  sort change in both pagination modes; the Dioxus grid does so only in client-side mode. It is a
  rendering-policy difference the owners of those adapters accept; this crate's `set_filter` /
  `toggle_sort` always reset, and an adapter that keeps route state decides what to persist.
- **`cargo` verification is a CI/maintainer step for this change set.** The sandbox that produced it
  has no Rust toolchain, so the new and edited Rust is grammar-checked
  (`scripts/audit/rust_syntax_check.mjs`) and structurally reviewed, not compiled or test-run here.
