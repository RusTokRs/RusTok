//! Dioxus 0.6 DataGrid adapter for RusToK.
//!
//! This crate is a thin **adapter**: every piece of state and all the maths
//! live in [`rustok_grid`], here we only translate them into markup and DOM
//! events. Nothing is created inside an effect, so the tree renders
//! identically on the server (`dioxus-ssr`) and on the client — the first
//! paint is already the final paint.
//!
//! # Data flow
//!
//! Every stateful slice of the grid (filters, sort, selection, pagination) is
//! **uncontrolled by default and controlled on demand**:
//!
//! * pass nothing — the grid owns the state internally;
//! * pass a value *and* its `on_*_change` handler — your value becomes the
//!   single source of truth and the grid only reports intents;
//! * pass a value *without* a handler — the value is used as the initial
//!   state and the grid takes ownership from there.
//!
//! ```ignore
//! use dioxus::prelude::*;
//! use rustok_grid_dioxus::prelude::*;
//!
//! #[derive(Clone, PartialEq)]
//! struct Row { id: String, title: String }
//!
//! #[component]
//! fn Products(rows: Vec<Row>) -> Element {
//!     let columns = vec![
//!         GridColumnDef::checkbox(),
//!         GridColumnDef::new("title", "Title").filter(GridFilterType::text()),
//!     ];
//!
//!     rsx! {
//!         DataGrid {
//!             columns,
//!             data: rows,
//!             row_key: Callback::new(|row: Row| row.id.clone()),
//!             cell_renderer: Callback::new(|cell: GridCell<Row>| match cell.column_id.as_str() {
//!                 "title" => rsx! { span { "{cell.item.title}" } },
//!                 _ => rsx! {},
//!             }),
//!         }
//!     }
//! }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod filter_inputs;
pub mod grid;
pub mod header;
pub mod pagination;
pub mod resize_handle;
pub mod row;
pub mod toolbar;

pub use filter_inputs::{FilterCommitMode, GridFilterCell};
pub use grid::{DataGrid, GridCell};
pub use header::GridHeader;
pub use pagination::{DEFAULT_PAGE_SIZE_OPTIONS, GridPaginationBar};
pub use resize_handle::ColumnResizeHandle;
pub use row::GridRow;
pub use toolbar::GridToolbar;

pub use rustok_grid::*;

/// Everything needed to render a grid, in one import.
pub mod prelude {
    pub use rustok_grid::prelude::*;

    pub use crate::filter_inputs::{FilterCommitMode, GridFilterCell};
    pub use crate::grid::{DataGrid, GridCell};
    pub use crate::header::GridHeader;
    pub use crate::pagination::{DEFAULT_PAGE_SIZE_OPTIONS, GridPaginationBar};
    pub use crate::resize_handle::ColumnResizeHandle;
    pub use crate::row::GridRow;
    pub use crate::toolbar::GridToolbar;
}
