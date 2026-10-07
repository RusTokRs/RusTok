#![recursion_limit = "512"]
#![allow(clippy::too_many_arguments)]

pub mod catalog_controls;
pub mod catalog_transport;
pub mod core;
mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use core::{product_grid_columns, ProductKind};
pub use model::{
    AxisAllowedValue, ProductCatalogSearchOption, ProductCatalogSearchOptions, SetVariantAxesDraft,
    VariantAxisConfig, VariantAxisDraft, VariantAxisValue, VariantAxisValueDraft,
};
pub use transport::fetch_catalog_search_options;
// One mounted admin per surface: the host codegen mounts `ui::root::ProductAdmin`.
// The reference compositions (`ui::catalog_admin`, `ui::leptos`) stay reachable
// through their module paths, but they are no longer advertised as entry points.
pub use ui::root::ProductAdmin;
pub use ui::{CategoriesPage, ProductEditorPage, ProductGridPage};
