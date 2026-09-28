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

pub use rustok_grid::*;

pub mod prelude {
    pub use rustok_grid::prelude::*;

    pub use crate::filter_inputs::GridFilterCell;
    pub use crate::grid::DataGrid;
    pub use crate::header::GridHeader;
    pub use crate::pagination::{DEFAULT_PAGE_SIZE_OPTIONS, GridPaginationBar};
    pub use crate::resize_handle::ColumnResizeHandle;
    pub use crate::row::GridRow;
    pub use crate::toolbar::GridToolbar;
}
