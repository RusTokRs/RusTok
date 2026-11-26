# rustok-grid-leptos

SSR-first Leptos 0.8 adapter for [`rustok-grid`](..).

For component contracts, public interfaces, and accessibility invariants, see [CRATE_API.md](CRATE_API.md).

Provides the interactive `<DataGrid />` component, resizable column headers, a per-column filter row, a toolbar with column visibility, selection management, and pagination (classic or infinite scroll).

The crate is a *thin* adapter: it owns no business rules, all state transitions come from `rustok-grid`.

## Data flow

`<DataGrid />` is **uncontrolled by default, controlled on demand** — filters, sort, selection and pagination can each be passed in as an `RwSignal`, otherwise they are created locally.

Pagination has two modes, selected implicitly:

| | client-side (no `on_page_change`) | server-side (`on_page_change` provided) |
| --- | --- | --- |
| `data` | full data set | current page only |
| `pagination.total` | derived from `data` by the grid | **must be supplied by the caller** (otherwise the grid can only know about page 1) |
| slicing | done by the grid (`GridPagination::slice_bounds`) | done by the caller |
| callbacks | — | receive the *clamped, effective* page, never an out-of-range number |

## Sorting

Header sorting is only *offered* when it can change the rows, so a column never grows an affordance
that does nothing:

| call site | header | who orders the rows |
| --- | --- | --- |
| `sort_cell` provided | sort button + `aria-sort` | the grid: `(row, column) -> CellValue`, ordered by the core comparator through `SortState::sort_items` |
| `on_sort_change` provided, or server-side pagination | sort button + `aria-sort` | the caller (it re-queries) |
| neither | plain column label | nobody — and the header says so |

`sort_cell` keys are resolved once per row before the sort, so the comparison itself does not
allocate; equal keys keep their input order.

## Locale

Grid chrome (pager, page-size picker, empty state, table name, filter labels) comes from
`GridMessages`. An explicit `messages` prop wins. Otherwise the grid resolves its catalog against the
ambient `GridLocale` the host provides once through `provide_grid_locale`, which is what keeps a
table that never passes `messages` from rendering English copy inside a Russian admin.

## Column widths and CSP

Column geometry is emitted as a presentational `<colgroup><col width="…">` hint, not as a `style`
attribute: hosts serve UI documents with `style-src-attr 'none'`, which silently drops inline widths
and turns resizing into a no-op that only shows up in production. The table is `table-fixed`, so the
hints are authoritative — with auto layout a single wide cell would win over the hint and dragging a
column edge would look broken. Browsers distribute the remainder of the container width
proportionally, so a stored width is a weight, not a guarantee to the pixel. Keep the grid free of
inline styles for the same reason.

## SSR

Nothing that affects the first paint happens in an `Effect` (effects never run on the server): totals and page slices are derived signals, so the server-rendered markup matches the hydrated one. `web-sys` pointer/keyboard code only runs from event handlers, which never fire during SSR.

## Rendering & reactivity

- Rows are rendered with a keyed `<For>` (`key_fn`), so a data refresh does not rebuild every row and in-progress DOM state survives.
- `columns` is held in a signal and passed to the header and the rows, so toggling column visibility or dragging a column edge re-renders only what changed.
- Filter inputs are semi-controlled: local state while a 250 ms debounce is pending, but externally applied values (reset, restored state) are adopted immediately.

## Accessibility

- `<th scope="col">` with a live `aria-sort`, and sorting is triggered by a real `<button>` (keyboard operable).
- Resize handles follow the WAI-ARIA *window splitter* pattern: `role="separator"`, `tabindex="0"`, `aria-valuemin/valuenow/valuemax`, Arrow keys (±16 px) and Home/End.
- A pointer resize takes pointer capture on the handle, so the drag survives a cursor that leaves the 8 px handle. A drag starts only when the capture was taken: without it a release outside the handle would never reach the handle and the drag would stay active. The browser releases the capture after `pointerup` and `pointercancel`; the handle does not release it itself. The keyboard path does not depend on capture.
- The header checkbox exposes the `indeterminate` state for a partially selected page; all checkboxes and filter controls have accessible names.
- Pagination lives in a `<nav aria-label="Pagination">`; infinite mode exposes a keyboard-reachable **Load more** button alongside its intersection-observer sentinel. The row-count summary and the loading/empty row are `aria-live="polite"`.
- The columns popover is closed by a backdrop click or `Escape` and reports `aria-expanded`.
