#![allow(clippy::too_many_arguments)]

pub mod catalog_controls;
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
pub use ui::catalog_admin::ProductAdmin as CatalogProductAdmin;
pub use ui::leptos::ProductAdmin as LeptosProductAdmin;
pub use ui::root::ProductAdmin;
pub use ui::{CategoriesPage, ProductEditorPage, ProductGridPage};
