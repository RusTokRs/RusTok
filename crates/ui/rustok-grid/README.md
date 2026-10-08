# rustok-grid

Framework-agnostic DataGrid state machine, column math, and filtering engine for the RusToK platform.

## Architecture

Following Hexagonal / Ports & Adapters architecture, `rustok-grid` is 100% pure Rust with zero web framework dependencies (its only dependency is `serde`):

- **Columns**: column definitions, alignment, sizing, and pin configuration.
- **Resize**: column resizing math with min/max clamps (pointer and keyboard).
- **Filters**: column filter specifications (Text, Select, NumberRange, DateRange, Boolean).
- **Facets**: server-computed facet descriptors (buckets with counts), the enumerable/open domain split, truncation, and the drill-down rule that excludes a facet's own selection while counting it.
- **Facet panel**: framework-free panel state built from those descriptors — selection markers, rendered counts, bounded/unbounded hints, and the toggle/clear rules every adapter shares.
- **Sorting**: deterministic single-column sort state machine (the adapter can delegate actual ordering to the server).
- **Selection**: row selection, bulk action tracking, all/partial helpers.
- **Pagination**: dual pagination models (classic paged vs infinite scroll), with overflow-safe bounds and one-based display indexes.
- **State**: unified grid state composite.

## Invariants

Every type repairs its own state, so no renderer and no server handler has to defend itself against nonsense:

| Type | Invariant |
| --- | --- |
| `ColumnWidth` | `min <= current <= max`; builders are order-independent — `.width(px)` widens the range instead of discarding a previously configured `min`/`max`. |
| `ColumnWidths` | Persisted widths are clamped against the *current* column definition on read and on write (`get_clamped` / `set_clamped`). |
| `ColumnFilters` | Only normalized, non-empty values are stored: text is trimmed, `NaN`/infinite numbers are dropped, reversed number/date ranges are swapped. "Key present" therefore means "filter active". `set`/`clear` return whether anything actually changed. |
| `SortState` | `column_id` and `direction` are either both set or both unset; `normalize()` repairs deserialized state. |
| `GridPagination` | `page ∈ 1..=total_pages()`, `page_size >= 1`, `total_pages() >= 1`; all arithmetic is integer and overflow-safe (`u64` totals, `saturating_*`, no `f64` rounding on large data sets). `go_to_page` returns the *effective* page. |
| `RowSelection` | Ordered (deterministic serialization); `is_all_selected` on an empty page is `false`, `retain_ids` prunes rows that left the data set. |
| `GridFacet` | Bucket lists are cut at `MAX_GRID_FACET_VALUES` with `is_truncated` reporting it; open domains never carry buckets; `other_selection` never returns the facet's own entries. |
| `FacetPanel` | Renders at most `MAX_GRID_FACETS` facets, keeps selection order on toggle, and never owns UI copy: labels and markers always come from the adapter. |

`GridState` composes all of the above and offers `normalize()`, `set_filter()`, `toggle_sort()` and `clamp_widths()` so that a filter or sort change always resets pagination to page 1.

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
- [`rustok-grid-dioxus`](./dioxus): Dioxus 0.6 `DataGrid` UI adapter with the same component set, props and markup.
