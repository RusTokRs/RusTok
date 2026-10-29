# rustok-grid-dioxus / CRATE_API

Dioxus 0.7 DataGrid adapter for RusToK.

## Public surface

### Primary Public Components
- `DataGrid`: Root Dioxus DataGrid RSX view component managing signals, header, filter row, data rows, and pagination bar.
- `GridCell`: Data context supplied to custom cell renderers (`column_id`, `item`).
- `GridHeader`: Table header rendering column cells, sort triggers, resize handles, and pinned columns.
- `GridFilterCell`: Input component for column-level filter values.
- `GridRow`: Table row component rendering cell items and selection checkboxes.
- `GridPaginationBar`: Bottom pagination bar rendering page navigation and page-size selector.
- `GridToolbar`: Grid action bar for global search, bulk selection actions, and facet toggles.
- `ColumnResizeHandle`: Pointer and keyboard accessible resize gripper.

### Re-exported Core Types
- `GridState`: Unified state composite.
- `GridColumnDef`: Column definition builder.
- `ColumnWidth` / `ColumnWidths`: Width specifications.
- `CellValue`: Variant row data model.
- `ColumnFilters`: Filter state map.
- `SortState`: Sort specification.
- `GridPagination`: Pagination state.
- `RowSelection`: Selection tracker.

## Invariants

1. **Hydration and SSR Stability**: State machine renders identically on server (`dioxus-ssr`) and client without effect-driven hydration mismatches.
2. **Component Parity**: Full parity with `rustok-grid-leptos` API structure, accessibility bindings, and keyboard behavior.
3. **Accessible Keyboard Controls**: Column resizing (`ColumnResizeHandle`) and header sorts support arrow keys and standard WAI-ARIA grid keys.
4. **Boundary Isolation**: Acts purely as a visual and DOM event adapter over `rustok-grid` state primitives.
