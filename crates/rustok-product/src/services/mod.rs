pub mod catalog;
mod catalog_attribute_terms;
pub mod catalog_schema;
pub mod catalog_schema_service;
mod index_channel_relation;
mod index_channel_relation_convergence;
mod index_channel_relation_freshness;
mod index_refresh;
mod index_refresh_event;
mod index_refresh_publication;
mod index_refresh_relay;
mod write_transaction;

pub use catalog::{
    AdminProductList, AdminProductListItem, AdminProductListQuery, CatalogService,
    MAX_STOREFRONT_PRODUCT_SEARCH_BYTES, ProductAttributeFilter, StorefrontProductList,
    StorefrontProductListItem, StorefrontProductListQuery, StorefrontProductSortBy,
    StorefrontProductSortDirection,
};
pub use catalog_attribute_terms::{
    ProductAttributeTermError, ProductAttributeTermExpr, ProductResolvedAttributeFilter,
    product_attribute_boolean_term, product_attribute_date_term, product_attribute_datetime_term,
    product_attribute_decimal_term, product_attribute_integer_term,
    product_attribute_localized_presence_term, product_attribute_localized_text_expr,
    product_attribute_localized_text_term, product_attribute_option_term,
    product_attribute_text_term,
};
pub use catalog_schema::*;
pub use catalog_schema_service::*;
pub use index_channel_relation::{
    MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS, MAX_PRODUCT_SALES_CHANNEL_RELATION_PAGE,
    MAX_PRODUCT_SALES_CHANNEL_RELATION_TARGETS, ProductSalesChannelIndexRelationError,
    ProductSalesChannelIndexRelationRecord, ProductSalesChannelIndexRelationStore,
    ProductSalesChannelIndexRelationWriteOutcome,
};
pub use index_channel_relation_convergence::{
    MAX_PRODUCT_SALES_CHANNEL_CONVERGENCE_ERROR_BYTES,
    ProductSalesChannelIndexRelationConvergenceClaim,
    ProductSalesChannelIndexRelationConvergenceClaimOutcome,
    ProductSalesChannelIndexRelationConvergenceError,
    ProductSalesChannelIndexRelationConvergenceStore,
    ProductSalesChannelIndexRelationConvergenceWork,
};
pub use index_channel_relation_freshness::{
    MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES, ProductSalesChannelIndexRelationFreshnessError,
    ProductSalesChannelIndexRelationFreshnessRecord,
    ProductSalesChannelIndexRelationFreshnessStore,
    ProductSalesChannelIndexRelationFreshnessWriteOutcome,
};
pub use index_refresh::{
    MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE, MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE,
    ProductIndexLocaleRefreshRecord, ProductIndexLocaleRefreshSource,
    ProductIndexVariantRefreshRecord, ProductIndexVariantRefreshSource,
};
pub use index_refresh_event::CanonicalProductIndexRefreshEventFactory;
pub use index_refresh_publication::{
    ProductIndexRefreshCanonicalWriter, ProductIndexRefreshContract,
    ProductIndexRefreshContractTarget, ProductIndexRefreshPublicationError,
};
pub use index_refresh_relay::{
    ProductIndexRefreshEventFactory, ProductIndexRefreshRelayError, ProductIndexRefreshRelayStep,
    ProductIndexRefreshRelayStepOutcome,
};

pub(crate) use write_transaction::with_product_operation_receipt;
