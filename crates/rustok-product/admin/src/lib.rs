#![allow(clippy::too_many_arguments)]
mod catalog_controls;
#[path = "catalog_transport.rs"]
mod legacy_transport;
#[path = "catalog_transport_retry.rs"]
mod transport;
mod core;
mod i18n;
mod lifecycle_retry_identity;
mod model;
#[path = "transport/product_lifecycle_graphql.rs"]
mod product_lifecycle_graphql;
mod ui;

pub use model::{ProductCatalogSearchOption, ProductCatalogSearchOptions};
pub use transport::fetch_catalog_search_options;
pub use ui::catalog_admin::ProductAdmin;
