# rustok-grid / CRATE_API

## Purpose
`rustok-grid` is RusToK's framework-agnostic headless DataGrid engine under Fluid Frontend Architecture (FFA). It owns the complete state machine, column math, cell representation, sorting, filtering, row selection, pagination, and facet panel calculations without coupling to any web framework or DOM APIs.

## Architecture & Responsibilities
- **Headless State Container (`GridState`)**: Combines column definitions, width overrides, active filters, sorting criteria, pagination windows, and row selection into an atomic state object.
- **Client & Server Execution Pipeline (`apply_rows`)**: Executes the deterministic transformation pipeline: Filter -> Sort -> Page, guaranteeing identical outcomes across browser memory and server query projections.
- **Strict Invariant Repair**: Every type self-repairs on creation, mutation, and deserialization:
  - `ColumnWidth` guarantees `min <= current <= max`.
  - `GridPagination` clamps `page ∈ 1..=total_pages()` and ensures non-zero page counts even for empty datasets.
  - `SortState` normalizes empty strings and cycles predictably (`Asc` -> `Desc` -> `None`).
- **Cell Value Taxonomy (`CellValue`)**: Encapsulates primitive cell types (`Empty`, `Text`, `Number`, `Boolean`, `Date`) with a strict, total, locale-free ordering.
- **Facet & Filter Engine (`FacetPanel`, `ColumnFilters`)**: Generates facet bucket snapshots, counts, drill-downs, and bounds-checked multi-select query toggles.

## Public Surface
### Primary Structs & Enums
- `GridState`: Unified composite grid state container.
- `GridColumnDef`: Column definition specifying id, label, alignment, initial/min/max width, and pinning.
- `ColumnWidth`: Range-bound column width specification (`min <= current <= max`).
- `ColumnWidths`: Map of column width overrides with bounds clamping.
- `CellValue`: Variant type representing cell content (`Empty`, `Text`, `Number`, `Boolean`, `Date`).
- `ColumnFilters`: Map of active normalized column filters.
- `GridFilterType`: Specification of column filtering modes (`Text`, `Select`, `NumberRange`, `DateRange`, `Boolean`).
- `SortState`: Deterministic column sort state machine (`column_id`, `direction`).
- `SortDirection`: Sort order (`Asc`, `Desc`).
- `GridPagination`: Dual pagination model (paged vs infinite scroll) with bounds clamping.
- `PaginationMode`: Pagination style (`Paged`, `InfiniteScroll`).
- `RowSelection`: Row selection tracking for single and bulk actions.
- `GridFacet`: Server-computed facet descriptor (buckets with counts and domain kinds).
- `FacetPanel`: Framework-free facet panel view-model state.

### Modules & Re-exports
- `cell`: Cell value variants, comparators, and search predicates (`text_contains_ignore_case`).
- `column`: Column definitions, alignments (`ColumnAlign`), and pinning (`PinnedSide`).
- `facet` / `facet_panel`: Facet descriptors, selection encoding, and toggle logic.
- `filter`: Column filter types, options, and value representations.
- `pagination`: Pagination models, fetch/render windows, and page-size options.
- `resize`: Pointer and keyboard resize calculations (`calculate_resized_width`, `step_resized_width`).
- `selection`: Row selection tracking.
- `sort`: Sorting state machine.
- `state`: Unified state machine and row projection engine.

## Invariants & Guarantees
- **Framework-Free Core (FFA)**: 100% pure Rust with zero DOM or UI framework dependencies (no Leptos, Dioxus, or web-sys types).
- **Deterministic Total Ordering**: `CellValue::compare` defines a strict total order (empty < boolean < number < text) avoiding locale-dependent page reordering.
- **Internal Repair and Normalization**: Every type repairs its own state upon deserialization or mutation (`ColumnWidth` clamps within `min..=max`, `GridPagination` clamps `page ∈ 1..=total_pages()`).
- **Execution Pipeline Ordering**: Client-side `GridState::apply_rows` strictly executes Filter -> Sort -> Page in sequence.

## Dependencies & Boundaries
- **Direct Dependencies**:
  - `serde`: Serialization traits for grid state, columns, and facets.
- **Zero Host or UI Framework Coupling**: Independent from any frontend application, renderer, or database backend.
