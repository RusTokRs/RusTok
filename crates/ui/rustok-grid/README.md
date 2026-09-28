# rustok-grid

Framework-agnostic DataGrid state machine, column math, and filtering engine for the RusToK platform.

## Architecture

Following Hexagonal / Ports & Adapters architecture, `rustok-grid` is 100% pure Rust with zero web framework dependencies (its only dependency is `serde`):

- **Columns**: column definitions, alignment, sizing, and pin configuration.
- **Resize**: column resizing math with min/max clamps (pointer and keyboard).
- **Filters**: column filter specifications (Text, Select, NumberRange, DateRange, Boolean).
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

`GridState` composes all of the above and offers `normalize()`, `set_filter()`, `toggle_sort()` and `clamp_widths()` so that a filter or sort change always resets pagination to page 1.

The id of the synthetic selection column is exported as `CHECKBOX_COLUMN_ID` — adapters must never hard-code the string.

## Adapters

- [`rustok-grid-leptos`](./leptos): SSR-first Leptos 0.8 `<DataGrid />` UI adapter.
