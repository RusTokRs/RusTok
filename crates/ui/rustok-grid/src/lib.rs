//! Framework-agnostic DataGrid core for the RusToK platform.
//!
//! This crate owns the *state machine* of a data grid — columns, widths,
//! filters, sorting, selection and pagination — and deliberately knows
//! nothing about DOM, HTTP or any web framework (Hexagonal / Ports &
//! Adapters). UI adapters such as `rustok-grid-leptos` translate this state
//! into a rendering, and servers can reuse exactly the same types to answer
//! a query.
//!
//! Every type upholds its invariants internally: widths stay inside
//! `min..=max`, pagination stays inside `1..=total_pages()`, and filters only
//! ever store normalized, non-empty values.

#![forbid(unsafe_code)]

pub mod column;
pub mod facet;
pub mod facet_panel;
pub mod filter;
pub mod pagination;
pub mod resize;
pub mod selection;
pub mod sort;
pub mod state;

pub use column::{
    CHECKBOX_COLUMN_ID, ColumnAlign, ColumnId, ColumnWidth, GridColumnDef, PinnedSide,
    has_filter_row, visible_column_count, visible_columns,
};
pub use facet::{
    FacetDomain, FacetValue, GridFacet, MAX_GRID_FACET_VALUES, MAX_GRID_FACETS,
    SELECTION_SEPARATOR, selection_except, selection_key,
};
pub use facet_panel::{
    FacetPanel, FacetPanelFacet, FacetPanelLabels, FacetPanelValue, count_label,
    has_selection_for_key, is_selection_selected, selection_after_clear, selection_after_clear_key,
    selection_after_toggle, selection_entry, selection_for_key, split_selection,
};
pub use filter::{ColumnFilters, FilterOption, FilterValue, GridFilterType};
pub use pagination::{DEFAULT_PAGE_SIZE, GridPagination, PaginationMode};
pub use resize::{ColumnWidths, KEYBOARD_RESIZE_STEP, calculate_resized_width, step_resized_width};
pub use selection::RowSelection;
pub use sort::{SortDirection, SortState};
pub use state::GridState;

pub mod prelude {
    pub use crate::column::{
        CHECKBOX_COLUMN_ID, ColumnAlign, ColumnId, ColumnWidth, GridColumnDef, PinnedSide,
        has_filter_row, visible_column_count, visible_columns,
    };
    pub use crate::facet::{
        FacetDomain, FacetValue, GridFacet, MAX_GRID_FACET_VALUES, MAX_GRID_FACETS,
        SELECTION_SEPARATOR, selection_except, selection_key,
    };
    pub use crate::facet_panel::{
        FacetPanel, FacetPanelFacet, FacetPanelLabels, FacetPanelValue, count_label,
        has_selection_for_key, is_selection_selected, selection_after_clear,
        selection_after_clear_key, selection_after_toggle, selection_entry, selection_for_key,
        split_selection,
    };
    pub use crate::filter::{ColumnFilters, FilterOption, FilterValue, GridFilterType};
    pub use crate::pagination::{DEFAULT_PAGE_SIZE, GridPagination, PaginationMode};
    pub use crate::resize::{
        ColumnWidths, KEYBOARD_RESIZE_STEP, calculate_resized_width, step_resized_width,
    };
    pub use crate::selection::RowSelection;
    pub use crate::sort::{SortDirection, SortState};
    pub use crate::state::GridState;
}
