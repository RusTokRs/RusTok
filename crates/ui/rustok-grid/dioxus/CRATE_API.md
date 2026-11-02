# rustok-grid-dioxus / CRATE_API

## Purpose
`rustok-grid-dioxus` is the Dioxus 0.7 adapter for the framework-agnostic `rustok-grid` library. It translates headless grid state, column math, and filtering into accessible Dioxus RSX markup and DOM events, maintaining complete SSR and hydration stability.

## Architecture & Responsibilities
- **Dioxus RSX View Adapter**: Renders `<DataGrid>` roots, header rows, filter bars, virtualized data rows, and pagination footers.
- **Controlled & Uncontrolled State Integration**: Supports both uncontrolled self-managed state and controlled external signal state with `on_*_change` notification callbacks.
- **WAI-ARIA Accessibility**: Binds grid table semantics, `aria-sort`, `aria-selected`, column header IDs, and keyboard navigation keys.
- **Pointer & Keyboard Resizing**: Integrates `ColumnResizeHandle` with pointer capture and keyboard step events for accessible column resizing.
- **Filter Commit Modes**: Supports configurable filter dispatch via `FilterCommitMode::OnCommit` and `FilterCommitMode::Live`.

## Public Surface
### Primary Components
- `DataGrid`: Root Dioxus DataGrid RSX view component managing signals, header, filter row, data rows, and pagination bar.
- `GridCell`: Data context supplied to custom cell renderers (`column_id`, `item`, `row_id`).
- `GridHeader`: Table header rendering column cells, sort triggers, resize handles, and pinned columns.
- `GridFilterCell`: Input component for column-level filter values.
- `GridRow`: Table row component rendering cell items and selection checkboxes.
- `GridPaginationBar`: Bottom pagination bar rendering page navigation and page-size selector.
- `GridToolbar`: Grid action bar for global search, bulk selection actions, and facet toggles.
- `ColumnResizeHandle`: Pointer and keyboard accessible resize gripper.

### Types & Enums
- `FilterCommitMode`: Filter dispatch timing (`OnCommit`, `Live`).
- Re-exports of all core `rustok-grid` state and model types (`GridState`, `GridColumnDef`, `ColumnWidth`, `ColumnWidths`, `CellValue`, `ColumnFilters`, `SortState`, `GridPagination`, `RowSelection`).

## Invariants & Guarantees
- **Hydration & SSR Stability**: Renders identically on server (`dioxus-ssr`) and client without effect-driven hydration mismatches.
- **Component Parity**: Maintained with 1:1 structural, capability, and accessibility parity against `rustok-grid-leptos`.
- **Accessible Keyboard Controls**: Column resizing (`ColumnResizeHandle`) and header sorts support arrow keys and standard WAI-ARIA grid keys.
- **Boundary Isolation**: Acts purely as a visual and DOM event adapter over `rustok-grid` state primitives without holding private domain logic.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `rustok-grid`: Framework-agnostic DataGrid state machine and column math core.
  - `dioxus`: Dioxus 0.7 reactive primitives, component lifecycle, and RSX macro.
- **Zero Host Coupling**: Can be used in any Dioxus 0.7 application (web, desktop, or mobile).
