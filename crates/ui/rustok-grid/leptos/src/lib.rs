//! # RusToK Grid Leptos
//!
//! SSR-first Leptos 0.8 DataGrid adapter for RusToK.
//!
//! This crate is a thin **adapter**: all state and math live in
//! [`rustok_grid`], here we only translate them into markup and DOM events.
//! Everything renders identically on the server and after hydration — no
//! state is created inside an `Effect`, so the first paint is already the
//! final paint.
//!
//! ```ignore
//! use rustok_grid_leptos::prelude::*;
//!
//! let columns = vec![
//!     GridColumnDef::checkbox(),
//!     GridColumnDef::new("title", "Title").filter(GridFilterType::text()),
//! ];
//! view! {
//!     <DataGrid
//!         columns=columns
//!         data=rows
//!         key_fn=|row: &Row| row.id.clone()
//!         cell_renderer=renderer
//!     />
//! }
//! ```

use leptos::prelude::*;

pub mod filter_inputs;
pub mod grid;
pub mod header;
pub mod pagination;
pub mod resize_handle;
pub mod row;
pub mod toolbar;

pub use filter_inputs::GridFilterCell;
pub use grid::DataGrid;
pub use header::GridHeader;
pub use pagination::{DEFAULT_PAGE_SIZE_OPTIONS, GridPaginationBar};
pub use resize_handle::ColumnResizeHandle;
pub use row::GridRow;
pub use toolbar::GridToolbar;

/// Ambient locale for every grid rendered below the providing owner.
///
/// `GridMessages` is the canonical catalog, and a table that never passes one
/// would silently render English chrome in a Russian admin. A host therefore
/// provides the effective locale once, and [`DataGrid`] resolves its chrome
/// against it unless the call site chooses a catalog explicitly.
#[derive(Clone, Copy)]
pub struct GridLocale(pub Signal<Option<String>>);

impl GridLocale {
    /// The locale in force when a grid renders, if the host knows one.
    ///
    /// Grid chrome is resolved once per mount from the catalog, so an operator
    /// switching the admin language re-resolves it on the next mount rather
    /// than on the same frame; the signal keeps the host free to make the value
    /// as reactive as it likes.
    pub fn current(self) -> Option<String> {
        self.0.get_untracked()
    }
}

/// Provide the ambient grid locale for the current owner subtree.
///
/// `None` is a valid value: the grid then keeps the catalog default (`en`).
pub fn provide_grid_locale(locale: Signal<Option<String>>) {
    provide_context(GridLocale(locale));
}

/// Read the ambient grid locale, if the host provided one.
pub fn use_grid_locale() -> Option<String> {
    use_context::<GridLocale>().and_then(GridLocale::current)
}

pub use rustok_grid::{
    CHECKBOX_COLUMN_ID, CellValue, ColumnAlign, ColumnFilters, ColumnId, ColumnWidth, ColumnWidths,
    DEFAULT_PAGE_SIZE, DEFAULT_PAGE_SIZE_OPTIONS as GRID_DEFAULT_PAGE_SIZE_OPTIONS, FacetDomain,
    FacetPanel, FacetValue, FilterOption, FilterValue, GridColumnDef, GridFacet, GridFilterType,
    GridPagination, GridState, PaginationMode, PinnedSide, RowSelection, SortDirection, SortState,
};

pub mod prelude {
    pub use rustok_grid::prelude::{
        CHECKBOX_COLUMN_ID, CellValue, ColumnAlign, ColumnFilters, ColumnId, ColumnWidth,
        ColumnWidths, DEFAULT_PAGE_SIZE,
        DEFAULT_PAGE_SIZE_OPTIONS as GRID_DEFAULT_PAGE_SIZE_OPTIONS, FacetDomain, FacetPanel,
        FacetValue, FilterOption, FilterValue, GridColumnDef, GridFacet, GridFilterType,
        GridPagination, GridState, PaginationMode, PinnedSide, RowSelection, SortDirection,
        SortState,
    };

    pub use crate::filter_inputs::GridFilterCell;
    pub use crate::grid::DataGrid;
    pub use crate::header::GridHeader;
    pub use crate::pagination::{DEFAULT_PAGE_SIZE_OPTIONS, GridPaginationBar};
    pub use crate::resize_handle::ColumnResizeHandle;
    pub use crate::row::GridRow;
    pub use crate::toolbar::GridToolbar;
    pub use crate::{GridLocale, provide_grid_locale, use_grid_locale};
}

#[cfg(test)]
mod contract_tests;
