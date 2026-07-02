#![allow(clippy::too_many_arguments)]
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions};
pub use transport::fetch_catalog_search_options;
pub use ui::leptos::ProductAdmin;
