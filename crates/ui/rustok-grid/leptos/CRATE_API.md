# rustok-grid-leptos / CRATE_API

SSR-first Leptos 0.8 DataGrid adapter for RusToK.

## Public surface

### Primary Public Components
- `DataGrid`: Root Leptos DataGrid view component managing reactive signals, header, filter row, data rows, and pagination bar.
- `GridHeader`: Table header rendering column cells, sort toggles, resize handles, and pinned columns.
- `GridFilterCell`: Input component for column-level filter values (Text, Select, Range, Boolean).
- `GridRow`: Table row component rendering cell values, selections, and pinned columns.
- `GridPaginationBar`: Bottom pagination bar rendering page numbers, navigation buttons, and page-size selector.
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

1. **Hydration and SSR Stability**: State machine renders identically on server and client without effect-driven hydration mismatches.
2. **Accessible Keyboard Controls**: Column resizing (`ColumnResizeHandle`) and header sorts support arrow keys and standard WAI-ARIA grid keys.
3. **Double-Click Reset**: Resizing handles reset column width to auto-fit / default on double-click.
4. **Boundary Isolation**: Acts purely as a visual and DOM event adapter over `rustok-grid` state primitives.
