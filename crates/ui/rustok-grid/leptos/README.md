# rustok-grid-leptos

SSR-first Leptos 0.8 adapter for [`rustok-grid`](..).

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

## SSR

Nothing that affects the first paint happens in an `Effect` (effects never run on the server): totals and page slices are derived signals, so the server-rendered markup matches the hydrated one. `web-sys` pointer/keyboard code only runs from event handlers, which never fire during SSR.

## Rendering & reactivity

- Rows are rendered with a keyed `<For>` (`key_fn`), so a data refresh does not rebuild every row and in-progress DOM state survives.
- `columns` is held in a signal and passed to the header and the rows, so toggling column visibility or dragging a column edge re-renders only what changed.
- Filter inputs are semi-controlled: local state while a 250 ms debounce is pending, but externally applied values (reset, restored state) are adopted immediately.

## Accessibility

- `<th scope="col">` with a live `aria-sort`, and sorting is triggered by a real `<button>` (keyboard operable).
- Resize handles follow the WAI-ARIA *window splitter* pattern: `role="separator"`, `tabindex="0"`, `aria-valuemin/valuenow/valuemax`, Arrow keys (±16 px) and Home/End.
- The header checkbox exposes the `indeterminate` state for a partially selected page; all checkboxes and filter controls have accessible names.
- Pagination lives in a `<nav aria-label="Pagination">`, the row-count summary and the loading/empty row are `aria-live="polite"`.
- The columns popover is closed by a backdrop click or `Escape` and reports `aria-expanded`.
