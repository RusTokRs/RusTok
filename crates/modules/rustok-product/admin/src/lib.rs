#![recursion_limit = "512"]
#![allow(clippy::too_many_arguments)]

pub mod catalog_controls;
pub mod catalog_transport;
pub mod core;
pub mod facets;
mod i18n;
// The retry-identity contract is crate-level: it is the caller identity that
// the admin package owns for every lifecycle command, independent of the
// transport module that consumes it.
pub(crate) mod lifecycle_retry_identity;
pub mod model;
pub mod transport;
pub mod ui;

pub use core::{ProductKind, product_grid_columns};
pub use facets::{
    ProductAdminFacetLabels, ProductAdminFacetPanelView, ProductAdminFacetValueView,
    ProductAdminFacetView, admin_catalog_facets_to_grid, build_product_admin_facet_codes,
    build_product_admin_facet_labels, build_product_admin_facet_panel,
};
pub use model::{
    AdminCatalogFacet, AdminCatalogFacetValue, AxisAllowedValue, ProductCatalogSearchOption,
    ProductCatalogSearchOptions, SetVariantAxesDraft, VariantAxisConfig, VariantAxisDraft,
    VariantAxisValue, VariantAxisValueDraft,
};
pub use transport::fetch_catalog_search_options;
// One mounted admin per surface: the host codegen mounts `ui::root::ProductAdmin`.
// The reference compositions (`ui::catalog_admin`, `ui::leptos`) stay reachable
// through their module paths, but they are no longer advertised as entry points.
pub use ui::root::ProductAdmin;
pub use ui::{CategoriesPage, ProductEditorPage, ProductGridPage};
