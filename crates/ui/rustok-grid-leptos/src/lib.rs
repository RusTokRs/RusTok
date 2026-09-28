//! SSR-first Leptos 0.8 DataGrid adapter for RusToK.

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
pub use pagination::GridPaginationBar;
pub use resize_handle::ColumnResizeHandle;
pub use row::GridRow;
pub use toolbar::GridToolbar;

pub use rustok_grid::*;

pub mod prelude {
    pub use rustok_grid::prelude::*;

    pub use crate::filter_inputs::GridFilterCell;
    pub use crate::grid::DataGrid;
    pub use crate::header::GridHeader;
    pub use crate::pagination::GridPaginationBar;
    pub use crate::resize_handle::ColumnResizeHandle;
    pub use crate::row::GridRow;
    pub use crate::toolbar::GridToolbar;
}
