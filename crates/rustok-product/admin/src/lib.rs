#![allow(clippy::too_many_arguments)]
mod catalog_controls;
#[path = "catalog_transport.rs"]
mod transport;
mod core;
mod i18n;
mod lifecycle_retry_identity;
mod model;
mod ui;

pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions};
pub use transport::fetch_catalog_search_options;
pub use ui::catalog_admin::ProductAdmin;
