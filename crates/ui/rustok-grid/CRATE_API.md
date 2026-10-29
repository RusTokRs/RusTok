# rustok-grid / CRATE_API

Framework-agnostic DataGrid state machine, column math, and filtering engine for RusToK.

## Public surface

### Primary Public Types
- `GridState`: Unified composite grid state (columns, filters, sort, pagination, widths, selection).
- `GridColumnDef`: Column definition specifying id, label, alignment, initial/min/max width, and pinning.
- `ColumnWidth`: Range-bound column width specification (`min <= current <= max`).
- `ColumnWidths`: Map of column width overrides with bounds clamping.
- `CellValue`: Variant type representing cell content (`Empty`, `Text`, `Number`, `Boolean`, `Date`).
- `ColumnFilters`: Map of active normalized column filters.
- `GridFilterType`: Specification of column filtering modes (`Text`, `Select`, `NumberRange`, `DateRange`, `Boolean`).
- `SortState`: Deterministic column sort state machine (`column_id`, `direction`).
- `GridPagination`: Dual pagination model (paged vs infinite scroll) with bounds clamping.
- `RowSelection`: Row selection tracking for single and bulk actions.
- `GridFacet`: Server-computed facet descriptor (buckets with counts and domain kinds).
- `FacetPanel`: Framework-free facet panel view-model state.

### Public Modules
- `cell`: Cell value variants, comparators, and search predicates.
- `column`: Column definitions, alignments, and pinning.
- `facet`: Facet descriptors and selection encoding.
- `facet_panel`: Facet panel projection and toggle state machine.
- `filter`: Column filter types, options, and normalization.
- `pagination`: Pagination models, offsets, and page-size options.
- `resize`: Pointer and keyboard resize calculations.
- `selection`: Row selection tracking and helpers.
- `sort`: Sort directions and item ordering.
- `state`: Unified `GridState` composite and client-side `apply_rows` pipeline.

## Invariants

1. **Framework-Free Core (FFA)**: 100% pure Rust with zero DOM or UI framework dependencies.
2. **Deterministic Total Ordering**: `CellValue::compare` defines a strict total order (empty < boolean < number < text) avoiding locale-dependent page reordering.
3. **Internal Repair and Normalization**: Every type repairs its own state upon deserialization or mutation (`ColumnWidth` clamps within `min..=max`, `GridPagination` clamps `page ∈ 1..=total_pages()`).
4. **Execution Pipeline Ordering**: Client-side `GridState::apply_rows` strictly executes Filter -> Sort -> Page in sequence.
