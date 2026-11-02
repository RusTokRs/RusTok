# rustok-grid-leptos / CRATE_API

## Purpose
`rustok-grid-leptos` is the Leptos 0.8 SSR-first adapter for the framework-agnostic `rustok-grid` library. It translates headless grid state, column geometry, and filtering into accessible Leptos component views and DOM event listeners with full hydration parity.

## Architecture & Responsibilities
- **Leptos Component View Adapter**: Renders `<DataGrid>` roots, header rows, filter rows, table rows, and pagination bars.
- **SSR & Hydration Parity**: Server-side rendering emits identical initial markup without layout shifts or effect-driven hydration mismatches.
- **WAI-ARIA Accessibility**: Binds grid table semantics, `aria-sort`, `aria-selected`, column header IDs, and keyboard navigation keys.
- **Pointer & Keyboard Resizing**: Integrates `ColumnResizeHandle` with pointer capture and keyboard step events for accessible column resizing.
- **Controlled & Uncontrolled State Integration**: Supports both uncontrolled self-managed state and controlled external signal state with `on_*_change` notification callbacks.

## Public Surface
### Primary Components
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

## Invariants & Guarantees
- **Hydration & SSR Stability**: State machine renders identically on server and client without effect-driven hydration mismatches.
- **Component Parity**: Maintained with 1:1 structural, capability, and accessibility parity against `rustok-grid-dioxus`.
- **Accessible Keyboard Controls**: Column resizing (`ColumnResizeHandle`) and header sorts support arrow keys and standard WAI-ARIA grid keys.
- **Double-Click Reset**: Resizing handles reset column width to auto-fit / default on double-click.
- **Boundary Isolation**: Acts purely as a visual and DOM event adapter over `rustok-grid` state primitives without holding private domain logic.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `rustok-grid`: Framework-agnostic DataGrid state machine and column math core.
  - `leptos`: Leptos 0.8 reactive primitives, view macro, signals, and SSR/hydration support.
  - `leptos-use`: Composable reactive hooks for browser events and layout measurements.
  - `web-sys`: Web DOM APIs for element geometry and keyboard/pointer events.
- **Zero Host Coupling**: Can be used in any Leptos 0.8 SSR, CSR, or hybrid application.
