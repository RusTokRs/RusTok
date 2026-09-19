/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

mod domain;
mod catalog_command_port;
mod catalog_schema_read_port;
mod catalog_schema_write_port;
pub mod dto;
mod module;
mod entities;
pub mod error;
mod image_translation_progress_target;
mod image_translation_target;
mod migrations;
pub mod ports;
mod public_error;
mod runtime;
mod seo_targets;
pub mod services;
mod storefront_http_read_port;
mod storefront_tag_read_port;
mod translation_target;
mod variant_translation_progress_target;
mod variant_translation_target;

pub use catalog_command_port::ProductCatalogCommandPort;
pub use catalog_schema_read_port::{
    ProductAttributeValuesRequest, ProductCatalogSchemaReadPort,
    ProductEffectiveFormAttributeProjection, ProductEffectiveFormProjection,
    ProductEffectiveFormRequest, ProductEffectiveFormSubject,
    ProductStorefrontAttributeFilterResolutionRequest,
};
pub use catalog_schema_write_port::ProductCatalogSchemaWritePort;
pub use dto::ProductStatus;
pub use error::{CommerceError, CommerceResult};
pub use image_translation_progress_target::ProductImageTranslationTargetProvider;
pub use ports::*;
pub use public_error::{ProductPublicError, map_product_public_error};
pub use runtime::{
    ProductCatalogCommandProfile, ProductCatalogCommandRuntime, ProductCatalogReadProfile,
    ProductCatalogReadRuntime,
};
pub use services::{
    AdminProductList, AdminProductListItem, AdminProductListQuery, CatalogService,
    MAX_PRODUCT_INDEX_LOCALE_REFRESH_PAGE, MAX_PRODUCT_INDEX_VARIANT_REFRESH_PAGE,
    MAX_PRODUCT_SALES_CHANNEL_CONVERGENCE_ERROR_BYTES, MAX_PRODUCT_SALES_CHANNEL_RELATION_CHANNELS,
    MAX_PRODUCT_SALES_CHANNEL_RELATION_PAGE, MAX_PRODUCT_SALES_CHANNEL_RELATION_TARGETS,
    MAX_PRODUCT_SALES_CHANNEL_VISIBILITY_KEY_BYTES, ProductAttributeFilter,
    ProductAttributeTermError, ProductAttributeTermExpr, ProductCatalogSchemaService,
    ProductImageTranslationExactLocaleApply, ProductImageTranslationExactLocaleApplyReceipt,
    ProductImageTranslationExactLocaleError, ProductImageTranslationExactLocaleRecord,
    ProductImageTranslationExactLocaleResult, ProductImageTranslationExactLocaleSnapshot,
    ProductIndexLocaleRefreshRecord, ProductIndexLocaleRefreshSource,
    ProductIndexRefreshCanonicalWriter, ProductIndexRefreshContract,
    ProductIndexRefreshContractTarget, ProductIndexRefreshEventFactory,
    ProductIndexRefreshPublicationError, ProductIndexRefreshRelayError,
    ProductIndexRefreshRelayStep, ProductIndexRefreshRelayStepOutcome,
    ProductIndexVariantRefreshRecord, ProductIndexVariantRefreshSource,
    ProductResolvedAttributeFilter, ProductSalesChannelIndexRelationConvergenceClaim,
    ProductSalesChannelIndexRelationConvergenceClaimOutcome,
    ProductSalesChannelIndexRelationConvergenceError,
    ProductSalesChannelIndexRelationConvergenceStore,
    ProductSalesChannelIndexRelationConvergenceWork, ProductSalesChannelIndexRelationError,
    ProductSalesChannelIndexRelationFreshnessError,
    ProductSalesChannelIndexRelationFreshnessRecord,
    ProductSalesChannelIndexRelationFreshnessStore,
    ProductSalesChannelIndexRelationFreshnessWriteOutcome, ProductSalesChannelIndexRelationRecord,
    ProductSalesChannelIndexRelationStore, ProductSalesChannelIndexRelationWriteOutcome,
    ProductTranslationExactLocaleApply, ProductTranslationExactLocaleApplyReceipt,
    ProductTranslationExactLocaleError, ProductTranslationExactLocaleRecord,
    ProductTranslationExactLocaleResult, ProductTranslationExactLocaleSnapshot,
    ProductTranslationExactResourcePage, ProductTranslationExactResourceSummary,
    ProductVariantTranslationExactLocaleApply, ProductVariantTranslationExactLocaleApplyReceipt,
    ProductVariantTranslationExactLocaleError, ProductVariantTranslationExactLocaleRecord,
    ProductVariantTranslationExactLocaleResult, ProductVariantTranslationExactLocaleSnapshot,
    StorefrontProductList, StorefrontProductListItem, StorefrontProductListQuery,
    StorefrontProductSortBy, StorefrontProductSortDirection, product_attribute_boolean_term,
    product_attribute_date_term, product_attribute_datetime_term, product_attribute_decimal_term,
    product_attribute_integer_term, product_attribute_localized_presence_term,
    product_attribute_localized_text_expr, product_attribute_localized_text_term,
    product_attribute_option_term, product_attribute_text_term,
};
pub use storefront_http_read_port::{
    LegacyStorefrontHttpProductsRequest, ProductStorefrontHttpReadPort,
};
pub use storefront_tag_read_port::{
    ProductStorefrontTagHydration, ProductStorefrontTagHydrationItem,
    ProductStorefrontTagHydrationRequest, ProductStorefrontTagReadPort,
};
pub use translation_target::ProductTranslationTargetProvider;
pub use variant_translation_progress_target::ProductVariantTranslationTargetProvider;

pub use module::{ProductModule, ProductRuntimeSelected};
#[cfg(test)]
mod contract_tests;
