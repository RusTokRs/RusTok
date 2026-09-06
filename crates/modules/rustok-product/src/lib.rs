/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, ModuleRuntimeExtensions, RusToKModule};
use rustok_seo_targets::register_seo_target_provider;
use sea_orm_migration::MigrationTrait;

mod catalog_command_port;
mod catalog_schema_read_port;
mod catalog_schema_write_port;
pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
mod public_error;
mod runtime;
mod seo_targets;
pub mod services;
mod storefront_http_read_port;
mod storefront_tag_read_port;

pub use catalog_command_port::ProductCatalogCommandPort;
pub use catalog_schema_read_port::{
    ProductAttributeValuesRequest, ProductCatalogSchemaReadPort,
    ProductEffectiveFormAttributeProjection, ProductEffectiveFormProjection,
    ProductEffectiveFormRequest, ProductEffectiveFormSubject,
    ProductStorefrontAttributeFilterResolutionRequest,
};
pub use catalog_schema_write_port::ProductCatalogSchemaWritePort;
pub use error::{CommerceError, CommerceResult};
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

/// Typed marker proving that `ProductModule` participated in runtime extension registration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProductRuntimeSelected;

pub struct ProductModule;

#[async_trait]
impl RusToKModule for ProductModule {
    fn slug(&self) -> &'static str {
        "product"
    }

    fn name(&self) -> &'static str {
        "Product"
    }

    fn description(&self) -> &'static str {
        "Product catalog, variants, translations, options, and publication lifecycle"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["taxonomy"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::PRODUCTS_CREATE,
            Permission::PRODUCTS_READ,
            Permission::PRODUCTS_UPDATE,
            Permission::PRODUCTS_DELETE,
            Permission::PRODUCTS_LIST,
            Permission::PRODUCTS_MANAGE,
        ]
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        register_seo_target_provider(extensions, seo_targets::ProductSeoTargetProvider).map_err(
            |error| {
                rustok_core::Error::Validation(format!(
                    "product SEO target registration failed: {error}"
                ))
            },
        )?;
        extensions.insert(ProductRuntimeSelected);
        Ok(())
    }
}

impl MigrationSource for ProductModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}

#[cfg(test)]
mod contract_tests;
