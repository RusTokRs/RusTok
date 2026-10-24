//! Owner-defined cross-boundary ports for the product catalog.
//!
//! Provides transport-neutral read projections (`ProductCatalogReadPort`)
//! for checkout, storefront, and administrative commerce consumers.

mod catalog_read;
mod diagnostics;
mod types;

pub use catalog_read::ProductCatalogReadPort;
pub use types::{
    AdminCatalogFacetsRequest, AdminProductsRequest, FilteredPublishedProductsRequest,
    LegacyAdminProductsRequest, LegacyStorefrontProductList, LegacyStorefrontProductListItem,
    LegacyStorefrontProductsRequest, ProductProjectionRequest, PublishedProductsRequest,
    StorefrontCatalogFacetsRequest, StorefrontProductProjectionRequest,
    StorefrontProductProjectionSubject, StorefrontVariantProductProjectionRequest,
    VariantProductProjectionRequest, MAX_ADMIN_PRODUCTS_PER_PAGE, MAX_PUBLISHED_PRODUCTS_PER_PAGE,
};

pub(crate) use diagnostics::{product_owner_error_facts, product_port_context_facts};
